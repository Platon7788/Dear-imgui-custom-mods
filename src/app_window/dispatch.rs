//! Per-frame event dispatch and idle scheduling.

use std::time::{Duration, Instant};
use winit::event::WindowEvent;
use winit::event_loop::ControlFlow;

use super::{gpu, AppHandler};
#[cfg(windows)]
use super::win32;

pub(super) fn handle_window_event<H: AppHandler + 'static>(
    app: &mut super::WinitApp<H>,
    event_loop: &winit::event_loop::ActiveEventLoop,
    _window_id: winit::window::WindowId,
    event: WindowEvent,
) {
    let (Some(g), Some(handler)) = (app.gpu.as_mut(), app.handler.as_mut()) else {
        return;
    };

    // First-frame hook: AppHandler::on_ready.
    if !app.on_ready_fired {
        app.on_ready_fired = true;
        handler.on_ready(&mut g.app_state);
    }

    // ── Raw event hook ──────────────────────────────────────────
    // Hand the event to the user **before** Dear ImGui's platform
    // layer sees it. The handler can read drag-drop file paths, run
    // layout-independent hotkeys, intercept gestures — and consume
    // the event by returning `true` so Dear ImGui does not see it.
    //
    // **`consumed` only suppresses the ImGui platform handler** —
    // the framework's own routing (Resized → surface reconfigure,
    // CloseRequested → exit, Focused → titlebar tint, Redraw) still
    // runs. Consuming a structural event is normally pointless;
    // doing so previously skipped these handlers and produced
    // black-rect / stuck-window bugs.
    let consumed = handler.on_window_event(&event, &mut g.app_state);

    // ── Layout-independent keyboard / IME fixes ─────────────────
    // `dear-imgui-winit` derives Dear ImGui keys from the *logical*
    // key (post-keyboard-layout). On Cyrillic / Greek / CJK layouts
    // the physical `C` key arrives as Cyrillic 'с', which neither
    // maps to `Key::C` nor reaches `InputText` as a shortcut — the
    // user has to switch to English to use `Ctrl+C` / `Ctrl+V`,
    // which is unacceptable UX. We inject the right Dear ImGui key
    // **based on the physical scan code** before the platform layer
    // sees the event, then skip the forward so the Cyrillic
    // character isn't typed into the focused field.
    //
    // Numpad digits (`Keypad0..9`) need similar handling: ImGui
    // treats them as navigation, not text — without injection,
    // typing `1` on the numpad never appears in `InputText`.
    //
    // IME commits (CJK composition) are ignored by
    // `dear-imgui-winit` entirely; we forward the committed string
    // as input characters directly.
    let mut kbd_handled = false;
    let mut kbd_event: Option<winit::event::KeyEvent> = None;
    if !consumed {
        match &event {
            WindowEvent::KeyboardInput { event: ke, .. } => {
                let io = g.context.io_mut();
                if crate::input::keyboard::try_inject_numpad_text(io, ke)
                    || crate::input::keyboard::try_inject_ctrl_alt_shortcut(io, ke)
                {
                    kbd_handled = true;
                    // Bump the redraw budget — modifiers + key press
                    // is genuine input, the user expects an immediate
                    // visual response (selection change, cursor move).
                    g.pending_frames = g.pending_frames.max(2);
                    g.window.request_redraw();
                } else {
                    // Save a clone for the post-forward reinforce pass
                    // below — fixes "stuck Key::C" when Ctrl is
                    // released *before* the letter on non-Latin
                    // layouts.
                    kbd_event = Some(ke.clone());
                }
            }
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                crate::input::keyboard::inject_ime_commit(g.context.io_mut(), text);
                kbd_handled = true;
                g.pending_frames = g.pending_frames.max(2);
                g.window.request_redraw();
            }
            _ => {}
        }
    }

    if !consumed && !kbd_handled {
        g.platform.handle_window_event(
            &mut g.context,
            &g.window,
            &event,
        );
        // Reinforce physical-key state so the eventual release
        // matches the press we recorded — see
        // `reinforce_physical_key_state` doc-comment.
        if let Some(ref ke) = kbd_event {
            crate::input::keyboard::reinforce_physical_key_state(g.context.io_mut(), ke);
        }
    }

    // Classify any event that should kick the renderer out of idle.
    // Two frames is the minimum ImGui needs for hover-state to settle
    // (one to detect, one to draw the resulting style change).
    //
    // Coverage: pointer / cursor / mouse / wheel, keyboard / modifiers /
    // IME, scale / theme system changes, touch + touchpad gestures,
    // drag-and-drop hover/cancel/drop. Excluded: `Occluded` (just OS
    // bookkeeping), `RedrawRequested` (handled separately), focus and
    // resize (handled in their own match arms).
    let needs_redraw = matches!(
        &event,
        WindowEvent::CursorMoved { .. }
            | WindowEvent::CursorEntered { .. }
            | WindowEvent::CursorLeft { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::KeyboardInput { .. }
            | WindowEvent::ModifiersChanged(..)
            | WindowEvent::Ime(..)
            | WindowEvent::ScaleFactorChanged { .. }
            | WindowEvent::ThemeChanged(..)
            | WindowEvent::Touch(..)
            | WindowEvent::TouchpadPressure { .. }
            | WindowEvent::PinchGesture { .. }
            | WindowEvent::PanGesture { .. }
            | WindowEvent::RotationGesture { .. }
            | WindowEvent::DoubleTapGesture { .. }
            | WindowEvent::DroppedFile(..)
            | WindowEvent::HoveredFile(..)
            | WindowEvent::HoveredFileCancelled
    );
    if needs_redraw {
        g.pending_frames = g.pending_frames.max(2);
        g.window.request_redraw();
    }

    match event {
        WindowEvent::CloseRequested => {
            handler.on_close_requested(&mut g.app_state);
            if g.app_state.should_exit {
                event_loop.exit();
            }
        }

        WindowEvent::Focused(focused) => {
            g.focused = focused;
            g.app_state.titlebar.set_focused(focused);
            // Force a paint so the inactive titlebar tint applies
            // before any unfocused throttle silences the next frames.
            g.pending_frames = g.pending_frames.max(2);
            g.window.request_redraw();
        }

        WindowEvent::Resized(s) => {
            let is_min = g.window.is_minimized().unwrap_or(false);
            let restored = g.was_minimized && !is_min;
            g.was_minimized = is_min;

            if s.width == 0 || s.height == 0 {
                return;
            }

            g.surface_cfg.width = s.width.max(1);
            g.surface_cfg.height = s.height.max(1);
            g.surface.configure(&g.device, &g.surface_cfg);

            let is_max = g.window.is_maximized();
            if restored && g.pending_remax && !is_max {
                g.pending_remax = false;
                g.window.set_maximized(true);
                g.pending_frames = g.pending_frames.max(2);
                g.window.request_redraw();
                return;
            }
            if g.app_state.titlebar.maximized != is_max {
                g.app_state.titlebar.set_maximized(is_max);
            }

            #[cfg(windows)]
            if let Some(hwnd) = win32::hwnd_of(&g.window) {
                win32::update_rounded_region(hwnd, app.config.corner_radius);
            }
            g.pending_frames = g.pending_frames.max(2);
            g.window.request_redraw();
        }

        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            // The user dragged the window between monitors of
            // differing DPI. Rebuild the font atlas at the new
            // scale so text doesn't render blurry through a 2× /
            // 0.5× upscale.
            gpu::rebuild_fonts_for_scale(
                &mut g.context,
                &mut g.renderer,
                &app.config,
                scale_factor as f32,
            );
            g.pending_frames = g.pending_frames.max(2);
            g.window.request_redraw();
        }

        WindowEvent::RedrawRequested => {
            gpu::render_frame(g, &mut app.config, handler, event_loop);
        }

        _ => {}
    }
}

