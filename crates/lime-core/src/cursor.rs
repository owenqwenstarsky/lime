#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    pub position: Position,
    pub preferred_column: Option<usize>,
}

impl Cursor {
    pub const fn new(position: Position) -> Self {
        Self {
            position,
            preferred_column: None,
        }
    }

    pub fn with_position(mut self, position: Position) -> Self {
        self.position = position;
        self
    }

    pub fn clear_preferred_column(&mut self) {
        self.preferred_column = None;
    }
}
