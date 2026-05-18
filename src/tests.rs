#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use crate::analyzer::LogAnalyzer;
    use crate::cli::Config;
    use crate::error::ParseError;
    use crate::parser::parse_log_level;
    use crate::types::LogLevel;

    // -- parser --

    #[test]
    fn parse_info_level() {
        assert_eq!(
            parse_log_level("2025-01-01T12:00:00Z|INFO|auth|ok").unwrap(),
            LogLevel::Info
        );
    }

    #[test]
    fn parse_warn_level() {
        assert_eq!(
            parse_log_level("2025-01-01T12:00:00Z|WARN|auth|slow").unwrap(),
            LogLevel::Warn
        );
    }

    #[test]
    fn parse_error_level() {
        assert_eq!(
            parse_log_level("2025-01-01T12:00:00Z|ERROR|auth|fail").unwrap(),
            LogLevel::Error
        );
    }

    #[test]
    fn parse_rejects_unknown_level() {
        assert!(matches!(
            parse_log_level("2025-01-01T12:00:00Z|CRITICAL|auth|x"),
            Err(ParseError::InvalidLevel)
        ));
    }

    #[test]
    fn parse_rejects_no_pipes() {
        assert!(matches!(
            parse_log_level("just-a-string"),
            Err(ParseError::InvalidFormat)
        ));
    }

    #[test]
    fn parse_rejects_empty() {
        assert!(matches!(parse_log_level(""), Err(ParseError::EmptyLine)));
    }

    #[test]
    fn parse_rejects_oversized_line() {
        let line = format!("ts|INFO|svc|{}", "x".repeat(64 * 1024 + 1));
        assert!(matches!(
            parse_log_level(&line),
            Err(ParseError::LineTooLong)
        ));
    }

    // -- analyzer --

    fn analyze(lines: &[&str]) -> crate::types::AnalysisReport {
        let mut file = NamedTempFile::new().unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
        LogAnalyzer::run(&Config {
            file_path: file.path().to_path_buf(),
            chunk_size: 8 * 1024 * 1024,
            read_buffer_size: 64 * 1024,
        })
        .unwrap()
    }

    #[test]
    fn counts_all_levels() {
        let r = analyze(&[
            "2025-01-01T12:00:00Z|INFO|a|ok",
            "2025-01-01T12:00:00Z|WARN|a|slow",
            "2025-01-01T12:00:00Z|ERROR|a|fail",
            "garbage",
        ]);
        assert_eq!(
            (
                r.counters.info,
                r.counters.warn,
                r.counters.error,
                r.counters.malformed
            ),
            (1, 1, 1, 1)
        );
    }

    #[test]
    fn empty_file() {
        let r = analyze(&[]);
        assert_eq!(
            (
                r.counters.info,
                r.counters.warn,
                r.counters.error,
                r.counters.malformed
            ),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn all_malformed() {
        let r = analyze(&["bad", "also bad", "nope"]);
        assert_eq!(r.counters.info + r.counters.warn + r.counters.error, 0);
        assert_eq!(r.counters.malformed, 3);
    }

    #[test]
    fn empty_lines_not_malformed() {
        let r = analyze(&[
            "2025-01-01T12:00:00Z|ERROR|a|x",
            "",
            "2025-01-01T12:00:00Z|WARN|a|y",
        ]);
        assert_eq!(r.counters.malformed, 0);
        assert_eq!(r.counters.error, 1);
        assert_eq!(r.counters.warn, 1);
    }
}