pub(super) fn schedule<H: AppHandler + 'static>(
    app: &mut super::WinitApp<H>,
    event_loop: &winit::event_loop::ActiveEventLoop,
) {
    let Some(g) = app.gpu.as_ref() else { return };

    // ── 1. Minimized: park the loop entirely ─────────────────────
    // The OS will wake us on restore / close / focus / shell event.
    // Until then CPU and GPU draw zero work.
    if g.was_minimized {
        event_loop.set_control_flow(ControlFlow::Wait);
        return;
    }

    // ── 2. Continuous-render mode ────────────────────────────────
    // Game-style: every loop iteration repaints, gated by `fps_mode`
    // and `unfocused_fps`.
    if !g.event_driven {
        g.window.request_redraw();
        let interval = if !g.focused && g.unfocused_fps_interval > Duration::ZERO {
            g.unfocused_fps_interval
        } else {
            g.fps_interval
        };
        if interval > Duration::ZERO {
            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + interval));
        } else {
            event_loop.set_control_flow(ControlFlow::Poll);
        }
        return;
    }

    // ── 3. Event-driven mode (default) ───────────────────────────
    //
    // Pending frames in flight. Re-arm the redraw — winit only
    // buffers a single paint event per call to `request_redraw`, so
    // any `pending_frames > 1` budget needs to be reissued each loop
    // iteration. The next iteration's `RedrawRequested` decrements
    // it by exactly one.
    if g.pending_frames > 0 {
        g.window.request_redraw();
        event_loop.set_control_flow(ControlFlow::Wait);
        return;
    }

    // Pick the focused / unfocused idle pulse. Either (or both) can
    // be `None` — `event_driven_minimal()` disables both, giving
    // strictly-zero-idle behaviour.
    let pulse = if g.focused {
        g.idle_pulse
    } else {
        g.unfocused_idle_pulse
    };

    match pulse {
        Some(dt) => {
            let next = g.last_redraw + dt;
            let now = Instant::now();
            if next <= now {
                g.window.request_redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(next));
            }
        }
        None => {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}
