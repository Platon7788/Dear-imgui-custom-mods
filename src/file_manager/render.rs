//! UI rendering functions for the file manager dialog.
//!
//! All rendering is implemented as free-standing `pub(crate)` functions rather
//! than methods on `FileManager`. This avoids borrow conflicts: each function
//! borrows only the data it needs, allowing `FileManager` to hold `&mut self`
//! references to other fields simultaneously.
//!
//! ## Render pipeline
//!
//! Called from [`FileManager::render()`](super::FileManager::render) in this order:
//!
//! 1. [`render_drive_bar()`] — row of drive/root buttons
//! 2. [`render_toolbar()`] — Back, Forward, Up, New Folder, New File, Refresh
//! 3. [`render_breadcrumb_bar()`] — clickable path segments or text input
//! 4. [`render_favorites_panel()`] — left sidebar (if enabled)
//! 5. [`render_file_table()`] — 4-column ImGui Table with ListClipper
//! 6. [`render_footer()`] — filter dropdown + Confirm/Cancel buttons
//! 7. [`render_overwrite_confirm()`] — nested modal (SaveFile only)
//!
//! Each function returns `Option<Action>` for deferred state mutations.

use std::fmt::Write;
use std::path::Path;

use dear_imgui_rs::{
    Key, ListClipper, MouseButton, SelectableFlags, StyleColor, StyleVar, TableColumnFlags,
    TableFlags, Ui, WindowFlags,
};

use crate::{icons, theme};

use super::config::{DialogMode, FileFilter, FileManagerConfig, FmStrings};
use super::entry::{FsEntry, SortColumn, SortOrder};
use super::favorites::FavoritesPanel;
use super::Action;

// ─── Theme-derived button color presets ──────────────────────────────────────

/// Warm gold text color for folder names in the file table.
const CLR_FOLDER_TEXT: [f32; 4] = [0.88, 0.82, 0.55, 1.0];

/// Pack three button state colors into `[base, hovered, active]`.
fn btn_colors(base: [f32; 4], hover: [f32; 4], active: [f32; 4]) -> [[f32; 4]; 3] {
    [base, hover, active]
}

/// Navigation button colors (Back, Forward, Up, Refresh) — subtle background.
fn nav_btn() -> [[f32; 4]; 3] {
    btn_colors(theme::BG_CHILD, theme::BG_CHILD_HOVER, theme::BG_FRAME)
}

/// Confirm/success button colors (Open, Save, Select Folder, Create, Yes).
fn confirm_btn() -> [[f32; 4]; 3] {
    btn_colors(theme::SUCCESS, theme::SUCCESS_HOVER, theme::SUCCESS_ACTIVE)
}

/// Cancel/danger button colors (Cancel, No).
fn cancel_btn() -> [[f32; 4]; 3] {
    btn_colors(theme::DANGER, theme::DANGER_HOVER, theme::DANGER_ACTIVE)
}

// ─── Scratch buffer helper ──────────────────────────────────────────────────

/// Write `"icon text"` into the scratch buffer, return a borrowed `&str`.
/// Reuses the same allocation across all calls — zero per-frame alloc.
fn icon_label<'a>(buf: &'a mut String, icon: &str, text: &str) -> &'a str {
    buf.clear();
    buf.push_str(icon);
    buf.push(' ');
    buf.push_str(text);
    buf.as_str()
}

// ─── File type icon mapping ──────────────────────────────────────────────────

