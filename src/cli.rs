use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Config {
    /// Path to the log file to analyze.
    #[arg(env = "LOG_FILE_PATH")]
    pub file_path: PathBuf,

    /// How many bytes to accumulate per parallel chunk.
    /// Larger = fewer thread dispatches; smaller = better for small files.
    #[arg(long, env = "CHUNK_SIZE", default_value_t = 8 * 1024 * 1024)]
    pub chunk_size: usize,

    /// Size of the internal read buffer for BufReader in bytes.
    #[arg(long, env = "READ_BUFFER_SIZE", default_value_t = 64 * 1024)]
    pub read_buffer_size: usize,
}

impl Config {
    pub fn parse_args() -> Result<Self, clap::Error> {
        Self::try_parse()
    }
}
