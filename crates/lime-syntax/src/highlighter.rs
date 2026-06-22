use tree_sitter::{Node, Parser};

use crate::language::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightKind {
    Normal,
    Keyword,
    String,
    Comment,
    Function,
    TypeName,
    Number,
    Error,
    Punctuation,
    Heading,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub line_start: usize,
    pub column_start: usize,
    pub line_end: usize,
    pub column_end: usize,
    pub kind: HighlightKind,
}

impl HighlightSpan {
    pub const fn new(
        line_start: usize,
        column_start: usize,
        line_end: usize,
        column_end: usize,
        kind: HighlightKind,
    ) -> Self {
        Self {
            line_start,
            column_start,
            line_end,
            column_end,
            kind,
        }
    }

    pub fn intersects_line(&self, line: usize) -> bool {
        self.line_start <= line && line <= self.line_end
    }
}

#[derive(Default)]
pub struct Highlighter {
    cache: Option<HighlightCache>,
}

#[derive(Clone)]
struct HighlightCache {
    revision: u64,
    language: Language,
    spans: Vec<HighlightSpan>,
}

impl Highlighter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.cache = None;
    }

    pub fn highlight(
        &mut self,
        text: &str,
        language: Language,
        revision: u64,
    ) -> Vec<HighlightSpan> {
        if let Some(cache) = &self.cache {
            if cache.revision == revision && cache.language == language {
                return cache.spans.clone();
            }
        }

        let mut spans = if text.len() <= 2_000_000 {
            self.highlight_with_tree_sitter(text, language)
                .unwrap_or_else(|| lexical_highlight(text, language))
        } else {
            lexical_highlight(text, language)
        };

        spans.sort_by_key(|span| {
            (
                span.line_start,
                span.column_start,
                span.line_end,
                span.column_end,
            )
        });
        self.cache = Some(HighlightCache {
            revision,
            language,
            spans: spans.clone(),
        });
        spans
    }

    fn highlight_with_tree_sitter(
        &self,
        text: &str,
        language: Language,
    ) -> Option<Vec<HighlightSpan>> {
        let mut parser = Parser::new();
        let grammar = grammar_for(language)?;
        parser.set_language(&grammar).ok()?;
        let tree = parser.parse(text, None)?;
        let lines: Vec<&str> = text.lines().collect();
        let mut spans = Vec::new();
        collect_node_spans(tree.root_node(), &lines, &mut spans);

        if spans.is_empty() {
            None
        } else {
            Some(spans)
        }
    }
}

fn grammar_for(language: Language) -> Option<tree_sitter::Language> {
    match language {
        Language::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
        Language::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
        Language::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        Language::Python => Some(tree_sitter_python::LANGUAGE.into()),
        Language::Json => Some(tree_sitter_json::LANGUAGE.into()),
        Language::Toml | Language::Markdown | Language::PlainText => None,
    }
}

