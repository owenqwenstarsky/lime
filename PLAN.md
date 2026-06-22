# Lime Implementation Plan

Lime is a clean, modern terminal/TUI text editor written in Rust. It should feel approachable like Nano, efficient and clean like Vim, and easy to use like VS Code, while using a standard non-modal cursor editing model.

This plan intentionally avoids time estimates so it can be handed directly to an implementation agent.

## 1. Product Direction

Initial product goal:

```bash
lime path/to/file.rs
```

opens a clean single-file terminal editor with:

- Standard typing/editing behavior
- Save and quit shortcuts
- Unsaved-changes confirmation
- Syntax highlighting
- Line numbers
- Status bar and help bar
- File search/open popup with `Ctrl-F`
- Recursive file discovery from the current working directory
- Large-file warning before opening very large files
- Good behavior on macOS and Linux

Lime should not be modal like Vim. It should be keyboard-first, but obvious and friendly.

## 2. Recommended Technical Stack

Use a Rust-native terminal stack:

- `crossterm` for terminal control and input events
- `ratatui` for layout, widgets, status bars, popups, and drawing
- `ropey` for text storage
- `tree-sitter` for syntax highlighting
- `ignore` for recursive file scanning with `.gitignore` support
- `nucleo-matcher` for fuzzy file matching

Do not build a custom terminal renderer at first. `ratatui + crossterm` gives enough control for a polished TUI while keeping implementation manageable.

Suggested dependencies:

```toml
[dependencies]
anyhow = "1"
thiserror = "1"
clap = { version = "4", features = ["derive"] }
ropey = "1"
crossterm = "0.28"
ratatui = "0.29"
unicode-segmentation = "1"
unicode-width = "0.2"
tree-sitter = "0.24"
tree-sitter-rust = "0.23"
tree-sitter-javascript = "0.23"
tree-sitter-typescript = "0.23"
tree-sitter-python = "0.23"
tree-sitter-json = "0.23"
tree-sitter-toml-ng = "0.6"
tree-sitter-md = "0.3"
ignore = "0.4"
nucleo-matcher = "0.3"
notify = "7"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
dirs = "5"
```

Optional later:

```toml
arboard = "3"
similar = "2"
tempfile = "3"
insta = "1"
```

## 3. Repository Structure

Use a Cargo workspace from the beginning:

```txt
lime/
  Cargo.toml
  README.md
  LICENSE
  PLAN.md
  crates/
    lime-core/
      Cargo.toml
      src/
        lib.rs
        buffer.rs
        cursor.rs
        selection.rs
        edit.rs
        history.rs
        movement.rs
        search.rs
        file.rs
        command.rs
    lime-syntax/
      Cargo.toml
      src/
        lib.rs
        language.rs
        highlighter.rs
        theme.rs
    lime-ui/
      Cargo.toml
      src/
        lib.rs
        app.rs
        terminal.rs
        layout.rs
        editor_view.rs
        status_bar.rs
        command_bar.rs
        file_picker.rs
        prompt.rs
        theme.rs
        input.rs
    lime-cli/
      Cargo.toml
      src/
        main.rs
```

Root `Cargo.toml`:

```toml
[workspace]
members = [
  "crates/lime-core",
  "crates/lime-syntax",
  "crates/lime-ui",
  "crates/lime-cli"
]
resolver = "2"
```

The installed binary should be named `lime`.

## 4. Crate Responsibilities

### `lime-core`

Pure editor logic.

Must not depend on terminal UI, Ratatui, themes, syntax highlighting, or input events.

Responsibilities:

- Text buffer
- Cursor state
- Selections, even if selection editing comes later
- Editing operations
- Undo/redo
- File load/save
- Dirty state
- Search inside the current buffer
- Line/column movement
- Large-file metadata and open policy types

### `lime-syntax`

Syntax parsing and highlighting.

Responsibilities:

- Language detection
- Tree-sitter parser setup
- Highlight spans for visible ranges
- Theme scopes
- Graceful fallback for unknown languages or parser failures

### `lime-ui`

Terminal application.

Responsibilities:

- Raw mode
- Alternate screen
- Event loop
- Keybindings
- Rendering
- Editor viewport
- Popups
- File picker
- Status bar
- Help bar
- Prompts and warnings

### `lime-cli`

CLI entry point.

Responsibilities:

- Parse CLI arguments
- Resolve starting path
- Load config
- Start app
- Ensure terminal cleanup on error or panic

## 5. Core Data Model

