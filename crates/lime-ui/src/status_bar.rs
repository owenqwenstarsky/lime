use lime_core::Position;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{layout::Rect, Frame};

use crate::theme::UiTheme;

#[derive(Debug, Clone, Copy)]
pub struct StatusInfo<'a> {
    pub file_name: &'a str,
    pub language: &'a str,
    pub position: Position,
    pub dirty: bool,
    pub message: Option<&'a str>,
}

pub fn render_status_bar(frame: &mut Frame<'_>, area: Rect, theme: &UiTheme, info: StatusInfo<'_>) {
    let dirty_mark = if info.dirty { "modified" } else { "saved" };
    let base = format!(
        " lime  {}  {}  Ln {}, Col {}  {}",
        info.file_name,
        info.language,
        info.position.line + 1,
        info.position.column + 1,
        dirty_mark
    );

    let line = if let Some(message) = info.message {
        Line::from(vec![
            Span::raw(base),
            Span::raw("  │  "),
            Span::styled(message.to_string(), theme.normal().fg(theme.status_fg)),
        ])
    } else {
        Line::from(base)
    };

    frame.render_widget(
        Paragraph::new(line).style(theme.normal().fg(theme.status_fg).bg(theme.status_bg)),
        area,
    );
}