/// Return (icon, color) for a file based on its lowercase extension.
///
/// Covers 100+ file types grouped by category. Uses Material Design Icons (MDI).
/// Users can override individual mappings via `FileManagerConfig::icon_override`.
fn file_icon_for_ext(ext: &str) -> (&'static str, [f32; 4]) {
    match ext {
        // ── Rust ──
        "rs" | "toml" => (icons::LANGUAGE_RUST, [0.87, 0.49, 0.26, 1.0]),
        "lock" => (icons::LOCK, [0.60, 0.60, 0.60, 1.0]),

        // ── Python ──
        "py" | "pyi" | "pyw" | "pyx" | "pxd" => (icons::LANGUAGE_PYTHON, [0.30, 0.60, 0.88, 1.0]),
        "ipynb" => (icons::NOTEBOOK, [0.90, 0.55, 0.20, 1.0]),

        // ── JavaScript / TypeScript ──
        "js" | "mjs" | "cjs" | "jsx" => (icons::LANGUAGE_JAVASCRIPT, [0.95, 0.85, 0.30, 1.0]),
        "ts" | "tsx" | "mts" | "cts" => (icons::LANGUAGE_TYPESCRIPT, [0.20, 0.50, 0.85, 1.0]),

        // ── C / C++ / Objective-C ──
        "c" | "h" => (icons::LANGUAGE_C, [0.40, 0.55, 0.80, 1.0]),
        "cpp" | "cxx" | "cc" | "hpp" | "hxx" | "hh" | "inl" => {
            (icons::LANGUAGE_CPP, [0.40, 0.55, 0.80, 1.0])
        }
        "m" | "mm" => (icons::APPLE, [0.60, 0.60, 0.70, 1.0]),

        // ── C# / F# ──
        "cs" | "csx" => (icons::LANGUAGE_CSHARP, [0.55, 0.35, 0.78, 1.0]),
        "fs" | "fsx" | "fsi" => (icons::LANGUAGE_CSHARP, [0.30, 0.55, 0.75, 1.0]),

        // ── Java / Kotlin / Scala / Groovy ──
        "java" | "class" | "jar" => (icons::LANGUAGE_JAVA, [0.80, 0.35, 0.25, 1.0]),
        "kt" | "kts" => (icons::LANGUAGE_KOTLIN, [0.60, 0.40, 0.85, 1.0]),
        "scala" | "sc" => (icons::FILE_CODE, [0.85, 0.30, 0.25, 1.0]),
        "groovy" | "gradle" => (icons::LANGUAGE_JAVA, [0.40, 0.60, 0.50, 1.0]),

        // ── Go ──
        "go" | "mod" | "sum" => (icons::LANGUAGE_GO, [0.00, 0.68, 0.84, 1.0]),

        // ── Swift / Dart ──
        "swift" => (icons::LANGUAGE_SWIFT, [0.95, 0.45, 0.25, 1.0]),
        "dart" => (icons::GOOGLE, [0.30, 0.70, 0.90, 1.0]),

        // ── Ruby / Perl / PHP / Lua ──
        "rb" | "rake" | "gemspec" => (icons::LANGUAGE_RUBY, [0.85, 0.20, 0.20, 1.0]),
        "pl" | "pm" | "pod" => (icons::FILE_CODE, [0.40, 0.55, 0.70, 1.0]),
        "php" | "phtml" => (icons::LANGUAGE_PHP, [0.55, 0.55, 0.80, 1.0]),
        "lua" => (icons::LANGUAGE_LUA, [0.20, 0.20, 0.80, 1.0]),

        // ── Haskell / Elixir / Erlang / R ──
        "hs" | "lhs" => (icons::LANGUAGE_HASKELL, [0.55, 0.45, 0.65, 1.0]),
        "ex" | "exs" => (icons::WATER, [0.45, 0.30, 0.60, 1.0]),
        "erl" | "hrl" => (icons::FILE_CODE, [0.70, 0.20, 0.30, 1.0]),
        "r" | "rmd" => (icons::LANGUAGE_R, [0.28, 0.48, 0.75, 1.0]),

        // ── Web ──
        "html" | "htm" | "xhtml" => (icons::LANGUAGE_HTML5, [0.90, 0.35, 0.20, 1.0]),
        "css" | "scss" | "sass" | "less" | "styl" => {
            (icons::LANGUAGE_CSS3, [0.20, 0.55, 0.85, 1.0])
        }
        "vue" => (icons::VUEJS, [0.30, 0.75, 0.55, 1.0]),
        "svelte" => (icons::FILE_CODE, [0.95, 0.30, 0.15, 1.0]),
        "wasm" => (icons::WEB, [0.40, 0.35, 0.80, 1.0]),

        // ── Data / Config ──
        "json" | "jsonc" | "json5" | "geojson" => (icons::CODE_JSON, [0.90, 0.80, 0.30, 1.0]),
        "xml" | "xsl" | "xslt" | "xsd" | "dtd" => (icons::XML, [0.85, 0.55, 0.20, 1.0]),
        "yaml" | "yml" => (icons::FILE_COG, theme::TEXT_SECONDARY),
        "ini" | "cfg" | "conf" | "properties" | "env" => {
            (icons::COG_OUTLINE, theme::TEXT_SECONDARY)
        }
        "csv" | "tsv" => (icons::FILE_DELIMITED, [0.20, 0.60, 0.30, 1.0]),
        "sql" | "sqlite" | "db" => (icons::DATABASE, [0.55, 0.45, 0.70, 1.0]),
        "graphql" | "gql" => (icons::GRAPH_OUTLINE, [0.85, 0.25, 0.55, 1.0]),
        "proto" | "protobuf" => (icons::FILE_CODE, [0.50, 0.65, 0.45, 1.0]),

        // ── Documents ──
        "pdf" => (icons::FILE_PDF_BOX, [0.85, 0.25, 0.22, 1.0]),
        "doc" | "docx" | "odt" | "rtf" => (icons::FILE_WORD, [0.25, 0.45, 0.80, 1.0]),
        "xls" | "xlsx" | "ods" => (icons::FILE_EXCEL, [0.20, 0.60, 0.30, 1.0]),
        "ppt" | "pptx" | "odp" => (icons::FILE_POWERPOINT, [0.85, 0.40, 0.20, 1.0]),
        "txt" | "log" | "readme" | "nfo" | "diz" => (icons::TEXT_BOX, theme::TEXT_SECONDARY),
        "md" | "mdx" | "rst" | "adoc" | "tex" | "latex" => {
            (icons::LANGUAGE_MARKDOWN, [0.50, 0.70, 0.90, 1.0])
        }
        "epub" | "mobi" | "azw" => (icons::BOOK_OPEN_VARIANT, [0.65, 0.50, 0.35, 1.0]),

        // ── Images ──
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp" | "tiff" | "tif" | "tga"
        | "hdr" | "exr" => (icons::FILE_IMAGE, [0.45, 0.75, 0.45, 1.0]),
        "svg" => (icons::SVG, [0.90, 0.65, 0.20, 1.0]),
        "psd" | "ai" | "sketch" | "fig" | "xd" => {
            (icons::PALETTE, [0.35, 0.65, 0.95, 1.0])
        }
        "blend" | "fbx" | "obj" | "stl" | "gltf" | "glb" | "3ds" | "dae" => {
            (icons::CUBE_OUTLINE, [0.80, 0.60, 0.40, 1.0])
        }

        // ── Audio ──
        "mp3" | "wav" | "ogg" | "flac" | "aac" | "wma" | "m4a" | "opus" | "ape" | "aiff" => {
            (icons::FILE_MUSIC, [0.70, 0.45, 0.80, 1.0])
        }
        "mid" | "midi" => (icons::PIANO, [0.55, 0.55, 0.70, 1.0]),

        // ── Video ──
        "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg"
        | "vob" => (icons::FILE_VIDEO, [0.85, 0.55, 0.35, 1.0]),
        "srt" | "sub" | "ssa" | "ass" | "vtt" => {
            (icons::SUBTITLES_OUTLINE, [0.65, 0.65, 0.50, 1.0])
        }

        // ── Archives ──
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "lz" | "lzma"
        | "cab" | "iso" | "dmg" | "img" => (icons::ZIP_BOX, [0.75, 0.65, 0.40, 1.0]),
        "deb" | "rpm" | "pkg" | "apk" | "snap" | "flatpak" | "appimage" => {
            (icons::PACKAGE_VARIANT_CLOSED, [0.50, 0.70, 0.50, 1.0])
        }

        // ── Executables / Libraries ──
        "exe" | "msi" | "com" => (icons::APPLICATION, [0.70, 0.70, 0.70, 1.0]),
        "dll" | "so" | "dylib" | "a" | "lib" | "o" => {
            (icons::PUZZLE, [0.60, 0.60, 0.70, 1.0])
        }
        "bin" | "dat" | "raw" => (icons::FILE, [0.55, 0.55, 0.55, 1.0]),

        // ── Shell scripts ──
        "sh" | "bash" | "zsh" | "fish" => {
            (icons::CONSOLE, [0.50, 0.75, 0.50, 1.0])
        }
        "bat" | "cmd" | "ps1" | "psm1" => {
            (icons::POWERSHELL, [0.30, 0.45, 0.70, 1.0])
        }

        // ── DevOps / CI ──
        "dockerfile" | "containerfile" => (icons::DOCKER, [0.20, 0.60, 0.85, 1.0]),
        "tf" | "hcl" => (icons::TERRAFORM, [0.40, 0.35, 0.75, 1.0]),
        "nix" => (icons::SNOWFLAKE, [0.45, 0.60, 0.85, 1.0]),

        // ── Fonts ──
        "ttf" | "otf" | "woff" | "woff2" | "eot" => {
            (icons::FORMAT_FONT, [0.65, 0.65, 0.70, 1.0])
        }

        // ── Certificates / Keys ──
        "pem" | "crt" | "cer" | "key" | "p12" | "pfx" | "csr" => {
            (icons::CERTIFICATE, [0.85, 0.70, 0.25, 1.0])
        }
        "pub" | "gpg" | "asc" => (icons::KEY, [0.75, 0.65, 0.30, 1.0]),

        // ── Misc ──
        "gitignore" | "gitattributes" | "gitmodules" => {
            (icons::GIT, [0.90, 0.35, 0.20, 1.0])
        }
        "editorconfig" | "prettierrc" | "eslintrc" => {
            (icons::COG_OUTLINE, theme::TEXT_SECONDARY)
        }
        "license" | "licence" => (icons::SCALE_BALANCE, [0.70, 0.70, 0.50, 1.0]),

        // ── Default ──
        _ => (icons::FILE, theme::ACCENT),
    }
}

