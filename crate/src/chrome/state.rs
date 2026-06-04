//! Stateful [`Chrome`](super::Chrome) wrapper: builders, runtime setters,
//! and the `dear-app` runner callbacks (`on_setup` / `on_event` / `render`).
//!
//! Split out of `mod.rs`; the `Chrome` struct itself lives in the parent so
//! these methods (and the parent's tests) share access to its private fields.

use super::*;

impl Chrome {
    /// Create a new chrome with the given titlebar configuration.
    pub fn new(config: TitlebarConfig) -> Self {
        let theme = Theme::Dark;
        let palette = theme.titlebar();
        Self {
            config,
            title: String::new(),
            theme,
            palette,
            corner_radius: 8,
            resize_zone: 6.0,
            last_cursor: CursorIcon::Default,
            last_size: (0, 0),
            last_maximized: false,
            pending_remax: false,
            pending_close: false,
        }
    }

    /// Builder: set the window title (used by the titlebar text).
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Builder: set the chrome theme (palette source). Default `Theme::Dark`.
    /// Cheaper than `set_theme` since no replacement happens — caches
    /// the palette on construction.
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self.palette = theme.titlebar();
        self
    }

    /// Builder: set the rounded-corner radius (Win10 only — Win11 DWM
    /// owns the corners). Default `8`.
    pub fn with_corner_radius(mut self, r: i32) -> Self {
        self.corner_radius = r;
        self
    }

    /// Builder: set the edge-resize hit zone width (logical pixels).
    /// Default `6.0`.
    pub fn with_resize_zone(mut self, px: f32) -> Self {
        self.resize_zone = px.max(1.0);
        self
    }

    /// Update the title at runtime.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Update the theme at runtime — refreshes the cached palette.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.palette = theme.titlebar();
    }

    /// The theme currently driving the titlebar palette. Symmetric with
    /// [`set_theme`](Self::set_theme) — lets hosts query the active theme
    /// (e.g. to toggle, or to mirror it elsewhere in their UI).
    pub fn theme(&self) -> Theme {
        self.theme
    }

    /// Read-only access to the underlying [`TitlebarConfig`].
    pub fn config(&self) -> &TitlebarConfig {
        &self.config
    }

    /// Mutable access to the underlying [`TitlebarConfig`] — use to
    /// flip button visibility, switch close mode, change height, etc.
    /// at runtime. The change applies on the next frame.
    pub fn config_mut(&mut self) -> &mut TitlebarConfig {
        &mut self.config
    }

    /// Read & clear the close-request flag. Returns `Some(close_mode)`
    /// once after the user clicked close, then `None` until the next
    /// click. The embedded [`CloseMode`] tells the host whether to
    /// exit immediately ([`CloseMode::Immediate`]) or surface a
    /// confirmation flow ([`CloseMode::Confirm`]).
    ///
    /// ```ignore
    /// match chrome.lock().unwrap().take_close_request() {
    ///     Some(CloseMode::Immediate) => std::process::exit(0),
    ///     Some(CloseMode::Confirm)   => self.show_confirm_dialog(),
    ///     None => {}
    /// }
    /// ```
    pub fn take_close_request(&mut self) -> Option<CloseMode> {
        if std::mem::replace(&mut self.pending_close, false) {
            Some(self.config.close_mode)
        } else {
            None
        }
    }

    /// One-shot setup — call from `on_gpu_init`. Strips OS chrome,
    /// applies Win32 dark mode + rounded corners, and shrinks the window
    /// if it came up at a fullscreen-equivalent size (regression guard
    /// against Windows' borderless-fullscreen heuristic on small / hi-DPI
    /// monitors).
    pub fn on_setup(&mut self, window: &Arc<Window>) {
        // Strip decorations FIRST so the rounded-region / DWM corner
        // preference is computed against the borderless geometry.
        window.set_decorations(false);

        #[cfg(windows)]
        win32::setup_window(window, self.corner_radius);

        // Defensive shrink: if the dear-app `RunnerConfig::window_size`
        // matched (or exceeded) the monitor's logical size — common
        // when the developer's machine has a 1920×1080 monitor and the
        // user is on a 1366×768 laptop — Windows treats the borderless
        // window as fullscreen, hides the taskbar, and the chrome is
        // unreachable. Resize down before showing.
        Self::shrink_to_monitor_after_create(window);

        let sz = window.inner_size();
        self.last_size = (sz.width, sz.height);
        self.last_maximized = window.is_maximized();
    }

    /// Hosts that can't call [`clamp_size_to_monitor`] before window
    /// creation (most `dear-app` users — `RunnerConfig` is built before
    /// the `EventLoop` exists) can rely on this post-create fallback.
    /// Called automatically by [`Chrome::on_setup`].
    pub fn shrink_to_monitor_after_create(window: &Arc<Window>) {
        let Some(mon) = window.current_monitor() else {
            return;
        };
        let ms = mon.size();
        let inner = window.inner_size();
        if let Some((new_w, new_h)) = shrink_size_logic(
            ms.width,
            ms.height,
            mon.scale_factor(),
            (inner.width, inner.height),
        ) {
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(new_w, new_h));
        }
    }

    /// Per-event update — call from `on_event`. Tracks resize / maximise
    /// transitions so:
    ///
    /// - The Win10 clip region stays in sync with maximise / restore
    ///   (no-op on Win11 — DWM owns the corners there).
    /// - The cached `last_maximized` flag matches OS state, so per-frame
    ///   render reads it without an extra `window.is_maximized()` call.
    /// - The Win11 `pending_remax` workaround triggers when a window
    ///   that minimised from maximised state restores.
    /// - Layout-independent Ctrl+C/V/X/A/Z shortcuts work on non-Latin
    ///   keyboard layouts (RU / DE / FR / …). Delegated to
    ///   [`crate::input::keyboard::dispatch_dear_app_event`] — see that
    ///   function's docs for the residual upstream quirk (Cyrillic char
    ///   still added by `dear-imgui-winit`).
    ///
    /// Events otherwise handled internally: `WindowEvent::Resized`,
    /// `WindowEvent::Focused`. Everything else is forwarded to the
    /// platform handler unchanged.
    pub fn on_event(
        &mut self,
        event: &winit::event::Event<()>,
        window: &Arc<Window>,
        ctx: &mut dear_imgui_rs::Context,
    ) {
        // Layout-independent shortcut fix — must run BEFORE dear-app forwards
        // the event to `platform.handle_event`. Idempotent / side-effect-free
        // for non-keyboard events.
        //
        // Order-critical: this call MUST stay above the WindowEvent
        // `let..else`. Moving the early return up would silently drop
        // keyboard normalisation for every non-WindowEvent path (which
        // never happens today but is a footgun for future refactors).
        crate::input::keyboard::dispatch_dear_app_event(ctx, event);

        let winit::event::Event::WindowEvent { event: we, .. } = event else {
            return;
        };
        match we {
            WindowEvent::Resized(s) => {
                if s.width == 0 || s.height == 0 {
                    return;
                }
                let new_size = (s.width, s.height);
                let new_max = window.is_maximized();

                // Win11 remax workaround: if a minimised-from-maximised
                // window restores back to non-maximised, the OS doesn't
                // restore the maximised flag automatically. Re-set it.
                //
                // We still cache `last_size` before returning — otherwise
                // the next genuine Resized would think the size changed
                // (it didn't) and fire an unnecessary `sync_region`.
                if self.pending_remax && !new_max {
                    self.pending_remax = false;
                    self.last_size = new_size;
                    window.set_maximized(true);
                    self.last_maximized = true;
                    return;
                }

                if new_size != self.last_size || new_max != self.last_maximized {
                    self.last_size = new_size;
                    self.last_maximized = new_max;
                    #[cfg(windows)]
                    win32::sync_region(window, self.corner_radius, new_max);
                }
            }
            WindowEvent::Focused(_) => {
                // A focus change can race with maximise / minimise — re-poll.
                self.last_maximized = window.is_maximized();
            }
            _ => {}
        }
    }

    /// Per-frame render — call from `on_frame`. Wraps the host's content
    /// in a single full-display ImGui root window so:
    ///
    /// 1. The titlebar draws into the same window as the content (one
    ///    Z-layer; foreground / background draw-list overlays paint
    ///    relative to it correctly).
    /// 2. Resize-edge detection covers the **full window**, not just
    ///    the titlebar strip — drag-resize works on every edge / corner.
    /// 3. There's a single hit-test surface, so dockspace-less hosts
    ///    don't have two stacked root windows competing for click input.
    ///
    /// The host renders inside the `content` closure, which receives a
    /// [`ContentArea`] (origin + size in logical pixels, relative to the
    /// root window). The cursor is already positioned at `area.origin`
    /// when the closure fires.
    ///
    /// Drag / resize / minimise / maximise / close are dispatched to
    /// `window` automatically. Close requests surface via
    /// [`Self::take_close_request`] for the host to honour.
    pub fn render<F: FnOnce(&Ui, ContentArea)>(
        &mut self,
        ui: &Ui,
        window: &Arc<Window>,
        content: F,
    ) {
        use dear_imgui_rs::{Condition, StyleVar, WindowFlags};

        let display = ui.io().display_size();
        let _np = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _ns = ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));
        let _bs = ui.push_style_var(StyleVar::WindowBorderSize(0.0));

        let h = self.config.height;
        let mut tb_result = TitlebarResult::none();
        let maximized = self.last_maximized;
        let os_resizable = !maximized;

        let root_flags = WindowFlags::NO_TITLE_BAR
            | WindowFlags::NO_RESIZE
            | WindowFlags::NO_MOVE
            | WindowFlags::NO_SCROLLBAR
            | WindowFlags::NO_SCROLL_WITH_MOUSE
            | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
            | WindowFlags::NO_NAV_FOCUS;

        let area = ContentArea {
            origin: [0.0, h],
            size: [display[0], (display[1] - h).max(0.0)],
        };

        ui.window("##chrome_root")
            .position([0.0, 0.0], Condition::Always)
            .size(display, Condition::Always)
            .flags(root_flags)
            .build(|| {
                tb_result = render_titlebar(
                    ui,
                    &self.config,
                    &self.title,
                    &self.palette,
                    maximized,
                    self.resize_zone,
                    os_resizable,
                );

                ui.set_cursor_pos([0.0, h]);
                ui.dummy([0.0, 0.0]);

                content(ui, area);
            });

        // Cursor — only update when changed (avoids Win32 flicker).
        let want_cursor = cursor_for_edge(tb_result.hover_edge);
        if want_cursor != self.last_cursor {
            window.set_cursor(want_cursor);
            self.last_cursor = want_cursor;
        }

        // Dispatch actions.
        match tb_result.action {
            TitlebarAction::None => {}
            TitlebarAction::Minimize => {
                // Win11 quirk: minimising a maximised borderless window
                // can leave it in a fullscreen-like state on restore.
                // Drop maximise BEFORE minimise and re-set it via
                // `pending_remax` once the OS sends the next Resized.
                #[cfg(windows)]
                if win32::is_win11() && self.last_maximized {
                    window.set_maximized(false);
                    self.pending_remax = true;
                }
                window.set_minimized(true);
            }
            TitlebarAction::Maximize => {
                // Use cached state — avoids an extra Win32 round-trip.
                let next = !self.last_maximized;
                window.set_maximized(next);
                self.last_maximized = next;
            }
            TitlebarAction::Close => {
                self.pending_close = true;
            }
            TitlebarAction::DragStart => {
                // `clicked` is one-frame; no need to guard against
                // re-firing across frames. The OS owns the mouse during
                // the modal drag — winit won't deliver further click
                // events until the drag ends.
                let _ = window.drag_window();
            }
            TitlebarAction::ResizeStart(edge) => {
                let _ = window.drag_resize_window(resize_direction(edge));
            }
        }
    }

    /// Read the configured titlebar height (logical px). Useful when the
    /// host wants to position content manually rather than using the
    /// [`ContentArea`] from [`Self::render`].
    pub fn titlebar_height(&self) -> f32 {
        self.config.height
    }
}
