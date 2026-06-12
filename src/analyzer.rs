use crate::cli::Config;
use crate::error::AnalyzerError;
use crate::parser::parse_log_level;
use crate::types::{AnalysisReport, Counters};
use rayon::prelude::*;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub struct LogAnalyzer;

impl LogAnalyzer {
    /// Reads the log file in parallel chunks and returns the final counts.
    pub fn run(config: &Config) -> Result<AnalysisReport, AnalyzerError> {
        let file = File::open(&config.file_path)?;
        let reader = BufReader::with_capacity(config.read_buffer_size, file);

        let counters = ChunkReader {
            reader,
            chunk_size: config.chunk_size,
        }
        .par_bridge()
        .map(|chunk| Self::count_chunk(&chunk))
        .reduce(Counters::default, Self::merge);

        Ok(AnalysisReport { counters })
    }

    fn count_chunk(chunk: &str) -> Counters {
        let mut counts = Counters::default();
        for line in chunk.lines() {
            if line.is_empty() {
                continue;
            }
            match parse_log_level(line) {
                Ok(level) => counts.increment(level),
                Err(_) => counts.malformed += 1,
            }
        }
        counts
    }

    fn merge(mut a: Counters, b: Counters) -> Counters {
        a.merge(&b);
        a
    }
}

// Reads a BufRead line-by-line into fixed-size String chunks.
// Lines are never split because we use read_line, so each chunk is always
// a set of complete log entries.
struct ChunkReader<R> {
    reader: R,
    chunk_size: usize,
}

impl<R: BufRead> Iterator for ChunkReader<R> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let mut chunk = String::with_capacity(self.chunk_size + 1024);
        let mut bytes_read = 0;

        while bytes_read < self.chunk_size {
            match self.reader.read_line(&mut chunk) {
                Ok(0) => break,
                Ok(n) => bytes_read += n,
                Err(_) => break,
            }
        }

        if chunk.is_empty() { None } else { Some(chunk) }
    }
}
