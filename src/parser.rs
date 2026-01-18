//! High-performance log parser with multi-threaded file processing.
//!
//! This module provides efficient parsing of log files by dividing them into chunks
//! and processing them in parallel using multiple threads.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Seek};
use std::ops::AddAssign;

const THREADS_LIMIT: usize = 4;

/// Stores counts for different log levels.
#[derive(Debug, Default, PartialEq)]
pub struct LogCounts {
    pub info: u64,
    pub warn: u64,
    pub error: u64,
    pub malformed: u64,
}

impl AddAssign for LogCounts {
    fn add_assign(&mut self, rhs: Self) {
        self.info += rhs.info;
        self.warn += rhs.warn;
        self.error += rhs.error;
        self.malformed += rhs.malformed;
    }
}

/// Extracts the log level from a log line.
///
/// Expected format: `<timestamp>|<level>|<service>|<message>`
/// Returns the log level as a byte slice, or None if the format is invalid.
fn parse_log_level(line: &[u8]) -> Option<&[u8]> {
    let start = line.iter().position(|&c| c == b'|')? + 1;
    let end = line[start..].iter().position(|&c| c == b'|')?;
    Some(&line[start..start + end])
}

/// Processes a chunk of a file.
///
/// Reads from the start position up to the end position,
/// counting log entries of different levels.
fn process_chunk(path: &str, start: u64, end: u64) -> io::Result<LogCounts> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buf = Vec::new();
    let mut counts = LogCounts::default();

    let mut bytes_read = start;

    // Skip partial first line
    if start != 0 {
        reader.seek(io::SeekFrom::Start(start - 1))?;
        bytes_read += reader.read_until(b'\n', &mut buf)? as u64;
        buf.clear();
    }

    if bytes_read >= end {
        return Ok(counts);
    }

    loop {
        let read = reader.read_until(b'\n', &mut buf)? as u64;
        if read == 0 {
            break;
        }

        match parse_log_level(&buf) {
            Some(b"INFO") => counts.info += 1,
            Some(b"WARN") => counts.warn += 1,
            Some(b"ERROR") => counts.error += 1,
            _ => counts.malformed += 1,
        }

        bytes_read += read;
        buf.clear();

        if bytes_read >= end {
            break;
        }
    }

    Ok(counts)
}

/// Analyzes a log file and counts different log levels.
///
/// For files larger than 1KB, this function uses multiple threads to process
/// different chunks of the file in parallel, significantly improving performance
/// on large files.
///
/// # Arguments
///
/// * `path` - Path to the log file to analyze
///
/// # Returns
///
/// A Result containing LogCounts with the counts for each log level,
/// or an IO error if the file cannot be read.
pub fn analyze_log_file(path: &str) -> io::Result<LogCounts> {
    let file_size = std::fs::metadata(path)?.len();
    let threads = if file_size < 1024 {
        1
    } else {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .min(THREADS_LIMIT)
    };

    let chunk_size = file_size / threads as u64;
    let mut handles = Vec::with_capacity(threads);

    for i in 0..threads {
        let path = path.to_owned();
        let chunk_start = i as u64 * chunk_size;
        let chunk_end = if i + 1 == threads {
            file_size
        } else {
            chunk_start + chunk_size
        };

        handles.push(std::thread::spawn(move || {
            process_chunk(&path, chunk_start, chunk_end)
        }));
    }

    let mut total = LogCounts::default();
    for handle in handles {
        total += handle.join().unwrap()?;
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_log_level() {
        assert_eq!(
            parse_log_level("2025-01-01T12:00:00Z|INFO|auth|valid message".as_bytes()),
            Some("INFO".as_bytes())
        );
        assert_eq!(
            parse_log_level("2025-01-01T12:00:00Z|WARN|api|warning message".as_bytes()),
            Some("WARN".as_bytes())
        );
        assert_eq!(
            parse_log_level("2025-01-01T12:00:00Z|ERROR|db|error message".as_bytes()),
            Some("ERROR".as_bytes())
        );

        assert_eq!(parse_log_level("invalid line".as_bytes()), None);
        assert_eq!(parse_log_level("2025-01-01T12:00:00Z|".as_bytes()), None); // Missing fields
        assert_eq!(
            parse_log_level("2025-01-01T12:00:00Z|INVALID_LEVEL|auth|message".as_bytes()),
            Some("INVALID_LEVEL".as_bytes())
        );
    }

    #[test]
    fn test_analyze_log_file() {
        let mut temp_file = NamedTempFile::new().unwrap();

        writeln!(temp_file, "2025-01-01T12:00:00Z|INFO|auth|valid message").unwrap();
        writeln!(temp_file, "2025-01-01T12:01:00Z|WARN|api|warning message").unwrap();
        writeln!(temp_file, "2025-01-01T12:02:00Z|ERROR|db|error message").unwrap();
        writeln!(temp_file, "invalid line").unwrap(); // malformed

        let counts = analyze_log_file(temp_file.path().to_str().unwrap()).unwrap();

        assert_eq!(counts.info, 1);
        assert_eq!(counts.warn, 1);
        assert_eq!(counts.error, 1);
        assert_eq!(counts.malformed, 1);
    }

    #[test]
    fn test_empty_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let counts = analyze_log_file(temp_file.path().to_str().unwrap()).unwrap();

        assert_eq!(counts, LogCounts::default());
    }

    #[test]
    fn test_chunk_boundary_handling() {
        let mut temp_file = NamedTempFile::new().unwrap();
        for i in 0..500 {
            writeln!(
                temp_file,
                "2025-01-01T12:00:00Z|INFO|service|message{:03}",
                i
            )
            .unwrap();
        }

        let result = analyze_log_file(temp_file.path().to_str().unwrap()).unwrap();
        assert_eq!(result.info, 500);
        assert_eq!(result.warn, 0);
        assert_eq!(result.error, 0);
        assert_eq!(result.malformed, 0);
    }

    #[test]
    fn test_empty_and_whitespace_lines() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "2025-01-01T12:00:00Z|INFO|auth|valid message").unwrap();
        writeln!(temp_file, "").unwrap(); // empty line
        writeln!(temp_file, "   ").unwrap(); // whitespace only
        writeln!(temp_file, "2025-01-01T12:01:00Z|WARN|api|another message").unwrap();

        let result = analyze_log_file(temp_file.path().to_str().unwrap()).unwrap();
        // Empty lines and whitespace-only lines should be considered malformed
        assert_eq!(result.info, 1);
        assert_eq!(result.warn, 1);
        assert_eq!(result.error, 0);
        assert_eq!(result.malformed, 2);
    }
}
