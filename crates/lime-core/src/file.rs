use std::fs;
use std::path::Path;

use crate::error::Result;

#[derive(Debug, Clone, Copy)]
pub struct FileOpenPolicy {
    pub warn_threshold_bytes: u64,
    pub confirm_threshold_bytes: u64,
    pub hard_threshold_bytes: u64,
    pub force: bool,
}

impl Default for FileOpenPolicy {
    fn default() -> Self {
        Self {
            warn_threshold_bytes: 5 * 1024 * 1024,
            confirm_threshold_bytes: 25 * 1024 * 1024,
            hard_threshold_bytes: 100 * 1024 * 1024,
            force: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOpenDecision {
    Open { size: u64 },
    Warn { size: u64 },
    Confirm { size: u64 },
    Refuse { size: u64 },
}

impl FileOpenPolicy {
    pub fn evaluate_size(&self, size: u64) -> FileOpenDecision {
        if self.force {
            return FileOpenDecision::Open { size };
        }

        if size > self.hard_threshold_bytes {
            FileOpenDecision::Refuse { size }
        } else if size > self.confirm_threshold_bytes {
            FileOpenDecision::Confirm { size }
        } else if size > self.warn_threshold_bytes {
            FileOpenDecision::Warn { size }
        } else {
            FileOpenDecision::Open { size }
        }
    }

    pub fn evaluate_path(&self, path: &Path) -> Result<FileOpenDecision> {
        let metadata = fs::metadata(path)?;
        Ok(self.evaluate_size(metadata.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_thresholds() {
        let policy = FileOpenPolicy::default();
        assert_eq!(
            policy.evaluate_size(1024),
            FileOpenDecision::Open { size: 1024 }
        );
        assert!(matches!(
            policy.evaluate_size(6 * 1024 * 1024),
            FileOpenDecision::Warn { .. }
        ));
        assert!(matches!(
            policy.evaluate_size(30 * 1024 * 1024),
            FileOpenDecision::Confirm { .. }
        ));
        assert!(matches!(
            policy.evaluate_size(101 * 1024 * 1024),
            FileOpenDecision::Refuse { .. }
        ));
    }

    #[test]
    fn force_opens_any_size() {
        let policy = FileOpenPolicy {
            force: true,
            ..FileOpenPolicy::default()
        };
        assert_eq!(
            policy.evaluate_size(500 * 1024 * 1024),
            FileOpenDecision::Open {
                size: 500 * 1024 * 1024
            }
        );
    }
}
