use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lime_core::EditorCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    Editor(EditorCommand),
    Save,
    Quit,
    OpenFilePicker,
    OpenGoToLine,
    OpenSearch,
    ToggleMarkdownPreview,
    ClosePopup,
    Confirm,
    Cancel,
    None,
}

pub fn map_editing_key(key: KeyEvent) -> AppAction {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    if ctrl {
        return match key.code {
            KeyCode::Char('s') | KeyCode::Char('S') => AppAction::Save,
            KeyCode::Char('q') | KeyCode::Char('Q') => AppAction::Quit,
            KeyCode::Char('f') | KeyCode::Char('F') => AppAction::OpenFilePicker,
            KeyCode::Char('g') | KeyCode::Char('G') => AppAction::OpenGoToLine,
            KeyCode::Char('r') | KeyCode::Char('R') => AppAction::OpenSearch,
            KeyCode::Char('p') | KeyCode::Char('P') => AppAction::ToggleMarkdownPreview,
            KeyCode::Char('z') | KeyCode::Char('Z') => AppAction::Editor(EditorCommand::Undo),
            KeyCode::Char('y') | KeyCode::Char('Y') => AppAction::Editor(EditorCommand::Redo),
            KeyCode::Char('a') | KeyCode::Char('A') => {
                AppAction::Editor(EditorCommand::MoveLineStart)
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                AppAction::Editor(EditorCommand::MoveLineEnd)
            }
            _ => AppAction::None,
        };
    }

    if alt {
        return AppAction::None;
    }

    match key.code {
        KeyCode::Esc => AppAction::ClosePopup,
        KeyCode::Left => AppAction::Editor(EditorCommand::MoveLeft),
        KeyCode::Right => AppAction::Editor(EditorCommand::MoveRight),
        KeyCode::Up => AppAction::Editor(EditorCommand::MoveUp),
        KeyCode::Down => AppAction::Editor(EditorCommand::MoveDown),
        KeyCode::Home => AppAction::Editor(EditorCommand::MoveLineStart),
        KeyCode::End => AppAction::Editor(EditorCommand::MoveLineEnd),
        KeyCode::PageUp => AppAction::Editor(EditorCommand::PageUp),
        KeyCode::PageDown => AppAction::Editor(EditorCommand::PageDown),
        KeyCode::Backspace => AppAction::Editor(EditorCommand::Backspace),
        KeyCode::Delete => AppAction::Editor(EditorCommand::Delete),
        KeyCode::Enter => AppAction::Editor(EditorCommand::Newline),
        KeyCode::Tab => AppAction::Editor(EditorCommand::InsertText("\t".to_string())),
        KeyCode::Char(ch) => AppAction::Editor(EditorCommand::InsertChar(ch)),
        _ => AppAction::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    #[test]
    fn maps_save_shortcut() {
        let action = map_editing_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(action, AppAction::Save);
    }

    #[test]
    fn maps_text_input() {
        let action = map_editing_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(action, AppAction::Editor(EditorCommand::InsertChar('x')));
    }

    #[test]
    fn maps_preview_toggle_shortcut() {
        let action = map_editing_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
        assert_eq!(action, AppAction::ToggleMarkdownPreview);
    }
}
