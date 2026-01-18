use std::env;
use std::path::PathBuf;

use log_analyzer::error::LogError;
use log_analyzer::log_format::LogFormat;
use log_analyzer::processor::LogProcessor;
use log_analyzer::processor::buffered_line_reader::BufferedLineReader;
use log_analyzer::processor::parallel_reader::ParallelReader;
use log_analyzer::time_block;

fn print_usage(program: &str) {
    eprintln!("Usage: {} <log_file> [OPTIONS]", program);
    eprintln!("\nOption:s");
    eprintln!(" --parallel");
    eprintln!(" --format <preset>         Log format preset(standard|space)");
    eprintln!(" --delimiter <char>        Custom delimiter (e.g., '|', ',', ' ')");
    eprintln!(" --level-pos <index>        Position of level field (0-indexed)");
    eprintln!("\nExamples:");
    eprintln!(" {} logs.txt", program);
    eprintln!(" {} logs.txt --parallel", program);
    eprintln!(" {} logs.txt --format  space", program);
    eprintln!(" {} logs.txt --delimiter ',' --level-pos 2", program);
    eprintln!(
        " {} logs.txt --parallel --delimiter ',' --level-pos 1",
        program
    );
}

fn main() -> Result<(), LogError> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        std::process::exit(1);
    }

    let file_path = PathBuf::from(&args[1]);

    if !file_path.exists() {
        eprintln!("Error: File '{}' does not exist", file_path.display());
        std::process::exit(1);
    }

    let mut use_parallel = false;

    let mut format = LogFormat::standard();
    let mut custom_delimiter: Option<char> = None;
    let mut custom_level_pos: Option<usize> = None;

    let mut i = 2;

    while i < args.len() {
        match args[i].as_str() {
            "--parallel" => {
                use_parallel = true;
            }
            "--format" => {
                if i + 1 < args.len() {
                    format = match args[i + 1].as_str() {
                        "standard" => LogFormat::standard(),
                        "space" => LogFormat::space_delimited(),
                        other => {
                            eprintln!("Error: Unknown format '{}'", other);
                            eprintln!("Available presets: standard, space");
                            std::process::exit(1);
                        }
                    };
                    i += 1;
                }
            }
            "--delimiter" => {
                if i + 1 < args.len() {
                    let delim_str = &args[i + 1];
                    if delim_str.len() == 1 {
                        custom_delimiter = Some(delim_str.chars().next().unwrap());
                    } else {
                        eprintln!("Delimiter must be single character");
                        std::process::exit(1);
                    }
                    i += 1;
                }
            }
            "--level-pos" => {
                if i + 1 < args.len() {
                    custom_level_pos = args[i + 1].parse().ok();
                    i += 1;
                }
            }
            "--help" | "-h" => {
                print_usage(&args[0]);
                std::process::exit(1);
            }
            _ => {
                eprintln!("Error: Unknown option '{}'", args[i]);
                print_usage(&args[0]);
                std::process::exit(1);
            }
        }

        i += 1;
    }

    if custom_delimiter.is_some() || custom_level_pos.is_some() {
        let delimiter = custom_delimiter.unwrap_or('|');
        let level_pos = custom_level_pos.unwrap_or(1);
        format = LogFormat::custom(delimiter, level_pos);
    }

    println!("Analyzing: {}", file_path.display());
    println!(
        "Format: delimiter='{}', level_position={}",
        format.delimiter, format.level_position
    );

    let summary = if use_parallel {
        println!("Using parallel processing");
        let mut processor =
            ParallelReader::with_format(file_path, 256,  704 * 1024, 256, format);
        time_block!("Parallel processing", { processor.process()? })
    } else {
        println!("Using single-threaded processing\n");
        let mut processor = BufferedLineReader::with_format(file_path, 1024 * 1024, format);
        time_block!("Single-threaded", { processor.process()? })
    };
    summary.display();

    Ok(())
}
