use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{layout::Rect, Frame};

use crate::theme::UiTheme;

pub fn render_help_bar(frame: &mut Frame<'_>, area: Rect, theme: &UiTheme) {
    let line = Line::from(vec![
        Span::styled(
            " Ctrl-S ",
            theme.normal().fg(theme.heading).bg(theme.help_bg),
        ),
        Span::styled("Save ", theme.normal().fg(theme.help_fg).bg(theme.help_bg)),
        Span::styled(
            " Ctrl-F ",
            theme.normal().fg(theme.heading).bg(theme.help_bg),
        ),
        Span::styled("Files ", theme.normal().fg(theme.help_fg).bg(theme.help_bg)),
        Span::styled(
            " Ctrl-R ",
            theme.normal().fg(theme.heading).bg(theme.help_bg),
        ),
        Span::styled(
            "Search ",
            theme.normal().fg(theme.help_fg).bg(theme.help_bg),
        ),
        Span::styled(
            " Ctrl-G ",
            theme.normal().fg(theme.heading).bg(theme.help_bg),
        ),
        Span::styled(
            "Go line ",
            theme.normal().fg(theme.help_fg).bg(theme.help_bg),
        ),
        Span::styled(
            " Ctrl-P ",
            theme.normal().fg(theme.heading).bg(theme.help_bg),
        ),
        Span::styled(
            "Preview ",
            theme.normal().fg(theme.help_fg).bg(theme.help_bg),
        ),
        Span::styled(
            " Ctrl-Q ",
            theme.normal().fg(theme.heading).bg(theme.help_bg),
        ),
        Span::styled("Quit", theme.normal().fg(theme.help_fg).bg(theme.help_bg)),
    ]);

    frame.render_widget(
        Paragraph::new(line).style(theme.normal().bg(theme.help_bg)),
        area,
    );
}
