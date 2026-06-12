use crate::format::LogFormat;

/// Log level extracted from a line.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// Outcome of attempting to parse a single line.
#[derive(Debug, PartialEq)]
pub enum ParseOutcome {
    Ok(LogLevel),
    /// Blank line — silently skipped, not counted as malformed.
    Empty,
    /// Line does not conform to the expected format.
    Malformed,
}

/// Parses a single log line from a byte slice, respecting the given format.
///
/// Uses `memchr` for SIMD-accelerated delimiter scanning. No heap allocation.
/// All matching is done on `&[u8]` — no UTF-8 decode required.
pub fn parse_line(line: &[u8], format: LogFormat) -> ParseOutcome {
    // Strip trailing \r for CRLF files.
    let line = match line.last() {
        Some(&b'\r') => &line[..line.len() - 1],
        _ => line,
    };

    if line.is_empty() {
        return ParseOutcome::Empty;
    }

    let delim = format.delimiter;
    let target = format.level_position;

    // Walk to the field at `level_position` using memchr for each delimiter.
    let mut pos = 0usize;
    let mut field_start = 0usize;

    for field_index in 0..=target {
        if field_index == target {
            // We are now at the start of the level field.
            // Find where this field ends (next delimiter or end of line).
            let field_end = memchr::memchr(delim, &line[pos..])
                .map(|i| pos + i)
                .unwrap_or(line.len());

            // There must be at least one more delimiter after the level field
            // (the service field), otherwise the line is malformed.
            // Exception: if level_position is the last field, skip this check.
            let level_bytes = &line[field_start..field_end];

            // Match level — byte comparison, no allocation.
            let level = match level_bytes {
                b"INFO" => LogLevel::Info,
                b"WARN" => LogLevel::Warn,
                b"ERROR" => LogLevel::Error,
                _ => return ParseOutcome::Malformed,
            };

            // Ensure there are still fields after the level (service + message).
            // We need at least 2 more delimiters after the level field start.
            let after_level = &line[field_end..];
            let pipe_after_level = memchr::memchr(delim, after_level);
            let pipe_after_service = pipe_after_level
                .and_then(|i| memchr::memchr(delim, &after_level[i + 1..]));

            if pipe_after_level.is_none() || pipe_after_service.is_none() {
                return ParseOutcome::Malformed;
            }

            return ParseOutcome::Ok(level);
        }

        // Advance to the next delimiter.
        match memchr::memchr(delim, &line[pos..]) {
            Some(offset) => {
                pos += offset + 1;
                field_start = pos;
            }
            None => return ParseOutcome::Malformed,
        }
    }

    ParseOutcome::Malformed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn std() -> LogFormat { LogFormat::standard() }

    #[test]
    fn parses_info() {
        assert_eq!(
            parse_line(b"2025-01-01T12:00:00Z|INFO|auth|user logged in", std()),
            ParseOutcome::Ok(LogLevel::Info)
        );
    }

    #[test]
    fn parses_warn() {
        assert_eq!(
            parse_line(b"2025-01-01T12:00:00Z|WARN|svc|msg", std()),
            ParseOutcome::Ok(LogLevel::Warn)
        );
    }

    #[test]
    fn parses_error() {
        assert_eq!(
            parse_line(b"2025-01-01T12:00:00Z|ERROR|svc|msg", std()),
            ParseOutcome::Ok(LogLevel::Error)
        );
    }

    #[test]
    fn empty_line_is_empty() {
        assert_eq!(parse_line(b"", std()), ParseOutcome::Empty);
    }

    #[test]
    fn crlf_stripped() {
        assert_eq!(
            parse_line(b"2025-01-01T12:00:00Z|WARN|svc|msg\r", std()),
            ParseOutcome::Ok(LogLevel::Warn)
        );
    }

    #[test]
    fn message_with_extra_pipes_is_valid() {
        assert_eq!(
            parse_line(b"2025-01-01T12:00:00Z|INFO|api|key=val|extra=data|more=stuff", std()),
            ParseOutcome::Ok(LogLevel::Info)
        );
    }

    #[test]
    fn missing_message_field_is_malformed() {
        // Only 2 pipes: timestamp|level|service — no message
        assert_eq!(
            parse_line(b"2025-01-01T12:00:00Z|INFO|no-message-field", std()),
            ParseOutcome::Malformed
        );
    }

    #[test]
    fn no_pipes_is_malformed() {
        assert_eq!(parse_line(b"no pipes here at all", std()), ParseOutcome::Malformed);
    }

    #[test]
    fn unknown_level_is_malformed() {
        assert_eq!(
            parse_line(b"2025-01-01T12:00:00Z|DEBUG|svc|msg", std()),
            ParseOutcome::Malformed
        );
    }

    #[test]
    fn lowercase_level_is_malformed() {
        assert_eq!(
            parse_line(b"2025-01-01T12:00:00Z|error|svc|msg", std()),
            ParseOutcome::Malformed
        );
    }

    #[test]
    fn csv_format() {
        assert_eq!(
            parse_line(b"2025-01-01T12:00:00Z,INFO,auth,user logged in", LogFormat::csv()),
            ParseOutcome::Ok(LogLevel::Info)
        );
    }

    #[test]
    fn space_format() {
        assert_eq!(
            parse_line(b"2025-01-01T12:00:00Z INFO auth user logged in", LogFormat::space()),
            ParseOutcome::Ok(LogLevel::Info)
        );
    }

    #[test]
    fn wrong_format_is_malformed() {
        // Pipe-delimited line fed to csv format parser
        assert_eq!(
            parse_line(b"2025-01-01T12:00:00Z|INFO|auth|msg", LogFormat::csv()),
            ParseOutcome::Malformed
        );
    }
}