// ─── Push button style (3-color) ────────────────────────────────────────────

/// Apply a 3-color button style (base, hovered, active) for the duration of `f`.
fn with_btn_style<R>(ui: &Ui, colors: [[f32; 4]; 3], f: impl FnOnce() -> R) -> R {
    let _c0 = ui.push_style_color(StyleColor::Button, colors[0]);
    let _c1 = ui.push_style_color(StyleColor::ButtonHovered, colors[1]);
    let _c2 = ui.push_style_color(StyleColor::ButtonActive, colors[2]);
    f()
}

// ─── Drive bar ──────────────────────────────────────────────────────────────

/// Render a horizontal row of drive buttons (e.g. `C:\`, `D:\`).
///
/// The current drive is highlighted with accent colors.
/// Clicking a drive navigates to its root.
pub(crate) fn render_drive_bar(
    ui: &Ui,
    drives: &[String],
    current_drive: Option<char>,
    buf: &mut String,
) -> Option<Action> {
    let mut action = None;
    let _spacing = ui.push_style_var(StyleVar::ItemSpacing([4.0, 4.0]));
    let _rounding = ui.push_style_var(StyleVar::FrameRounding(4.0));

    for drive in drives {
        let drive_letter = drive.chars().next().unwrap_or('?');
        let is_current = current_drive == Some(drive_letter);

        let colors = if is_current {
            btn_colors(theme::ACCENT, theme::ACCENT_HOVER, theme::ACCENT_ACTIVE)
        } else {
            nav_btn()
        };

        let label = icon_label(buf, icons::HARDDISK, drive);
        with_btn_style(ui, colors, || {
            if is_current {
                let _tc = ui.push_style_color(StyleColor::Text, [0.90, 0.94, 1.00, 1.0]);
                if ui.button(label) {
                    action = Some(Action::NavigateTo(std::path::PathBuf::from(drive.as_str())));
                }
            } else if ui.button(label) {
                action = Some(Action::NavigateTo(std::path::PathBuf::from(drive.as_str())));
            }
        });
        ui.same_line();
    }
    ui.new_line();
    action
}

// ─── Breadcrumb bar ─────────────────────────────────────────────────────────

/// Render the breadcrumb path bar with two modes:
///
/// - **Browse mode** (default): clickable path segments separated by chevrons.
///   Clicking a segment navigates to that directory. Double-clicking the empty
///   area switches to edit mode.
/// - **Edit mode**: a text input where the user can type a path directly.
///   Enter navigates, Escape cancels.
pub(crate) fn render_breadcrumb_bar(
    ui: &Ui,
    path: &Path,
    editing: &mut bool,
    path_buf: &mut String,
    buf: &mut String,
) -> Option<Action> {
    // If editing, show text input
    if *editing {
        let _bg = ui.push_style_color(StyleColor::FrameBg, theme::BG_FRAME);
        ui.text_colored(theme::WARNING, icons::FOLDER_OPEN);
        ui.same_line_with_spacing(0.0, 6.0);
        ui.set_next_item_width(ui.content_region_avail()[0]);
        let enter = ui
            .input_text("##pathbar", path_buf)
            .enter_returns_true(true)
            .build();

        if enter {
            *editing = false;
            return Some(Action::NavigateToInput(path_buf.clone()));
        }
        if ui.is_key_pressed(Key::Escape) {
            *editing = false;
            path_buf.clear();
            path_buf.push_str(&path.to_string_lossy());
        }
        return None;
    }

    // Breadcrumb mode: clickable path segments
    let _bg = ui.push_style_color(StyleColor::ChildBg, theme::BG_FRAME);

    // Collect components
    let path_str = path.to_string_lossy();
    let segments: Vec<&str> = path_str
        .split(['\\', '/'])
        .filter(|s| !s.is_empty())
        .collect();

    ui.text_colored(theme::WARNING, icons::FOLDER_OPEN);
    ui.same_line_with_spacing(0.0, 6.0);

    // Breadcrumb overflow: if too many segments, collapse early ones into "..."
    let avail_w = ui.content_region_avail()[0];
    let max_visible = if avail_w < 300.0 {
        2
    } else if avail_w < 500.0 {
        3
    } else {
        segments.len()
    };
    let skip_count = segments.len().saturating_sub(max_visible);

    for (i, segment) in segments.iter().enumerate() {
        if i > 0 {
            ui.same_line_with_spacing(0.0, 1.0);
            ui.text_colored(theme::TEXT_MUTED, icons::CHEVRON_RIGHT);
            ui.same_line_with_spacing(0.0, 1.0);
        }

        // Collapse early segments into "..."
        if i > 0 && i < skip_count {
            continue;
        }
        if i == 1 && skip_count > 1 {
            ui.text_colored(theme::TEXT_MUTED, "...");
            ui.same_line_with_spacing(0.0, 1.0);
            continue;
        }

        let _id = ui.push_id(i);
        // Add drive separator back for first segment on Windows
        buf.clear();
        buf.push_str(segment);
        if ui.small_button(buf.as_str()) {
            // Reconstruct path up to this segment
            let first = segments[0];
            let mut target = if first.ends_with(':') {
                // Windows drive root: "C:" → "C:\"
                std::path::PathBuf::from(format!("{first}\\"))
            } else {
                std::path::PathBuf::from(first)
            };
            for s in &segments[1..=i] {
                target.push(s);
            }
            return Some(Action::NavigateTo(target));
        }
    }

    // Double-click on empty area → switch to edit mode
    ui.same_line();
    let avail = ui.content_region_avail()[0];
    if avail > 20.0 {
        ui.invisible_button("##path_edit_zone", [avail, ui.text_line_height()]);
        if ui.is_item_hovered() && ui.is_mouse_double_clicked(MouseButton::Left) {
            *editing = true;
            path_buf.clear();
            path_buf.push_str(&path.to_string_lossy());
        }
    }

    None
}

