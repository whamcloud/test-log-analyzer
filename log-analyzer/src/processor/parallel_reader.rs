use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

use crate::error::LogError;
use crate::log_format::LogFormat;
use crate::log_summary::LogSummary;
use crate::processor::{LogProcessor, parse_log_level_with_format};

pub struct ParallelReader {
    path: PathBuf,
    target_chunk_size: u64,
    read_buffer_size: usize,
    line_buffer_capacity: usize,
    format: LogFormat,
}

impl ParallelReader {
    pub fn new(
        path: PathBuf,
        target_chunk_size_mb: u64,
        read_buffer_size: usize,
        line_buffer_capacity: usize,
    ) -> Self {
        Self {
            path,
            target_chunk_size: target_chunk_size_mb * 1024 * 1024,
            read_buffer_size: read_buffer_size.max(64 * 1024),
            line_buffer_capacity: line_buffer_capacity.max(128),
            format: LogFormat::standard(),
        }
    }

    pub fn with_format(
        path: PathBuf,
        target_chunk_size_mb: u64,
        read_buffer_size: usize,
        line_buffer_capacity: usize,
        format: LogFormat,
    ) -> Self {
        Self {
            path,
            target_chunk_size: target_chunk_size_mb * 1024 * 1024,
            read_buffer_size: read_buffer_size.max(64 * 1024),
            line_buffer_capacity: line_buffer_capacity.max(128),
            format,
        }
    }

    /// Find the position of the next line start from given position
    /// Works with any Read implementation (File, Cursor, etc.)
    pub fn find_next_line_start_from_reader<R: Read>(
        reader: &mut R,
        start_position: u64,
    ) -> Result<u64, LogError> {
        if start_position == 0 {
            return Ok(0);
        }

        let mut buf = vec![0u8; 8192];
        let mut current_pos = start_position;

        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                // EOF reached, return current position
                return Ok(current_pos);
            }

            // Look for newline in the buffer we just read
            if let Some(offset) = buf[..n].iter().position(|&b| b == b'\n') {
                // Found newline at offset, return position after it
                return Ok(current_pos + offset as u64 + 1);
            }

