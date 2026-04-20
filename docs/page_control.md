# PageControl

Generic tabbed container with Dashboard and Tabs views for Dear ImGui.

## Overview

`PageControl<T>` manages a collection of pages (tabs) with two presentation modes:

| View | Description |
|------|-------------|
| **Dashboard** | Interactive tile grid showing all pages as cards with status indicators |
| **Tabs** | Traditional tab strip with content area, 4 visual styles |

Pages are generic over `T: PageItem`, so any user type can serve as a page.

## Features

- **Dashboard view**: tile grid with headers, subtitles, status badges, close buttons, context menus
- **Dashboard "+" tile**: optional add-tile at end of grid (`show_add_tile`)
- **Dashboard fixed columns**: `dashboard_columns` config for fixed grid layout
- **Tabs view**: scrollable tab strip with 4 styles (Pill, Underline, Card, Square)
- **Runtime style switching** between all tab styles
- **Smooth scroll** animation for tab strip (exponential ease-out)
- **Tab overflow dropdown**: popup listing all tabs for quick navigation when scrolled
- **View toggle button**: built-in Dashboard↔Tabs switcher in the tab strip
- **Tab min/max width**: configurable tab size bounds
- **Status indicators**: Active, Inactive, Warning, Error (colored dots on tabs/tiles)
- **Badges**: notification counters on tabs (e.g. "3 new messages")
- **Close confirmation**: modal dialog before removing a page
- **Drag-and-drop tab reorder** with `PageAction::Reordered` notification
- **Double-click on tabs** with `PageAction::DoubleClicked` notification
- **Keyboard navigation**: arrow keys, Ctrl+W to close
- **Middle-click close** on tabs
- **Context menus** on tiles and tabs
- **Add button** (+) for creating new pages
- **External content rendering** (caller renders content after tab strip)
- **Per-tab accent color** override via `tab_color()`
- **Custom tile rendering** via `has_custom_tile()` + `render_tile()`
- **Zero per-frame allocations** (internal buffers reused, index-based deferred operations)

## Quick Start

```rust
use dear_imgui_custom_mod::page_control::*;

// 1. Define your page type
struct EditorTab {
    title: String,
    content: String,
    modified: bool,
}

// 2. Implement PageItem trait
impl PageItem for EditorTab {
    fn title(&self) -> &str { &self.title }
    fn is_closable(&self) -> bool { true }

    fn status(&self) -> PageStatus {
        if self.modified { PageStatus::Warning } else { PageStatus::Active }
    }

    fn render_content(&mut self, ui: &dear_imgui_rs::Ui) {
        ui.input_text_multiline("##editor", &mut self.content, [0.0, 0.0])
            .build();
    }
}

// 3. Create and use
let mut pc: PageControl<EditorTab> = PageControl::new("editor_tabs");
pc.add(EditorTab {
    title: "main.rs".into(),
    content: "fn main() {}".into(),
    modified: false,
});

// 4. Render each frame
if let Some(action) = pc.render(&ui) {
    match action {
        PageAction::Activated(id) => { /* tab focused */ }
        PageAction::Closed(id) => { pc.remove(id); }
        PageAction::TileClicked(id) => { pc.set_active(id); pc.view = ContentView::Tabs; }
        PageAction::AddRequested => { /* show "new tab" dialog */ }
        PageAction::DoubleClicked(id) => { /* rename, detach, etc. */ }
        PageAction::Reordered(id) => { /* persist tab order */ }
        PageAction::ViewToggled => { /* switch pc.view */ }
        _ => {}
    }
}
```

## Tab Styles

Switch at runtime via `pc.config.tab_style`:

| Style | Look |
|-------|------|
| `TabStyle::Pill` | Rounded capsule background (default) |
| `TabStyle::Underline` | Bottom border highlight (Material Design) |
| `TabStyle::Card` | Raised card with top accent line (Chrome/browser) |
| `TabStyle::Square` | Flat rectangular tabs with 3-sided border (classic) |

## ContentView

`pc.view` controls which presentation mode is active:

| Variant | Description |
|---------|-------------|
| `ContentView::Dashboard` | Interactive tile grid showing all pages as cards (default) |
| `ContentView::Tabs` | Traditional tab strip with content area |
| `ContentView::Custom(u8)` | Component renders nothing; caller handles all content. The `u8` is a user-defined view index for distinguishing multiple custom views |

```rust
// Switch to Tabs after clicking a dashboard tile:
PageAction::TileClicked(id) => {
    pc.set_active(id);
    pc.view = ContentView::Tabs;
}
// Use a custom view:
pc.view = ContentView::Custom(0);
```

## PageItem Trait

Required:

```rust
fn title(&self) -> &str;
```

Optional (all have defaults):

| Method | Default | Description |
|--------|---------|-------------|
| `icon(&self)` | `None` | MDI icon codepoint (from `icons` module) |
| `is_closable(&self)` | `true` | Show close button |
| `status(&self)` | `Active` | Status indicator color |
| `badge(&self)` | `None` | Notification badge on tab |
| `tooltip(&self)` | `None` | Hover tooltip text |
| `tab_color(&self)` | `None` | Per-tab accent `[R, G, B]` override |
| `subtitle(&self)` | `None` | Subtitle shown on dashboard tiles (supports `\n`) |
| `on_activated(&mut self)` | no-op | Called when page becomes active |
| `on_deactivated(&mut self)` | no-op | Called when page loses focus |
| `has_custom_tile(&self)` | `false` | Use fully custom tile rendering |
| `render_tile(&self, ui, area)` | no-op | Custom tile rendering (when `has_custom_tile` is true) |
| `render_tile_body(&self, ui, area)` | no-op | Custom tile body (Dashboard view); `area: [x, y, w, h]` |
| `render_content(&mut self, ui)` | no-op | Render page content (Tabs view) |
| `body_height(&self)` | `None` | Override body height for custom tiles |

