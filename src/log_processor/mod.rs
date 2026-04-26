use std::collections::HashMap;

use crate::{
    config::{Config, Target},
    errors::LogAnalyzerErrors,
};

mod parallel;
mod sequential;

pub use parallel::ParallelLogProcessor;
pub use sequential::SequentialLogProcessor;

pub struct Summary {
    target: Target,
    records: HashMap<String, u64>,
}

impl Summary {
    pub fn print(&self) {
        println!("\n== Final Counts (By {:?}) ==", self.target);
        for (key, count) in &self.records {
            println!("{}: {}", key, count);
        }
        println!("=============================");
    }
}

pub trait LogProcessor {
    fn process(&self) -> Result<Summary, LogAnalyzerErrors<'_>>;
}

pub struct LogEntry<'a> {
    level: &'a str,
    _time: &'a str,
    service: &'a str,
    _message: &'a str,
}

pub fn process_log_line<'a>(cfg: &Config, line: &'a str) -> Result<LogEntry<'a>, String> {
    let content: Vec<&str> = line.split(&cfg.delimiter).collect();

    if content.len() != 4 {
        return Err("Malformed".to_string());
    }

    let level = if cfg.levels.contains(&content[1].to_string()) {
        content[1]
    } else {
        "UNKNOWN"
    };

    Ok(LogEntry {
        _time: content[0],
        level,
        service: content[2],
        _message: content[3],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_process_valid_log_line() {
        let cfg = Config::default();
        let line = "2025-01-01T12:00:00Z|ERROR|auth|invalid token";
        let result = process_log_line(&cfg, line);

        assert!(result.is_ok());
        let entry = result.unwrap();
        assert_eq!(entry.level, "ERROR");
        assert_eq!(entry.service, "auth");
    }

    #[test]
    fn test_process_malformed_line() {
        let cfg = Config::default();
        // Missing a field (only 3 parts)
        let line = "2025-01-01T12:00:00Z|ERROR|auth";
        let result = process_log_line(&cfg, line);

        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "Malformed");
    }

    #[test]
    fn test_unknown_log_level() {
        let cfg = Config::default();
        let line = "2025-01-01T12:00:00Z|FATAL|auth|something died";
        let result = process_log_line(&cfg, line);

        assert!(result.is_ok());
        assert_eq!(result.unwrap().level, "UNKNOWN");
    }

    #[test]
    fn test_custom_delimiter() {
        let mut cfg = Config::default();
        cfg.delimiter = ",".to_string();
        let line = "2025-01-01,INFO,api,success";
        let result = process_log_line(&cfg, line);

        assert!(result.is_ok());
        let entry = result.unwrap();
        assert_eq!(entry.level, "INFO");
        assert_eq!(entry.service, "api");
    }
}
