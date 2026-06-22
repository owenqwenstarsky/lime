use crate::edit::TextRange;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub range: TextRange,
}

impl Selection {
    pub const fn new(range: TextRange) -> Self {
        Self { range }
    }
}
