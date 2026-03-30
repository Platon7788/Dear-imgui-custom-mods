# FileManager

Native-style file/folder picker dialog for Dear ImGui.

## Overview

`FileManager` provides a complete file browser UI with three modes:

| Mode | Description |
|------|-------------|
| `SelectFolder` | Pick a directory |
| `OpenFile` | Pick one or more files with optional filters |
| `SaveFile` | Choose save location with filename input + overwrite confirmation |

## Features

- **Breadcrumb navigation** with double-click-to-edit path bar
- **Back/forward history** (browser-style)
- **Favorites sidebar** (Desktop, Documents, Downloads + custom bookmarks)
- **Drive selector** (Windows drive letters / Unix mount points)
- **File table** with sortable columns: Name, Size, Date Modified, Type
- **File filters** (`*.rs`, `*.png`, etc.) with dropdown
- **Multi-select** (Ctrl+Click) in OpenFile mode
- **Type-to-search** with auto-reset timer
- **Context menu**: rename, delete, copy path
- **Keyboard navigation**: arrows, Enter (open), Backspace (parent), Escape (cancel)
- **Keyboard shortcuts**: Ctrl+A (select all), Ctrl+L (edit path), Ctrl+H (toggle hidden), Page Up/Down, Home/End
- **Filename validation**: cross-platform checks (length, reserved names, invalid chars)
- **100+ file type icons** with Material Design Icons (Rust, Python, JS/TS, C/C++, Go, Java, images, audio, video, archives, etc.)
- **Custom icon override** callback for user-defined file type icons
- **Overwrite confirmation modal** in SaveFile mode
- **Configurable layout**: button sizes, filter width, inline input width
- **Zero per-frame allocations** (display strings pre-computed on directory change)

## Quick Start

```rust
use dear_imgui_custom_mod::file_manager::{FileManager, FileFilter};

let mut fm = FileManager::new();

// Open a file picker with filters
fm.open_file(None, vec![
    FileFilter::new("Rust Files (*.rs)", &["rs"]),
    FileFilter::new("TOML (*.toml)", &["toml"]),
    FileFilter::all(),
]);

// Each frame inside your ImGui render loop:
if fm.render(&ui) {
    // Dialog completed (OK pressed)
    if let Some(path) = &fm.selected_path {
        println!("Selected: {}", path.display());
    }
}
```

### Save File

```rust
fm.save_file(
    Some("/home/user/projects"),  // starting directory
    "untitled.rs",                // default filename
    vec![FileFilter::new("Rust (*.rs)", &["rs"])],
);
```

### Select Folder

```rust
fm.open_folder(None);  // None = start from current directory
```

## Configuration

All configuration lives in `FileManagerConfig`:

```rust
use dear_imgui_custom_mod::file_manager::FileManagerConfig;

let config = FileManagerConfig {
    // Visibility
    show_hidden_files: false,       // show dotfiles (default: false)
    show_favorites: true,           // favorites sidebar (default: true)
    show_column_size: true,         // Size column (default: true)
    show_column_date: true,         // Date Modified column (default: true)
    show_column_type: true,         // Type column (default: true)

    // Behavior
    enable_multi_select: true,      // Ctrl+Click in OpenFile mode (default: false)
    enable_breadcrumbs: true,       // breadcrumb path bar (default: true)
    enable_history: true,           // back/forward navigation (default: true)
    enable_type_to_search: true,    // type to jump to files (default: true)
    dirs_first: true,               // directories before files (default: true)

    // Window
    initial_size: [750.0, 520.0],   // dialog size in pixels
    min_size: [500.0, 350.0],       // minimum dialog size
    custom_title: None,             // override mode-specific title (default: None)

    // Layout
    favorites_width: 150.0,         // sidebar width (default: 150.0)
    button_width: 120.0,            // footer button width (default: 120.0)
    button_height: 28.0,            // footer button height (default: 28.0)
    filter_width: 180.0,            // filter dropdown width (default: 180.0)
    inline_input_width: 200.0,      // new folder/file input width (default: 200.0)

    // Tuning
    max_history: 100,               // max navigation history entries (default: 100)
    search_timeout: 0.5,            // type-to-search reset timeout in seconds (default: 0.5)

    // Custom icon mapping (default: None — uses built-in 100+ file type icons)
    icon_override: None,

    ..Default::default()
};
```

### Custom Icon Override

Override the built-in file type icon mapping with your own:

```rust
use dear_imgui_custom_mod::file_manager::config::IconOverrideFn;

fn my_icons(ext: &str) -> Option<(&'static str, [f32; 4])> {
    match ext {
        "custom" => Some(("\u{f15b}", [1.0, 0.5, 0.0, 1.0])),  // orange file icon
        _ => None,  // fall back to built-in icons
    }
}

let config = FileManagerConfig {
    icon_override: Some(my_icons as IconOverrideFn),
    ..Default::default()
};
```

### Localization

All UI strings are configurable via `FmStrings` (36 fields). Pass a static reference:

```rust
static MY_STRINGS: FmStrings = FmStrings {
    title_open: "Open File",
    btn_ok: "Open",
    btn_cancel: "Cancel",
    shortcut_hint: "F2: Rename · Del: Delete · Backspace: Parent",
    select_all: "Select All",
    // ... all other fields
};

let config = FileManagerConfig {
    strings: &MY_STRINGS,
    ..Default::default()
};
```

## Architecture

```
file_manager/
  mod.rs        FileManager struct, public API, dialog lifecycle
  config.rs     DialogMode, FileFilter, FmStrings, FileManagerConfig
  render.rs     All ImGui rendering (drive bar, breadcrumb, toolbar, table, footer)
  entry.rs      FsEntry with pre-computed display strings, sorting
  favorites.rs  Favorites sidebar (Desktop, Documents, Downloads + custom)
  history.rs    Back/forward navigation stack
```

## Public API

| Method | Description |
|--------|-------------|
| `new()` | Create a new file manager |
| `open_file(start_dir, filters)` | Open file picker with filters |
| `save_file(start_dir, filename, filters)` | Open save dialog |
| `open_folder(start_dir)` | Open folder picker |
| `render(ui) -> bool` | Render dialog; returns `true` when completed |
| `selected_path` | `Option<PathBuf>` — selected file/folder path |
| `selected_paths` | `Vec<PathBuf>` — multiple selected paths (multi-select mode) |
| `is_open() -> bool` | Whether the dialog is currently open |
| `close()` | Close the dialog programmatically |

## Key Types

| Type | Description |
|------|-------------|
| `FileManager` | Main dialog struct. Call `open_file()` / `save_file()` / `open_folder()` then `render()` each frame |
| `DialogMode` | Enum: `SelectFolder`, `OpenFile`, `SaveFile` |
| `FileFilter` | Filter by extension. `FileFilter::new("Images", &["png", "jpg"])` |
| `FsEntry` | Internal: one directory entry with pre-formatted size/date/type strings |
| `IconOverrideFn` | Type alias for custom icon callback: `fn(&str) -> Option<(&'static str, [f32; 4])>` — return tuple is `(icon_glyph, rgba_color)` |

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Enter | Open selected folder / confirm file |
| Backspace | Navigate to parent directory |
| Escape | Cancel and close dialog |
| Delete | Delete selected file/folder |
| F2 | Rename selected file/folder |
| Ctrl+A | Select all files |
| Ctrl+L | Edit path (switch to text input) |
| Ctrl+H | Toggle hidden files |
| Up/Down | Navigate file list |
| Page Up/Down | Scroll file list by page |
| Home/End | Jump to first/last file |
| Type characters | Jump to matching filename (resets after timeout) |
