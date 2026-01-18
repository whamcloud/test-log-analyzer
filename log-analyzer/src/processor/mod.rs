use crate::error::LogError;
use crate::log_format::LogFormat;
use crate::log_summary::LogSummary;

pub mod buffered_line_reader;
pub mod parallel_reader;

pub trait LogProcessor {
    fn process(&mut self) -> Result<LogSummary, LogError>;
}

pub fn parse_log_level(line: &str) -> Result<&str, LogError> {
    parse_log_level_with_format(line, LogFormat::standard())
}

pub fn parse_log_level_with_format(line: &str, format: LogFormat) -> Result<&str, LogError> {
    format.parse_level(line)
}