Use `ropey::Rope` for buffer storage.

```rust
pub struct TextBuffer {
    rope: Rope,
    path: Option<PathBuf>,
    dirty: bool,
    line_ending: LineEnding,
}
```

Track positions as line/column character positions, not byte offsets.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub position: Position,
    pub preferred_column: Option<usize>,
}
```

Use `preferred_column` for vertical movement so up/down preserve the intended visual column across lines of different lengths.

Also define:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: Position,
    pub end: Position,
}

pub enum LineEnding {
    Lf,
    Crlf,
}
```

## 6. Editing Commands

Create a command abstraction in `lime-core`:

```rust
pub enum EditorCommand {
    InsertChar(char),
    InsertText(String),
    Newline,
    Backspace,
    Delete,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveLineStart,
    MoveLineEnd,
    MoveFileStart,
    MoveFileEnd,
    PageUp,
    PageDown,
    Save,
    Undo,
    Redo,
    Search(String),
}
```

The UI should convert terminal key events into app actions or editor commands. The core applies commands and returns effects.

```rust
pub enum CommandResult {
    None,
    Modified,
    Saved,
    CursorMoved,
    NeedsPath,
}
```

## 7. Undo/Redo

Implement undo/redo early.

Represent edits as reversible transactions:

```rust
pub struct EditTransaction {
    pub edits: Vec<TextEdit>,
    pub before_cursor: Cursor,
    pub after_cursor: Cursor,
}

pub enum TextEdit {
    Insert {
        at: Position,
        text: String,
    },
    Delete {
        range: TextRange,
        deleted_text: String,
    },
}
```

Typing consecutive characters should be grouped into one undo transaction until one of these happens:

- Cursor movement
- Newline
- Backspace/delete
- Save
- Different command type
- Focus changes to a popup/prompt

## 8. Large File Handling

Before reading a file, inspect metadata.

Recommended thresholds:

```txt
<= 5 MB: open normally
5-25 MB: open with warning prompt
> 25 MB: require explicit confirmation
> 100 MB: refuse by default unless --force
```

CLI/TUI warning example:

```txt
This file is 84 MB. Lime may be slower with very large files.
Open anyway? [y/N]
```

Core policy type:

```rust
pub struct FileOpenPolicy {
    pub warn_threshold_bytes: u64,
    pub confirm_threshold_bytes: u64,
    pub hard_threshold_bytes: u64,
    pub force: bool,
}
```

The file picker should also use this policy before replacing the current buffer.

## 9. Terminal UI Layout

Default layout:

```txt
┌────────────────────────────────────────────┐
│  1 │ fn main() {                           │
│  2 │     println!("hello");                │
│  3 │ }                                     │
│    │                                       │
│    │                                       │
├────────────────────────────────────────────┤
│ lime  file.rs  rust  Ln 2, Col 5  modified │
│ Ctrl-S Save  Ctrl-F Files  Ctrl-Q Quit      │
└────────────────────────────────────────────┘
```

Main regions:

1. Editor viewport
2. Status bar
3. Help/action bar
4. Optional popup overlay

Use `ratatui::Layout` for the main shell.

## 10. Rendering Strategy

Render only visible lines.

UI viewport state:

```rust
pub struct Viewport {
    pub top_line: usize,
    pub left_col: usize,
    pub height: usize,
    pub width: usize,
}
```

Editor rendering should:

- Render visible lines only
- Draw line numbers
- Draw cursor
- Draw selected text later
- Apply syntax spans when available
- Horizontally scroll for long lines
- Vertically scroll when the cursor nears viewport edges
- Avoid allocating the entire file each frame

Gutter width:

```rust
let gutter_width = total_lines.to_string().len() + 2;
```

## 11. Input Model

Recommended shortcuts:

```txt
Ctrl-S        Save
Ctrl-Q        Quit
Ctrl-F        File picker
Ctrl-G        Go to line
Ctrl-Z        Undo
Ctrl-Y        Redo
Ctrl-A        Start of line
Ctrl-E        End of line
Ctrl-R        Search in current file
Esc           Close popup/cancel prompt
Arrow keys    Move cursor
PageUp        Page up
PageDown      Page down
Home          Start of line
End           End of line
Backspace     Delete backward
Delete        Delete forward
Enter         Newline
Tab           Insert spaces according to config
```

Use an app-level action enum:

```rust
pub enum AppAction {
    Editor(EditorCommand),
    Save,
    Quit,
    OpenFilePicker,
    OpenGoToLine,
    OpenSearch,
    ClosePopup,
    Confirm,
    Cancel,
}
```

