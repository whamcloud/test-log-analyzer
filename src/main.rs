use std::process;

use log_analyzer::analyzer::LogAnalyzer;
use log_analyzer::cli::Config;

fn main() {
    // Load .env file if present. Silently ignored when missing so the binary
    // works fine without one (CI, production servers, etc.).
    let _ = dotenvy::dotenv();

    // Default to "info" so the report is visible without RUST_LOG being set.
    // Operators can suppress with RUST_LOG=error or silence with RUST_LOG=off.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = match Config::parse_args() {
        Ok(c) => c,
        Err(e) => {
            log::error!("{e}");
            process::exit(1);
        }
    };

    match LogAnalyzer::run(&config) {
        Ok(report) => log::info!("{report}"),
        Err(err) => {
            log::error!("{err}");
            process::exit(1);
        }
    }
}
