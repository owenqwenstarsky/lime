use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use lime_core::{
    CommandResult, Editor, EditorCommand, FileOpenDecision, FileOpenPolicy, TextBuffer,
};
use lime_syntax::{HighlightSpan, Highlighter, Language};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::command_bar::render_help_bar;
use crate::config::Config;
use crate::editor_view::{gutter_width, render_editor, Viewport};
use crate::file_picker::FilePickerState;
use crate::input::{map_editing_key, AppAction};
use crate::layout::{app_layout, centered_rect, with_preview};
use crate::markdown_preview::render_preview;
use crate::prompt::{ConfirmKind, ConfirmState, PromptKind, PromptState};
use crate::status_bar::{render_status_bar, StatusInfo};
use crate::terminal::TerminalSession;
use crate::theme::UiTheme;

#[derive(Debug, Clone)]
pub struct AppOptions {
    pub path: Option<PathBuf>,
    pub force: bool,
    pub config_path: Option<PathBuf>,
}

impl AppOptions {
    pub fn new(path: Option<PathBuf>, force: bool, config_path: Option<PathBuf>) -> Self {
        Self {
            path,
            force,
            config_path,
        }
    }
}

#[derive(Debug, Clone)]
enum Mode {
    Editing,
    FilePicker(FilePickerState),
    Prompt(PromptState),
    Confirm(ConfirmState),
}

pub struct App {
    editor: Editor,
    config: Config,
    theme: UiTheme,
    mode: Mode,
    viewport: Viewport,
    root: PathBuf,
    should_quit: bool,
    status_message: Option<String>,
    status_at: Option<Instant>,
    file_policy: FileOpenPolicy,
    highlighter: Highlighter,
    language: Language,
    highlights: Vec<HighlightSpan>,
    highlighted_revision: Option<u64>,
    highlighted_language: Language,
    preview_open: bool,
    preview_top_line: usize,
    preview_revision: Option<u64>,
}

impl App {
    pub fn new(options: AppOptions) -> Result<Self> {
        let (config, config_warning) = Config::load(options.config_path.as_deref());
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut app = Self {
            editor: Editor::new(),
            config,
            theme: UiTheme::default(),
            mode: Mode::Editing,
            viewport: Viewport::default(),
            root: cwd.clone(),
            should_quit: false,
            status_message: config_warning,
            status_at: Some(Instant::now()),
            file_policy: FileOpenPolicy {
                force: options.force,
                ..FileOpenPolicy::default()
            },
            highlighter: Highlighter::new(),
            language: Language::PlainText,
            highlights: Vec::new(),
            highlighted_revision: None,
            highlighted_language: Language::PlainText,
            preview_open: false,
            preview_top_line: 0,
            preview_revision: None,
        };

        if let Some(path) = options.path {
            app.open_initial_path(path, &cwd)?;
        }

        app.refresh_language();
        app.refresh_highlights();
        Ok(app)
    }

    fn open_initial_path(&mut self, path: PathBuf, cwd: &Path) -> Result<()> {
        if path.is_dir() {
            self.root = path.canonicalize().unwrap_or(path);
            self.mode = Mode::FilePicker(FilePickerState::new(&self.root));
            return Ok(());
        }

        if let Some(parent) = path.parent() {
            if parent.exists() {
                self.root = cwd.to_path_buf();
            }
        }

        if !path.exists() {
            let mut buffer = TextBuffer::from_text("");
            buffer.set_path(path);
            self.editor.replace_buffer(buffer);
            self.set_status("New file");
            return Ok(());
        }

        match self.evaluate_open_path(&path)? {
            OpenEvaluation::Open => self.open_file_unchecked(&path),
            OpenEvaluation::Prompt(size) => {
                self.mode = Mode::Confirm(ConfirmState::new(
                    ConfirmKind::OpenLarge { path, size },
                    format!(
                        "This file is {}. Lime may be slower with very large files. Open anyway? [y/N]",
                        format_bytes(size)
                    ),
                ));
                Ok(())
            }
            OpenEvaluation::Refuse(size) => {
                self.set_status(format!(
                    "Refused to open {} file. Re-run with --force to open it.",
                    format_bytes(size)
                ));
                Ok(())
            }
        }
    }

    fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
        self.status_at = Some(Instant::now());
    }

    fn maybe_clear_status(&mut self) {
        if self
            .status_at
            .is_some_and(|instant| instant.elapsed() > Duration::from_secs(5))
        {
            self.status_message = None;
            self.status_at = None;
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut terminal = TerminalSession::enter()?;

        while !self.should_quit {
            self.maybe_clear_status();
            self.refresh_highlights();
            terminal.terminal_mut().draw(|frame| self.draw(frame))?;

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => self.handle_key(key)?,
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame<'_>) {
        let area = frame.area();
        let base_layout = app_layout(area);
        let show_preview = self.preview_open && self.language == Language::Markdown;
        let layout = if show_preview {
            with_preview(base_layout, 55)
        } else {
            base_layout
        };
        let total_lines = self.editor.line_count();
        let gutter = gutter_width(total_lines, self.config.show_line_numbers);
        let text_width = layout.editor.width.saturating_sub(gutter) as usize;
        self.viewport.ensure_cursor_visible(
            self.editor.cursor().position,
            total_lines,
            layout.editor.height as usize,
            text_width,
        );

        render_editor(
            frame,
            layout.editor,
            &self.editor,
            &self.viewport,
            &self.config,
            &self.theme,
            &self.highlights,
        );

        if let Some(preview_area) = layout.preview {
            let revision = self.editor.revision();
            if self.preview_revision != Some(revision) {
                self.preview_revision = Some(revision);
            }
            let preview_text = self.editor.text();
            let preview_lines = crate::markdown_preview::render_lines(&preview_text, &self.theme);
            let height = preview_area.height.saturating_sub(2) as usize;
            self.ensure_preview_visible(preview_lines.len(), height);
            render_preview(
                frame,
                preview_area,
                &preview_text,
                self.preview_top_line,
                &self.theme,
            );
        }

        let file_name = self.file_label();
        render_status_bar(
            frame,
            layout.status,
            &self.theme,
            StatusInfo {
                file_name: &file_name,
                language: self.language.name(),
                position: self.editor.cursor().position,
                dirty: self.editor.is_dirty(),
                message: self.status_message.as_deref(),
            },
        );
        render_help_bar(frame, layout.help, &self.theme);

        match &self.mode {
            Mode::Editing => {}
            Mode::FilePicker(state) => self.render_file_picker(frame, area, state),
            Mode::Prompt(state) => self.render_prompt(frame, area, state),
            Mode::Confirm(state) => self.render_confirm(frame, area, state),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match &mut self.mode {
            Mode::Editing => self.handle_editing_key(key),
            Mode::FilePicker(_) => self.handle_file_picker_key(key),
            Mode::Prompt(_) => self.handle_prompt_key(key),
            Mode::Confirm(_) => self.handle_confirm_key(key),
        }
    }

    fn handle_editing_key(&mut self, key: KeyEvent) -> Result<()> {
        let action = map_editing_key(key);
        self.handle_action(action)
    }

    fn handle_action(&mut self, action: AppAction) -> Result<()> {
        match action {
            AppAction::Editor(command) => self.apply_editor_command(command),
            AppAction::Save => self.save_current(),
            AppAction::Quit => self.request_quit(),
            AppAction::OpenFilePicker => {
                self.mode = Mode::FilePicker(FilePickerState::new(&self.root));
                Ok(())
            }
            AppAction::OpenGoToLine => {
                self.mode = Mode::Prompt(PromptState::new(PromptKind::GoToLine, "Go to line"));
                Ok(())
            }
            AppAction::OpenSearch => {
                self.mode = Mode::Prompt(PromptState::new(PromptKind::Search, "Search"));
                Ok(())
            }
            AppAction::ToggleMarkdownPreview => {
                self.toggle_preview();
                Ok(())
            }
            AppAction::ClosePopup | AppAction::Cancel => {
                self.mode = Mode::Editing;
                Ok(())
            }
            AppAction::Confirm | AppAction::None => Ok(()),
        }
    }

    fn apply_editor_command(&mut self, command: EditorCommand) -> Result<()> {
        let command = match command {
            EditorCommand::InsertText(text) if text == "\t" && self.config.insert_spaces => {
                EditorCommand::InsertText(" ".repeat(self.config.tab_width))
            }
            other => other,
        };

        match self.editor.apply_command(command)? {
            CommandResult::Saved => self.set_status("Saved"),
            CommandResult::NeedsPath => {
                self.mode = Mode::Prompt(PromptState::new(PromptKind::SaveAs, "Save as"));
            }
            CommandResult::Modified => {
                self.refresh_language();
            }
            CommandResult::CursorMoved | CommandResult::None => {}
        }
        Ok(())
    }

    fn save_current(&mut self) -> Result<()> {
        match self.editor.save() {
            Ok(()) => {
                self.set_status("Saved");
                Ok(())
            }
            Err(lime_core::LimeError::MissingPath) => {
                self.mode = Mode::Prompt(PromptState::new(PromptKind::SaveAs, "Save as"));
                Ok(())
            }
            Err(err) => {
                self.set_status(format!("Save failed: {err}"));
                Ok(())
            }
        }
    }

    fn save_then_quit(&mut self) -> Result<()> {
        match self.editor.save() {
            Ok(()) => {
                self.set_status("Saved");
                self.should_quit = true;
            }
            Err(lime_core::LimeError::MissingPath) => {
                self.mode = Mode::Prompt(PromptState::new(PromptKind::SaveAsThenQuit, "Save as"));
            }
            Err(err) => self.set_status(format!("Save failed: {err}")),
        }
        Ok(())
    }

    fn save_then_open(&mut self, path: PathBuf) -> Result<()> {
        match self.editor.save() {
            Ok(()) => {
                self.set_status("Saved");
                self.open_with_policy_or_prompt(path)?;
            }
            Err(lime_core::LimeError::MissingPath) => {
                self.mode = Mode::Prompt(PromptState::new(
                    PromptKind::SaveAsThenOpen { path },
                    "Save as",
                ));
            }
            Err(err) => self.set_status(format!("Save failed: {err}")),
        }
        Ok(())
    }

    fn request_quit(&mut self) -> Result<()> {
        if self.editor.is_dirty() {
            self.mode = Mode::Confirm(ConfirmState::new(
                ConfirmKind::QuitDirty,
                "Unsaved changes. Save before quitting? [s]ave [d]iscard [c]ancel",
            ));
        } else {
            self.should_quit = true;
        }
        Ok(())
    }

    fn handle_file_picker_key(&mut self, key: KeyEvent) -> Result<()> {
        let mut open_path = None;
        let mut close = false;

        if let Mode::FilePicker(state) = &mut self.mode {
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            match key.code {
                KeyCode::Esc => close = true,
                KeyCode::Char('c') if ctrl => close = true,
                KeyCode::Char(ch) if !ctrl => state.input_char(ch),
                KeyCode::Backspace => state.backspace(),
                KeyCode::Up => state.move_up(),
                KeyCode::Down => state.move_down(),
                KeyCode::Enter => open_path = state.selected_path(),
                _ => {}
            }
        }

        if close {
            self.mode = Mode::Editing;
        }

        if let Some(path) = open_path {
            self.request_open_file(path)?;
        }

        Ok(())
    }

    fn handle_prompt_key(&mut self, key: KeyEvent) -> Result<()> {
        let mut submit = false;
        let mut cancel = false;

        if let Mode::Prompt(prompt) = &mut self.mode {
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            match key.code {
                KeyCode::Esc => cancel = true,
                KeyCode::Char('c') if ctrl => cancel = true,
                KeyCode::Enter => submit = true,
                KeyCode::Backspace => {
                    prompt.input.pop();
                }
                KeyCode::Char(ch) if !ctrl => prompt.input.push(ch),
                _ => {}
            }
        }

        if cancel {
            self.mode = Mode::Editing;
            return Ok(());
        }

        if submit {
            let prompt = match std::mem::replace(&mut self.mode, Mode::Editing) {
                Mode::Prompt(prompt) => prompt,
                other => {
                    self.mode = other;
                    return Ok(());
                }
            };
            self.submit_prompt(prompt)?;
        }

        Ok(())
    }

    fn submit_prompt(&mut self, prompt: PromptState) -> Result<()> {
        match prompt.kind {
            PromptKind::SaveAs => {
                self.submit_save_as(&prompt.input)?;
            }
            PromptKind::SaveAsThenQuit => {
                if self.submit_save_as(&prompt.input)? {
                    self.should_quit = true;
                }
            }
            PromptKind::SaveAsThenOpen { path } => {
                if self.submit_save_as(&prompt.input)? {
                    self.open_with_policy_or_prompt(path)?;
                }
            }
            PromptKind::GoToLine => {
                if let Ok(line) = prompt.input.trim().parse::<usize>() {
                    self.editor.go_to_line(line);
                } else {
                    self.set_status("Invalid line number");
                }
            }
            PromptKind::Search => {
                let query = prompt.input;
                if let Some(found) = self.editor.search_next(&query) {
                    self.editor.set_cursor(found.range.start);
                    self.set_status(format!("Found: {query}"));
                } else {
                    self.set_status(format!("Not found: {query}"));
                }
            }
        }
        Ok(())
    }

    fn submit_save_as(&mut self, input: &str) -> Result<bool> {
        let input = input.trim();
        if input.is_empty() {
            self.set_status("Save cancelled: empty path");
            return Ok(false);
        }

        let path = expand_tilde(input);
        match self.editor.save_as(&path) {
            Ok(()) => {
                self.refresh_language();
                self.set_status(format!("Saved {}", path.display()));
                Ok(true)
            }
            Err(err) => {
                self.set_status(format!("Save failed: {err}"));
                Ok(false)
            }
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> Result<()> {
        let confirm = match &self.mode {
            Mode::Confirm(confirm) => confirm.clone(),
            _ => return Ok(()),
        };

        match confirm.kind {
            ConfirmKind::QuitDirty => match key.code {
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.mode = Mode::Editing;
                    self.save_then_quit()?;
                }
                KeyCode::Char('d') | KeyCode::Char('D') => self.should_quit = true,
                KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => self.mode = Mode::Editing,
                _ => {}
            },
            ConfirmKind::OpenDirty { path } => match key.code {
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.mode = Mode::Editing;
                    self.save_then_open(path)?;
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    self.mode = Mode::Editing;
                    self.open_with_policy_or_prompt(path)?;
                }
                KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => self.mode = Mode::Editing,
                _ => {}
            },
            ConfirmKind::OpenLarge { path, .. } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.mode = Mode::Editing;
                    self.open_file_unchecked(&path)?;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => self.mode = Mode::Editing,
                _ => {}
            },
        }

        Ok(())
    }

    fn request_open_file(&mut self, path: PathBuf) -> Result<()> {
        if self.editor.is_dirty() {
            self.mode = Mode::Confirm(ConfirmState::new(
                ConfirmKind::OpenDirty { path },
                "Unsaved changes. Save before opening another file? [s]ave [d]iscard [c]ancel",
            ));
        } else {
            self.open_with_policy_or_prompt(path)?;
        }
        Ok(())
    }

    fn open_with_policy_or_prompt(&mut self, path: PathBuf) -> Result<()> {
        match self.evaluate_open_path(&path)? {
            OpenEvaluation::Open => self.open_file_unchecked(&path)?,
            OpenEvaluation::Prompt(size) => {
                self.mode = Mode::Confirm(ConfirmState::new(
                    ConfirmKind::OpenLarge { path, size },
                    format!(
                        "This file is {}. Lime may be slower with very large files. Open anyway? [y/N]",
                        format_bytes(size)
                    ),
                ));
            }
            OpenEvaluation::Refuse(size) => {
                self.mode = Mode::Editing;
                self.set_status(format!(
                    "Refused to open {} file. Re-run with --force to open it.",
                    format_bytes(size)
                ));
            }
        }
        Ok(())
    }

    fn evaluate_open_path(&self, path: &Path) -> Result<OpenEvaluation> {
        if !path.exists() {
            return Ok(OpenEvaluation::Open);
        }

        match self.file_policy.evaluate_path(path)? {
            FileOpenDecision::Open { .. } => Ok(OpenEvaluation::Open),
            FileOpenDecision::Warn { size } | FileOpenDecision::Confirm { size } => {
                if self.config.confirm_large_files {
                    Ok(OpenEvaluation::Prompt(size))
                } else {
                    Ok(OpenEvaluation::Open)
                }
            }
            FileOpenDecision::Refuse { size } => Ok(OpenEvaluation::Refuse(size)),
        }
    }

    fn open_file_unchecked(&mut self, path: &Path) -> Result<()> {
        let buffer = if path.exists() {
            match TextBuffer::from_file(path) {
                Ok(buffer) => buffer,
                Err(err) => {
                    self.set_status(format!("Could not open {}: {err}", path.display()));
                    return Ok(());
                }
            }
        } else {
            TextBuffer::from_text_with_path("", path)
        };
        self.editor.replace_buffer(buffer);
        self.mode = Mode::Editing;
        self.viewport = Viewport::default();
        self.refresh_language();
        self.highlighter.clear();
        self.refresh_highlights();
        self.set_status(format!("Opened {}", path.display()));
        Ok(())
    }

    fn refresh_language(&mut self) {
        let language = Language::detect_path(self.editor.buffer().path());
        if language != self.language {
            self.language = language;
            self.highlighted_revision = None;
            self.highlighter.clear();
            if language != Language::Markdown {
                self.preview_open = false;
            }
        }
    }

    fn refresh_highlights(&mut self) {
        let revision = self.editor.revision();
        if self.highlighted_revision == Some(revision) && self.highlighted_language == self.language
        {
            return;
        }

        let text = self.editor.text();
        self.highlights = self.highlighter.highlight(&text, self.language, revision);
        self.highlighted_revision = Some(revision);
        self.highlighted_language = self.language;
    }

    fn toggle_preview(&mut self) {
        if self.language != Language::Markdown {
            self.set_status("Preview available for markdown only");
            return;
        }
        self.preview_open = !self.preview_open;
        if self.preview_open {
            self.preview_top_line = 0;
            self.preview_revision = None;
            self.set_status("Preview opened");
        } else {
            self.set_status("Preview closed");
        }
    }

    fn ensure_preview_visible(&mut self, total_preview_lines: usize, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }
        let cursor_line = self.editor.cursor().position.line;
        let margin = 2usize.min(viewport_height / 3);
        if cursor_line < self.preview_top_line + margin {
            self.preview_top_line = cursor_line.saturating_sub(margin);
        } else if cursor_line >= self.preview_top_line + viewport_height.saturating_sub(margin) {
            self.preview_top_line =
                cursor_line.saturating_sub(viewport_height.saturating_sub(margin + 1));
        }
        let max_top = total_preview_lines.saturating_sub(viewport_height);
        self.preview_top_line = self.preview_top_line.min(max_top);
    }

    fn file_label(&self) -> String {
        self.editor
            .buffer()
            .path()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| "[untitled]".to_string())
    }

    fn render_file_picker(&self, frame: &mut Frame<'_>, area: Rect, state: &FilePickerState) {
        let popup = centered_rect(72, 70, area);
        frame.render_widget(Clear, popup);
        let block = Block::default()
            .title(" Open File ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.popup_border))
            .style(self.theme.normal().bg(self.theme.popup_bg));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)])
            .split(inner);

        let search = Paragraph::new(format!("search: {}", state.query))
            .style(self.theme.normal().bg(self.theme.popup_bg));
        frame.render_widget(search, chunks[0]);

        let visible_count = chunks[1].height as usize;
        let selected = state.selected.min(state.matches.len().saturating_sub(1));
        let start = selected.saturating_sub(visible_count.saturating_sub(1));
        let items: Vec<ListItem<'_>> = state
            .matches
            .iter()
            .skip(start)
            .take(visible_count)
            .map(|m| {
                ListItem::new(Line::from(vec![
                    Span::styled(" ", self.theme.normal().bg(self.theme.popup_bg)),
                    Span::raw(m.display.clone()),
                ]))
                .style(self.theme.normal().bg(self.theme.popup_bg))
            })
            .collect();

        let mut list_state = ListState::default();
        if !items.is_empty() {
            list_state.select(Some(selected - start));
        }

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .fg(self.theme.background)
                    .bg(self.theme.heading)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, chunks[1], &mut list_state);
    }

    fn render_prompt(&self, frame: &mut Frame<'_>, area: Rect, state: &PromptState) {
        let popup = centered_rect(60, 20, area);
        frame.render_widget(Clear, popup);
        let block = Block::default()
            .title(format!(" {} ", state.title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.popup_border))
            .style(self.theme.normal().bg(self.theme.popup_bg));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        let label = match state.kind {
            PromptKind::SaveAs | PromptKind::SaveAsThenQuit | PromptKind::SaveAsThenOpen { .. } => {
                "path"
            }
            PromptKind::GoToLine => "line",
            PromptKind::Search => "query",
        };
        let paragraph = Paragraph::new(format!("{label}: {}", state.input))
            .style(self.theme.normal().bg(self.theme.popup_bg));
        frame.render_widget(paragraph, inner);
        let cursor_x = inner
            .x
            .saturating_add(label.len() as u16)
            .saturating_add(2)
            .saturating_add(state.input.chars().count() as u16)
            .min(inner.x.saturating_add(inner.width.saturating_sub(1)));
        frame.set_cursor_position((cursor_x, inner.y));
    }

    fn render_confirm(&self, frame: &mut Frame<'_>, area: Rect, state: &ConfirmState) {
        let popup = centered_rect(64, 24, area);
        frame.render_widget(Clear, popup);
        let block = Block::default()
            .title(" Confirm ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.popup_border))
            .style(self.theme.normal().bg(self.theme.popup_bg));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        let paragraph = Paragraph::new(state.message.clone())
            .style(self.theme.normal().bg(self.theme.popup_bg))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
    }
}

#[derive(Debug, Clone, Copy)]
enum OpenEvaluation {
    Open,
    Prompt(u64),
    Refuse(u64),
}

pub fn run_app(options: AppOptions) -> Result<()> {
    let mut app = App::new(options)?;
    app.run()
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let bytes_f = bytes as f64;
    if bytes_f >= GB {
        format!("{:.1} GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.1} MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.1} KB", bytes_f / KB)
    } else {
        format!("{bytes} B")
    }
}

fn expand_tilde(input: &str) -> PathBuf {
    if input == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(input));
    }
    if let Some(rest) = input.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(input)
}