Do not hardcode behavior directly inside the event loop. Route input through a mapping layer.

## 12. File Picker

`Ctrl-F` opens a centered popup listing files recursively from the current directory.

Behavior:

- Scan current working directory or selected workspace root
- Respect `.gitignore`
- Skip hidden files by default
- Skip common bulky directories:
  - `.git`
  - `target`
  - `node_modules`
  - `dist`
  - `.next`
  - `build`
- Fuzzy filter as the user types
- Arrow up/down changes selection
- Enter opens selected file
- Escape closes picker
- Warn if current file has unsaved changes before replacing it
- Warn before opening very large files

Popup layout:

```txt
╭─ Open File ─────────────────────────────╮
│ search: main                            │
│                                         │
│ > crates/lime-cli/src/main.rs           │
│   crates/lime-core/src/buffer.rs        │
│   README.md                             │
╰─────────────────────────────────────────╯
```

State:

```rust
pub struct FilePickerState {
    pub query: String,
    pub all_files: Vec<PathBuf>,
    pub matches: Vec<FileMatch>,
    pub selected: usize,
}

pub struct FileMatch {
    pub path: PathBuf,
    pub score: u32,
}
```

## 13. Syntax Highlighting

Start with these languages:

- Rust
- JavaScript
- TypeScript
- Python
- JSON
- TOML
- Markdown

Language detection:

```rust
pub enum Language {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Json,
    Toml,
    Markdown,
    PlainText,
}
```

Detection should use file extension first.

Highlighting should be resilient:

- Unknown language falls back to plain text
- Parser failure falls back to plain text
- Initial implementation may highlight visible ranges only
- Incremental parsing can come later

Theme model:

```rust
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub gutter: Color,
    pub cursor: Color,
    pub selection: Color,
    pub keyword: Color,
    pub string: Color,
    pub comment: Color,
    pub function: Color,
    pub type_name: Color,
    pub number: Color,
    pub error: Color,
}
```

Use a clean default dark theme with calm contrast.

## 14. Save/Quit Behavior

If there are no unsaved changes, `Ctrl-Q` exits immediately.

If the buffer is dirty:

```txt
Unsaved changes. Save before quitting? [s]ave [d]iscard [c]ancel
```

If saving an unnamed buffer:

```txt
Save as: _
```

Save operation should:

- Write to a temporary file
- Flush data
- Rename over original path
- Preserve line endings where practical
- Clear dirty flag after success
- Show a status message after success or failure

## 15. Config

Keep config simple at first.

Locations:

```txt
macOS: ~/Library/Application Support/lime/config.toml
Linux: ~/.config/lime/config.toml
```

Initial config:

```toml
theme = "lime-dark"
tab_width = 4
insert_spaces = true
show_line_numbers = true
confirm_large_files = true
```

Rust model:

```rust
pub struct Config {
    pub theme: String,
    pub tab_width: usize,
    pub insert_spaces: bool,
    pub show_line_numbers: bool,
    pub confirm_large_files: bool,
}
```

If config is missing or invalid, use defaults and show a non-blocking warning.

## 16. Error Handling

Use `anyhow` at app/CLI boundaries.

Use `thiserror` for library errors.

Example:

```rust
#[derive(thiserror::Error, Debug)]
pub enum LimeError {
    #[error("file is too large: {size} bytes")]
    FileTooLarge { size: u64 },

    #[error("buffer has no file path")]
    MissingPath,

    #[error("invalid cursor position")]
    InvalidPosition,
}
```

Terminal cleanup must always happen.

Use a guard:

```rust
pub struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // disable raw mode
        // leave alternate screen
        // show cursor
    }
}
```

Never leave the terminal in raw mode after an error or panic.

## 17. CLI Design

Basic usage:

```bash
lime
lime file.rs
lime .
lime --force huge.log
lime --config path/to/config.toml
```

CLI args:

```rust
#[derive(Parser)]
pub struct Cli {
    pub path: Option<PathBuf>,

    #[arg(long)]
    pub force: bool,

    #[arg(long)]
    pub config: Option<PathBuf>,
}
```

Behavior:

- No path: open empty unnamed buffer
- File path: open file
- Directory path: open empty buffer with file picker rooted at that directory
- Nonexistent file: create a new buffer with that path
- Huge file: warn/confirm unless `--force`

## 18. Testing Strategy

### `lime-core`