// ─── Toolbar ────────────────────────────────────────────────────────────────

/// Render the navigation toolbar: Back, Forward, Up, New Folder, New File, Refresh.
///
/// Disabled buttons are shown as grayed-out text. The "New Folder" / "New File"
/// buttons toggle inline input fields with Create/Cancel buttons.
/// Only one inline input can be open at a time.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_toolbar(
    ui: &Ui,
    strings: &FmStrings,
    has_parent: bool,
    can_back: bool,
    can_forward: bool,
    show_new_folder: &mut bool,
    new_folder_buf: &mut String,
    show_new_file: &mut bool,
    new_file_buf: &mut String,
    show_hidden: bool,
    config: &FileManagerConfig, // NOTE: added parameter — update call sites in mod.rs
    buf: &mut String,
) -> Option<Action> {
    let mut action = None;
    let _spacing = ui.push_style_var(StyleVar::ItemSpacing([6.0, 4.0]));

    // Back button
    if can_back {
        let label = icon_label(buf, icons::ARROW_LEFT, strings.back);
        with_btn_style(ui, nav_btn(), || {
            if ui.button(label) {
                action = Some(Action::GoBack);
            }
        });
    } else {
        let label = icon_label(buf, icons::ARROW_LEFT, strings.back);
        ui.text_disabled(label);
    }
    ui.same_line();

    // Forward button
    if can_forward {
        let label = icon_label(buf, icons::ARROW_RIGHT, strings.forward);
        with_btn_style(ui, nav_btn(), || {
            if ui.button(label) {
                action = Some(Action::GoForward);
            }
        });
    } else {
        let label = icon_label(buf, icons::ARROW_RIGHT, strings.forward);
        ui.text_disabled(label);
    }
    ui.same_line();

    // Up button
    if has_parent {
        let label = icon_label(buf, icons::ARROW_UP, strings.up);
        with_btn_style(ui, nav_btn(), || {
            if ui.button(label) {
                action = Some(Action::GoParent);
            }
        });
    } else {
        let label = icon_label(buf, icons::ARROW_UP, strings.up);
        ui.text_disabled(label);
    }
    ui.same_line();

    // New folder button
    {
        let nf_colors = confirm_btn();
        let label = icon_label(buf, icons::FOLDER_PLUS, strings.new_folder);
        with_btn_style(ui, nf_colors, || {
            if ui.button(label) {
                let opening = !*show_new_folder;
                *show_new_folder = opening;
                if opening {
                    *show_new_file = false;
                    new_folder_buf.clear();
                }
            }
        });
    }
    ui.same_line();

    // New file button
    {
        let nf_colors = btn_colors(theme::ACCENT, theme::ACCENT_HOVER, theme::ACCENT_ACTIVE);
        let label = icon_label(buf, icons::FILE_PLUS, strings.new_file);
        with_btn_style(ui, nf_colors, || {
            if ui.button(label) {
                let opening = !*show_new_file;
                *show_new_file = opening;
                if opening {
                    *show_new_folder = false;
                    new_file_buf.clear();
                }
            }
        });
    }
    ui.same_line();

    // Refresh button
    {
        buf.clear();
        let _ = write!(buf, "{}##refresh", icons::REFRESH);
        with_btn_style(ui, nav_btn(), || {
            if ui.button(buf.as_str()) {
                action = Some(Action::Refresh);
            }
        });
    }
    ui.same_line();

    // Hidden files toggle
    {
        let icon = if show_hidden {
            icons::EYE
        } else {
            icons::EYE_OFF_OUTLINE
        };
        let label = icon_label(buf, icon, strings.show_hidden);
        let colors = if show_hidden {
            btn_colors(theme::ACCENT, theme::ACCENT_HOVER, theme::ACCENT_ACTIVE)
        } else {
            nav_btn()
        };
        with_btn_style(ui, colors, || {
            if ui.button(label) {
                action = Some(Action::ToggleHidden);
            }
        });
    }

    // New folder inline input
    if *show_new_folder {
        let input_w = if config.inline_input_width > 0.0 {
            config.inline_input_width
        } else {
            ui.content_region_avail()[0].min(300.0)
        };
        ui.set_next_item_width(input_w);
        if !ui.is_any_item_active() {
            ui.set_keyboard_focus_here();
        }
        let enter = ui
            .input_text("##newfolder", new_folder_buf)
            .enter_returns_true(true)
            .build();
        ui.same_line();

        with_btn_style(ui, confirm_btn(), || {
            buf.clear();
            let _ = write!(buf, "{}##nf_create", strings.create);
            if (ui.button(buf.as_str()) || enter) && !new_folder_buf.is_empty() {
                action = Some(Action::CreateFolder(new_folder_buf.clone()));
            }
        });
        ui.same_line();
        with_btn_style(ui, cancel_btn(), || {
            buf.clear();
            let _ = write!(buf, "{}##nf_cancel", strings.cancel);
            if ui.button(buf.as_str()) {
                *show_new_folder = false;
                new_folder_buf.clear();
            }
        });
    }

    // New file inline input
    if *show_new_file {
        let input_w = if config.inline_input_width > 0.0 {
            config.inline_input_width
        } else {
            ui.content_region_avail()[0].min(300.0)
        };
        ui.set_next_item_width(input_w);
        if !ui.is_any_item_active() {
            ui.set_keyboard_focus_here();
        }
        let enter = ui
            .input_text("##newfile", new_file_buf)
            .enter_returns_true(true)
            .build();
        ui.same_line();

        with_btn_style(ui, confirm_btn(), || {
            buf.clear();
            let _ = write!(buf, "{}##nfile_create", strings.create);
            if (ui.button(buf.as_str()) || enter) && !new_file_buf.is_empty() {
                action = Some(Action::CreateFile(new_file_buf.clone()));
            }
        });
        ui.same_line();
        with_btn_style(ui, cancel_btn(), || {
            buf.clear();
            let _ = write!(buf, "{}##nfile_cancel", strings.cancel);
            if ui.button(buf.as_str()) {
                *show_new_file = false;
                new_file_buf.clear();
            }
        });
    }

    action
}

