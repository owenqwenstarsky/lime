use std::path::Path;

use crate::buffer::{advance_position, TextBuffer};
use crate::command::{CommandResult, EditorCommand};
use crate::cursor::{Cursor, Position};
use crate::error::{LimeError, Result};
use crate::history::EditTransaction;
use crate::movement::DEFAULT_PAGE_LINES;
use crate::search::SearchMatch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: Position,
    pub end: Position,
}

impl TextRange {
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub fn normalized(self) -> Self {
        if self.start <= self.end {
            self
        } else {
            Self {
                start: self.end,
                end: self.start,
            }
        }
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
pub struct Editor {
    buffer: TextBuffer,
    cursor: Cursor,
    undo_stack: Vec<EditTransaction>,
    redo_stack: Vec<EditTransaction>,
    revision: u64,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Self::from_buffer(TextBuffer::default())
    }

    pub fn from_text(text: &str) -> Self {
        Self::from_buffer(TextBuffer::from_text(text))
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::from_buffer(TextBuffer::from_file(path)?))
    }

    pub fn from_buffer(buffer: TextBuffer) -> Self {
        Self {
            cursor: Cursor::default(),
            buffer,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            revision: 0,
        }
    }

    pub fn replace_buffer(&mut self, buffer: TextBuffer) {
        self.buffer = buffer;
        self.cursor = Cursor::default();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut TextBuffer {
        &mut self.buffer
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn set_cursor(&mut self, position: Position) {
        self.cursor.position = self.buffer.clamp_position(position);
        self.cursor.preferred_column = None;
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.buffer.is_dirty()
    }

    pub fn text(&self) -> String {
        self.buffer.text()
    }

    pub fn line_count(&self) -> usize {
        self.buffer.line_count()
    }

    pub fn line_len_chars(&self, line: usize) -> usize {
        self.buffer.line_len_chars(line)
    }

    pub fn line_text_without_ending(&self, line: usize) -> String {
        self.buffer.line_text_without_ending(line)
    }

    pub fn apply_command(&mut self, command: EditorCommand) -> Result<CommandResult> {
        match command {
            EditorCommand::InsertChar(ch) => {
                self.insert_char(ch);
                Ok(CommandResult::Modified)
            }
            EditorCommand::InsertText(text) => {
                self.insert_text(&text);
                Ok(CommandResult::Modified)
            }
            EditorCommand::Newline => {
                self.newline();
                Ok(CommandResult::Modified)
            }
            EditorCommand::Backspace => Ok(if self.backspace() {
                CommandResult::Modified
            } else {
                CommandResult::None
            }),
            EditorCommand::Delete => Ok(if self.delete() {
                CommandResult::Modified
            } else {
                CommandResult::None
            }),
            EditorCommand::MoveLeft => Ok(if self.move_left() {
                CommandResult::CursorMoved
            } else {
                CommandResult::None
            }),
            EditorCommand::MoveRight => Ok(if self.move_right() {
                CommandResult::CursorMoved
            } else {
                CommandResult::None
            }),
            EditorCommand::MoveUp => Ok(if self.move_up() {
                CommandResult::CursorMoved
            } else {
                CommandResult::None
            }),
            EditorCommand::MoveDown => Ok(if self.move_down() {
                CommandResult::CursorMoved
            } else {
                CommandResult::None
            }),
            EditorCommand::MoveLineStart => Ok(if self.move_line_start() {
                CommandResult::CursorMoved
            } else {
                CommandResult::None
            }),
            EditorCommand::MoveLineEnd => Ok(if self.move_line_end() {
                CommandResult::CursorMoved
            } else {
                CommandResult::None
            }),
            EditorCommand::MoveFileStart => Ok(if self.move_file_start() {
                CommandResult::CursorMoved
            } else {
                CommandResult::None
            }),
            EditorCommand::MoveFileEnd => Ok(if self.move_file_end() {
                CommandResult::CursorMoved
            } else {
                CommandResult::None
            }),
            EditorCommand::PageUp => Ok(if self.page_up(DEFAULT_PAGE_LINES) {
                CommandResult::CursorMoved
            } else {
                CommandResult::None
            }),
            EditorCommand::PageDown => Ok(if self.page_down(DEFAULT_PAGE_LINES) {
                CommandResult::CursorMoved
            } else {
                CommandResult::None
            }),
            EditorCommand::Save => match self.save() {
                Ok(()) => Ok(CommandResult::Saved),
                Err(LimeError::MissingPath) => Ok(CommandResult::NeedsPath),
                Err(err) => Err(err),
            },
            EditorCommand::Undo => Ok(if self.undo() {
                CommandResult::Modified
            } else {
                CommandResult::None
            }),
            EditorCommand::Redo => Ok(if self.redo() {
                CommandResult::Modified
            } else {
                CommandResult::None
            }),
            EditorCommand::Search(_) => Ok(CommandResult::None),
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        let mut buf = [0; 4];
        self.insert_text_grouped(ch.encode_utf8(&mut buf), true);
    }

    pub fn insert_text(&mut self, text: &str) {
        self.insert_text_grouped(text, false);
    }

    fn insert_text_grouped(&mut self, text: &str, allow_grouping: bool) {
        if text.is_empty() {
            return;
        }

        let before_cursor = self.cursor;
        let at = self.cursor.position;
        let after = self.buffer.insert(at, text);
        self.cursor = Cursor::new(self.buffer.clamp_position(after));
        self.revision = self.revision.wrapping_add(1);

        let tx = EditTransaction::new(
            vec![TextEdit::Insert {
                at,
                text: text.to_string(),
            }],
            before_cursor,
            self.cursor,
        );
        self.record_transaction(tx, allow_grouping && !text.contains(['\n', '\r']));
    }

    pub fn newline(&mut self) {
        let line_ending = self.buffer.line_ending().as_str().to_string();
        self.insert_text_grouped(&line_ending, false);
    }

    pub fn backspace(&mut self) -> bool {
        let current = self.buffer.position_to_char(self.cursor.position);
        if current == 0 {
            return false;
        }

        let start = self.buffer.char_to_position(current - 1);
        self.delete_range_with_cursor(
            TextRange::new(start, self.cursor.position),
            self.cursor,
            start,
        )
    }

    pub fn delete(&mut self) -> bool {
        let current = self.buffer.position_to_char(self.cursor.position);
        if current >= self.buffer.len_chars() {
            return false;
        }

        let mut end_idx = current + 1;
        if self.buffer.char(current) == Some('\r') && self.buffer.char(current + 1) == Some('\n') {
            end_idx = current + 2;
        }
        let end = self.buffer.char_to_position(end_idx);
        self.delete_range_with_cursor(
            TextRange::new(self.cursor.position, end),
            self.cursor,
            self.cursor.position,
        )
    }

    fn delete_range_with_cursor(
        &mut self,
        range: TextRange,
        before_cursor: Cursor,
        after_position: Position,
    ) -> bool {
        let normalized = range.normalized();
        if normalized.is_empty() {
            return false;
        }

        let deleted_text = self.buffer.remove(normalized);
        if deleted_text.is_empty() {
            return false;
        }

        self.cursor = Cursor::new(self.buffer.clamp_position(after_position));
        self.revision = self.revision.wrapping_add(1);
        let tx = EditTransaction::new(
            vec![TextEdit::Delete {
                range: normalized,
                deleted_text,
            }],
            before_cursor,
            self.cursor,
        );
        self.record_transaction(tx, false);
        true
    }

    pub fn move_left(&mut self) -> bool {
        let pos = self.cursor.position;
        let next = if pos.column > 0 {
            Position::new(pos.line, pos.column - 1)
        } else if pos.line > 0 {
            let previous_line = pos.line - 1;
            Position::new(previous_line, self.buffer.line_len_chars(previous_line))
        } else {
            return false;
        };
        self.set_cursor(next);
        true
    }

    pub fn move_right(&mut self) -> bool {
        let pos = self.cursor.position;
        let line_len = self.buffer.line_len_chars(pos.line);
        let next = if pos.column < line_len {
            Position::new(pos.line, pos.column + 1)
        } else if pos.line + 1 < self.buffer.line_count() {
            Position::new(pos.line + 1, 0)
        } else {
            return false;
        };
        self.set_cursor(next);
        true
    }

    pub fn move_up(&mut self) -> bool {
        let pos = self.cursor.position;
        if pos.line == 0 {
            return false;
        }
        let preferred = self.cursor.preferred_column.unwrap_or(pos.column);
        let target_line = pos.line - 1;
        self.cursor.position = Position::new(
            target_line,
            preferred.min(self.buffer.line_len_chars(target_line)),
        );
        self.cursor.preferred_column = Some(preferred);
        true
    }

    pub fn move_down(&mut self) -> bool {
        let pos = self.cursor.position;
        if pos.line + 1 >= self.buffer.line_count() {
            return false;
        }
        let preferred = self.cursor.preferred_column.unwrap_or(pos.column);
        let target_line = pos.line + 1;
        self.cursor.position = Position::new(
            target_line,
            preferred.min(self.buffer.line_len_chars(target_line)),
        );
        self.cursor.preferred_column = Some(preferred);
        true
    }

    pub fn move_line_start(&mut self) -> bool {
        let pos = self.cursor.position;
        if pos.column == 0 {
            return false;
        }
        self.set_cursor(Position::new(pos.line, 0));
        true
    }

    pub fn move_line_end(&mut self) -> bool {
        let pos = self.cursor.position;
        let end = self.buffer.line_len_chars(pos.line);
        if pos.column == end {
            return false;
        }
        self.set_cursor(Position::new(pos.line, end));
        true
    }

    pub fn move_file_start(&mut self) -> bool {
        if self.cursor.position == Position::new(0, 0) {
            return false;
        }
        self.set_cursor(Position::new(0, 0));
        true
    }

    pub fn move_file_end(&mut self) -> bool {
        let last_line = self.buffer.line_count().saturating_sub(1);
        let end = Position::new(last_line, self.buffer.line_len_chars(last_line));
        if self.cursor.position == end {
            return false;
        }
        self.set_cursor(end);
        true
    }

    pub fn page_up(&mut self, lines: usize) -> bool {
        let pos = self.cursor.position;
        if pos.line == 0 {
            return false;
        }
        let target_line = pos.line.saturating_sub(lines.max(1));
        let preferred = self.cursor.preferred_column.unwrap_or(pos.column);
        self.cursor.position = Position::new(
            target_line,
            preferred.min(self.buffer.line_len_chars(target_line)),
        );
        self.cursor.preferred_column = Some(preferred);
        true
    }

    pub fn page_down(&mut self, lines: usize) -> bool {
        let pos = self.cursor.position;
        let last_line = self.buffer.line_count().saturating_sub(1);
        if pos.line >= last_line {
            return false;
        }
        let target_line = (pos.line + lines.max(1)).min(last_line);
        let preferred = self.cursor.preferred_column.unwrap_or(pos.column);
        self.cursor.position = Position::new(
            target_line,
            preferred.min(self.buffer.line_len_chars(target_line)),
        );
        self.cursor.preferred_column = Some(preferred);
        true
    }

    pub fn save(&mut self) -> Result<()> {
        self.buffer.save()
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.buffer.save_as(path)
    }

    pub fn undo(&mut self) -> bool {
        let Some(tx) = self.undo_stack.pop() else {
            return false;
        };

        for edit in tx.edits.iter().rev() {
            self.apply_inverse_edit(edit);
        }
        self.cursor = tx.before_cursor;
        self.buffer.set_dirty(true);
        self.redo_stack.push(tx);
        self.revision = self.revision.wrapping_add(1);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(tx) = self.redo_stack.pop() else {
            return false;
        };

        for edit in &tx.edits {
            self.apply_edit(edit);
        }
        self.cursor = tx.after_cursor;
        self.buffer.set_dirty(true);
        self.undo_stack.push(tx);
        self.revision = self.revision.wrapping_add(1);
        true
    }

    fn apply_edit(&mut self, edit: &TextEdit) {
        match edit {
            TextEdit::Insert { at, text } => {
                self.buffer.insert(*at, text);
            }
            TextEdit::Delete { range, .. } => {
                self.buffer.remove(*range);
            }
        }
    }

    fn apply_inverse_edit(&mut self, edit: &TextEdit) {
        match edit {
            TextEdit::Insert { at, text } => {
                let end = advance_position(*at, text);
                self.buffer.remove(TextRange::new(*at, end));
            }
            TextEdit::Delete {
                range,
                deleted_text,
            } => {
                self.buffer.insert(range.start, deleted_text);
            }
        }
    }

    fn record_transaction(&mut self, tx: EditTransaction, allow_grouping: bool) {
        self.redo_stack.clear();

        if allow_grouping {
            if let Some(last) = self.undo_stack.last_mut() {
                if let (
                    [TextEdit::Insert {
                        at: _,
                        text: last_text,
                    }],
                    [TextEdit::Insert { at: _, text }],
                ) = (last.edits.as_mut_slice(), tx.edits.as_slice())
                {
                    if last.after_cursor == tx.before_cursor
                        && !last_text.contains(['\n', '\r'])
                        && !text.contains(['\n', '\r'])
                    {
                        last_text.push_str(text);
                        last.after_cursor = tx.after_cursor;
                        return;
                    }
                }
            }
        }

        self.undo_stack.push(tx);
    }

    pub fn find_all(&self, query: &str) -> Vec<SearchMatch> {
        if query.is_empty() {
            return Vec::new();
        }

        let text = self.buffer.text();
        text.match_indices(query)
            .map(|(byte_start, matched)| {
                let start_char = text[..byte_start].chars().count();
                let end_char = start_char + matched.chars().count();
                SearchMatch {
                    range: TextRange::new(
                        self.buffer.char_to_position(start_char),
                        self.buffer.char_to_position(end_char),
                    ),
                }
            })
            .collect()
    }

    pub fn search_next(&self, query: &str) -> Option<SearchMatch> {
        let cursor_char = self.buffer.position_to_char(self.cursor.position);
        let matches = self.find_all(query);
        matches
            .iter()
            .find(|m| self.buffer.position_to_char(m.range.start) > cursor_char)
            .cloned()
            .or_else(|| matches.first().cloned())
    }

    pub fn go_to_line(&mut self, one_based_line: usize) -> bool {
        let target = one_based_line.saturating_sub(1);
        let target = target.min(self.buffer.line_count().saturating_sub(1));
        self.set_cursor(Position::new(target, 0));
        true
    }

    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn insert_characters_groups_undo() {
        let mut editor = Editor::new();
        editor.insert_char('h');
        editor.insert_char('i');

        assert_eq!(editor.text(), "hi");
        assert_eq!(editor.undo_len(), 1);
        assert!(editor.undo());
        assert_eq!(editor.text(), "");
        assert_eq!(editor.cursor().position, Position::new(0, 0));
    }

    #[test]
    fn insert_newline() {
        let mut editor = Editor::from_text("hello");
        editor.move_file_end();
        editor.newline();
        editor.insert_text("world");

        assert_eq!(editor.text(), "hello\nworld");
        assert_eq!(editor.cursor().position, Position::new(1, 5));
    }

    #[test]
    fn backspace_joins_lines() {
        let mut editor = Editor::from_text("hello\nworld");
        editor.set_cursor(Position { line: 1, column: 0 });
        editor.backspace();

        assert_eq!(editor.text(), "helloworld");
        assert_eq!(editor.cursor().position, Position { line: 0, column: 5 });
    }

    #[test]
    fn delete_joins_lines() {
        let mut editor = Editor::from_text("hello\nworld");
        editor.set_cursor(Position::new(0, 5));
        editor.delete();

        assert_eq!(editor.text(), "helloworld");
        assert_eq!(editor.cursor().position, Position::new(0, 5));
    }

    #[test]
    fn cursor_moves_across_lines() {
        let mut editor = Editor::from_text("a\nbc");
        assert!(editor.move_right());
        assert_eq!(editor.cursor().position, Position::new(0, 1));
        assert!(editor.move_right());
        assert_eq!(editor.cursor().position, Position::new(1, 0));
        assert!(editor.move_left());
        assert_eq!(editor.cursor().position, Position::new(0, 1));
    }

    #[test]
    fn vertical_movement_preserves_preferred_column() {
        let mut editor = Editor::from_text("abcd\nx\nabcdef");
        editor.set_cursor(Position::new(0, 4));
        editor.move_down();
        assert_eq!(editor.cursor().position, Position::new(1, 1));
        editor.move_down();
        assert_eq!(editor.cursor().position, Position::new(2, 4));
    }

    #[test]
    fn undo_redo_delete() {
        let mut editor = Editor::from_text("abc");
        editor.set_cursor(Position::new(0, 1));
        editor.delete();
        assert_eq!(editor.text(), "ac");
        assert!(editor.undo());
        assert_eq!(editor.text(), "abc");
        assert!(editor.redo());
        assert_eq!(editor.text(), "ac");
    }

    #[test]
    fn search_finds_matches() {
        let editor = Editor::from_text("one two one");
        let matches = editor.find_all("one");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].range.start, Position::new(0, 0));
        assert_eq!(matches[1].range.start, Position::new(0, 8));
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.txt");
        let mut editor = Editor::from_text("hello\nworld");
        editor.save_as(&path).unwrap();
        assert!(!editor.is_dirty());

        let loaded = Editor::from_file(&path).unwrap();
        assert_eq!(loaded.text(), "hello\nworld");
    }

    #[test]
    fn unicode_text_moves_and_deletes() {
        let mut editor = Editor::from_text("åß∂");
        editor.move_file_end();
        assert_eq!(editor.cursor().position, Position::new(0, 3));
        editor.backspace();
        assert_eq!(editor.text(), "åß");
    }
}
