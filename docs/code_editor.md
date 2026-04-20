# CodeEditor

Full-featured code editor widget for Dear ImGui with syntax highlighting, line numbers, cursor/selection, undo/redo, bracket matching, find/replace, code folding, word wrap, built-in fonts, and draw-call-batched rendering.

Built entirely on ImGui's DrawList API (no `InputTextMultiline`), giving full control over rendering and input handling.

## Overview

`CodeEditor` is a self-contained editor widget that manages its own text buffer, tokenization, input handling, and rendering. It supports multiple languages via built-in tokenizers and a custom `SyntaxDefinition` trait for user-defined grammars.

## Features

- **Syntax highlighting**: Rust, TOML, RON, Rhai, JSON, YAML, XML, ASM (x86/ARM/RISC-V), Hex, and custom languages via `SyntaxDefinition` trait
- **6 built-in themes**: Dark Default, Monokai, One Dark, Solarized Dark, Solarized Light, GitHub Light
- **3 built-in monospace fonts**: Hack (default), JetBrains Mono NL, JetBrains Mono — embedded via `include_bytes!`, zero-config
- **MDI icons**: Material Design Icons merged into font atlas for crisp UI icons
- **Line numbers** with active-line highlighting
- **Code folding** with MDI chevron icons (`▸`/`▾`), hover highlight, and `"... N lines"` badge for collapsed regions
- **Word wrap** with smart word-boundary breaking (handles HEX and long lines correctly)
- **Undo/redo** with action grouping (typing "hello" = one undo step)
- **Find/replace** with case-insensitive toggle, match navigation, replace-all
- **Bracket matching** for `()`, `{}`, `[]` with highlight
- **Auto-close** brackets and quotes
- **Multi-cursor** support (Ctrl+D to select next occurrence)
- **Selection**: mouse drag, Shift+Arrow, Shift+Click, Ctrl+A
- **Clipboard**: Cut (Ctrl+X), Copy (Ctrl+C), Paste (Ctrl+V) — respects `max_lines`/`max_line_length` limits
- **Text transforms**: UPPERCASE, lowercase, Title Case, trim whitespace
- **Line operations**: duplicate (Ctrl+Shift+D), delete (Ctrl+Shift+K), move up/down (Alt+Up/Down)
- **Toggle comment** (Ctrl+/)
- **Font zoom**: Ctrl+Scroll or Ctrl+Plus/Minus
- **Smooth scrolling** with adaptive speed (accelerates when cursor moves fast)
- **Hex editing mode**: auto-space after byte pairs, auto-uppercase, value-based coloring
- **Color swatches** next to hex color literals (`#RGB`, `0xRRGGBB`)
- **Whitespace visualization** (toggle via context menu)
- **Error/warning markers** with underlines and gutter icons
- **Breakpoints** with gutter indicators
- **I-beam cursor** — text-area I-beam cursor when hovering the editor
- **Right-click context menu** with fine-grained section control
- **Read-only mode**
- **Auto English keyboard layout** on focus (Windows, opt-in)
- **Content limits**: configurable `max_lines` and `max_line_length`

## Architecture

```text
code_editor/
├── mod.rs          CodeEditor widget — render, input, drawing
├── buffer.rs       TextBuffer — lines, cursor, selection, editing
├── tokenizer.rs    Rust/TOML/RON/Hex tokenizer (legacy, used as fallback)
├── config.rs       EditorConfig, SyntaxColors, Language, EditorTheme, BuiltinFont
├── token.rs        Token and TokenKind types
├── undo.rs         UndoStack with VecDeque and action grouping
└── lang/           Per-language tokenizer modules
    ├── mod.rs      Language dispatch
    ├── rust.rs     Rust tokenizer
    ├── toml.rs     TOML tokenizer
    ├── json.rs     JSON tokenizer
    ├── yaml.rs     YAML tokenizer
    ├── xml.rs      XML tokenizer
    ├── rhai.rs     Rhai tokenizer
    ├── hex.rs      Hex byte tokenizer
    └── asm.rs      ASM tokenizer (x86/ARM/RISC-V, AT&T + Intel + NASM)
```

### Key Optimizations

