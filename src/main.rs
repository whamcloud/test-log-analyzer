//! Log Analyzer - A high-performance log parsing utility
//!
//! This binary reads a log file and counts occurrences of different log levels
//! (INFO, WARN, ERROR) as well as malformed entries.

use std::env;
use std::process;

mod parser;

/// Main entry point for the log analyzer application.
///
/// Expects a single command-line argument: the path to the log file to analyze.
/// Outputs counts of INFO, WARN, ERROR, and MALFORMED log entries.
fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <log_file_path>", args[0]);
        process::exit(1);
    }

    let file_path = &args[1];

    match parser::analyze_log_file(file_path) {
        Ok(counts) => {
            println!("INFO: {}", counts.info);
            println!("WARN: {}", counts.warn);
            println!("ERROR: {}", counts.error);

            // Only print malformed count if there are any
            if counts.malformed > 0 {
                println!("MALFORMED: {}", counts.malformed);
            }
        }
        Err(e) => {
            eprintln!("Error reading file {}: {}", file_path, e);
            process::exit(1);
        }
    }
}
