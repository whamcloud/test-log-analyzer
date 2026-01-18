use crate::error::LogError;
use crate::log_format::LogFormat;
use crate::log_summary::LogSummary;
use crate::processor::{LogProcessor, parse_log_level_with_format};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub struct BufferedLineReader {
    path: std::path::PathBuf,
    buffer: usize,
    format: LogFormat,
}

impl BufferedLineReader {
    pub fn new<P: AsRef<Path>>(path: P, buffer: usize) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            buffer,
            format: LogFormat::standard(),
        }
    }

    pub fn with_format<P: AsRef<Path>>(path: P, buffer: usize, format: LogFormat) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            buffer,
            format,
        }
    }

    pub fn process_lines<R: BufRead>(&self, mut reader: R) -> Result<LogSummary, LogError> {
        let mut summary = LogSummary::default();
        let mut line = String::with_capacity(self.buffer); // Single allocation
        let mut line_num = 0;

        loop {
            line.clear();

            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    line_num += 1;

                    let trimmed = line.trim();

                    // parse_log_level_with_format returns &str slice - no allocation!
                    match parse_log_level_with_format(trimmed, self.format) {
                        Ok("INFO") => summary.info += 1,
                        Ok("WARN") => summary.warn += 1,
                        Ok("ERROR") => summary.error += 1,
                        _ => summary.malformed += 1,
                    }
                }
                Err(e) => {
                    eprintln!("Error reading line {}: {}", line_num, e);
                    summary.malformed += 1;
                }
            }
        }

        Ok(summary)
    }
}

