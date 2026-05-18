pub mod file_ops;
pub mod types;

// Flatten the public API so benchmarks and external callers
// can use ddnn::run_analyzer, ddnn::Config etc. directly.
pub use file_ops::run_analyzer;
pub use types::{Config, LogLevel, LogOutput, parse_log_line};