// ─── Favorites panel ────────────────────────────────────────────────────────

/// Render the favorites sidebar (Desktop, Documents, Downloads, custom bookmarks).
///
/// Each entry is a selectable row with an icon. The current directory is highlighted.
pub(crate) fn render_favorites_panel(
    ui: &Ui,
    favorites: &FavoritesPanel,
    current_path: &Path,
    strings: &FmStrings,
    buf: &mut String,
) -> Option<Action> {
    let mut action = None;

    ui.text_colored(theme::TEXT_SECONDARY, icons::STAR);
    ui.same_line_with_spacing(0.0, 4.0);
    ui.text_colored(theme::TEXT_SECONDARY, strings.favorites);
    ui.separator();

    for (i, fav) in favorites.entries.iter().enumerate() {
        let _id = ui.push_id(i);
        let is_current = fav.path == current_path;

        let label = icon_label(buf, fav.icon, &fav.label);

        // Guard must live until after selectable_config().build()
        let _bg = is_current
            .then(|| ui.push_style_color(StyleColor::Header, theme::ACCENT_ACTIVE));

        if ui
            .selectable_config(label)
            .selected(is_current)
            .build()
            && fav.path.is_dir()
        {
            action = Some(Action::NavigateTo(fav.path.clone()));
        }
    }

    action
}

// ─── File table (ImGui Table with 4 columns) ────────────────────────────────

/// Result from [`render_file_table()`] — contains a deferred action if any.
/// Combined result from [`render_file_table()`]: a deferred action and/or a delete request.
pub(crate) struct FileTableResult {
    /// Deferred navigation/selection action to apply after the frame.
    pub action: Option<Action>,
    /// Index of entry the user wants to delete (needs confirmation first).
    pub request_delete: Option<usize>,
}

