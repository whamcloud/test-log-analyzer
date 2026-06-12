use crate::error::ParseError;
use crate::types::LogLevel;
use memchr::memchr;

/// Extracts the log level from a single line.
///
/// # Format
/// `<timestamp>|<level>|<service>|<message>`
///
/// Uses `memchr` for zero-copy SIMD byte search — no allocation on the happy path.
#[inline]
pub fn parse_log_level(line: &str) -> Result<LogLevel, ParseError> {
    if line.is_empty() {
        return Err(ParseError::EmptyLine);
    }

    if line.len() > 64 * 1024 {
        return Err(ParseError::LineTooLong);
    }

    let bytes = line.as_bytes();

    let first = memchr(b'|', bytes).ok_or(ParseError::InvalidFormat)?;
    let second_rel = memchr(b'|', &bytes[first + 1..]).ok_or(ParseError::InvalidFormat)?;
    let second = first + 1 + second_rel;

    match &line[first + 1..second] {
        "INFO" => Ok(LogLevel::Info),
        "WARN" => Ok(LogLevel::Warn),
        "ERROR" => Ok(LogLevel::Error),
        _ => Err(ParseError::InvalidLevel),
    }
}
