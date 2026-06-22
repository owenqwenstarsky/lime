use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

impl Language {
    pub fn detect_path(path: Option<&Path>) -> Self {
        let Some(path) = path else {
            return Self::PlainText;
        };

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        match file_name {
            "Cargo.toml" | "config.toml" | "lime.toml" => return Self::Toml,
            "README" | "README.md" => return Self::Markdown,
            _ => {}
        }

        match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
            "rs" => Self::Rust,
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "ts" | "tsx" | "mts" | "cts" => Self::TypeScript,
            "py" | "pyw" => Self::Python,
            "json" | "jsonc" => Self::Json,
            "toml" => Self::Toml,
            "md" | "markdown" => Self::Markdown,
            _ => Self::PlainText,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Python => "python",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Markdown => "markdown",
            Self::PlainText => "text",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn detects_common_extensions() {
        assert_eq!(
            Language::detect_path(Some(Path::new("main.rs"))),
            Language::Rust
        );
        assert_eq!(
            Language::detect_path(Some(Path::new("app.ts"))),
            Language::TypeScript
        );
        assert_eq!(
            Language::detect_path(Some(Path::new("script.py"))),
            Language::Python
        );
        assert_eq!(
            Language::detect_path(Some(Path::new("README.md"))),
            Language::Markdown
        );
    }

    #[test]
    fn unknown_is_plain_text() {
        assert_eq!(
            Language::detect_path(Some(Path::new("unknown.blob"))),
            Language::PlainText
        );
        assert_eq!(Language::detect_path(None), Language::PlainText);
    }
}
