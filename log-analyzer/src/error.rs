use std::fmt;
use std::io;

#[derive(Debug)]
pub enum LogError {
    IoError(io::Error),
    InvalidFormat(String),
}

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogError::IoError(e) => write!(f, "I/O error: {}", e),
            LogError::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
        }
    }
}

impl std::error::Error for LogError {}

impl From<io::Error> for LogError {
    fn from(err: io::Error) -> Self {
        LogError::IoError(err)
    }
}