fn collect_node_spans(node: Node<'_>, lines: &[&str], spans: &mut Vec<HighlightSpan>) {
    if let Some(kind) = kind_for_node(node.kind()) {
        if let Some(span) = span_from_node(node, lines, kind) {
            spans.push(span);
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_node_spans(child, lines, spans);
    }
}

fn kind_for_node(kind: &str) -> Option<HighlightKind> {
    if kind == "ERROR" {
        return Some(HighlightKind::Error);
    }

    if kind.contains("comment") {
        return Some(HighlightKind::Comment);
    }

    if kind.contains("string") || kind == "template_string" || kind == "raw_string_literal" {
        return Some(HighlightKind::String);
    }

    if kind.contains("number")
        || kind.contains("integer")
        || kind.contains("float")
        || kind == "true"
        || kind == "false"
        || kind == "null"
        || kind == "none"
    {
        return Some(HighlightKind::Number);
    }

    if kind == "type_identifier"
        || kind == "primitive_type"
        || kind == "scoped_type_identifier"
        || kind == "generic_type"
    {
        return Some(HighlightKind::TypeName);
    }

    if is_keyword(kind) {
        return Some(HighlightKind::Keyword);
    }

    if matches!(
        kind,
        "(" | ")" | "{" | "}" | "[" | "]" | ":" | ";" | "," | "." | "->" | "=>"
    ) {
        return Some(HighlightKind::Punctuation);
    }

    None
}

fn is_keyword(kind: &str) -> bool {
    matches!(
        kind,
        "as" | "async"
            | "await"
            | "break"
            | "class"
            | "const"
            | "continue"
            | "def"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "fn"
            | "for"
            | "from"
            | "function"
            | "if"
            | "impl"
            | "import"
            | "in"
            | "interface"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "mut"
            | "new"
            | "null"
            | "pub"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "try"
            | "type"
            | "use"
            | "var"
            | "while"
            | "yield"
            | "where"
    )
}

fn span_from_node(node: Node<'_>, lines: &[&str], kind: HighlightKind) -> Option<HighlightSpan> {
    let start = node.start_position();
    let end = node.end_position();
    if start.row >= lines.len() || end.row >= lines.len().max(1) {
        return None;
    }

    let start_col = byte_col_to_char_col(lines.get(start.row).copied().unwrap_or(""), start.column);
    let end_col = byte_col_to_char_col(lines.get(end.row).copied().unwrap_or(""), end.column);

    if start.row == end.row && start_col == end_col {
        return None;
    }

    Some(HighlightSpan::new(
        start.row, start_col, end.row, end_col, kind,
    ))
}

fn byte_col_to_char_col(line: &str, byte_col: usize) -> usize {
    let mut byte = byte_col.min(line.len());
    while byte > 0 && !line.is_char_boundary(byte) {
        byte -= 1;
    }
    line[..byte].chars().count()
}

fn lexical_highlight(text: &str, language: Language) -> Vec<HighlightSpan> {
    let mut spans = Vec::new();

    for (line_idx, line) in text.lines().enumerate() {
        if language == Language::Markdown {
            highlight_markdown_line(line_idx, line, &mut spans);
            continue;
        }

        highlight_comments(line_idx, line, language, &mut spans);
        highlight_strings(line_idx, line, &mut spans);
        highlight_numbers(line_idx, line, &mut spans);
        highlight_keywords(line_idx, line, language, &mut spans);
    }

    spans
}

fn highlight_markdown_line(line_idx: usize, line: &str, spans: &mut Vec<HighlightSpan>) {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    if trimmed.starts_with('#') {
        spans.push(HighlightSpan::new(
            line_idx,
            indent,
            line_idx,
            line.chars().count(),
            HighlightKind::Heading,
        ));
    } else if trimmed.starts_with('>') {
        spans.push(HighlightSpan::new(
            line_idx,
            indent,
            line_idx,
            line.chars().count(),
            HighlightKind::Comment,
        ));
    }

    highlight_strings(line_idx, line, spans);
}

fn highlight_comments(
    line_idx: usize,
    line: &str,
    language: Language,
    spans: &mut Vec<HighlightSpan>,
) {
    let marker = match language {
        Language::Rust | Language::JavaScript | Language::TypeScript => Some("//"),
        Language::Python | Language::Toml => Some("#"),
        _ => None,
    };

    if let Some(marker) = marker {
        if let Some(byte_idx) = line.find(marker) {
            let col = line[..byte_idx].chars().count();
            spans.push(HighlightSpan::new(
                line_idx,
                col,
                line_idx,
                line.chars().count(),
                HighlightKind::Comment,
            ));
        }
    }
}

fn highlight_strings(line_idx: usize, line: &str, spans: &mut Vec<HighlightSpan>) {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let quote = chars[i];
        if quote != '"' && quote != '\'' && quote != '`' {
            i += 1;
            continue;
        }

        let start = i;
        i += 1;
        let mut escaped = false;
        while i < chars.len() {
            let ch = chars[i];
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                i += 1;
                break;
            }
            i += 1;
        }

        spans.push(HighlightSpan::new(
            line_idx,
            start,
            line_idx,
            i.min(chars.len()),
            HighlightKind::String,
        ));
    }
}

fn highlight_numbers(line_idx: usize, line: &str, spans: &mut Vec<HighlightSpan>) {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len()
            && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '.')
        {
            i += 1;
        }
        spans.push(HighlightSpan::new(
            line_idx,
            start,
            line_idx,
            i,
            HighlightKind::Number,
        ));
    }
}

fn highlight_keywords(
    line_idx: usize,
    line: &str,
    language: Language,
    spans: &mut Vec<HighlightSpan>,
) {
    let keywords = keywords_for(language);
    if keywords.is_empty() {
        return;
    }

    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if !is_word_start(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        while i < chars.len() && is_word_continue(chars[i]) {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();
        if keywords.contains(&word.as_str()) {
            spans.push(HighlightSpan::new(
                line_idx,
                start,
                line_idx,
                i,
                HighlightKind::Keyword,
            ));
        }
    }
}

fn is_word_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_word_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn keywords_for(language: Language) -> &'static [&'static str] {
    match language {
        Language::Rust => &[
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
            "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
            "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
            "trait", "true", "type", "unsafe", "use", "where", "while",
        ],
        Language::JavaScript | Language::TypeScript => &[
            "async",
            "await",
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "debugger",
            "default",
            "delete",
            "do",
            "else",
            "export",
            "extends",
            "false",
            "finally",
            "for",
            "function",
            "if",
            "import",
            "in",
            "instanceof",
            "interface",
            "let",
            "new",
            "null",
            "return",
            "super",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "type",
            "typeof",
            "var",
            "void",
            "while",
            "yield",
        ],
        Language::Python => &[
            "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
            "elif", "else", "except", "False", "finally", "for", "from", "global", "if", "import",
            "in", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return",
            "True", "try", "while", "with", "yield",
        ],
        Language::Json | Language::Toml | Language::Markdown | Language::PlainText => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexical_rust_keywords() {
        let spans = lexical_highlight("fn main() { let n = 1; }", Language::Rust);
        assert!(spans.iter().any(|span| span.kind == HighlightKind::Keyword));
        assert!(spans.iter().any(|span| span.kind == HighlightKind::Number));
    }

    #[test]
    fn detects_markdown_heading() {
        let spans = lexical_highlight("# Hello", Language::Markdown);
        assert_eq!(spans[0].kind, HighlightKind::Heading);
    }
}
