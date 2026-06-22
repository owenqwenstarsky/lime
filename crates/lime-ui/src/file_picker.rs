use std::path::{Path, PathBuf};

use ignore::{DirEntry, WalkBuilder};

#[derive(Debug, Clone)]
pub struct FileMatch {
    pub path: PathBuf,
    pub display: String,
    pub score: u32,
}

#[derive(Debug, Clone)]
pub struct FilePickerState {
    pub root: PathBuf,
    pub query: String,
    pub all_files: Vec<PathBuf>,
    pub matches: Vec<FileMatch>,
    pub selected: usize,
}

impl FilePickerState {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let all_files = scan_files(&root);
        let mut state = Self {
            root,
            query: String::new(),
            all_files,
            matches: Vec::new(),
            selected: 0,
        };
        state.update_matches();
        state
    }

    pub fn input_char(&mut self, ch: char) {
        self.query.push(ch);
        self.update_matches();
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.update_matches();
    }

    pub fn move_up(&mut self) {
        if self.matches.is_empty() {
            self.selected = 0;
        } else if self.selected == 0 {
            self.selected = self.matches.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.matches.is_empty() {
            self.selected = 0;
        } else {
            self.selected = (self.selected + 1) % self.matches.len();
        }
    }

    pub fn selected_path(&self) -> Option<PathBuf> {
        self.matches.get(self.selected).map(|m| m.path.clone())
    }

    pub fn update_matches(&mut self) {
        let query = self.query.trim();
        let root = self.root.clone();
        let mut matches: Vec<FileMatch> = self
            .all_files
            .iter()
            .filter_map(|path| {
                let display = path
                    .strip_prefix(&root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();
                fuzzy_score(query, &display).map(|score| FileMatch {
                    path: path.clone(),
                    display,
                    score,
                })
            })
            .collect();

        matches.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.display.len().cmp(&b.display.len()))
                .then_with(|| a.display.cmp(&b.display))
        });
        matches.truncate(500);
        self.matches = matches;
        self.selected = self.selected.min(self.matches.len().saturating_sub(1));
    }
}

pub fn scan_files(root: &Path) -> Vec<PathBuf> {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .filter_entry(|entry| !is_ignored_dir(entry));

    let mut files = Vec::new();
    for entry in builder.build().filter_map(Result::ok) {
        if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();
    files
}

fn is_ignored_dir(entry: &DirEntry) -> bool {
    if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
        return false;
    }

    let Some(name) = entry.file_name().to_str() else {
        return false;
    };

    matches!(
        name,
        ".git" | "target" | "node_modules" | "dist" | ".next" | "build"
    )
}

pub fn fuzzy_score(query: &str, candidate: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(1);
    }

    let query = query.to_lowercase();
    let candidate_lower = candidate.to_lowercase();
    let mut candidate_chars = candidate_lower.chars().enumerate();
    let mut last_match: Option<usize> = None;
    let mut score = 0u32;

    for q in query.chars() {
        let mut found = None;
        for (idx, ch) in candidate_chars.by_ref() {
            if ch == q {
                found = Some(idx);
                break;
            }
        }
        let idx = found?;
        score += 10;
        if let Some(last) = last_match {
            if idx == last + 1 {
                score += 15;
            }
        } else if idx == 0 {
            score += 20;
        }
        if candidate.as_bytes().get(idx) == Some(&b'/') {
            score += 3;
        }
        last_match = Some(idx);
    }

    Some(score.saturating_sub(candidate.len() as u32 / 5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_matches_in_order() {
        assert!(fuzzy_score("lcore", "crates/lime-core/src/lib.rs").is_some());
        assert!(fuzzy_score("zz", "crates/lime-core/src/lib.rs").is_none());
    }
}
