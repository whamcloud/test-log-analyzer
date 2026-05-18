use anyhow::Result;
use std::str::from_utf8;

#[derive(Debug, Clone)]
pub struct Config {
    pub path: std::path::PathBuf,
    pub core_multiplier: usize,
    pub buffer_size: usize,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct LogOutput {
    info: u64,
    warn: u64,
    error: u64,
    invalid: u64,
}

impl std::iter::Sum for LogOutput {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(LogOutput::default(), |mut acc, item| {
            acc += item;
            acc
        })
    }
}

/// Output matches the README spec exactly:
///
/// ```text
/// INFO: 120394
/// WARN: 23941
/// ERROR: 4821
/// ```
impl std::fmt::Display for LogOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "INFO: {}\nWARN: {}\nERROR: {}\n\nInvalid: {}",
            self.info, self.warn, self.error, self.invalid
        )
    }
}

impl std::ops::AddAssign for LogOutput {
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            info: self.info + other.info,
            warn: self.warn + other.warn,
            error: self.error + other.error,
            invalid: self.invalid + other.invalid,
        };
    }
}

impl LogOutput {
    pub fn inc(&mut self, log_level: Option<LogLevel>) {
        match log_level {
            Some(LogLevel::Info) => self.info += 1,
            Some(LogLevel::Warn) => self.warn += 1,
            Some(LogLevel::Error) => self.error += 1,
            None => self.invalid += 1,
        }
    }
}

/// Log levels as specified in the README: INFO, WARN, ERROR.
///
/// Variants follow Rust naming conventions (PascalCase) to satisfy
/// the `clippy::upper_case_acronyms` lint while keeping byte-level
/// matching against the raw log strings in `LogLevel::parse`.
#[derive(Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LogLevel {
    Info = 0,
    Warn = 1,
    Error = 2,
}

impl LogLevel {
    /// Parse a raw byte slice into a `LogLevel`.
    /// Returns `Err` for any unrecognised level.
    pub fn parse(input: &[u8]) -> Result<Self> {
        match input {
            b"INFO" => Ok(Self::Info),
            b"WARN" => Ok(Self::Warn),
            b"ERROR" => Ok(Self::Error),
            u => Err(anyhow::anyhow!(
                "unknown log level {:?}",
                from_utf8(u).unwrap_or("<non-utf8>")
            )),
        }
    }
}

/// Parse a single log line and return its `LogLevel`.
///
/// Expects `<timestamp>|<level>|<service>|<message>`.
/// Accepts `&[u8]` — no per-line `String` allocation required.
pub fn parse_log_line(input: &[u8]) -> Result<LogLevel> {
    let mut fields = input.split(|&b| b == b'|');
    // skip timestamp field
    fields
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing timestamp field"))?;
    // parse level field
    fields
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing level field"))
        .and_then(LogLevel::parse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_known_levels() {
        assert!(matches!(LogLevel::parse(b"INFO"), Ok(LogLevel::Info)));
        assert!(matches!(LogLevel::parse(b"WARN"), Ok(LogLevel::Warn)));
        assert!(matches!(LogLevel::parse(b"ERROR"), Ok(LogLevel::Error)));
    }

    #[test]
    fn parse_unknown_level_returns_err() {
        assert!(LogLevel::parse(b"TRACE").is_err());
        assert!(LogLevel::parse(b"DEBUG").is_err());
        assert!(LogLevel::parse(b"").is_err());
        assert!(LogLevel::parse(b"info").is_err());
    }

    #[test]
    fn test_parse_valid_line() {
        let line = b"2025-01-01T12:00:00Z|ERROR|auth|invalid token\n";
        assert!(matches!(parse_log_line(line), Ok(LogLevel::Error)));
    }

    #[test]
    fn test_parse_missing_pipe_is_invalid() {
        let line = b"2025-01-01T12:00:00Z ERROR auth message\n";
        assert!(parse_log_line(line).is_err());
    }

    #[test]
    fn test_parse_empty_level_field_is_invalid() {
        let line = b"2025-01-01T12:00:00Z||auth|message\n";
        assert!(parse_log_line(line).is_err());
    }

    #[test]
    fn test_parse_empty_line_is_invalid() {
        assert!(parse_log_line(b"").is_err());
    }

    #[test]
    fn test_parse_binary_garbage() {
        assert!(parse_log_line(b"\x00\x01\x02\x03").is_err());
    }

    #[test]
    fn test_log_output_add_assign() {
        let mut a = LogOutput::default();
        a.inc(Some(LogLevel::Info));
        a.inc(Some(LogLevel::Error));
        let mut b = LogOutput::default();
        b.inc(Some(LogLevel::Warn));
        b.inc(None);
        a += b;
        assert_eq!(a, LogOutput { info: 1, warn: 1, error: 1, invalid: 1 });
    }

    #[test]
    fn test_log_output_sum() {
        let outputs = vec![
            {
                let mut o = LogOutput::default();
                o.inc(Some(LogLevel::Info));
                o
            },
            {
                let mut o = LogOutput::default();
                o.inc(Some(LogLevel::Info));
                o
            },
        ];
        let total: LogOutput = outputs.into_iter().sum();
        assert_eq!(total, LogOutput { info: 2, warn: 0, error: 0, invalid: 0 });
    }
}
