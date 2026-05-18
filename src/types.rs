use std::fmt::{Display, Formatter};

/// A valid log level extracted from a log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// Running totals for each category.
#[derive(Default, Clone, Copy)]
pub struct Counters {
    pub info: u64,
    pub warn: u64,
    pub error: u64,
    pub malformed: u64,
}

impl Counters {
    #[inline]
    pub fn increment(&mut self, level: LogLevel) {
        match level {
            LogLevel::Info => self.info += 1,
            LogLevel::Warn => self.warn += 1,
            LogLevel::Error => self.error += 1,
        }
    }

    pub fn merge(&mut self, other: &Self) {
        self.info += other.info;
        self.warn += other.warn;
        self.error += other.error;
        self.malformed += other.malformed;
    }
}

/// The final report printed after processing the file.
pub struct AnalysisReport {
    pub counters: Counters,
}

impl Display for AnalysisReport {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "INFO: {}", self.counters.info)?;
        writeln!(f, "WARN: {}", self.counters.warn)?;
        writeln!(f, "ERROR: {}", self.counters.error)?;
        writeln!(f, "MALFORMED: {}", self.counters.malformed)
    }
}
