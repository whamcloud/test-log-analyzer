use anyhow::Result;
use ddnn::{Config, run_analyzer};
use std::path::PathBuf;

fn main() -> Result<()> {
    let path_arg = std::env::args().nth(1).unwrap_or_else(|| "test.dat".to_string());
    let enable_error_reporting = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(false);
    let config = Config {
        path: PathBuf::from(path_arg),
        core_multiplier: 4, // workers = core_multipliers * no_of_cores
        buffer_size: 1024 * 1024 * 100, // 100MB buffer per worker
        enable_error_reporting,
    };
    let res = run_analyzer(config)?;
    println!("{}", res);
    Ok(())
}
