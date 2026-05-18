use anyhow::Result;
use ddnn::{Config, run_analyzer};
use std::path::PathBuf;

fn main() -> Result<()> {
    let path_arg = std::env::args().nth(1).unwrap_or_else(|| "test.dat".to_string());
    let config = Config {
        path: PathBuf::from(path_arg),
        core_multiplier: 4,
        buffer_size: 1024 * 1024 * 100,
    };
    let res = run_analyzer(config)?;
    println!("{}", res);
    Ok(())
}
