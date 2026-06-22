use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Debug, Clone, Copy)]
pub struct AppLayout {
    pub editor: Rect,
    pub preview: Option<Rect>,
    pub status: Rect,
    pub help: Rect,
}

/// Split `area` into the editor viewport, an optional markdown preview pane
/// on the right, the status bar, and the help bar.
pub fn app_layout(area: Rect) -> AppLayout {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    AppLayout {
        editor: chunks[0],
        preview: None,
        status: chunks[1],
        help: chunks[2],
    }
}

/// Split the editor region horizontally to make room for a preview pane on
/// the right. `editor_pct` is the width percentage (0-100) allocated to the
/// editor; the remainder goes to the preview, with a one-column divider.
pub fn with_preview(layout: AppLayout, editor_pct: u16) -> AppLayout {
    let editor_pct = editor_pct.clamp(20, 94);
    let div = if layout.editor.width > 30 { 1 } else { 0 };
    let constraints = [
        Constraint::Percentage(editor_pct),
        Constraint::Length(div),
        Constraint::Percentage(100u16.saturating_sub(editor_pct)),
    ];
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(layout.editor);

    AppLayout {
        editor: split[0],
        preview: Some(split[2]),
        status: layout.status,
        help: layout.help,
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_split_gives_two_panes() {
        let area = Rect::new(0, 0, 100, 24);
        let base = app_layout(area);
        let split = with_preview(base, 55);
        assert!(split.preview.is_some());
        assert!(split.editor.width < base.editor.width);
        assert!(split.preview.unwrap().width > 0);
    }

    #[test]
    fn preview_disabled_has_no_pane() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = app_layout(area);
        assert!(layout.preview.is_none());
    }
}