Most tests should live here.

Test:

- Insert characters
- Insert text
- Insert newline
- Backspace in middle of line
- Backspace at start of line joins lines
- Delete in middle of line
- Delete at end of line joins lines
- Cursor movement left/right/up/down
- Line start/end
- File start/end
- Undo/redo
- Search
- Save/load roundtrip
- Unicode text
- Tabs
- Long lines
- Empty file behavior

Example test:

```rust
#[test]
fn backspace_joins_lines() {
    let mut editor = Editor::from_text("hello\nworld");
    editor.set_cursor(Position { line: 1, column: 0 });
    editor.backspace();

    assert_eq!(editor.text(), "helloworld");
    assert_eq!(editor.cursor().position, Position { line: 0, column: 5 });
}
```

### `lime-syntax`

Test:

- Language detection
- Parser setup does not panic
- Highlight spans are valid
- Unknown files fall back to plain text

### `lime-ui`

Test pure/non-terminal pieces:

- Keybinding mapping
- Viewport scrolling
- File picker filtering
- Status string formatting
- Layout calculations
- Prompt state transitions

Avoid depending heavily on full terminal snapshot tests in the first version.

## 19. Implementation Order

### Step 1: Create workspace

Create all workspace crates and make `cargo test` pass.

Deliverables:

- Root `Cargo.toml`
- `lime-core`
- `lime-syntax`
- `lime-ui`
- `lime-cli`
- Minimal compiling binary named `lime`

### Step 2: Implement `lime-core`

Implement:

- `TextBuffer`
- `Position`
- `TextRange`
- `Cursor`
- Load from string
- Load from file
- Save to file
- Insert char/text
- Newline
- Backspace
- Delete
- Cursor movement
- Dirty flag

Add comprehensive tests.

### Step 3: Implement undo/redo

Add:

- `EditTransaction`
- Undo stack
- Redo stack
- Transaction grouping for typing
- Tests for edit operations and grouped undo behavior

### Step 4: Implement basic terminal app

In `lime-ui`, implement:

- Raw mode
- Alternate screen
- Panic-safe cleanup
- Event loop
- Render empty editor
- Render file content
- Status bar
- Help bar
- Cursor rendering

At this point:

```bash
cargo run -p lime-cli -- README.md
```

should open a navigable file.

### Step 5: Wire editing input

Add keyboard handling for:

- Text input
- Enter
- Tab
- Backspace
- Delete
- Arrow movement
- Home/end
- Page up/down
- Save
- Quit

After this step, Lime should be a usable single-file editor.

### Step 6: Add viewport scrolling

Implement:

- Vertical scroll
- Horizontal scroll
- Cursor-follow behavior
- Page up/down
- Long-line handling

Do not render the whole file.

### Step 7: Add file open policy

Before reading files:

- Check size
- Show warning prompt in TUI
- Support `--force`
- Refuse extreme files by default

### Step 8: Add file picker

Implement:

- Recursive file scan
- Fuzzy matching
- Popup rendering
- Query input
- Selection movement
- Opening selected files
- Dirty-buffer confirmation before replacing current file

Bind to `Ctrl-F`.

### Step 9: Add syntax highlighting

Implement `lime-syntax`.

Start with:

- Language detection
- Rust highlighting
- Plain-text fallback

Then add:

- JavaScript
- TypeScript
- Python
- JSON
- TOML
- Markdown

Integrate highlighting into editor rendering.

### Step 10: Add quality-of-life features

Add:

- Go to line popup
- Search in current file
- Unsaved quit prompt
- Save-as prompt
- Status messages
- Error display
- Config loading
- Tab width setting
- Insert spaces setting

## 20. Minimum Definition of Done

The first complete version should support:

```bash
lime file.rs
```

and include:

- Clean terminal UI
- Standard text editing
- Natural cursor movement
- Save with `Ctrl-S`
- Quit with `Ctrl-Q`
- Unsaved changes warning
- Syntax highlighting for known languages
- File picker with `Ctrl-F`
- Recursive current-directory browsing
- Fuzzy file filtering
- Moderately large file handling without freezing
- Warning before opening very large files
- Passing core editing tests
- macOS and Linux support

## 21. Explicit Non-Goals For First Version

Do not implement these yet:

- Plugin system
- Extension marketplace
- LSP
- Multi-pane editor
- Integrated terminal
- Git UI
- Debugger
- AI features
- Remote editing
- Modal Vim emulation

These can be added later only after the core editor feels excellent.
