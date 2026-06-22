use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use ropey::Rope;

use crate::cursor::Position;
use crate::edit::TextRange;
use crate::error::{LimeError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
}

impl LineEnding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }

    pub fn detect(text: &str) -> Self {
        if text.contains("\r\n") {
            Self::Crlf
        } else {
            Self::Lf
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextBuffer {
    rope: Rope,
    path: Option<PathBuf>,
    dirty: bool,
    line_ending: LineEnding,
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::from_text("")
    }
}

impl TextBuffer {
    pub fn from_text(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            path: None,
            dirty: false,
            line_ending: LineEnding::detect(text),
        }
    }

    pub fn from_text_with_path(text: &str, path: impl Into<PathBuf>) -> Self {
        let mut buffer = Self::from_text(text);
        buffer.path = Some(path.into());
        buffer
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)?;
        Ok(Self {
            rope: Rope::from_str(&text),
            path: Some(path.to_path_buf()),
            dirty: false,
            line_ending: LineEnding::detect(&text),
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, path: impl Into<PathBuf>) {
        self.path = Some(path.into());
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    pub fn set_line_ending(&mut self, line_ending: LineEnding) {
        self.line_ending = line_ending;
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines().max(1)
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn char(&self, char_idx: usize) -> Option<char> {
        if char_idx < self.len_chars() {
            Some(self.rope.char(char_idx))
        } else {
            None
        }
    }

    pub fn line_to_string(&self, line_idx: usize) -> String {
        if line_idx >= self.line_count() {
            return String::new();
        }
        self.rope.line(line_idx).to_string()
    }

    pub fn line_text_without_ending(&self, line_idx: usize) -> String {
        let mut line = self.line_to_string(line_idx);
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        line
    }

    pub fn line_len_chars(&self, line_idx: usize) -> usize {
        if line_idx >= self.line_count() {
            return 0;
        }

        let line = self.rope.line(line_idx);
        let mut len = line.len_chars();
        if len > 0 && line.char(len - 1) == '\n' {
            len -= 1;
            if len > 0 && line.char(len - 1) == '\r' {
                len -= 1;
            }
        }
        len
    }

    pub fn clamp_position(&self, position: Position) -> Position {
        let line = position.line.min(self.line_count().saturating_sub(1));
        let column = position.column.min(self.line_len_chars(line));
        Position { line, column }
    }

    pub fn position_to_char(&self, position: Position) -> usize {
        let position = self.clamp_position(position);
        self.rope.line_to_char(position.line) + position.column
    }

    pub fn char_to_position(&self, char_idx: usize) -> Position {
        let char_idx = char_idx.min(self.len_chars());
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        let column = (char_idx - line_start).min(self.line_len_chars(line));
        Position { line, column }
    }

    pub fn range_to_chars(&self, range: TextRange) -> (usize, usize) {
        let normalized = range.normalized();
        (
            self.position_to_char(normalized.start),
            self.position_to_char(normalized.end),
        )
    }

    pub fn slice_range(&self, range: TextRange) -> String {
        let (start, end) = self.range_to_chars(range);
        self.rope.slice(start..end).to_string()
    }

    pub fn insert(&mut self, position: Position, text: &str) -> Position {
        let position = self.clamp_position(position);
        let char_idx = self.position_to_char(position);
        self.rope.insert(char_idx, text);
        self.dirty = true;
        advance_position(position, text)
    }

    pub fn remove(&mut self, range: TextRange) -> String {
        let normalized = range.normalized();
        let (start, end) = self.range_to_chars(normalized);
        if start >= end {
            return String::new();
        }
        let deleted = self.rope.slice(start..end).to_string();
        self.rope.remove(start..end);
        self.dirty = true;
        deleted
    }

    pub fn save(&mut self) -> Result<()> {
        let path = self.path.clone().ok_or(LimeError::MissingPath)?;
        self.save_as(path)
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("lime-buffer");
        let temp_path = parent.join(format!(".{}.{}.lime-tmp", file_name, std::process::id()));

        {
            let file = File::create(&temp_path)?;
            let mut writer = BufWriter::new(file);
            for chunk in self.rope.chunks() {
                writer.write_all(chunk.as_bytes())?;
            }
            writer.flush()?;
            writer.get_ref().sync_all()?;
        }

        fs::rename(&temp_path, path)?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }
}

pub fn advance_position(start: Position, text: &str) -> Position {
    let mut line = start.line;
    let mut column = start.column;
    let mut saw_cr = false;

    for ch in text.chars() {
        match ch {
            '\r' => saw_cr = true,
            '\n' => {
                line += 1;
                column = 0;
                saw_cr = false;
            }
            _ => {
                if saw_cr {
                    column += 1;
                    saw_cr = false;
                }
                column += 1;
            }
        }
    }

    if saw_cr {
        column += 1;
    }

    Position { line, column }
}
