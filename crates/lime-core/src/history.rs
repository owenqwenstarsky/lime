use crate::cursor::Cursor;
use crate::edit::TextEdit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditTransaction {
    pub edits: Vec<TextEdit>,
    pub before_cursor: Cursor,
    pub after_cursor: Cursor,
}

impl EditTransaction {
    pub fn new(edits: Vec<TextEdit>, before_cursor: Cursor, after_cursor: Cursor) -> Self {
        Self {
            edits,
            before_cursor,
            after_cursor,
        }
    }
}
