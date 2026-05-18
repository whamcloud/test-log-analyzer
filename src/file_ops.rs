use crate::types::LogOutput;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::BufReader;
use std::io::{BufRead, Seek, SeekFrom};
use std::sync::Arc;

pub struct ChunkedWorker {
    start_offset: u64,
    end_offset: u64,
    path: Arc<std::path::PathBuf>,
    buffer_size: usize,
    error_tx: Option<crossbeam_channel::Sender<crate::types::ErrorMessage>>,
}

impl ChunkedWorker {
    fn new(
        path: Arc<std::path::PathBuf>,
        start_offset: u64,
        end_offset: u64,
        buffer_size: usize,
        error_tx: Option<crossbeam_channel::Sender<crate::types::ErrorMessage>>,
    ) -> Result<Self> {
        Ok(Self {
            start_offset,
            end_offset,
            path,
            buffer_size,
            error_tx,
        })
    }

    pub fn analyze(mut self) -> Result<LogOutput> {
        let mut log_output = LogOutput::default();
        let mut file = File::open(self.path.as_path())?;
        file.seek(SeekFrom::Start(self.start_offset))?;
        let mut reader = BufReader::with_capacity(self.buffer_size, file);

        let mut buffer = Vec::new();
        while self.start_offset < self.end_offset {
            buffer.clear();
            let bytes_read = reader.read_until(b'\n', &mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            match crate::types::parse_log_line(&buffer) {
                Ok(log_level) => log_output.inc(Some(log_level)),
                Err(err) => {
                    log_output.inc(None);
                    if let Some(tx) = &self.error_tx {
                        let _ = tx.send(crate::types::ErrorMessage {
                            offset: self.start_offset,
                            msg: err.as_str(),
                        });
                    }
                }
            }
            self.start_offset += bytes_read as u64;
        }
        Ok(log_output)
    }
}

pub fn chunk_file(
    path: &std::sync::Arc<std::path::PathBuf>,
    chunks: usize,
    buffer_size: usize,
    error_tx: Option<crossbeam_channel::Sender<crate::types::ErrorMessage>>,
) -> Result<Vec<ChunkedWorker>> {
    let file_metadata = std::fs::metadata(path.as_path()).context("metadata file read failed")?;
    let file_size = file_metadata.len();
    let mut workers = Vec::with_capacity(chunks);
    let chunk_size = file_size / (chunks as u64);

    let mut start_chunk = 0;
    let mut end_chunk;
    let file = File::open(path.as_path())?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();

    for _i in 0..(chunks - 1) {
        buffer.clear();
        end_chunk = start_chunk + chunk_size;
        if end_chunk > file_size {
            break;
        }
        reader.seek(SeekFrom::Start(end_chunk))?;
        let bytes_read = reader.read_until(b'\n', &mut buffer)?;
        end_chunk += bytes_read as u64;
        workers.push(ChunkedWorker::new(
            path.clone(),
            start_chunk,
            end_chunk,
            buffer_size,
            error_tx.clone(),
        )?);
        start_chunk = end_chunk + 1;
    }
    workers.push(ChunkedWorker::new(
        Arc::clone(path),
        start_chunk,
        file_size,
        buffer_size,
        error_tx,
    )?);
    Ok(workers)
}

// Write to a temp file and returns the file path
// if error reporting is not enabled, return None
fn setup_error_handler(
    enable: bool,
) -> (
    Option<std::thread::JoinHandle<()>>,
    Option<crossbeam_channel::Sender<crate::types::ErrorMessage>>,
) {
    if enable {
        let (tx, rx) = crossbeam_channel::unbounded::<crate::types::ErrorMessage>();
        use std::{env, process};

        let mut path = env::temp_dir();
        path.push(format!("ddnn_error_{}.log", process::id()));
        println!("Error log at: {}", path.to_str().unwrap());

        let thread = std::thread::spawn(move || {
            use std::io::Write;

            let file = std::fs::File::create(path).expect("failed to create error.log");
            let mut handle = std::io::BufWriter::new(file);
            while let Ok(err) = rx.recv() {
                let _ = writeln!(handle, "Error at offset {}: {}", err.offset, err.msg);
            }
            let _ = handle.flush();
        });
        (Some(thread), Some(tx))
    } else {
        (None, None)
    }
}

pub fn run_analyzer(config: crate::types::Config) -> Result<crate::types::LogOutput> {
    let path = std::sync::Arc::new(config.path);
    let core_ids = core_affinity::get_core_ids().ok_or(anyhow::anyhow!("couldnt get core id"))?;
    let num = core_ids.len() * config.core_multiplier;

    let (err_thread, err_tx) = setup_error_handler(config.enable_error_reporting);

    let mut workers = chunk_file(&path, num, config.buffer_size, err_tx).unwrap();

    let final_output = if workers.len() == 1 {
        workers
            .pop()
            .ok_or(anyhow::anyhow!("failed to get worker"))
            .unwrap()
            .analyze()
    } else {
        let mut result = Ok(crate::types::LogOutput::default());
        let point = &mut result;

        let (tx, rx) = crossbeam_channel::unbounded();
        for worker in workers {
            tx.send(worker)?;
        }
        drop(tx);
        std::thread::scope(move |scope| {
            let mut handles = Vec::with_capacity(core_ids.len());
            for core_id in core_ids {
                let rx = rx.clone();
                let handle = scope.spawn(move || {
                    core_affinity::set_for_current(core_id);
                    let mut local_log_output = crate::types::LogOutput::default();

                    while let Ok(worker) = rx.recv() {
                        if let Ok(res) = worker.analyze() {
                            local_log_output += res;
                        }
                    }
                    local_log_output
                });
                handles.push(handle);
            }
            *point = Ok(handles
                .into_iter()
                .map(|handle| handle.join())
                .filter_map(Result::ok)
                .sum());
        });
        result
    };

    if let Some(thread) = err_thread {
        thread.join().ok();
    }
    final_output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp_log(lines: &[&str]) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
        f
    }

    #[test]
    fn test_chunk_file() {
        let f = write_temp_log(&[
            "2025-01-01T12:00:00Z|INFO|svc|msg",
            "2025-01-01T12:00:01Z|ERROR|svc|msg",
        ]);
        let path = std::sync::Arc::new(f.path().to_path_buf());
        let workers = chunk_file(&path, 4, 1024, None).unwrap();
        assert!(!workers.is_empty());
    }

    #[test]
    fn test_run_analyzer_correctness() {
        let f = write_temp_log(&[
            "2025-01-01T12:00:00Z|INFO|svc|msg",
            "2025-01-01T12:00:01Z|ERROR|svc|msg",
            "2025-01-01T12:00:02Z|WARN|svc|msg",
            "CORRUPT_LINE",
        ]);
        let config = crate::types::Config {
            path: f.path().to_path_buf(),
            core_multiplier: 1,
            buffer_size: 1024,
            enable_error_reporting: false,
        };
        let result = run_analyzer(config).unwrap();
        
        let mut expected = crate::types::LogOutput::default();
        expected.inc(Some(crate::types::LogLevel::Info));
        expected.inc(Some(crate::types::LogLevel::Error));
        expected.inc(Some(crate::types::LogLevel::Warn));
        expected.inc(None);

        assert_eq!(result, expected);
    }
}