            current_pos += n as u64;
        }
    }

    fn find_next_line_start(position: u64, path: &PathBuf) -> Result<u64, LogError> {
        if position == 0 {
            return Ok(0);
        }

        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(position))?;

        Self::find_next_line_start_from_reader(&mut file, position)
    }

    pub fn process_chunk_from_reader<R: BufRead>(
        reader: &mut R,
        start: u64,
        end: u64,
        line_buffer_capacity: usize,
        format: LogFormat,
    ) -> Result<LogSummary, LogError> {
        let mut summary = LogSummary::default();
        let mut line = String::with_capacity(line_buffer_capacity);
        let mut bytes_read_total = 0u64;

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line)?;

            if bytes_read == 0 {
                break; // EOF
            }

            bytes_read_total += bytes_read as u64;

            // CRITICAL: Stop if we've read past the chunk boundary
            if start + bytes_read_total > end {
                break;
            }

            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                summary.malformed += 1;
                continue;
            }

            match parse_log_level_with_format(trimmed, format) {
                Ok("INFO") => summary.info += 1,
                Ok("WARN") => summary.warn += 1,
                Ok("ERROR") => summary.error += 1,
                _ => summary.malformed += 1,
            }
        }

        Ok(summary)
    }

    fn process_chunk(
        path: Arc<PathBuf>,
        start: u64,
        end: u64,
        read_buffer_size: usize,
        line_buffer_capacity: usize,
        format: LogFormat,
    ) -> Result<LogSummary, LogError> {
        let mut file = File::open(&*path)?;
        file.seek(SeekFrom::Start(start))?;

        let mut reader = BufReader::with_capacity(read_buffer_size, file);

        Self::process_chunk_from_reader(&mut reader, start, end, line_buffer_capacity, format)
    }

    /// Calculate chunk boundaries given total size and chunk size
    /// Uses find_next_line_start internally to align boundaries
    fn get_chunk_boundaries(
        chunk_size: u64,
        total_size: u64,
        path: &PathBuf,
    ) -> Result<Vec<(u64, u64)>, LogError> {
        Self::get_chunk_boundaries_with(chunk_size, total_size, |pos| {
            Self::find_next_line_start(pos, path)
        })
    }

    fn get_chunk_boundaries_with<F>(
        chunk_size: u64,
        total_size: u64,
        find_line_fn: F,
    ) -> Result<Vec<(u64, u64)>, LogError>
    where
        F: Fn(u64) -> Result<u64, LogError>,
    {
        let mut chunks = Vec::new();
        let mut prev_end = 0u64;

        while prev_end < total_size {
            let approx_end = (prev_end + chunk_size).min(total_size);
            let chunk_end = if approx_end == total_size {
                total_size
            } else {
                find_line_fn(approx_end)?
            };

            if prev_end < chunk_end {
                chunks.push((prev_end, chunk_end));
            }
            prev_end = chunk_end;
        }
        Ok(chunks)
    }

    fn num_chunks(total_size: u64, target_chunk_size: u64) -> u64 {
        total_size.div_ceil(target_chunk_size)
    }

    fn effective_threads(num_chunks: u64, num_cpus: usize) -> usize {
        //number of threads must be less that num of threads permitted by cpu and at least 1
        //and if there are 5 chunks, we shall only spawn 5 threads
        (num_chunks as usize).min(num_cpus).max(1)
    }

    // Testable version - accepts a chunk processor function
    fn process_chunks_parallel_with<F>(
        chunks: Vec<(u64, u64)>,
        effective_threads: usize,
        process_chunk_fn: F,
    ) -> Result<LogSummary, LogError>
    where
        F: Fn(u64, u64) -> Result<LogSummary, LogError> + Send + Sync + 'static,
    {
        use std::sync::Mutex;

        let total_info = Arc::new(AtomicU64::new(0));
        let total_warn = Arc::new(AtomicU64::new(0));
        let total_error = Arc::new(AtomicU64::new(0));
        let total_malformed = Arc::new(AtomicU64::new(0));

        let chunks_iter = Arc::new(Mutex::new(chunks.into_iter()));
        let process_fn = Arc::new(process_chunk_fn);

        let handles: Vec<_> = (0..effective_threads)
            .map(|_worker_id| {
                let chunks_iter = Arc::clone(&chunks_iter);
                let process_fn = Arc::clone(&process_fn);
                let info = Arc::clone(&total_info);
                let warn = Arc::clone(&total_warn);
                let error = Arc::clone(&total_error);
                let malformed = Arc::clone(&total_malformed);

                thread::spawn(move || {
                    loop {
                        let chunk = {
                            let mut iter = chunks_iter.lock().unwrap();
                            iter.next()
                        };

                        match chunk {
                            Some((start, end)) => {
                                match process_fn(start, end) {
                                    Ok(summary) => {
                                        info.fetch_add(summary.info, Ordering::Relaxed);
                                        warn.fetch_add(summary.warn, Ordering::Relaxed);
                                        error.fetch_add(summary.error, Ordering::Relaxed);
                                        malformed.fetch_add(summary.malformed, Ordering::Relaxed);
                                    }
                                    Err(_e) => {
                                        // Handle error
                                        eprintln!("Could not process chunk");
                                    }
                                }
                            }
                            None => break,
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle
                .join()
                .map_err(|_| LogError::InvalidFormat("Worker thread panicked".to_string()))?;
        }

        Ok(LogSummary {
            info: total_info.load(Ordering::Relaxed),
            warn: total_warn.load(Ordering::Relaxed),
            error: total_error.load(Ordering::Relaxed),
            malformed: total_malformed.load(Ordering::Relaxed),
        })
    }

    fn process_chunks_parallel(
        chunks: Vec<(u64, u64)>,
        path: PathBuf,
        effective_threads: usize,
        read_buffer_size: usize,
        line_buffer_capacity: usize,
        format: LogFormat,
    ) -> Result<LogSummary, LogError> {
        let path = Arc::new(path);
        Self::process_chunks_parallel_with(chunks, effective_threads, move |start, end| {
            Self::process_chunk(
                Arc::clone(&path),
                start,
                end,
                read_buffer_size,
                line_buffer_capacity,
                format,
            )
        })
    }

    fn num_cpus() -> usize {
        thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    }
}

impl LogProcessor for ParallelReader {
    fn process(&mut self) -> Result<LogSummary, LogError> {
        let file = File::open(&self.path)?;
        let total_size = file.metadata()?.len();
        drop(file);

        if total_size == 0 {
            return Ok(LogSummary::default());
        }

        // Use actual CPU cores
        let num_cpus = Self::num_cpus();

        let num_chunks_approx = Self::num_chunks(total_size, self.target_chunk_size);
        let effective_threads = Self::effective_threads(num_chunks_approx, num_cpus);

        // Build all chunk ranges upfront
        let chunks = Self::get_chunk_boundaries(self.target_chunk_size, total_size, &self.path)?;

        // Spawn threads directly - no thread pool overhead
        Self::process_chunks_parallel(
            chunks,
            self.path.clone(),
            effective_threads,
            self.read_buffer_size,
            self.line_buffer_capacity,
            self.format,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_find_line_start_at_position_zero() {
        //this test is only if total_size is 0
        let content = b"line1\nline2\nline3\n";
        let mut cursor = Cursor::new(content);
        let result = ParallelReader::find_next_line_start_from_reader(&mut cursor, 0).unwrap();
        assert_eq!(result, 0)
    }
    #[test]
    fn test_find_line_start_debug() {
        let content = b"line1\nline2\nline3\n";
        //             0 1 2 3 4  5 6 7 8 9 10 11
        //             l i n e 1 \n l i n e 2 \n

        let mut cursor = Cursor::new(content);
        cursor.set_position(1);
        let result = ParallelReader::find_next_line_start_from_reader(&mut cursor, 1).unwrap();

        assert_eq!(result, 6, "Should return 6");
    }
    #[test]
    fn test_find_line_start_immediately_after_newline() {
        let content = b"line1\nline2\nline3\n";
        //             0 1 2 3 4  5 6 7 8 9 10 11
        //             l i n e 1 \n l i n e 2 \n
        let mut cursor = Cursor::new(content);

        // Seek to position 6 (start of "line2")
        cursor.set_position(6);

        let result = ParallelReader::find_next_line_start_from_reader(&mut cursor, 6).unwrap();

        // Should find '\n' at position 11, return 12 (start of "line3")
        assert_eq!(
            result, 12,
            "From position 6, should find next newline at 11 and return 12"
        );
    }

    #[test]
    fn test_find_line_start_mid_line() {
        let content = b"line1\nline2\nline3\n";
        //             0 1 2 3 4  5 6 7 8 9 10 11
        //             l i n e 1 \n l i n e 2 \n
        let mut cursor = Cursor::new(content);

        // Position 3 is in the middle of "line1"
        cursor.set_position(3);

        let result = ParallelReader::find_next_line_start_from_reader(&mut cursor, 3).unwrap();

        // Should find '\n' at position 5, return 6 (start of "line2")
        assert_eq!(result, 6, "Should find newline at position 5 and return 6");
    }
    #[test]
    fn test_find_line_start_exactly_at_newline() {
        let content = b"line1\nline2\nline3\n";
        //             0 1 2 3 4  5 6 7 8 9 10 11
        //             l i n e 1 \n l i n e 2 \n
        let mut cursor = Cursor::new(content);

        // Position 5 is exactly at the first '\n'
        cursor.set_position(5);

        let result = ParallelReader::find_next_line_start_from_reader(&mut cursor, 5).unwrap();

        // Should immediately find '\n' at offset 0 in buffer, return 5 + 0 + 1 = 6
        assert_eq!(result, 6, "Should return position after the newline");
    }

    #[test]
    fn test_find_line_start_no_newline_eof() {
        let content = b"line1\nline2\nline3";
        //             0 1 2 3 4  5 6 7 8 9 10 11
        //             l i n e 1 \n l i n e 2 \n
        let mut cursor = Cursor::new(content);

        // Position 13 is in "line3" which has no trailing newline
        cursor.set_position(13);

        let result = ParallelReader::find_next_line_start_from_reader(&mut cursor, 13).unwrap();

        // Should reach EOF and return the position at EOF
        assert_eq!(
            result, 17,
            "Should return EOF position when no newline found"
        );
    }
    #[test]
    fn test_find_line_start_near_end_no_newline() {
        let content = b"line1\nline2\nline3";
        let mut cursor = Cursor::new(content);

        // Start near the end
        cursor.set_position(15);

        let result = ParallelReader::find_next_line_start_from_reader(&mut cursor, 15).unwrap();

        // Should reach EOF at position 17
        assert_eq!(result, 17, "Should return current position at EOF");
    }

    fn find_line_in_bytes(content: &[u8], position: u64) -> Result<u64, LogError> {
        if position >= content.len() as u64 {
            return Ok(content.len() as u64);
        }

        let mut cursor = Cursor::new(content);
        cursor.set_position(position);
        ParallelReader::find_next_line_start_from_reader(&mut cursor, position)
    }

    #[test]
    fn test_empty_file() {
        let content = b"";
        let chunks = ParallelReader::get_chunk_boundaries_with(1024, 0, |pos| {
            find_line_in_bytes(content, pos)
        })
        .unwrap();

        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_single_chunk_smaller_than_size() {
        let content = b"line1\nline2\nline3\n";
        let total_size = content.len() as u64;

        let chunks = ParallelReader::get_chunk_boundaries_with(1000, total_size, |pos| {
            find_line_in_bytes(content, pos)
        })
        .unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], (0, total_size));
    }

    #[test]
    fn test_exact_chunk_size() {
        let content = b"line1\nline2\n"; // 12 bytes
        //             0 1 2 3 4  5 6 7 8 9 10 11
        //             l i n e 1 \n l i n e 2 \n
        // (0,12) means 0 to 11 inclusive

        let chunks = ParallelReader::get_chunk_boundaries_with(12, 12, |pos| {
            find_line_in_bytes(content, pos)
        })
        .unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], (0, 12));
    }

    #[test]
    fn test_splits_at_newlines() {
        let content = b"line1\nline2\nline3\nline4\n";
        //                        0-5   6-11   12-17  18-23 = 24 bytes
        //if chunk size is 10 that means chunk shall be 0-12, 12-24
        let chunks = ParallelReader::get_chunk_boundaries_with(10, content.len() as u64, |pos| {
            find_line_in_bytes(content, pos)
        })
        .unwrap();

        // Should have multiple chunks
        assert!(chunks.len() > 1);

        // Coverage check
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks[0].1, 12);
        assert_eq!(chunks.last().unwrap().1, 24);

        // Contiguity check
        for i in 1..chunks.len() {
            assert_eq!(
                chunks[i].0,
                chunks[i - 1].1,
                "Gap between chunks {} and {}",
                i - 1,
                i
            );
        }
    }

    #[test]
    fn test_no_trailing_newline() {
        let content = b"line1\nline2\nline3";
        let total_size = content.len() as u64;

        let chunks = ParallelReader::get_chunk_boundaries_with(8, total_size, |pos| {
            find_line_in_bytes(content, pos)
        })
        .unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks.last().unwrap().1, total_size);
    }

    #[test]
    fn test_line_longer_than_chunk_size() {
        let mut long_line = vec![b'x'; 5000];
        long_line.push(b'\n');
        let total_size = long_line.len() as u64;

        let chunks = ParallelReader::get_chunk_boundaries_with(1000, total_size, |pos| {
            find_line_in_bytes(&long_line, pos)
        })
        .unwrap();
        println!("{:#?}", chunks);
        // Single chunk since line is longer than chunk size
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], (0, total_size));
    }

    #[test]
    fn test_many_small_lines() {
        let mut content = Vec::new();
        for i in 0..50 {
            content.extend_from_slice(format!("L{}\n", i).as_bytes());
        }
        let total_size = content.len() as u64;

        let chunks = ParallelReader::get_chunk_boundaries_with(30, total_size, |pos| {
            find_line_in_bytes(&content, pos)
        })
        .unwrap();

        assert!(chunks.len() > 1);
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks.last().unwrap().1, total_size);

        for i in 1..chunks.len() {
            assert_eq!(chunks[i].0, chunks[i - 1].1);
        }
    }

    #[test]
    fn test_only_newlines() {
        let content = b"\n\n\n\n\n";

        let chunks =
            ParallelReader::get_chunk_boundaries_with(2, 5, |pos| find_line_in_bytes(content, pos))
                .unwrap();
        println!("{:#?}", chunks);
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks.last().unwrap().1, 5);
    }

    #[test]
    fn test_chunk_size_larger_than_file() {
        let content = b"abc\ndef\n";

        let chunks = ParallelReader::get_chunk_boundaries_with(10000, 8, |pos| {
            find_line_in_bytes(content, pos)
        })
        .unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], (0, 8));
    }

    #[test]
    fn test_chunk_size_one_byte() {
        let content = b"a\nb\nc\n";

        let chunks =
            ParallelReader::get_chunk_boundaries_with(1, 6, |pos| find_line_in_bytes(content, pos))
                .unwrap();

        assert!(chunks.len() >= 1);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks.last().unwrap().1, 6);
    }

    #[test]
    fn test_boundary_exactly_at_newline() {
        let content = b"12345\n12345\n12345\n";
        //             0-5   6-11  12-17

        let chunks = ParallelReader::get_chunk_boundaries_with(6, 18, |pos| {
            find_line_in_bytes(content, pos)
        })
        .unwrap();

        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks.last().unwrap().1, 18);

        for i in 1..chunks.len() {
            assert_eq!(chunks[i].0, chunks[i - 1].1);
        }
    }

    #[test]
    fn test_mixed_line_lengths() {
        let mut content = Vec::new();
        content.extend_from_slice(b"short\n");
        content.extend_from_slice(&vec![b'x'; 100]);
        content.push(b'\n');
        content.extend_from_slice(b"medium\n");
        let total_size = content.len() as u64;

        let chunks = ParallelReader::get_chunk_boundaries_with(50, total_size, |pos| {
            find_line_in_bytes(&content, pos)
        })
        .unwrap();

        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks.last().unwrap().1, total_size);

        for i in 1..chunks.len() {
            assert_eq!(chunks[i].0, chunks[i - 1].1);
        }
    }

    #[test]
    fn test_single_byte_file() {
        let content = b"\n";

        let chunks = ParallelReader::get_chunk_boundaries_with(100, 1, |pos| {
            find_line_in_bytes(content, pos)
        })
        .unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], (0, 1));
    }

    #[test]
    fn test_no_newlines_at_all() {
        let content = b"no newlines in this content";
        let total_size = content.len() as u64;

        let chunks = ParallelReader::get_chunk_boundaries_with(10, total_size, |pos| {
            find_line_in_bytes(content, pos)
        })
        .unwrap();

        // Single chunk - can't split without newlines
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], (0, total_size));
    }

    #[test]
    fn test_three_equal_sized_chunks() {
        // 3 lines of 10 bytes each = 30 bytes total
        let content = b"123456789\n123456789\n123456789\n";

        let chunks = ParallelReader::get_chunk_boundaries_with(10, 30, |pos| {
            find_line_in_bytes(content, pos)
        })
        .unwrap();

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], (0, 20));
        assert_eq!(chunks[1], (20, 30));
    }

    #[test]
    fn test_alternating_short_long_lines() {
        let content = b"s\nvery_long_line_here\ns\nvery_long_line_here\ns\n";
        let total_size = content.len() as u64;

        let chunks = ParallelReader::get_chunk_boundaries_with(15, total_size, |pos| {
            find_line_in_bytes(content, pos)
        })
        .unwrap();

        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks.last().unwrap().1, total_size);

        // Verify no gaps
        for i in 1..chunks.len() {
            assert_eq!(chunks[i].0, chunks[i - 1].1);
        }
    }
    fn process_chunk_from_bytes(
        content: &[u8],
        start: u64,
        end: u64,
        format: LogFormat,
    ) -> Result<LogSummary, LogError> {
        let chunk_content = &content[start as usize..end as usize];
        let mut reader = std::io::BufReader::new(Cursor::new(chunk_content));

        ParallelReader::process_chunk_from_reader(&mut reader, start, end, 128, format)
    }

    #[test]
    fn test_parallel() {
        let content = b"\
                            2025-01-01T00:00:00Z|INFO|auth|request completed
                            2025-01-01T00:00:01Z|INFO|api|request completed
                            2025-01-01T00:00:02Z|WARN|db|connection timeout
                            2025-01-01T00:00:03Z|ERROR|cache|connection failed
                            2025-01-01T00:00:04Z|INFO|worker|request completed";
        let total_size = content.len() as u64;

        let chunks = ParallelReader::get_chunk_boundaries_with(100, total_size, |pos| {
            find_line_in_bytes(content, pos)
        })
        .unwrap();
        println!("{:#?}", chunks);
        let num_chunks = chunks.len();
        let content_clone = content.to_vec();
        let result = ParallelReader::process_chunks_parallel_with(
            chunks,
            ParallelReader::effective_threads(num_chunks as u64, ParallelReader::num_cpus()),
            move |start, end| {
                process_chunk_from_bytes(&content_clone, start, end, LogFormat::standard())
            },
        )
        .unwrap();

        assert_eq!(result.info, 3);
        assert_eq!(result.warn, 1);
    }
}
