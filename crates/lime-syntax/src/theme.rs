#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxTheme {
    pub name: &'static str,
}

impl Default for SyntaxTheme {
    fn default() -> Self {
        Self { name: "lime-dark" }
    }
}