- **Draw call batching**: consecutive tokens of the same color are merged into a single `AddText` call (~3-5x fewer draw calls)
- **Token cache**: per-line tokenization cached via FNV-1a hash; only recomputed when content or block-comment state changes
- **Viewport culling**: only visible lines are tokenized and rendered
- **HashSet caches**: O(1) lookup for error markers and breakpoints per line
- **VecDeque undo stack**: O(1) eviction of oldest entries
- **Adaptive smooth scrolling**: frame-rate-independent `1.0 - exp(-speed * dt)` with faster catch-up for large gaps
- **Word wrap cache**: per-line wrap points cached by edit version + width; only recomputed on change

## Quick Start

```rust
use dear_imgui_custom_mod::code_editor::{CodeEditor, Language};

// Create editor
let mut editor = CodeEditor::new("my_editor");
editor.set_language(Language::Rust);
editor.set_text("fn main() {\n    println!(\"Hello, world!\");\n}");

// In your render loop:
// editor.render(ui);
```

### Run the demo

```bash
cargo run --example demo_code_editor
```

## Built-in Fonts

The editor ships with 3 embedded monospace fonts and MDI icons. Call `install_code_editor_font` **before** the font atlas is built (i.e., before renderer creation):

```rust
use dear_imgui_custom_mod::code_editor::{install_code_editor_font, install_code_editor_font_ex, BuiltinFont};

// Default font (Hack, 16px) + MDI icons
install_code_editor_font(&mut ctx, 16.0);

// Or choose a specific font
install_code_editor_font_ex(&mut ctx, 16.0, BuiltinFont::JetBrainsMonoNL);
```

Available fonts:

| Font | Description | Size |
|------|-------------|------|
| `BuiltinFont::Hack` (default) | Highly legible, excellent at all sizes | ~302 KB |
| `BuiltinFont::JetBrainsMonoNL` | JetBrains Mono, no ligatures (smallest) | ~204 KB |
| `BuiltinFont::JetBrainsMono` | JetBrains Mono with ligature tables | ~274 KB |

For a fully custom font:

```rust
use dear_imgui_custom_mod::code_editor::install_custom_code_editor_font;

install_custom_code_editor_font(&mut ctx, my_ttf_bytes, 16.0, "My Font");
```

## Public API

### Construction & Content

| Method | Description |
|--------|-------------|
| `new(id)` | Create a new editor with the given ImGui ID |
| `set_text(text)` | Replace buffer content |
| `get_text()` | Get full text as `String` |
| `insert_text(text)` | Insert text at cursor position |
| `is_modified()` | Whether content has been modified since last `clear_modified()` |
| `clear_modified()` | Reset the modified flag |

### Navigation & State

| Method | Description |
|--------|-------------|
| `cursor()` | Current cursor position (`CursorPos { line, col }`) |
| `line_count()` | Number of lines in the buffer |
| `goto_line(line)` | Scroll to and place cursor at the given line |
| `word_at_cursor()` | Word under the cursor (for tooltips, autocomplete) |
| `selected_text()` | Currently selected text |
| `is_focused()` | Whether the editor has keyboard focus |

### Configuration

| Method | Description |
|--------|-------------|
| `config()` | Immutable reference to `EditorConfig` |
| `config_mut()` | Mutable reference to `EditorConfig` |
| `set_language(lang)` | Set syntax language |
| `set_read_only(bool)` | Toggle read-only mode |
| `is_read_only()` | Query read-only state |
| `text_scale()` | Current font zoom level |
| `set_text_scale(scale)` | Set font zoom (clamped to `0.5..=3.0`) |

### Find/Replace

| Method | Description |
|--------|-------------|
| `open_find()` | Open find bar (Ctrl+F) |
| `open_find_replace()` | Open find+replace bar (Ctrl+H) |
| `close_find()` | Close find bar (Escape) |

### Undo/Redo

| Method | Description |
|--------|-------------|
| `undo()` | Undo last edit (Ctrl+Z) |
| `redo()` | Redo last undone edit (Ctrl+Y / Ctrl+Shift+Z) |
| `can_undo()` | Whether undo is available |
| `can_redo()` | Whether redo is available |

### Markers

| Method | Description |
|--------|-------------|
| `set_error_markers(markers)` | Set error/warning line markers (underlines + gutter icons) |
| `set_breakpoints(bps)` | Set breakpoints (`Vec<Breakpoint>` structs with line + options) |