## PageAction

Actions returned by `render()` — at most one per frame:

| Action | Description |
|--------|-------------|
| `Activated(PageId)` | Tab was clicked — now the active page |
| `Closed(PageId)` | Page was closed (after optional confirmation) |
| `TileClicked(PageId)` | Dashboard tile was clicked |
| `AddRequested` | "+" button or add tile was clicked |
| `TileBodyAction(PageId, u64)` | Custom action from tile body |
| `DoubleClicked(PageId)` | Tab was double-clicked |
| `Reordered(PageId)` | Tab was moved via drag-and-drop |
| `ViewToggled` | View toggle button was clicked |

## Public API

| Method | Description |
|--------|-------------|
| `new(id)` | Create with default config |
| `with_config(id, config)` | Create with custom config |
| `add(item) → PageId` | Add page (auto-activated) |
| `remove(id) → Option<T>` | Remove page by ID |
| `get(id) → Option<&T>` | Shared reference to page |
| `get_mut(id) → Option<&mut T>` | Mutable reference to page |
| `active_id() → Option<PageId>` | Currently active page |
| `set_active(id)` | Set active page (calls lifecycle hooks + scrolls to it) |
| `page_count()` | Number of open pages |
| `is_empty()` | Whether there are no pages |
| `iter()` | Iterate over `(PageId, &T)` |
| `iter_mut()` | Iterate over `(PageId, &mut T)` |
| `force_invalidate()` | Force tab width recalculation (call when title/badge/icon changes dynamically) |
| `scroll_to_active()` | Scroll tab strip to make active tab visible |
| `render(ui) → Option<PageAction>` | Render the component |

## Configuration

```rust
PageControlConfig {
    // Behavior
    closable: true,                // show close buttons
    confirm_close: true,           // modal before closing
    middle_click_close: true,      // middle-click to close tab
    scroll_with_wheel: true,       // mouse wheel scrolls tab strip
    keyboard_nav: true,            // arrow keys, Ctrl+W
    show_add_button: false,        // (+) button in tab strip
    context_menu: true,            // right-click menus
    external_content: false,       // caller renders content after tab strip

    // Tab strip
    tab_style: TabStyle::Pill,     // Pill, Underline, Card, Square
    show_tab_underline: true,      // underline on active tab
    tab_height: 24.0,
    tab_rounding: 12.0,
    tab_padding_h: 8.0,
    tab_gap: 4.0,
    tab_min_width: 60.0,           // minimum tab width in pixels
    tab_max_width: 300.0,          // maximum tab width in pixels
    close_btn_size: 12.0,          // close button diameter in pixels
    close_btn_gap: 5.0,            // gap between tab text and close button
    strip_padding_v: 3.0,          // vertical padding above/below tabs in strip
    scroll_btn_width: 22.0,        // width of left/right scroll arrow buttons
    scroll_speed: 200.0,           // tab strip scroll speed (px/s)
    smooth_scroll: true,           // animated scroll for tab strip
    show_overflow_dropdown: true,  // dropdown listing all tabs when overflow
    show_view_toggle: false,       // Dashboard↔Tabs toggle button

    // Dashboard tiles
    tile_width: 210.0,
    tile_header_height: 40.0,
    tile_body_height: 100.0,
    tile_gap: 10.0,
    tile_rounding: 8.0,
    tile_padding: 10.0,
    dashboard_columns: None,       // None = auto-compute from tile_width
    show_add_tile: false,          // "+" tile at end of grid
    dashboard_title: None,         // optional title above grid
    dashboard_show_count: false,   // append (N) to dashboard title

    // Appearance
    colors: PcColors::default(),
    strings: PcStrings::default(),
    ..Default::default()
}
```

### PcColors

All color fields are `[u8; 3]` RGB in 0–255 range. Alpha is applied per-use.

| Field | Description |
|-------|-------------|
| `tab_bg` | Inactive tab background |
| `tab_hover` | Hovered tab background |
| `tab_active` | Active tab background |
| `accent` | Accent underline / active indicator |
| `text` | Primary tab text |
| `text_muted` | Secondary/dimmed text |
| `close_hover` | Close button hover color |
| `strip_bg` | Tab strip background |
| `separator` | Strip-content separator line |
| `tile_bg` | Dashboard tile background |
| `tile_hover` | Hovered dashboard tile background |
| `status_active` | Green status dot |
| `status_inactive` | Gray status dot |
| `status_warning` | Amber status dot |
| `status_error` | Red status dot |

### PcStrings

All fields are `&'static str`. Override for localization.

| Field | Default |
|-------|---------|
| `cancel` | `"Cancel"` |
| `close` | `"Close"` |
| `close_confirm` | `"Close this page?"` |
| `no_pages` | `"No pages"` |
| `empty_hint` | `"Add a page to begin…"` |
| `overflow_tooltip` | `"All tabs"` |
| `view_dashboard` | `"Dashboard"` |
| `view_tabs` | `"Tabs"` |
| `add_page` | `"Add page"` |

## Architecture

```
page_control/
  mod.rs      PageControl<T> struct, PageItem trait, public API, draw_mini_tile utility
  config.rs   PageControlConfig, TabStyle, PageAction, PcColors, PcStrings, Badge, ContentView
  render.rs   Dashboard tiles, tab strip (4 styles), scroll, overflow, close popup
```
