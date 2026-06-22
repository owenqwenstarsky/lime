use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub theme: String,
    pub tab_width: usize,
    pub insert_spaces: bool,
    pub show_line_numbers: bool,
    pub confirm_large_files: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "lime-dark".to_string(),
            tab_width: 4,
            insert_spaces: true,
            show_line_numbers: true,
            confirm_large_files: true,
        }
    }
}

impl Config {
    pub fn load(path: Option<&Path>) -> (Self, Option<String>) {
        let Some(path) = path.map(Path::to_path_buf).or_else(default_config_path) else {
            return (Self::default(), None);
        };

        if !path.exists() {
            return (Self::default(), None);
        }

        match fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Self>(&text) {
                Ok(config) => (config.normalized(), None),
                Err(err) => (
                    Self::default(),
                    Some(format!("Invalid config {}: {err}", path.display())),
                ),
            },
            Err(err) => (
                Self::default(),
                Some(format!("Could not read config {}: {err}", path.display())),
            ),
        }
    }

    fn normalized(mut self) -> Self {
        if self.tab_width == 0 || self.tab_width > 16 {
            self.tab_width = 4;
        }
        if self.theme.trim().is_empty() {
            self.theme = "lime-dark".to_string();
        }
        self
    }
}

pub fn default_config_path() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        dirs::home_dir().map(|home| {
            home.join("Library")
                .join("Application Support")
                .join("lime")
                .join("config.toml")
        })
    } else {
        dirs::config_dir().map(|dir| dir.join("lime").join("config.toml"))
    }
}
