use std::io;

#[derive(thiserror::Error, Debug)]
pub enum LimeError {
    #[error("file is too large: {size} bytes")]
    FileTooLarge { size: u64 },

    #[error("buffer has no file path")]
    MissingPath,

    #[error("invalid cursor position")]
    InvalidPosition,

    #[error("invalid text range")]
    InvalidRange,

    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, LimeError>;
