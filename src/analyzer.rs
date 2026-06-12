use std::ops::Add;

use rayon::prelude::*;

use crate::format::LogFormat;
use crate::parser::{parse_line, LogLevel, ParseOutcome};

/// Aggregated counts produced after analyzing a log buffer.
///
/// `impl Add` lets rayon's `reduce()` merge per-worker counts with zero
/// shared state during the parallel phase — no atomics, no locks.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LogCounts {
    pub info: u64,
    pub warn: u64,
    pub error: u64,
    pub malformed: u64,
}

impl LogCounts {
    pub fn total_valid(&self) -> u64 {
        self.info + self.warn + self.error
    }

    pub fn total(&self) -> u64 {
        self.info + self.warn + self.error + self.malformed
    }

    /// Pretty-print summary box.
    pub fn display(&self) {
        println!("================================");
        println!("|   Log Analysis Summary        |");
        println!("|==============================|");
        println!("| INFO:      {:>18} |", self.info);
        println!("| WARN:      {:>18} |", self.warn);
        println!("| ERROR:     {:>18} |", self.error);
        if self.malformed > 0 {
            println!("| MALFORMED: {:>18} |", self.malformed);
        }
        println!("|==============================|");
        println!("| TOTAL:     {:>18} |", self.total());
        println!("================================");
    }
}

/// Merge two `LogCounts` — called O(thread_count) times by rayon reduce.
impl Add for LogCounts {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            info:      self.info      + rhs.info,
            warn:      self.warn      + rhs.warn,
            error:     self.error     + rhs.error,
            malformed: self.malformed + rhs.malformed,
        }
    }
}

/// Processes one newline-aligned chunk, returning a thread-local `LogCounts`.
///
/// Operates entirely on `&[u8]` — no heap allocation per line.
fn process_chunk(chunk: &[u8], format: LogFormat) -> LogCounts {
    let mut counts = LogCounts::default();
    let mut pos = 0;

    while pos < chunk.len() {
        let end = match memchr::memchr(b'\n', &chunk[pos..]) {
            Some(offset) => pos + offset,
            None => chunk.len(),
        };

        match parse_line(&chunk[pos..end], format) {
            ParseOutcome::Ok(LogLevel::Info)  => counts.info      += 1,
            ParseOutcome::Ok(LogLevel::Warn)  => counts.warn      += 1,
            ParseOutcome::Ok(LogLevel::Error) => counts.error     += 1,
            ParseOutcome::Malformed           => counts.malformed += 1,
            ParseOutcome::Empty               => {}
        }

        pos = if end < chunk.len() { end + 1 } else { chunk.len() };
    }

    counts
}

/// Splits `data` into at most `n` chunks aligned to newline boundaries.
///
/// Each boundary is found by scanning *forward* from the ideal split point,
/// so no line is ever divided between two workers. Total extra scan is O(N/P).
fn split_into_chunks(data: &[u8], n: usize) -> Vec<&[u8]> {
    if data.is_empty() || n == 0 {
        return vec![];
    }

    let chunk_size = (data.len() / n).max(1);
    let mut chunks = Vec::with_capacity(n);
    let mut start = 0;

    while start < data.len() {
        let ideal_end = (start + chunk_size).min(data.len());

        let end = if ideal_end == data.len() {
            data.len()
        } else {
            match memchr::memchr(b'\n', &data[ideal_end..]) {
                Some(offset) => ideal_end + offset + 1,
                None => data.len(),
            }
        };

        chunks.push(&data[start..end]);
        start = end;
    }

    chunks
}

