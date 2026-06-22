//! Render markdown source into styled ratatui lines for the preview pane.
//!
//! This is a pure converter: feed it the buffer text and a theme, get back a
//! `Vec<Line>` that the caller can slice to the visible viewport. Parsing uses
//! `pulldown-cmark` so we get correct CommonMark handling for free.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{layout::Rect, Frame};

use crate::theme::UiTheme;

/// Render the full markdown document into styled lines.
pub fn render_lines(text: &str, theme: &UiTheme) -> Vec<Line<'static>> {
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM;
    let parser = Parser::new_ext(text, opts);
    let mut renderer = Renderer::new(theme);
    for event in parser {
        renderer.handle(event);
    }
    renderer.finish();
    renderer.lines
}

/// Draw a slice of the rendered markdown into the given area, scrolling by
/// `top_line` lines from the top of the document.
pub fn render_preview(
    frame: &mut Frame<'_>,
    area: Rect,
    text: &str,
    top_line: usize,
    theme: &UiTheme,
) {
    let block = Block::default()
        .title(" Preview ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.popup_border))
        .style(theme.normal().bg(theme.background));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let all_lines = render_lines(text, theme);
    let height = inner.height as usize;
    let start = top_line.min(all_lines.len());
    let end = (start + height).min(all_lines.len());
    let visible: Vec<Line<'_>> = all_lines
        .iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .cloned()
        .collect();

    frame.render_widget(
        Paragraph::new(visible)
            .style(theme.normal().bg(theme.background))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

struct Renderer<'a> {
    theme: &'a UiTheme,
    lines: Vec<Line<'static>>,
    /// Spans accumulated for the line currently being built.
    line_spans: Vec<Span<'static>>,
    /// Pending text run waiting to be flushed into a span.
    run: String,
    /// Style applied to the current text run.
    run_style: Style,
    /// Base style for the current block (heading, code, paragraph, ...).
    base_style: Style,
    /// Saved `run_style` values for nested inline tags (bold/italic/...).
    inline_saves: Vec<Style>,
    /// Prefix strings contributed by enclosing blocks (blockquote bar, list
    /// indent, ...). Concatenated to form the left margin of each line.
    indent_parts: Vec<&'static str>,
    /// One entry per open list. `Some(n)` is an ordered list whose next item
    /// marker is `n.`; `None` is an unordered list using `-`.
    list_stack: Vec<Option<u64>>,
    /// Marker to emit at the start of the current list item's first line.
    item_marker: Option<String>,
    /// Whether the next inline content should begin a fresh line.
    at_line_start: bool,
    /// Whether a blank separator line should be inserted before the next line.
    pending_blank: bool,
    /// Block-level state for fenced/indented code blocks.
    in_code_block: bool,
    /// Buffered lines for the current code block, emitted as a full box at
    /// `TagEnd::CodeBlock` once the width and line count are known.
    code_block_lines: Vec<String>,
    /// Partial line being accumulated for the current code block.
    code_block_current: String,
    /// Language label for the current fenced code block, if any.
    code_block_lang: Option<String>,
    /// Block-level state for blockquotes (affects prefix only).
    quote_depth: usize,
    /// Pending link destination URLs, one per nested `Link`/`Image` tag.
    link_urls: Vec<String>,
    /// Minimal table support: track current row cell separator state.
    table_cell_index: usize,
    in_table_row: bool,
}

impl<'a> Renderer<'a> {
    fn new(theme: &'a UiTheme) -> Self {
        Self {
            theme,
            lines: Vec::new(),
            line_spans: Vec::new(),
            run: String::new(),
            run_style: theme.normal(),
            base_style: theme.normal(),
            inline_saves: Vec::new(),
            indent_parts: Vec::new(),
            list_stack: Vec::new(),
            item_marker: None,
            at_line_start: true,
            pending_blank: false,
            in_code_block: false,
            code_block_lines: Vec::new(),
            code_block_current: String::new(),
            code_block_lang: None,
            quote_depth: 0,
            link_urls: Vec::new(),
            table_cell_index: 0,
            in_table_row: false,
        }
    }

    fn handle(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(end) => self.end_tag(end),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => self.inline_code(&code),
            Event::Html(html) | Event::InlineHtml(html) => self.text(&html),
            Event::SoftBreak => self.soft_break(),
            Event::HardBreak => self.hard_break(),
            Event::Rule => self.rule(),
            Event::TaskListMarker(checked) => self.task_marker(checked),
            Event::InlineMath(math) | Event::DisplayMath(math) => self.inline_code(&math),
            Event::FootnoteReference(label) => {
                let s = format!("[^{label}]");
                self.text(&s);
            }
        }
    }

    fn start_tag(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                self.end_line();
                self.reset_block_style();
            }
            Tag::Heading { level, .. } => {
                self.end_line();
                self.request_blank();
                self.base_style = self.heading_style(level);
                self.run_style = self.base_style;
                let hashes = "#".repeat(level as usize);
                self.line_spans
                    .push(Span::styled(format!("{hashes} "), self.base_style));
                self.at_line_start = false;
            }
            Tag::CodeBlock(kind) => {
                self.end_line();
                self.request_blank();
                self.in_code_block = true;
                self.base_style = self.code_style();
                self.run_style = self.base_style;
                self.code_block_lines.clear();
                self.code_block_current.clear();
                self.code_block_lang = match kind {
                    CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.into_string()),
                    _ => None,
                };
            }
            Tag::BlockQuote(_) => {
                self.end_line();
                self.indent_parts.push("│ ");
                self.quote_depth += 1;
                self.reset_block_style();
            }
            Tag::List(start) => {
                self.end_line();
                if !self.list_stack.is_empty() {
                    self.indent_parts.push("  ");
                }
                self.list_stack.push(start);
                self.reset_block_style();
            }
            Tag::Item => {
                self.end_line();
                self.item_marker = Some(self.next_item_marker());
                self.reset_block_style();
            }
            Tag::Emphasis => self.push_inline(self.italic_style()),
            Tag::Strong => self.push_inline(self.bold_style()),
            Tag::Strikethrough => self.push_inline(self.strike_style()),
            Tag::Link { dest_url, .. } => {
                self.link_urls.push(dest_url.into_string());
                self.push_inline(self.link_style());
            }
            Tag::Image { dest_url, .. } => {
                self.link_urls.push(dest_url.into_string());
                self.line_spans
                    .push(Span::styled("image: ", self.theme.dim()));
                self.push_inline(self.link_style());
            }
            Tag::Table(_) => {
                self.end_line();
                self.request_blank();
            }
            Tag::TableHead | Tag::TableRow => {
                self.end_line();
                self.in_table_row = true;
                self.table_cell_index = 0;
                self.reset_block_style();
            }
            Tag::TableCell => {
                if self.table_cell_index > 0 {
                    self.line_spans.push(Span::styled(" │ ", self.theme.dim()));
                }
                self.table_cell_index += 1;
                self.at_line_start = false;
            }
            Tag::HtmlBlock => {
                self.end_line();
                self.request_blank();
                self.base_style = self.theme.dim();
                self.run_style = self.base_style;
            }
            Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::MetadataBlock(_)
            | Tag::Superscript
            | Tag::Subscript => {
                self.reset_block_style();
            }
        }
    }

    fn end_tag(&mut self, end: TagEnd) {
        match end {
            TagEnd::Paragraph => {
                self.end_line();
                self.request_blank();
            }
            TagEnd::Heading(_) => {
                self.end_line();
                self.request_blank();
                self.reset_block_style();
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                if !self.code_block_current.is_empty() {
                    self.code_block_lines
                        .push(std::mem::take(&mut self.code_block_current));
                }
                self.render_code_box();
                self.request_blank();
                self.reset_block_style();
            }
            TagEnd::BlockQuote(_) => {
                self.end_line();
                if !self.indent_parts.is_empty() {
                    self.indent_parts.pop();
                }
                self.quote_depth = self.quote_depth.saturating_sub(1);
                self.request_blank();
                self.reset_block_style();
            }
            TagEnd::List(_) => {
                self.end_line();
                let was_nested = !self.list_stack.is_empty();
                self.list_stack.pop();
                if was_nested {
                    if !self.indent_parts.is_empty() {
                        self.indent_parts.pop();
                    }
                } else {
                    self.request_blank();
                }
                self.reset_block_style();
            }
            TagEnd::Item => {
                self.end_line();
                self.item_marker = None;
                self.reset_block_style();
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => self.pop_inline(),
            TagEnd::Link | TagEnd::Image => {
                self.pop_inline();
                if let Some(url) = self.link_urls.pop() {
                    if !url.is_empty() {
                        self.ensure_line_started();
                        self.flush_run();
                        self.line_spans
                            .push(Span::styled(format!(" ({url})"), self.theme.dim()));
                    }
                }
            }
            TagEnd::Table => {
                self.end_line();
                self.request_blank();
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                self.in_table_row = false;
                self.table_cell_index = 0;
            }
            TagEnd::TableCell => {
                // Cell content already emitted inline; nothing to do.
            }
            TagEnd::HtmlBlock => {
                self.end_line();
                self.request_blank();
                self.reset_block_style();
            }
            _ => {}
        }
    }

    fn text(&mut self, text: &str) {
        if self.in_code_block {
            self.code_block_text(text);
            return;
        }
        self.ensure_line_started();
        self.run.push_str(text);
    }

    fn code_block_text(&mut self, text: &str) {
        for (i, part) in text.split('\n').enumerate() {
            if i > 0 {
                self.code_block_lines
                    .push(std::mem::take(&mut self.code_block_current));
            }
            self.code_block_current.push_str(part);
        }
        self.at_line_start = false;
    }

    fn render_code_box(&mut self) {
        let lines = std::mem::take(&mut self.code_block_lines);
        if lines.is_empty() {
            return;
        }

        let prefix = self.current_prefix();
        let dim = self.theme.dim();
        let code_style = self.code_style();
        let line_num_width = lines.len().to_string().len();
        let max_code_len = lines
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0)
            .max(1);

        // Layout per line: "│ " + {n:>w} + " │ " + {code:<pad} + " │"
        let box_width = 2 + line_num_width + 3 + max_code_len + 2;

        let push_prefix = |spans: &mut Vec<Span<'static>>, p: &str| {
            if !p.is_empty() {
                spans.push(Span::styled(p.to_string(), dim));
            }
        };

        // Top border
        let top = {
            let header = match &self.code_block_lang {
                Some(lang) => format!("┌─ {lang} "),
                None => "┌".to_string(),
            };
            let header_len = header.chars().count();
            let fill = box_width.saturating_sub(header_len + 1);
            format!("{}{}┐", header, "─".repeat(fill))
        };
        let mut spans = Vec::new();
        push_prefix(&mut spans, &prefix);
        spans.push(Span::styled(top, dim));
        self.lines.push(Line::from(spans));

        // Code lines with line numbers
        for (idx, code) in lines.iter().enumerate() {
            let num = format!("{:>width$}", idx + 1, width = line_num_width);
            let code_len = code.chars().count();

            let mut spans = Vec::new();
            push_prefix(&mut spans, &prefix);
            spans.push(Span::styled("│ ", dim));
            spans.push(Span::styled(num, dim));
            spans.push(Span::styled(" │ ", dim));
            spans.push(Span::styled(code.clone(), code_style));
            let pad = max_code_len.saturating_sub(code_len);
            if pad > 0 {
                spans.push(Span::styled(" ".repeat(pad), code_style));
            }
            spans.push(Span::styled(" │", dim));
            self.lines.push(Line::from(spans));
        }

        // Bottom border
        let bottom = format!("└{}┘", "─".repeat(box_width.saturating_sub(2)));
        let mut spans = Vec::new();
        push_prefix(&mut spans, &prefix);
        spans.push(Span::styled(bottom, dim));
        self.lines.push(Line::from(spans));
    }

    fn inline_code(&mut self, code: &str) {
        self.ensure_line_started();
        self.flush_run();
        let style = self.code_style();
        self.line_spans.push(Span::styled(code.to_string(), style));
    }

    fn soft_break(&mut self) {
        if self.in_code_block {
            return;
        }
        self.end_line();
        self.at_line_start = true;
        // Continuation of the same block: reuse the current prefix/marker flow
        // but do not re-emit the item marker (it was consumed on the first
        // line of the item).
    }

    fn hard_break(&mut self) {
        if self.in_code_block {
            return;
        }
        self.end_line();
        self.end_line();
    }

    fn rule(&mut self) {
        self.end_line();
        self.request_blank();
        let prefix = self.current_prefix();
        if !prefix.is_empty() {
            self.line_spans.push(Span::styled(prefix, self.theme.dim()));
        }
        let width = 40usize;
        let rule = "─".repeat(width);
        self.line_spans.push(Span::styled(rule, self.theme.dim()));
        self.flush_line();
        self.request_blank();
    }

    fn task_marker(&mut self, checked: bool) {
        // Task list markers replace the bullet marker.
        self.item_marker = None;
        self.ensure_line_started();
        self.flush_run();
        let glyph = if checked { "[x] " } else { "[ ] " };
        self.line_spans.push(Span::styled(glyph, self.theme.dim()));
    }

    fn ensure_line_started(&mut self) {
        if self.at_line_start {
            self.begin_line();
            self.at_line_start = false;
        }
    }

    fn begin_line(&mut self) {
        if self.pending_blank {
            self.pending_blank = false;
            self.lines.push(Line::default());
        }
        let prefix = self.current_prefix();
        if !prefix.is_empty() {
            self.line_spans.push(Span::styled(prefix, self.theme.dim()));
        }
        if let Some(marker) = self.item_marker.take() {
            self.line_spans
                .push(Span::styled(marker, self.theme.normal()));
        }
    }

    fn end_line(&mut self) {
        self.flush_run();
        if !self.line_spans.is_empty() || self.in_code_block {
            self.lines
                .push(Line::from(std::mem::take(&mut self.line_spans)));
        }
        self.at_line_start = true;
    }

    fn flush_line(&mut self) {
        self.flush_run();
        self.lines
            .push(Line::from(std::mem::take(&mut self.line_spans)));
        self.at_line_start = true;
    }

    fn flush_run(&mut self) {
        if !self.run.is_empty() {
            let span = Span::styled(self.run.clone(), self.run_style);
            self.line_spans.push(span);
            self.run.clear();
        }
    }

    fn request_blank(&mut self) {
        if !self.lines.is_empty() {
            self.pending_blank = true;
        }
    }

    fn current_prefix(&self) -> String {
        let mut s = String::new();
        for part in &self.indent_parts {
            s.push_str(part);
        }
        s
    }

    fn next_item_marker(&mut self) -> String {
        match self.list_stack.last_mut() {
            Some(Some(n)) => {
                let marker = format!("{}. ", n);
                *n += 1;
                marker
            }
            _ => "- ".to_string(),
        }
    }

    fn push_inline(&mut self, style: Style) {
        self.ensure_line_started();
        self.flush_run();
        self.inline_saves.push(self.run_style);
        self.run_style = style;
    }

    fn pop_inline(&mut self) {
        self.flush_run();
        if let Some(saved) = self.inline_saves.pop() {
            self.run_style = saved;
        }
    }

    fn reset_block_style(&mut self) {
        self.base_style = self.theme.normal();
        self.run_style = self.base_style;
    }

    fn finish(&mut self) {
        self.end_line();
        // Trim trailing blank lines.
        while self.lines.last().is_some_and(|line| line.spans.is_empty()) {
            self.lines.pop();
        }
    }

    fn heading_style(&self, level: HeadingLevel) -> Style {
        let color = match level {
            HeadingLevel::H1 | HeadingLevel::H2 => self.theme.heading,
            HeadingLevel::H3 | HeadingLevel::H4 => self.theme.function,
            HeadingLevel::H5 | HeadingLevel::H6 => self.theme.type_name,
        };
        let mut style = Style::default().fg(color).bg(self.theme.background);
        if matches!(level, HeadingLevel::H1 | HeadingLevel::H2) {
            style = style.add_modifier(Modifier::BOLD);
        }
        style
    }

    fn code_style(&self) -> Style {
        Style::default()
            .fg(self.theme.string)
            .bg(self.theme.selection_bg)
    }

    fn bold_style(&self) -> Style {
        self.run_style.add_modifier(Modifier::BOLD)
    }

    fn italic_style(&self) -> Style {
        self.run_style.add_modifier(Modifier::ITALIC)
    }

    fn strike_style(&self) -> Style {
        self.run_style.add_modifier(Modifier::CROSSED_OUT)
    }

    fn link_style(&self) -> Style {
        Style::default()
            .fg(self.theme.keyword)
            .bg(self.theme.background)
            .add_modifier(Modifier::UNDERLINED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>()
            .join("")
    }

    #[test]
    fn renders_heading_with_hashes() {
        let theme = UiTheme::default();
        let lines = render_lines("# Title\n\nBody", &theme);
        assert!(line_text(&lines[0]).starts_with("# Title"));
        assert!(line_text(&lines[1]).is_empty());
        assert_eq!(line_text(&lines[2]), "Body");
    }

    #[test]
    fn renders_unordered_list() {
        let theme = UiTheme::default();
        let lines = render_lines("- one\n- two\n- three", &theme);
        assert_eq!(line_text(&lines[0]), "- one");
        assert_eq!(line_text(&lines[1]), "- two");
        assert_eq!(line_text(&lines[2]), "- three");
    }

    #[test]
    fn renders_ordered_list_with_increasing_numbers() {
        let theme = UiTheme::default();
        let lines = render_lines("1. first\n2. second\n3. third", &theme);
        assert_eq!(line_text(&lines[0]), "1. first");
        assert_eq!(line_text(&lines[1]), "2. second");
        assert_eq!(line_text(&lines[2]), "3. third");
    }

    #[test]
    fn renders_nested_list_with_indent() {
        let theme = UiTheme::default();
        let lines = render_lines("- top\n  - nested\n- back", &theme);
        assert_eq!(line_text(&lines[0]), "- top");
        assert_eq!(line_text(&lines[1]), "  - nested");
        assert_eq!(line_text(&lines[2]), "- back");
    }

    #[test]
    fn renders_blockquote_with_bar_prefix() {
        let theme = UiTheme::default();
        let lines = render_lines("> quoted text", &theme);
        assert_eq!(line_text(&lines[0]), "│ quoted text");
    }

    #[test]
    fn renders_fenced_code_block_with_box_and_line_numbers() {
        let theme = UiTheme::default();
        let md = "```rust\nfn main() {}\n```\n";
        let lines = render_lines(md, &theme);
        let combined: Vec<String> = lines.iter().map(|l| line_text(l)).collect();

        // Top border with language label
        assert!(
            combined.iter().any(|s| s.starts_with("┌─ rust ")),
            "missing top border: {combined:?}"
        );
        // Code line with line number and side borders
        assert!(
            combined
                .iter()
                .any(|s| s.contains("│ 1 │") && s.contains("fn main()")),
            "missing numbered code line: {combined:?}"
        );
        // Bottom border
        assert!(
            combined.iter().any(|s| s.starts_with('└')),
            "missing bottom border: {combined:?}"
        );
    }

    #[test]
    fn renders_code_block_line_number_padding() {
        let theme = UiTheme::default();
        let md = "```\na\nb\nc\nd\ne\nf\ng\nh\ni\nj\n```\n";
        let lines = render_lines(md, &theme);
        let combined: Vec<String> = lines.iter().map(|l| line_text(l)).collect();
        // Single-digit line numbers should be right-aligned to width 2
        assert!(
            combined.iter().any(|s| s.contains("│  1 │")),
            "line 1 not padded: {combined:?}"
        );
        // Double-digit line numbers should have no extra padding
        assert!(
            combined.iter().any(|s| s.contains("│ 10 │")),
            "line 10 not correct: {combined:?}"
        );
    }

    #[test]
    fn renders_code_block_without_language() {
        let theme = UiTheme::default();
        let md = "```\nhello\n```\n";
        let lines = render_lines(md, &theme);
        let combined: Vec<String> = lines.iter().map(|l| line_text(l)).collect();
        assert!(
            combined
                .iter()
                .any(|s| s.starts_with('┌') && !s.contains("┌─ ")),
            "plain code block should have bare top border: {combined:?}"
        );
        assert!(
            combined.iter().any(|s| s.contains("hello")),
            "missing code content: {combined:?}"
        );
    }

    #[test]
    fn renders_horizontal_rule() {
        let theme = UiTheme::default();
        let lines = render_lines("above\n\n---\n\nbelow", &theme);
        let combined: String = lines
            .iter()
            .map(|l| line_text(l))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(combined.contains("───"), "{combined}");
        assert!(combined.contains("above"));
        assert!(combined.contains("below"));
    }

    #[test]
    fn renders_link_with_url() {
        let theme = UiTheme::default();
        let lines = render_lines("[Lime](https://example.com)", &theme);
        let text = line_text(&lines[0]);
        assert!(text.contains("Lime"), "{text}");
        assert!(text.contains("(https://example.com)"), "{text}");
    }

    #[test]
    fn renders_task_list_markers() {
        let theme = UiTheme::default();
        let lines = render_lines("- [ ] todo\n- [x] done", &theme);
        assert_eq!(line_text(&lines[0]), "[ ] todo");
        assert_eq!(line_text(&lines[1]), "[x] done");
    }

    #[test]
    fn renders_bold_and_italic_text() {
        let theme = UiTheme::default();
        let lines = render_lines("**bold** and _italic_", &theme);
        let text = line_text(&lines[0]);
        assert!(text.contains("bold"), "{text}");
        assert!(text.contains("italic"), "{text}");
    }

    #[test]
    fn empty_input_yields_no_lines() {
        let theme = UiTheme::default();
        let lines = render_lines("", &theme);
        assert!(lines.is_empty());
    }
}
