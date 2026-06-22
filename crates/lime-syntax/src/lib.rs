pub mod highlighter;
pub mod language;
pub mod theme;

pub use highlighter::{HighlightKind, HighlightSpan, Highlighter};
pub use language::Language;
pub use theme::SyntaxTheme;