### Code Folding

| Method | Description |
|--------|-------------|
| `toggle_fold(line)` | Toggle fold state of the region starting at `line` |

### Rendering

| Method | Description |
|--------|-------------|
| `render(ui)` | Render the editor widget (call once per frame) |

### Find Bar (v1.69+)

The find bar features:
- **Match count display** — shows "N of M matches" with live update
- **Case-sensitive toggle** button
- **Match navigation** — F3/Shift+F3 or Enter to cycle through matches
- **Replace and Replace All** — when opened with Ctrl+H
- **Auto-close on Escape**

## EditorConfig

All configuration is in `editor.config_mut()`:

```rust
let cfg = editor.config_mut();

// Text editing
cfg.tab_size = 4;
cfg.insert_spaces = true;
cfg.auto_indent = true;
cfg.auto_close_brackets = true;
cfg.auto_close_quotes = true;
cfg.read_only = false;

// Display
cfg.show_line_numbers = true;
cfg.show_fold_indicators = true;   // fold/unfold chevrons in gutter
cfg.highlight_current_line = true;
cfg.bracket_matching = true;
cfg.show_whitespace = false;
cfg.show_color_swatches = true;
cfg.word_wrap = false;
cfg.smooth_scrolling = true;

// Limits (0 = unlimited)
cfg.max_lines = 0;
cfg.max_line_length = 0;

// Cursor & scroll
cfg.cursor_blink_rate = 0.53;     // seconds (0 = no blink)
cfg.scroll_speed = 3.0;
cfg.font_size_scale = 1.0;        // Ctrl+Scroll modifies this

// Hex mode options
cfg.hex_auto_space = true;         // auto-space after two hex digits
cfg.hex_auto_uppercase = true;     // auto-uppercase hex input

// Auto English keyboard on focus (Windows only)
cfg.force_english_on_focus = true;
```

## Themes

Switch themes programmatically or via the right-click context menu:

```rust
use dear_imgui_custom_mod::code_editor::EditorTheme;

editor.config_mut().set_theme(EditorTheme::Monokai);
```

Available themes: `DarkDefault`, `Monokai`, `OneDark`, `SolarizedDark`, `SolarizedLight`, `GithubLight`.

Individual colors can be overridden after `set_theme`:

```rust
editor.config_mut().colors.keyword = [1.0, 0.0, 0.0, 1.0]; // red keywords
```

### SyntaxColors Fields

All fields are `[f32; 4]` RGBA in `0.0..=1.0`.

#### Syntax Token Colors

| Field | Token type |
|-------|-----------|
| `keyword` | Language keywords (`fn`, `let`, `if`, …) |
| `type_name` | Type identifiers |
| `lifetime` | Rust lifetime annotations (`'a`) |
| `string` | String literals |
| `char_lit` | Character literals |
| `number` | Numeric literals |
| `comment` | Line and block comments |
| `attribute` | Attributes / decorators (`#[…]`) |
| `macro_call` | Macro invocations (`vec!`, `println!`, …) |
| `operator` | Operators (`+`, `->`, `=>`, …) |
| `punctuation` | Punctuation (`;`, `,`, `.`, …) |
| `identifier` | Plain identifiers (default text) |
| `user_code_marker` | `todo!` / `unimplemented!` call-sites |

#### Hex-Mode Value Colors

| Field | Byte range |
|-------|-----------|
| `hex_null` | `0x00` — red |
| `hex_ff` | `0xFF` — amber |
| `hex_default` | `0x01–0x1F`, `0x7F`, `0x80–0xFE` — silver |
| `hex_printable` | `0x20–0x7E` — green |

#### Editor UI Colors

| Field | Purpose |
|-------|---------|
| `current_line_bg` | Active line highlight background |
| `selection_bg` | Text selection highlight |
| `search_match_bg` | Non-current search match highlight |
| `search_current_bg` | Current search match highlight (brighter) |
| `line_number` | Gutter line number (inactive lines) |
| `line_number_active` | Gutter line number (active line) |
| `bracket_match_bg` | Matching bracket highlight background |
| `error_underline` | Error marker underline color |
| `warning_underline` | Warning marker underline color |
| `gutter_bg` | Line number gutter background |
| `editor_bg` | Editor text-area background |

## Languages

