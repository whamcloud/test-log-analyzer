use log_analyzer::analyzer::analyze;
use log_analyzer::format::LogFormat;
use log_analyzer::time_block;
use memmap2::Mmap;
use std::fs::File;
use std::path::PathBuf;
use std::process;

fn print_usage(program: &str) {
    eprintln!("Usage: {} <log-file> [OPTIONS]", program);
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --delimiter <char>     Field delimiter (default: '|')");
    eprintln!("  --level-pos <n>        0-indexed position of the level field (default: 1)");
    eprintln!("  --format <preset>      Preset: standard | space | csv");
    eprintln!("  --threads <n>          Override rayon thread count");
    eprintln!("  --help, -h             Show this message");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {} server.log", program);
    eprintln!("  {} server.log --format csv", program);
    eprintln!("  {} server.log --delimiter ',' --level-pos 2", program);
    eprintln!("  {} server.log --threads 4", program);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        process::exit(1);
    }

    // --- parse flags ---
    let mut format = LogFormat::standard();
    let mut custom_delimiter: Option<u8> = None;
    let mut custom_level_pos: Option<usize> = None;
    let mut thread_count: Option<usize> = None;
    let mut i = 2usize;

    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_usage(&args[0]);
                process::exit(0);
            }
            "--format" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --format requires an argument");
                    process::exit(1);
                }
                format = match args[i].as_str() {
                    "standard" => LogFormat::standard(),
                    "space"    => LogFormat::space(),
                    "csv"      => LogFormat::csv(),
                    other => {
                        eprintln!("error: unknown format '{}'. Use: standard | space | csv", other);
                        process::exit(1);
                    }
                };
            }
            "--delimiter" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --delimiter requires an argument");
                    process::exit(1);
                }
                match LogFormat::parse_delimiter(&args[i]) {
                    Some(d) => custom_delimiter = Some(d),
                    None => {
                        eprintln!("error: delimiter must be a single ASCII character");
                        process::exit(1);
                    }
                }
            }
            "--level-pos" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --level-pos requires an argument");
                    process::exit(1);
                }
                match args[i].parse::<usize>() {
                    Ok(n) => custom_level_pos = Some(n),
                    Err(_) => {
                        eprintln!("error: --level-pos must be a non-negative integer");
                        process::exit(1);
                    }
                }
            }
            "--threads" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --threads requires an argument");
                    process::exit(1);
                }
                match args[i].parse::<usize>() {
                    Ok(n) if n > 0 => thread_count = Some(n),
                    _ => {
                        eprintln!("error: --threads must be a positive integer");
                        process::exit(1);
                    }
                }
            }
            other if other.starts_with("--") => {
                eprintln!("error: unknown option '{}'. Run with --help for usage.", other);
                process::exit(1);
            }
            _ => {}
        }
        i += 1;
    }

    // --delimiter / --level-pos override --format
    if custom_delimiter.is_some() || custom_level_pos.is_some() {
        format = LogFormat::custom(
            custom_delimiter.unwrap_or(format.delimiter),
            custom_level_pos.unwrap_or(format.level_position),
        );
    }
    
    // apply --threads if requested
    if let Some(n) = thread_count {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()
            .unwrap_or_else(|e| {
                eprintln!("warning: could not set thread count: {}", e);
            });
    }

    let path = PathBuf::from(&args[1]);

    // Check file exists before attempting mmap.
    if !path.exists() {
        eprintln!("error: file '{}' does not exist", path.display());
        process::exit(1);
    }

    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: could not open '{}': {}", path.display(), e);
            process::exit(1);
        }
    };

    // Memory-map the file for zero-copy access.
    // Safety: documented — see DESIGN.md.
    let mmap = match unsafe { Mmap::map(&file) } {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: could not memory-map '{}': {}", path.display(), e);
            process::exit(1);
        }
    };

    eprintln!(
        "Analyzing: {}  ({} MB)  delimiter='{}' level_pos={}  threads={}",
        path.display(),
        mmap.len() / 1_048_576,
        format.delimiter as char,
        format.level_position,
        rayon::current_num_threads(),
    );

    let counts = time_block!("Analysis", { analyze(&mmap, format) });

    counts.display();
}
