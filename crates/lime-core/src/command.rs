#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandResult {
    None,
    Modified,
    Saved,
    CursorMoved,
    NeedsPath,
}