```rust
use dear_imgui_custom_mod::code_editor::Language;

editor.set_language(Language::Rust);  // Rust (default)
editor.set_language(Language::Toml);  // TOML
editor.set_language(Language::Ron);   // RON
editor.set_language(Language::Rhai);  // Rhai scripting language
editor.set_language(Language::Json);  // JSON
editor.set_language(Language::Yaml);  // YAML
editor.set_language(Language::Xml);   // XML / HTML
editor.set_language(Language::Asm);   // Assembly (x86/ARM/RISC-V, AT&T + Intel + NASM)
editor.set_language(Language::Hex);   // Hex byte editor
editor.set_language(Language::None);  // Plain text
```

### Custom Syntax

Implement the `SyntaxDefinition` trait:

```rust
use dear_imgui_custom_mod::code_editor::{SyntaxDefinition, token::{Token, TokenKind}};
use std::sync::Arc;

struct MySyntax;
impl SyntaxDefinition for MySyntax {
    fn name(&self) -> &str { "MyLang" }
    fn tokenize_line(&self, line: &str, in_block_comment: bool) -> (Vec<Token>, bool) {
        // tokenize line, return (tokens, still_in_block_comment)
        (vec![], false)
    }
    fn line_comment_prefix(&self) -> Option<&str> { Some("//") }
}

editor.set_language(Language::Custom(Arc::new(MySyntax)));
```

## Dedicated Font

The easiest way is to use the built-in fonts:

```rust
use dear_imgui_custom_mod::code_editor::install_code_editor_font;

// Call before renderer creation (font atlas must not be built yet)
install_code_editor_font(&mut ctx, 16.0);
```

For manual font setup:

```rust
use dear_imgui_custom_mod::code_editor::CODE_EDITOR_FONT_PTR;

// After font atlas is built:
let mono_font: *mut dear_imgui_rs::sys::ImFont = /* from atlas */;
CODE_EDITOR_FONT_PTR.store(mono_font as usize, std::sync::atomic::Ordering::SeqCst);
```

## Context Menu

Fine-tune what appears in the right-click menu:

```rust
let ctx = &mut editor.config_mut().context_menu;
ctx.enabled = true;             // show menu at all
ctx.show_clipboard = true;      // Cut / Copy / Paste
ctx.show_select_all = true;     // Select All
ctx.show_undo_redo = true;      // Undo / Redo
ctx.show_code_actions = true;   // Toggle Comment, Duplicate Line, Delete Line
ctx.show_transform = true;      // Transform submenu (UPPER, lower, Title, Trim)
ctx.show_find = true;           // Find...
ctx.show_view_toggles = true;   // View submenu (line numbers, whitespace, etc.)
ctx.show_language_selector = true;
ctx.show_theme_selector = true;
ctx.show_font_size = true;      // Font size +/- buttons
ctx.show_cursor_info = true;    // Cursor position info
```

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Ctrl+C | Copy |
| Ctrl+X | Cut |
| Ctrl+V | Paste |
| Ctrl+A | Select all |
| Ctrl+Z | Undo |
| Ctrl+Y / Ctrl+Shift+Z | Redo |
| Ctrl+F | Find |
| Ctrl+H | Find & Replace |
| Ctrl+/ | Toggle comment |
| Ctrl+D | Select next occurrence (multi-cursor) |
| Ctrl+Shift+D | Duplicate line |
| Ctrl+Shift+K | Delete line |
| Alt+Up/Down | Move line up/down |
| Ctrl+Backspace | Delete word left |
| Ctrl+Delete | Delete word right |
| Ctrl+Scroll / Ctrl+Plus/Minus | Font zoom |
| Home | Smart home (toggle between indent and column 0) |
| Tab / Shift+Tab | Indent / Unindent |
| Escape | Close find bar |
| F3 / Shift+F3 | Next / Previous match |
| Enter (in find bar) | Next match |

## Error Markers

```rust
use dear_imgui_custom_mod::code_editor::LineMarker;

editor.set_error_markers(vec![
    LineMarker { line: 5, message: "expected `;`".into(), is_error: true },
    LineMarker { line: 12, message: "unused variable".into(), is_error: false },  // warning
]);
```

Errors show red underlines, warnings show yellow underlines. Hover the gutter icon to see the message tooltip.
