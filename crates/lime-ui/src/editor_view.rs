use lime_core::{Editor, Position};
use lime_syntax::{HighlightKind, HighlightSpan};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::config::Config;
use crate::theme::UiTheme;

#[derive(Debug, Clone, Copy, Default)]
pub struct Viewport {
    pub top_line: usize,
    pub left_col: usize,
    pub height: usize,
    pub width: usize,
}

impl Viewport {
    pub fn ensure_cursor_visible(
        &mut self,
        cursor: Position,
        total_lines: usize,
        viewport_height: usize,
        text_width: usize,
    ) {
        self.height = viewport_height;
        self.width = text_width;

        let margin = 2usize.min(viewport_height.saturating_div(3));
        if cursor.line < self.top_line + margin {
            self.top_line = cursor.line.saturating_sub(margin);
        } else if cursor.line >= self.top_line + viewport_height.saturating_sub(margin) {
            self.top_line = cursor
                .line
                .saturating_sub(viewport_height.saturating_sub(margin + 1));
        }

        let max_top = total_lines.saturating_sub(viewport_height.max(1));
        self.top_line = self.top_line.min(max_top);

        if cursor.column < self.left_col {
            self.left_col = cursor.column;
        } else if cursor.column >= self.left_col + text_width.max(1) {
            self.left_col = cursor.column.saturating_sub(text_width.saturating_sub(1));
        }
    }
}

pub fn gutter_width(total_lines: usize, show_line_numbers: bool) -> u16 {
    if show_line_numbers {
        total_lines.max(1).to_string().len() as u16 + 3
    } else {
        0
    }
}

pub fn render_editor(
    frame: &mut Frame<'_>,
    area: Rect,
    editor: &Editor,
    viewport: &Viewport,
    config: &Config,
    theme: &UiTheme,
    highlights: &[HighlightSpan],
) {
    let total_lines = editor.line_count();
    let gutter = gutter_width(total_lines, config.show_line_numbers);
    let text_width = area.width.saturating_sub(gutter) as usize;
    let mut lines = Vec::with_capacity(area.height as usize);
    let cursor_line = editor.cursor().position.line;

    for screen_row in 0..area.height as usize {
        let line_idx = viewport.top_line + screen_row;
        if line_idx < total_lines {
            let mut spans = Vec::new();
            if config.show_line_numbers {
                let number = format!(
                    "{:>width$} │ ",
                    line_idx + 1,
                    width = (gutter as usize).saturating_sub(3)
                );
                let style = if line_idx == cursor_line {
                    Style::default()
                        .fg(theme.gutter_active)
                        .bg(theme.background)
                } else {
                    Style::default().fg(theme.gutter).bg(theme.background)
                };
                spans.push(Span::styled(number, style));
            }

            let text = editor.line_text_without_ending(line_idx);
            spans.extend(styled_line_spans(
                line_idx,
                &text,
                viewport.left_col,
                text_width,
                highlights,
                theme,
            ));
            lines.push(Line::from(spans));
        } else {
            let blank = if config.show_line_numbers {
                " ".repeat(gutter as usize)
            } else {
                String::new()
            };
            lines.push(Line::from(Span::styled(blank, theme.normal())));
        }
    }

    frame.render_widget(Paragraph::new(lines).style(theme.normal()), area);

    let cursor = editor.cursor().position;
    if cursor.line >= viewport.top_line
        && cursor.line < viewport.top_line + area.height as usize
        && cursor.column >= viewport.left_col
        && cursor.column < viewport.left_col + text_width.max(1)
    {
        let x = area.x + gutter + (cursor.column - viewport.left_col) as u16;
        let y = area.y + (cursor.line - viewport.top_line) as u16;
        if x < area.x + area.width && y < area.y + area.height {
            frame.set_cursor_position((x, y));
        }
    }
}

fn styled_line_spans(
    line_idx: usize,
    text: &str,
    left_col: usize,
    width: usize,
    highlights: &[HighlightSpan],
    theme: &UiTheme,
) -> Vec<Span<'static>> {
    let chars: Vec<char> = text.chars().collect();
    if width == 0 {
        return Vec::new();
    }

    let mut kinds = vec![HighlightKind::Normal; chars.len()];
    for span in highlights
        .iter()
        .filter(|span| span.intersects_line(line_idx))
    {
        let start = if line_idx == span.line_start {
            span.column_start
        } else {
            0
        };
        let end = if line_idx == span.line_end {
            span.column_end
        } else {
            chars.len()
        };
        let start = start.min(chars.len());
        let end = end.min(chars.len());
        for kind in kinds.iter_mut().take(end).skip(start) {
            *kind = span.kind;
        }
    }

    let start = left_col.min(chars.len());
    let end = (start + width).min(chars.len());
    if start >= end {
        return vec![Span::raw("")];
    }

    let mut spans = Vec::new();
    let mut current_kind = kinds[start];
    let mut current = String::new();

    for idx in start..end {
        let kind = kinds[idx];
        if kind != current_kind && !current.is_empty() {
            spans.push(Span::styled(
                current.clone(),
                theme.syntax_style(current_kind),
            ));
            current.clear();
            current_kind = kind;
        }
        current.push(chars[idx]);
    }

    if !current.is_empty() {
        spans.push(Span::styled(current, theme.syntax_style(current_kind)));
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_tracks_cursor() {
        let mut viewport = Viewport::default();
        viewport.ensure_cursor_visible(Position::new(50, 10), 100, 10, 20);
        assert!(viewport.top_line <= 50);
        assert!(viewport.top_line + 10 > 50);
    }
}
