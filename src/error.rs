use thiserror::Error;

/// Top-level error from the analysis pipeline.
#[derive(Debug, Error)]
pub enum AnalyzerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Why a single log line could not be parsed.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    EmptyLine,
    LineTooLong,
    InvalidFormat,
    InvalidLevel,
}