impl LogProcessor for BufferedLineReader {
    fn process(&mut self) -> Result<LogSummary, LogError> {
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        self.process_lines(reader)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // Helper function to create a reader from string
    fn create_test_reader(format: LogFormat) -> BufferedLineReader {
        BufferedLineReader {
            path: std::path::PathBuf::from("/dummy/path"),
            buffer: 256,
            format,
        }
    }

    #[test]
    fn test_process_standard_format() {
        let content = "\
                            2025-01-01T00:00:00Z|INFO|auth|request completed
                            2025-01-01T00:00:01Z|INFO|api|request completed
                            2025-01-01T00:00:02Z|WARN|db|connection timeout
                            2025-01-01T00:00:03Z|ERROR|cache|connection failed
                            2025-01-01T00:00:04Z|INFO|worker|request completed";

        let reader = create_test_reader(LogFormat::standard());
        let cursor = Cursor::new(content);
        let summary = reader.process_lines(cursor).unwrap();

        assert_eq!(summary.info, 3);
        assert_eq!(summary.warn, 1);
        assert_eq!(summary.error, 1);
        assert_eq!(summary.malformed, 0);
        assert_eq!(summary.total(), 5);
    }

    #[test]
    fn test_process_csv_format() {
        let content = "\
                                2025-01-01T00:00:00Z,INFO,auth,request completed
                                2025-01-01T00:00:01Z,INFO,api,request completed
                                2025-01-01T00:00:02Z,INFO,db,request completed
                                2025-01-01T00:00:03Z,INFO,cache,request completed
                                2025-01-01T00:00:04Z,INFO,worker,request completed
                                2025-01-01T00:00:05Z,INFO,auth,connection timeout
                                2025-01-01T00:00:06Z,INFO,api,connection timeout
                                2025-01-01T00:00:07Z,WARN,db,connection timeout";

        let format = LogFormat::custom(',', 1);
        let reader = create_test_reader(format);
        let cursor = Cursor::new(content);
        let summary = reader.process_lines(cursor).unwrap();

        assert_eq!(summary.info, 7);
        assert_eq!(summary.warn, 1);
        assert_eq!(summary.error, 0);
        assert_eq!(summary.malformed, 0);
        assert_eq!(summary.total(), 8);
    }

    #[test]
    fn test_process_space_delimited() {
        let content = "\
                            2025-01-01T00:00:00Z INFO auth request completed
                            2025-01-01T00:00:01Z WARN api connection timeout
                            2025-01-01T00:00:02Z ERROR db connection failed";

        let reader = create_test_reader(LogFormat::space_delimited());
        let cursor = Cursor::new(content);
        let summary = reader.process_lines(cursor).unwrap();

        assert_eq!(summary.info, 1);
        assert_eq!(summary.warn, 1);
        assert_eq!(summary.error, 1);
        assert_eq!(summary.malformed, 0);
        assert_eq!(summary.total(), 3);
    }

    #[test]
    fn test_process_with_malformed_lines() {
        let content = "\
                            2025-01-01T00:00:00Z|INFO|auth|request completed
                            invalid line without pipes
                            2025-01-01T00:00:01Z|WARN|api|connection timeout
                            another invalid line
                            |missing|fields
                            2025-01-01T00:00:02Z|ERROR|db|connection failed";

        let reader = create_test_reader(LogFormat::standard());
        let cursor = Cursor::new(content);
        let summary = reader.process_lines(cursor).unwrap();

        assert_eq!(summary.info, 1);
        assert_eq!(summary.warn, 1);
        assert_eq!(summary.error, 1);
        assert_eq!(summary.malformed, 3); // 3 invalid lines
        assert_eq!(summary.total(), 6);
    }
    #[test]
    fn test_process_with_wrong_format() {
        let content = "\
                            2025-01-01T00:00:00Z|INFO|auth|request completed
                            invalid line without pipes
                            2025-01-01T00:00:01Z|WARN|api|connection timeout
                            another invalid line
                            |missing|fields
                            2025-01-01T00:00:02Z|ERROR|db|connection failed";

        let reader = create_test_reader(LogFormat::csv_delimited());
        let cursor = Cursor::new(content);
        let summary = reader.process_lines(cursor).unwrap();

        assert_eq!(summary.info, 0);
        assert_eq!(summary.warn, 0);
        assert_eq!(summary.error, 0);
        assert_eq!(summary.malformed, 6);
        assert_eq!(summary.total(), 6);
    }

    #[test]
    fn test_process_empty_input() {
        let content = "";

        let reader = create_test_reader(LogFormat::standard());
        let cursor = Cursor::new(content);
        let summary = reader.process_lines(cursor).unwrap();

        assert_eq!(summary.info, 0);
        assert_eq!(summary.warn, 0);
        assert_eq!(summary.error, 0);
        assert_eq!(summary.malformed, 0);
        assert_eq!(summary.total(), 0);
    }

    #[test]
    fn test_process_only_malformed() {
        let content = "\
                            invalid line 1
                            invalid line 2
                            no pipes here
                            also bad";

        let reader = create_test_reader(LogFormat::standard());
        let cursor = Cursor::new(content);
        let summary = reader.process_lines(cursor).unwrap();

        assert_eq!(summary.info, 0);
        assert_eq!(summary.warn, 0);
        assert_eq!(summary.error, 0);
        assert_eq!(summary.malformed, 4);
        assert_eq!(summary.total(), 4);
    }

    #[test]
    fn test_process_unknown_log_level() {
        let content = "\
                            2025-01-01T00:00:00Z|INFO|auth|request completed
                            2025-01-01T00:00:01Z|DEBUG|api|debug message
                            2025-01-01T00:00:02Z|TRACE|db|trace message
                            2025-01-01T00:00:03Z|WARN|cache|warning";

        let reader = create_test_reader(LogFormat::standard());
        let cursor = Cursor::new(content);
        let summary = reader.process_lines(cursor).unwrap();

        assert_eq!(summary.info, 1);
        assert_eq!(summary.warn, 1);
        assert_eq!(summary.error, 0);
        assert_eq!(summary.malformed, 2); // DEBUG and TRACE are unknown
        assert_eq!(summary.total(), 4);
    }

    #[test]
    fn test_process_with_whitespace() {
        let content = "\
                            2025-01-01T00:00:00Z|INFO|auth|request completed

                            2025-01-01T00:00:01Z|WARN|api|connection timeout
                            2025-01-01T00:00:02Z|ERROR|db|connection failed
                            ";

        let reader = create_test_reader(LogFormat::standard());
        let cursor = Cursor::new(content);
        let summary = reader.process_lines(cursor).unwrap();

        assert_eq!(summary.info, 1);
        assert_eq!(summary.warn, 1);
        assert_eq!(summary.error, 1);
        // Empty lines are malformed
        assert_eq!(summary.malformed, 2);
        assert_eq!(summary.total(), 5);
    }

    #[test]
    fn test_process_custom_delimiter_semicolon() {
        let content = "\
                            2025-01-01T00:00:00Z;INFO;auth;request completed
                            2025-01-01T00:00:01Z;WARN;api;connection timeout
                            2025-01-01T00:00:02Z;ERROR;db;connection failed";

        let format = LogFormat::custom(';', 1);
        let reader = create_test_reader(format);
        let cursor = Cursor::new(content);
        let summary = reader.process_lines(cursor).unwrap();

        assert_eq!(summary.info, 1);
        assert_eq!(summary.warn, 1);
        assert_eq!(summary.error, 1);
        assert_eq!(summary.malformed, 0);
        assert_eq!(summary.total(), 3);
    }

    #[test]
    fn test_process_custom_level_position() {
        // Format: timestamp,service,LEVEL,message
        let content = "\
                            2025-01-01T00:00:00Z,auth,INFO,request completed
                            2025-01-01T00:00:01Z,api,WARN,connection timeout
                            2025-01-01T00:00:02Z,db,ERROR,connection failed";

        let format = LogFormat::custom(',', 2); // Level at position 2
        let reader = create_test_reader(format);
        let cursor = Cursor::new(content);
        let summary = reader.process_lines(cursor).unwrap();

        assert_eq!(summary.info, 1);
        assert_eq!(summary.warn, 1);
        assert_eq!(summary.error, 1);
        assert_eq!(summary.malformed, 0);
        assert_eq!(summary.total(), 3);
    }

    #[test]
    fn test_deterministic_ratio_7_2_1() {
        // Simulate deterministic generation with 7:2:1 ratio
        let mut content = String::new();
        let pattern = vec![
            "INFO", "INFO", "INFO", "INFO", "INFO", "INFO", "INFO", "WARN", "WARN", "ERROR",
        ];

        for (i, level) in pattern.iter().cycle().take(100).enumerate() {
            content.push_str(&format!(
                "2025-01-01T00:00:{:02}Z|{}|auth|message {}\n",
                i % 60,
                level,
                i
            ));
        }

        let reader = create_test_reader(LogFormat::standard());
        let cursor = Cursor::new(content.as_bytes());
        let summary = reader.process_lines(cursor).unwrap();

        assert_eq!(summary.info, 70); // 7/10 * 100
        assert_eq!(summary.warn, 20); // 2/10 * 100
        assert_eq!(summary.error, 10); // 1/10 * 100
        assert_eq!(summary.total(), 100);
    }
}