/// Render the main file listing as a 4-column ImGui Table.
///
/// ## Columns
///
/// | # | Name | Width | Content |
/// |---|------|-------|---------|
/// | 0 | Name | stretch | icon + filename (selectable spanning all columns) |
/// | 1 | Size | 80px | pre-computed human-readable size |
/// | 2 | Date Modified | 140px | pre-computed "YYYY-MM-DD HH:MM" |
/// | 3 | Type | 70px | extension or "Folder" |
///
/// ## Features
///
/// - **ListClipper**: only visible rows are rendered (virtualization)
/// - **Sortable headers**: click to sort, uses `table_get_sort_specs()`
/// - **Click/Ctrl+Click**: single or multi-select
/// - **Double-click**: navigate into directory or confirm file selection
/// - **Keyboard**: Up/Down arrows, Enter, Backspace (when no text input is active)
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_file_table(
    ui: &Ui,
    entries: &[FsEntry],
    selected_indices: &mut Vec<usize>,
    mode: DialogMode,
    multi_select: bool,
    filename_buf: &mut String,
    strings: &FmStrings,
    has_error: bool,
    sort_column: &mut SortColumn,
    sort_order: &mut SortOrder,
    rename_index: &mut Option<usize>,
    rename_buf: &mut String,
    context_menu_target: &mut Option<usize>,
    last_click_index: &mut Option<usize>,
    scroll_to_index: &mut Option<usize>,
    config: &FileManagerConfig,
    buf: &mut String,
) -> FileTableResult {
    let mut result = FileTableResult {
        action: None,
        request_delete: None,
    };

    let flags = TableFlags::RESIZABLE
        | TableFlags::SORTABLE
        | TableFlags::ROW_BG
        | TableFlags::SCROLL_Y
        | TableFlags::BORDERS_INNER_H
        | TableFlags::BORDERS_OUTER_H
        | TableFlags::BORDERS_OUTER_V
        | TableFlags::SIZING_FIXED_FIT;

    // Dynamic column count based on config
    let col_count = 1
        + config.show_column_size as usize
        + config.show_column_date as usize
        + config.show_column_type as usize;

    let _table = match ui.begin_table_with_flags("##file_table", col_count, flags) {
        Some(t) => t,
        None => return result,
    };

    // Column setup — Name always present, others optional
    ui.table_setup_column(
        strings.col_name,
        TableColumnFlags::WIDTH_STRETCH | TableColumnFlags::PREFER_SORT_ASCENDING,
        0.0,
        0,
    );
    if config.show_column_size {
        ui.table_setup_column(strings.col_size, TableColumnFlags::WIDTH_FIXED, 80.0, 1);
    }
    if config.show_column_date {
        ui.table_setup_column(strings.col_date, TableColumnFlags::WIDTH_FIXED, 140.0, 2);
    }
    if config.show_column_type {
        ui.table_setup_column(strings.col_type, TableColumnFlags::WIDTH_FIXED, 70.0, 3);
    }
    ui.table_setup_scroll_freeze(0, 1);
    ui.table_headers_row();

    // Sort handling
    if let Some(mut specs) = ui.table_get_sort_specs()
        && specs.is_dirty()
    {
        if let Some(s) = specs.iter().next() {
            let new_col = match s.column_user_id {
                1 => SortColumn::Size,
                2 => SortColumn::DateModified,
                3 => SortColumn::Type,
                _ => SortColumn::Name,
            };
            let new_order =
                if s.sort_direction == dear_imgui_rs::SortDirection::Ascending {
                    SortOrder::Ascending
                } else {
                    SortOrder::Descending
                };
            *sort_column = new_col;
            *sort_order = new_order;
            result.action = Some(Action::SetSort(new_col));
        }
        specs.clear_dirty();
    }

    if entries.is_empty() && !has_error {
        ui.table_next_row();
        ui.table_next_column();
        ui.text_disabled(strings.empty_parens);
    } else {
        // ListClipper for virtualization
        let clip = ListClipper::new(entries.len() as i32);
        let tok = clip.begin(ui);

        for row_idx in tok.iter() {
            let idx = row_idx as usize;
            let e = &entries[idx];
            let is_selected = selected_indices.contains(&idx);
            let is_renaming = *rename_index == Some(idx);

            ui.table_next_row();

            // Scroll to this row if requested (keyboard nav or type-to-search)
            if *scroll_to_index == Some(idx) {
                ui.set_scroll_here_y(0.5);
                *scroll_to_index = None;
            }

            // Column 0: Name with icon (selectable spanning all columns)
            ui.table_next_column();
            let _row_id = ui.push_id(idx);

            // Determine file icon
            let (file_icon, file_icon_color) = if e.is_dir {
                (icons::FOLDER, theme::WARNING)
            } else if let Some(f) = config.icon_override
                && let Some(result) = f(&e.extension)
            {
                result
            } else {
                file_icon_for_ext(&e.extension)
            };

            if is_renaming {
                // Inline rename input
                ui.text_colored(file_icon_color, file_icon);
                ui.same_line_with_spacing(0.0, 4.0);
                let rename_w = if config.inline_input_width > 0.0 {
                    ui.content_region_avail()[0].min(config.inline_input_width.max(200.0))
                } else {
                    ui.content_region_avail()[0].min(300.0)
                };
                ui.set_next_item_width(rename_w);
                if !ui.is_any_item_active() {
                    ui.set_keyboard_focus_here();
                }
                let enter = ui
                    .input_text("##rename", rename_buf)
                    .enter_returns_true(true)
                    .build();

                if enter && !rename_buf.is_empty() {
                    result.action = Some(Action::RenameEntry {
                        index: idx,
                        new_name: rename_buf.clone(),
                    });
                }
            } else {
                if ui
                    .selectable_config("##sel")
                    .flags(
                        SelectableFlags::SPAN_ALL_COLUMNS
                            | SelectableFlags::ALLOW_DOUBLE_CLICK
                            | SelectableFlags::ALLOW_OVERLAP,
                    )
                    .selected(is_selected)
                    .build()
                {
                    // Selection logic
                    let io = ui.io();
                    if multi_select && mode == DialogMode::OpenFile {
                        if io.key_shift() {
                            // Shift+Click: range select from last click to current
                            let anchor = last_click_index.unwrap_or(0);
                            let lo = anchor.min(idx);
                            let hi = anchor.max(idx);
                            selected_indices.clear();
                            for r in lo..=hi {
                                selected_indices.push(r);
                            }
                        } else if io.key_ctrl() {
                            // Ctrl+Click: toggle individual selection
                            if let Some(pos) = selected_indices.iter().position(|&r| r == idx) {
                                selected_indices.remove(pos);
                            } else {
                                selected_indices.push(idx);
                            }
                        } else {
                            selected_indices.clear();
                            selected_indices.push(idx);
                        }
                    } else {
                        selected_indices.clear();
                        selected_indices.push(idx);
                    }
                    *last_click_index = Some(idx);

                    // Update filename buf for SaveFile mode
                    if mode == DialogMode::SaveFile && !e.is_dir {
                        filename_buf.clear();
                        filename_buf.push_str(&e.name);
                    }

                    // Double-click handling
                    if ui.is_mouse_double_clicked(MouseButton::Left) {
                        if e.is_dir {
                            result.action = Some(Action::NavigateTo(e.path.clone()));
                        } else {
                            result.action = Some(Action::ConfirmSelection);
                        }
                    }
                }

                // Right-click context menu trigger
                if ui.is_item_clicked_with_button(MouseButton::Right) {
                    *context_menu_target = Some(idx);
                    selected_indices.clear();
                    selected_indices.push(idx);
                    ui.open_popup("##ctx_menu");
                }

                // Render icon + name on top of the selectable
                ui.same_line_with_spacing(0.0, 0.0);
                let cursor = ui.cursor_pos();
                ui.set_cursor_pos([cursor[0] + 4.0, cursor[1]]);

                let alpha = if e.is_hidden { 0.55 } else { 1.0 };
                let icon_clr = [
                    file_icon_color[0],
                    file_icon_color[1],
                    file_icon_color[2],
                    alpha,
                ];
                ui.text_colored(icon_clr, file_icon);
                ui.same_line_with_spacing(0.0, 4.0);
                if e.is_dir {
                    ui.text_colored(
                        [CLR_FOLDER_TEXT[0], CLR_FOLDER_TEXT[1], CLR_FOLDER_TEXT[2], alpha],
                        &e.name,
                    );
                } else if e.is_hidden {
                    ui.text_colored(theme::TEXT_MUTED, &e.name);
                } else {
                    ui.text(&e.name);
                }

                // Tooltip for truncated names (show full name on hover if clipped)
                if ui.is_item_hovered() {
                    let item_w = ui.item_rect_size()[0];
                    // Approximate: if name is long enough to likely be clipped
                    let text_w = crate::utils::text::calc_text_size(&e.name)[0];
                    if text_w > item_w {
                        ui.tooltip_text(&e.name);
                    }
                }
            }

            // Column 1: Size (if visible)
            if config.show_column_size {
                ui.table_next_column();
                if !e.is_dir {
                    ui.text_colored(theme::TEXT_SECONDARY, &e.size_display);
                }
            }

            // Column 2: Date Modified (if visible)
            if config.show_column_date {
                ui.table_next_column();
                if !e.date_display.is_empty() {
                    ui.text_colored(theme::TEXT_SECONDARY, &e.date_display);
                }
            }

            // Column 3: Type (if visible)
            if config.show_column_type {
                ui.table_next_column();
                ui.text_colored(theme::TEXT_MUTED, &e.type_display);
            }
        }

        // ── Context menu popup ──
        if let Some(_tok) = ui.begin_popup("##ctx_menu")
            && let Some(target_idx) = *context_menu_target
            && let Some(target_entry) = entries.get(target_idx)
        {
            // Open (for dirs) or Confirm (for files)
            if target_entry.is_dir {
                let label = icon_label(buf, icons::FOLDER_OPEN, strings.open);
                if ui.selectable(label) {
                    result.action = Some(Action::NavigateTo(target_entry.path.clone()));
                    *context_menu_target = None;
                }
            } else {
                let label = icon_label(buf, icons::CHECK_BOLD, strings.open);
                if ui.selectable(label) {
                    result.action = Some(Action::ConfirmSelection);
                    *context_menu_target = None;
                }
            }

            ui.separator();

            // Rename
            let label = icon_label(buf, icons::PENCIL, strings.rename);
            if ui.selectable(label) {
                *rename_index = Some(target_idx);
                rename_buf.clear();
                rename_buf.push_str(&target_entry.name);
                *context_menu_target = None;
            }

            // Delete (request confirmation)
            let label = icon_label(buf, icons::TRASH_CAN_OUTLINE, strings.delete);
            if ui.selectable(label) {
                result.request_delete = Some(target_idx);
                *context_menu_target = None;
            }

            ui.separator();

            // Copy Path
            let label = icon_label(buf, icons::CONTENT_COPY, strings.copy_path);
            if ui.selectable(label) {
                result.action = Some(Action::CopyPath(target_idx));
                *context_menu_target = None;
            }
        }
    }

    // Keyboard navigation (disabled when any text input is active)
    if !ui.is_any_item_active()
        && ui.is_window_focused_with_flags(dear_imgui_rs::FocusedFlags::ROOT_WINDOW | dear_imgui_rs::FocusedFlags::CHILD_WINDOWS)
    {
        if ui.is_key_pressed(Key::UpArrow) && !entries.is_empty() {
            let current = selected_indices.first().copied().unwrap_or(0);
            let new_idx = current.saturating_sub(1);
            selected_indices.clear();
            selected_indices.push(new_idx);
            *scroll_to_index = Some(new_idx);
        }
        if ui.is_key_pressed(Key::DownArrow) && !entries.is_empty() {
            let current = selected_indices.first().copied().unwrap_or(0);
            let new_idx = (current + 1).min(entries.len() - 1);
            selected_indices.clear();
            selected_indices.push(new_idx);
            *scroll_to_index = Some(new_idx);
        }
        if ui.is_key_pressed(Key::Enter)
            && let Some(&idx) = selected_indices.first()
            && let Some(e) = entries.get(idx)
        {
            if e.is_dir {
                result.action = Some(Action::NavigateTo(e.path.clone()));
            } else {
                result.action = Some(Action::ConfirmSelection);
            }
        }
        if ui.is_key_pressed(Key::Backspace) {
            result.action = Some(Action::GoParent);
        }

        // Page Up / Page Down
        if ui.is_key_pressed(Key::PageUp) && !entries.is_empty() {
            let current = selected_indices.first().copied().unwrap_or(0);
            let page_size = 20; // approximate visible rows
            let new_idx = current.saturating_sub(page_size);
            selected_indices.clear();
            selected_indices.push(new_idx);
            *scroll_to_index = Some(new_idx);
        }
        if ui.is_key_pressed(Key::PageDown) && !entries.is_empty() {
            let current = selected_indices.first().copied().unwrap_or(0);
            let page_size = 20;
            let new_idx = (current + page_size).min(entries.len() - 1);
            selected_indices.clear();
            selected_indices.push(new_idx);
            *scroll_to_index = Some(new_idx);
        }
        // Home / End
        if ui.is_key_pressed(Key::Home) && !entries.is_empty() {
            selected_indices.clear();
            selected_indices.push(0);
            *scroll_to_index = Some(0);
        }
        if ui.is_key_pressed(Key::End) && !entries.is_empty() {
            let last = entries.len() - 1;
            selected_indices.clear();
            selected_indices.push(last);
            *scroll_to_index = Some(last);
        }
    }

    result
}

