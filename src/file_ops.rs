use std::io::BufReader;
use anyhow::{Result, Context};
use crate::LogOutput;
use std::io::{Seek, BufRead, SeekFrom};
use std::fs::File;
use std::sync::Arc;

pub struct ChunkedWorker{
    start_offset: u64, 
    end_offset : u64,
    path: Arc<std::path::PathBuf>,
}

impl ChunkedWorker{
    fn new(path : Arc<std::path::PathBuf>, start_offset : u64, end_offset: u64) -> Result<Self>{
        Ok(Self{
            start_offset,
            end_offset,
            path,
        })
    }
    
    pub fn analyze(mut self) -> Result<LogOutput>{
        let mut log_output = LogOutput::default();
        let mut file = File::open(self.path.as_path())?;
        file.seek(SeekFrom::Start(self.start_offset))?;
        let mut reader = BufReader::with_capacity(1024* 1024 * 100, file);

        let mut buffer = Vec::new();
        while self.start_offset < self.end_offset {
            buffer.clear();
            let bytes_read = reader.read_until(b'\n', &mut buffer)?;
            if bytes_read ==0 {
                break;
            }
            let log_level = crate::types::parse_log_line(&buffer)
                .ok();
            log_output.inc(log_level);
            self.start_offset += bytes_read as u64;
        }
        Ok(log_output)
    }
}


pub fn chunk_file(path: &std::sync::Arc<std::path::PathBuf>, chunks: usize) -> Result<Vec<ChunkedWorker>>{
    let file_metadata = std::fs::metadata(path.as_path()).context("metadata file read failed")?;
    let file_size = file_metadata.len();
    let mut workers = Vec::with_capacity(chunks);
    let chunk_size = file_size / (chunks as u64);

    let mut start_chunk = 0;
    let mut end_chunk;
    let file = File::open(path.as_path())?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();

    for _i in 0..(chunks-1){
        buffer.clear();
        end_chunk = start_chunk + chunk_size;
        if end_chunk > file_size{
            break;
        }
        reader.seek(SeekFrom::Start(end_chunk))?;
        let bytes_read = reader.read_until(b'\n', &mut buffer)?;
        end_chunk += bytes_read as u64;
        workers.push(ChunkedWorker::new(Arc::clone(path), start_chunk, end_chunk)?);
        start_chunk = end_chunk +1;
    }
    workers.push(ChunkedWorker::new(Arc::clone(path), start_chunk, file_size)?);
    Ok(workers)
}
