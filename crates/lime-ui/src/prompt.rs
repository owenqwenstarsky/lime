use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum PromptKind {
    SaveAs,
    SaveAsThenQuit,
    SaveAsThenOpen { path: PathBuf },
    GoToLine,
    Search,
}

#[derive(Debug, Clone)]
pub struct PromptState {
    pub kind: PromptKind,
    pub title: String,
    pub input: String,
}

impl PromptState {
    pub fn new(kind: PromptKind, title: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            input: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConfirmKind {
    QuitDirty,
    OpenDirty { path: PathBuf },
    OpenLarge { path: PathBuf, size: u64 },
}

#[derive(Debug, Clone)]
pub struct ConfirmState {
    pub kind: ConfirmKind,
    pub message: String,
}

impl ConfirmState {
    pub fn new(kind: ConfirmKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