// ─── Footer (filter + confirm/cancel) ───────────────────────────────────────

/// Render the footer bar: filter dropdown, filename input (SaveFile), and buttons.
///
/// In SaveFile mode the filename input and buttons share a single row:
/// `[Filename: ___________] [Save] [Cancel]`
///
/// Returns `(confirmed, cancelled, filter_action)`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_footer(
    ui: &Ui,
    strings: &FmStrings,
    mode: DialogMode,
    entries: &[FsEntry],
    selected_indices: &[usize],
    filename_buf: &mut String,
    filters: &[FileFilter],
    active_filter: usize,
    config: &FileManagerConfig, // NOTE: added parameter — update call sites in mod.rs
    buf: &mut String,
) -> (bool, bool, Option<Action>) {
    let mut confirmed = false;
    let mut cancelled = false;
    let mut action = None;

    let (confirm_label, confirm_icon, can_confirm) = match mode {
        DialogMode::SelectFolder => (strings.select_folder, icons::CHECK_BOLD, true),
        DialogMode::OpenFile => {
            let has_file = selected_indices
                .iter()
                .any(|&i| entries.get(i).is_some_and(|e| !e.is_dir));
            (strings.open, icons::CHECK_BOLD, has_file)
        }
        DialogMode::SaveFile => (
            strings.save,
            icons::CONTENT_SAVE,
            !filename_buf.trim().is_empty(),
        ),
    };

    let btn_w = config.button_width;
    let btn_h = config.button_height;
    let gap = 8.0_f32;
    let total_btns = btn_w * 2.0 + gap;
    let filter_w = config.filter_width;

    let _rounding = ui.push_style_var(StyleVar::FrameRounding(4.0));

    // ── Left side: filter or filename ──
    let has_filter = mode != DialogMode::SelectFolder && filters.len() > 1;

    if mode == DialogMode::SaveFile {
        // SaveFile: [Filename: ___] [Save] [Cancel]
        ui.text_colored(theme::TEXT_SECONDARY, strings.filename);
        ui.same_line();

        let avail = ui.content_region_avail()[0];
        let input_w = (avail - total_btns - gap * 2.0).max(80.0);
        ui.set_next_item_width(input_w);
        let enter = ui
            .input_text("##filename", filename_buf)
            .enter_returns_true(true)
            .build();
        if enter {
            confirmed = true;
        }
        ui.same_line_with_spacing(0.0, gap);
    } else if has_filter {
        // OpenFile with filter: [Filter ▼]  ...  [Open] [Cancel]
        ui.set_next_item_width(filter_w);
        let preview = if filters[active_filter].extensions.is_empty() {
            strings.all_files
        } else {
            &filters[active_filter].label
        };
        if let Some(_tok) = ui.begin_combo("##filter", preview) {
            for (i, filter) in filters.iter().enumerate() {
                let sel = i == active_filter;
                let display = if filter.extensions.is_empty() {
                    strings.all_files
                } else {
                    &filter.label
                };
                if ui.selectable_config(display).selected(sel).build() && active_filter != i {
                    action = Some(Action::SelectFilter(i));
                }
            }
        }
        ui.same_line();

        // Right-align buttons after the filter
        let avail = ui.content_region_avail()[0];
        if avail > total_btns {
            ui.same_line_with_spacing(0.0, avail - total_btns);
        }
    } else {
        // SelectFolder / OpenFile without filter: right-align buttons
        let avail = ui.content_region_avail()[0];
        if avail > total_btns {
            ui.set_cursor_pos([ui.cursor_pos()[0] + avail - total_btns, ui.cursor_pos()[1]]);
        }
    }

    // ── Confirm button ──
    if can_confirm {
        let label = icon_label(buf, confirm_icon, confirm_label);
        with_btn_style(ui, confirm_btn(), || {
            if ui.button_with_size(label, [btn_w, btn_h]) {
                confirmed = true;
            }
        });
    } else {
        let label = icon_label(buf, confirm_icon, confirm_label);
        let _disabled = ui.push_style_var(StyleVar::Alpha(0.4));
        with_btn_style(ui, confirm_btn(), || {
            ui.button_with_size(label, [btn_w, btn_h]);
        });
    }

    // ── Cancel button ──
    ui.same_line_with_spacing(0.0, gap);
    {
        let label = icon_label(buf, icons::CLOSE, strings.cancel);
        with_btn_style(ui, cancel_btn(), || {
            if ui.button_with_size(label, [btn_w, btn_h]) {
                cancelled = true;
            }
        });
    }

    (confirmed, cancelled, action)
}

