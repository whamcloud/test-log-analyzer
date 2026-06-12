use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnalyzerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    EmptyLine,
    LineTooLong,
    InvalidFormat,
    InvalidLevel,
}
