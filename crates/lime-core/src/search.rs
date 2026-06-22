use crate::edit::TextRange;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub range: TextRange,
}
