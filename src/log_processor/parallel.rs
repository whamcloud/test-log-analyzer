use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    thread,
};

use crate::{
    config::{Config, Target},
    errors::LogAnalyzerErrors,
    log_processor::{LogProcessor, Summary, process_log_line},
};

pub struct ParallelLogProcessor<'a> {
    pub file_path: &'a String,
    pub cfg: &'a Config,
}

impl<'a> ParallelLogProcessor<'a> {
    fn max_available_threads() -> usize {
        thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    }

    fn find_chunk_boundaries(file: &File, num_chunks: usize) -> io::Result<Vec<(u64, u64)>> {
        let file_size = file.metadata()?.len();
        if file_size == 0 {
            return Ok(vec![]);
        }

        let chunk_size = file_size / num_chunks as u64;
        let mut boundaries = Vec::with_capacity(num_chunks);
        let mut reader = BufReader::new(file);

        let mut start = 0u64;

        for i in 0..num_chunks {
            let end = if i == num_chunks - 1 {
                file_size
            } else {
                let rough_end = start + chunk_size;
                if rough_end >= file_size {
                    file_size
                } else {
                    reader.seek(SeekFrom::Start(rough_end))?;
                    let mut discard = String::new();
                    reader.read_line(&mut discard)?;
                    // Position after consuming the partial line
                    reader.seek(SeekFrom::Current(0))?;
                    let pos = rough_end + discard.len() as u64;
                    pos.min(file_size)
                }
            };

            if start < end {
                boundaries.push((start, end));
            }
            start = end;

            if start >= file_size {
                break;
            }
        }

        Ok(boundaries)
    }

    fn process_chunk(
        file_path: &'a str,
        start: u64,
        end: u64,
        cfg: &'a Config,
    ) -> Result<HashMap<String, u64>, LogAnalyzerErrors<'a>> {
        let file = File::open(file_path).map_err(|err| {
            LogAnalyzerErrors::IoError(file_path, format!("Unable to read log file {err}"))
        })?;
        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(start)).map_err(|err| {
            LogAnalyzerErrors::IoError(
                file_path,
                format!("Unable to change current pos in log file {err}"),
            )
        })?;

        let mut current_pos = start;

        let mut counts: HashMap<String, u64> = HashMap::new();

        let mut line = String::new();
        while current_pos < end {
            line.clear();
            let bytes_read = reader.read_line(&mut line).map_err(|err| {
                LogAnalyzerErrors::IoError(file_path, format!("Unable to read log file {err}"))
            })?;
            if bytes_read == 0 {
                break; // EOF
            }
            current_pos += bytes_read as u64;

            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(log_entry) = process_log_line(&cfg, &line) {
                let key = match cfg.target {
                    Target::Level => log_entry.level,
                    Target::Service => log_entry.service,
                };
                *counts.entry(key.to_owned()).or_insert(0) += 1;
            } else {
                *counts.entry("MALFORMED".to_string()).or_insert(0) += 1;
            }
        }

        Ok(counts)
    }
}

impl<'a> LogProcessor for ParallelLogProcessor<'a> {
    fn process(&self) -> Result<Summary, LogAnalyzerErrors<'_>> {
        let file = File::open(self.file_path).unwrap();
        let num_chunks = Self::max_available_threads();
        let boundaries = Self::find_chunk_boundaries(&file, num_chunks).map_err(|err| {
            LogAnalyzerErrors::IoError(
                self.file_path,
                format!("Unable to get chunks of file: {err}"),
            )
        })?;
        drop(file);

        let mut total_counts = HashMap::new();

        std::thread::scope(|s| {
            let mut handles = Vec::new();

            for (start, end) in boundaries {
                let handle =
                    s.spawn(move || Self::process_chunk(self.file_path, start, end, self.cfg));
                handles.push(handle);
            }

            for handle in handles {
                let chunk_res = handle.join().map_err(|_| {
                    LogAnalyzerErrors::IoError(self.file_path, "Thread panicked".to_string())
                })?;

                for (key, val) in chunk_res? {
                    *total_counts.entry(key).or_insert(0) += val;
                }
            }

            Ok::<(), LogAnalyzerErrors>(())
        })?;

        Ok(Summary {
            target: self.cfg.target.clone(),
            records: total_counts,
        })
    }
}

#[cfg(test)]
mod parallel_tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_find_chunk_boundaries() {
        let mut file = NamedTempFile::new().unwrap();
        for _ in 0..10 {
            writeln!(file, "123456789").unwrap();
        }

        let boundaries = ParallelLogProcessor::find_chunk_boundaries(file.as_file(), 2).unwrap();

        assert_eq!(boundaries.len(), 2);
        assert!(boundaries[0].1 > 0);
        assert_eq!(boundaries[1].1, 100); // Total size
    }
}