/// Analyzes a byte buffer (typically a memory-mapped file) and returns
/// aggregated log-level counts.
///
/// The buffer is split into `rayon::current_num_threads()` newline-aligned
/// chunks. Each worker builds a fully thread-local `LogCounts`; they are
/// merged via `reduce()` at the end — zero contention, zero atomics.
pub fn analyze(data: &[u8], format: LogFormat) -> LogCounts {
    if data.is_empty() {
        return LogCounts::default();
    }

    let n = rayon::current_num_threads();
    let chunks = split_into_chunks(data, n);

    chunks
        .into_par_iter()
        .map(|chunk| process_chunk(chunk, format))
        .reduce(LogCounts::default, LogCounts::add)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::LogFormat;

    fn make_log(entries: &[(&str, &str)]) -> Vec<u8> {
        entries
            .iter()
            .map(|(ts, lvl)| format!("{}|{}|svc|msg\n", ts, lvl))
            .collect::<String>()
            .into_bytes()
    }

    #[test]
    fn counts_all_levels() {
        let data = make_log(&[
            ("2025-01-01T00:00:00Z", "INFO"),
            ("2025-01-01T00:00:01Z", "INFO"),
            ("2025-01-01T00:00:02Z", "WARN"),
            ("2025-01-01T00:00:03Z", "ERROR"),
        ]);
        let c = analyze(&data, LogFormat::standard());
        assert_eq!(c.info, 2);
        assert_eq!(c.warn, 1);
        assert_eq!(c.error, 1);
        assert_eq!(c.malformed, 0);
    }

    #[test]
    fn counts_malformed() {
        let data = b"not valid\n2025-01-01T00:00:00Z|INFO|svc|msg\ngarbage\n";
        let c = analyze(data, LogFormat::standard());
        assert_eq!(c.info, 1);
        assert_eq!(c.malformed, 2);
    }

    #[test]
    fn empty_buffer() {
        assert_eq!(analyze(b"", LogFormat::standard()), LogCounts::default());
    }

    #[test]
    fn no_trailing_newline_counted() {
        let c = analyze(b"2025-01-01T00:00:00Z|ERROR|svc|msg", LogFormat::standard());
        assert_eq!(c.error, 1);
    }

    #[test]
    fn empty_lines_not_malformed() {
        let data = b"2025-01-01T00:00:00Z|INFO|svc|msg\n\n2025-01-01T00:00:01Z|WARN|svc|msg\n";
        let c = analyze(data, LogFormat::standard());
        assert_eq!(c.info, 1);
        assert_eq!(c.warn, 1);
        assert_eq!(c.malformed, 0);
    }

    #[test]
    fn csv_format_works() {
        let data = b"2025-01-01T00:00:00Z,INFO,auth,msg\n2025-01-01T00:00:01Z,ERROR,svc,msg\n";
        let c = analyze(data, LogFormat::csv());
        assert_eq!(c.info, 1);
        assert_eq!(c.error, 1);
    }

    #[test]
    fn space_format_works() {
        let data = b"2025-01-01T00:00:00Z WARN auth msg\n";
        let c = analyze(data, LogFormat::space());
        assert_eq!(c.warn, 1);
    }

    #[test]
    fn large_parallel_exact() {
        let line = b"2025-01-01T00:00:00Z|INFO|svc|msg\n";
        let n = 100_000usize;
        let data: Vec<u8> = line.iter().cycle().take(line.len() * n).cloned().collect();
        let c = analyze(&data, LogFormat::standard());
        assert_eq!(c.info, n as u64);
        assert_eq!(c.malformed, 0);
    }

    #[test]
    fn chunks_cover_all_bytes() {
        let data = b"line1\nline2\nline3\nline4\nline5\n";
        let chunks = split_into_chunks(data, 3);
        let rebuilt: Vec<u8> = chunks.iter().flat_map(|c| c.iter().copied()).collect();
        assert_eq!(rebuilt, data.as_ref());
    }

    #[test]
    fn log_counts_add() {
        let a = LogCounts { info: 10, warn: 2, error: 1, malformed: 0 };
        let b = LogCounts { info: 5,  warn: 3, error: 0, malformed: 1 };
        assert_eq!(a + b, LogCounts { info: 15, warn: 5, error: 1, malformed: 1 });
    }

    #[test]
    fn total_and_total_valid() {
        let c = LogCounts { info: 10, warn: 5, error: 2, malformed: 3 };
        assert_eq!(c.total_valid(), 17);
        assert_eq!(c.total(), 20);
    }
}
