use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
};

use crate::{
    config::{Config, Target},
    errors::LogAnalyzerErrors,
    log_processor::{LogProcessor, Summary, process_log_line},
};

pub struct SequentialLogProcessor<'a> {
    pub file_path: &'a String,
    pub cfg: &'a Config,
}

impl<'a> LogProcessor for SequentialLogProcessor<'a> {
    fn process(&self) -> Result<Summary, LogAnalyzerErrors<'_>> {
        let file = File::open(self.file_path).map_err(|err| {
            LogAnalyzerErrors::IoError(self.file_path, format!("Unable to read log file {err}"))
        })?;

        let mut counts: HashMap<String, u64> = HashMap::new();
        let mut line = String::new();
        let mut reader = BufReader::new(file);

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).map_err(|err| {
                LogAnalyzerErrors::IoError(self.file_path, format!("Read error: {err}"))
            })?;

            if bytes_read == 0 {
                break; // EOF
            }

            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            match process_log_line(&self.cfg, trimmed) {
                Ok(log_entry) => {
                    let key = match self.cfg.target {
                        Target::Level => log_entry.level,
                        Target::Service => log_entry.service,
                    };

                    if let Some(count) = counts.get_mut(key) {
                        *count += 1;
                    } else {
                        counts.insert(key.to_owned(), 1);
                    }
                }
                Err(_) => {
                    *counts.entry("MALFORMED".to_string()).or_insert(0) += 1;
                }
            }
        }

        Ok(Summary {
            target: self.cfg.target.clone(),
            records: counts,
        })
    }
}
