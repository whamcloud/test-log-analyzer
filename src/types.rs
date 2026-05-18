

#[derive(Debug, Clone)]
pub struct Config {
    pub path: std::path::PathBuf,
    pub core_multiplier: usize,
    pub buffer_size: usize,
    pub enable_error_reporting: bool,
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

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    MissingTimestampField,
    MissingLevelField,
    UnknownLevel,
}

impl ParseError {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParseError::MissingTimestampField => "missing timestamp field",
            ParseError::MissingLevelField => "missing level field",
            ParseError::UnknownLevel => "unknown level",
        }
    }
}

pub struct ErrorMessage {
    pub offset: u64,
    pub msg: &'static str,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn parse(input: &[u8]) -> Result<Self, ParseError> {
        match input {
            b"INFO" => Ok(Self::Info),
            b"WARN" => Ok(Self::Warn),
            b"ERROR" => Ok(Self::Error),
            _ => Err(ParseError::UnknownLevel),
        }
    }
}


/// Format of logline `<timestamp>|<level>|<service>|<message>`.
pub fn parse_log_line(input: &[u8]) -> Result<LogLevel, ParseError> {
    let mut fields = input.split(|&b| b == b'|');
    // skip timestamp field
    fields
        .next()
        .ok_or(ParseError::MissingTimestampField)?;
    // parse level field
    fields
        .next()
        .ok_or(ParseError::MissingLevelField)
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
        assert_eq!(LogLevel::parse(b"TRACE"), Err(ParseError::UnknownLevel));
        assert_eq!(LogLevel::parse(b"DEBUG"), Err(ParseError::UnknownLevel));
        assert_eq!(LogLevel::parse(b""), Err(ParseError::UnknownLevel));
        assert_eq!(LogLevel::parse(b"info"), Err(ParseError::UnknownLevel));
    }

    #[test]
    fn test_parse_valid_line() {
        let line = b"2025-01-01T12:00:00Z|ERROR|auth|invalid token\n";
        assert_eq!(parse_log_line(line), Ok(LogLevel::Error));
    }

    #[test]
    fn test_parse_missing_pipe_is_invalid() {
        let line = b"2025-01-01T12:00:00Z ERROR auth message\n";
        assert_eq!(parse_log_line(line), Err(ParseError::MissingLevelField));
    }

    #[test]
    fn test_parse_empty_level_field_is_invalid() {
        let line = b"2025-01-01T12:00:00Z||auth|message\n";
        assert_eq!(parse_log_line(line), Err(ParseError::UnknownLevel));
    }

    #[test]
    fn test_parse_empty_line_is_invalid() {
        assert_eq!(parse_log_line(b""), Err(ParseError::MissingLevelField));
    }

    #[test]
    fn test_parse_binary_garbage() {
        assert_eq!(parse_log_line(b"\x00\x01\x02\x03"), Err(ParseError::MissingLevelField));
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