// ─── Overwrite confirmation modal ───────────────────────────────────────────

/// Returns `Some(true)` = overwrite confirmed, `Some(false)` = cancelled, `None` = still open.
pub(crate) fn render_overwrite_confirm(
    ui: &Ui,
    strings: &FmStrings,
    should_open: &mut bool,
    buf: &mut String,
) -> Option<bool> {
    if *should_open {
        *should_open = false;
        ui.open_popup("##overwrite_confirm");
    }

    let mut result = None;
    if let Some(_tok) = ui
        .begin_modal_popup_config("##overwrite_confirm")
        .flags(WindowFlags::ALWAYS_AUTO_RESIZE)
        .begin()
    {
        ui.text(strings.overwrite_message);
        ui.spacing();

        let label = icon_label(buf, icons::CHECK_BOLD, strings.yes);
        with_btn_style(ui, confirm_btn(), || {
            if ui.button_with_size(label, [80.0, 0.0]) {
                result = Some(true);
                ui.close_current_popup();
            }
        });
        ui.same_line();
        let label = icon_label(buf, icons::CLOSE, strings.no);
        with_btn_style(ui, cancel_btn(), || {
            if ui.button_with_size(label, [80.0, 0.0]) {
                result = Some(false);
                ui.close_current_popup();
            }
        });
    }

    result
}

// ─── Delete confirmation modal ──────────────────────────────────────────────

/// Render the delete confirmation modal.
///
/// Returns `Some(true)` = delete confirmed, `Some(false)` = cancelled, `None` = not open.
pub(crate) fn render_delete_confirm(
    ui: &Ui,
    strings: &FmStrings,
    should_open: &mut bool,
    entry_name: Option<&str>,
    buf: &mut String,
) -> Option<bool> {
    if *should_open {
        *should_open = false;
        ui.open_popup("##delete_confirm");
    }

    let mut result = None;
    if let Some(_tok) = ui
        .begin_modal_popup_config("##delete_confirm")
        .flags(WindowFlags::ALWAYS_AUTO_RESIZE)
        .begin()
    {
        buf.clear();
        if let Some(name) = entry_name {
            let _ = write!(buf, "{} \"{}\"?", strings.confirm_delete_message, name);
        } else {
            let _ = write!(buf, "{}?", strings.confirm_delete_message);
        }
        ui.text(buf.as_str());
        ui.spacing();

        let label = icon_label(buf, icons::TRASH_CAN_OUTLINE, strings.yes);
        with_btn_style(ui, cancel_btn(), || {
            if ui.button_with_size(label, [80.0, 0.0]) {
                result = Some(true);
                ui.close_current_popup();
            }
        });
        ui.same_line();
        let label = icon_label(buf, icons::CLOSE, strings.no);
        with_btn_style(ui, nav_btn(), || {
            if ui.button_with_size(label, [80.0, 0.0]) {
                result = Some(false);
                ui.close_current_popup();
            }
        });
    }

    result
}
