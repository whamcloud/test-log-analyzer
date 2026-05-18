

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
    InvalidTimestamp,
    InvalidService,
}

impl ParseError {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParseError::MissingTimestampField => "missing timestamp field",
            ParseError::MissingLevelField => "missing level field",
            ParseError::UnknownLevel => "unknown level",
            ParseError::InvalidTimestamp => "invalid timestamp",
            ParseError::InvalidService => "invalid service name",
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

#[derive(Debug, PartialEq, Eq)]
pub struct LogLine<'a> {
    pub timestamp: &'a [u8],
    pub level: LogLevel,
    pub service: &'a [u8],
    pub message: &'a [u8],
}

impl<'a> LogLine<'a> {
    /// Expected format: `<timestamp>|<level>|<service>|<message>[\n]`
    pub fn parse(input: &'a [u8]) -> Result<Self, ParseError> {
        // timestamp bytes
        let p1 = memchr::memchr(b'|', input).ok_or(ParseError::MissingLevelField)?;
        let timestamp = &input[..p1];
        let rest = &input[p1 + 1..];

        // log level bytes
        let p2 = memchr::memchr(b'|', rest).ok_or(ParseError::MissingLevelField)?;
        let level_bytes = &rest[..p2];
        let rest = &rest[p2 + 1..];

        // service bytes
        let p3 = memchr::memchr(b'|', rest).ok_or(ParseError::MissingLevelField)?;
        let service = &rest[..p3];
        let message_raw = &rest[p3 + 1..];

        validate_timestamp(timestamp)?;
        let level = parse_level(level_bytes)?;
        validate_service(service)?;
        let message = strip_newline(message_raw);

        Ok(LogLine {
            timestamp,
            level,
            service,
            message,
        })
    }
}

#[inline(always)]
fn parse_level(b: &[u8]) -> Result<LogLevel, ParseError> {
    match b {
            b"INFO" => Ok(LogLevel::Info),
            b"WARN" => Ok(LogLevel::Warn),
            b"ERROR" => Ok(LogLevel::Error),
            _ => Err(ParseError::UnknownLevel),
        }
}

/// Validate bytes are an ISO-8601 UTC timestamp: `YYYY-MM-DDTHH:MM:SSZ`.
/// checking digits and special characters are in specific position
#[inline(always)]
fn validate_timestamp(b: &[u8]) -> Result<(), ParseError> {
    if b.len() != 20 {
        return Err(ParseError::InvalidTimestamp);
    }

    let seps_ok = b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'Z';
    // Digit positions: 0-3 (year), 5-6 (month), 8-9 (day),
    //                  11-12 (hour), 14-15 (min), 17-18 (sec)
    const DIGIT_IDX: [usize; 14] = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    let digits_ok = DIGIT_IDX.iter().all(|&i| b[i].is_ascii_digit());
    if seps_ok && digits_ok {
        Ok(())
    } else {
        Err(ParseError::InvalidTimestamp)
    }
}

/// Validate bytes are non-empty ASCII alphanumeric
#[inline(always)]
fn validate_service(b: &[u8]) -> Result<(), ParseError> {
    if b.is_empty() || !b.iter().all(u8::is_ascii_alphanumeric) {
        Err(ParseError::InvalidService)
    } else {
        Ok(())
    }
}

/// Strip trailing `\n
#[inline(always)]
fn strip_newline(b: &[u8]) -> &[u8] {
    match b {
        [rest @ .., b'\n'] => rest,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::{LogLevel, LogLine, ParseError};

    #[test]
    fn valid_error_line_all_fields() {
        let line = b"2025-01-01T12:00:00Z|ERROR|auth|invalid token\n";
        let ll = LogLine::parse(line).unwrap();
        assert_eq!(ll.timestamp, b"2025-01-01T12:00:00Z");
        assert_eq!(ll.level, LogLevel::Error);
        assert_eq!(ll.service, b"auth");
        assert_eq!(ll.message, b"invalid token"); // \n stripped
    }

    #[test]
    fn valid_info_line_no_newline() {
        let line = b"2025-01-01T12:00:00Z|INFO|gateway|request received";
        let ll = LogLine::parse(line).unwrap();
        assert_eq!(ll.level, LogLevel::Info);
        assert_eq!(ll.service, b"gateway");
        assert_eq!(ll.message, b"request received");
    }

    #[test]
    fn empty_input_returns_missing_level() {
        assert_eq!(LogLine::parse(b""), Err(ParseError::MissingLevelField));
    }

    #[test]
    fn no_pipe_returns_missing_level() {
        assert_eq!(
            LogLine::parse(b"2025-01-01T12:00:00Z ERROR auth message\n"),
            Err(ParseError::MissingLevelField),
        );
    }

    #[test]
    fn only_one_pipe_returns_missing_level() {
        assert_eq!(
            LogLine::parse(b"2025-01-01T12:00:00Z|INFO"),
            Err(ParseError::MissingLevelField),
        );
    }


    #[test]
    fn empty_level_field_returns_unknown() {
        assert_eq!(
            LogLine::parse(b"2025-01-01T12:00:00Z||auth|message\n"),
            Err(ParseError::UnknownLevel),
        );
    }

    #[test]
    fn lowercase_level_returns_unknown() {
        assert_eq!(
            LogLine::parse(b"2025-01-01T12:00:00Z|info|svc|msg\n"),
            Err(ParseError::UnknownLevel),
        );
    }

    #[test]
    fn valid_timestamp_utc_z() {
        let line = b"2025-01-01T12:00:00Z|INFO|auth|msg\n";
        assert!(LogLine::parse(line).is_ok());
    }

    #[test]
    fn invalid_timestamp_space_instead_of_t() {
        let line = b"2025-01-01 12:00:00Z|INFO|auth|msg\n";
        assert_eq!(LogLine::parse(line), Err(ParseError::InvalidTimestamp));
    }

    #[test]
    fn invalid_timestamp_too_short() {
        let line = b"2025-01-01|INFO|auth|msg\n";
        assert_eq!(LogLine::parse(line), Err(ParseError::InvalidTimestamp));
    }

    #[test]
    fn valid_service_alpha_only() {
        let line = b"2025-01-01T12:00:00Z|INFO|auth|msg\n";
        assert!(LogLine::parse(line).is_ok());
    }

    #[test]
    fn valid_service_alphanumeric_mixed() {
        let line = b"2025-01-01T12:00:00Z|INFO|svc42|msg\n";
        assert!(LogLine::parse(line).is_ok());
    }

    #[test]
    fn invalid_service_empty() {
        let line = b"2025-01-02T15:30:22Z|INFO||empty module section\n";
        assert_eq!(LogLine::parse(line), Err(ParseError::InvalidService));
    }

    #[test]
    fn invalid_service_contains_hyphen() {
        let line = b"2025-01-01T12:00:00Z|INFO|auth-svc|msg\n";
        assert_eq!(LogLine::parse(line), Err(ParseError::InvalidService));
    }
}
