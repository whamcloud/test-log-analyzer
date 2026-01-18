use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

fn print_usage(program: &str) {
    eprintln!(
        "Usage: {} <num_lines> <base_name> <info_ratio> <warn_ratio> <error_ratio> [OPTIONS]",
        program
    );
    eprintln!("\nOptions:");
    eprintln!("  --format <preset>         Format preset: standard|space|csv");
    eprintln!("  --delimiter <char>        Custom delimiter (e.g., '|', ',', ' ', ';')");
    eprintln!("  --level-pos <index>       Position of level field (0-indexed)");
    eprintln!(
        "  --output-dir <path>       Output directory (default: /home/anchalshivank/Documents)"
    );
    eprintln!("\nExamples:");
    eprintln!("  Standard format (saved to ~/Documents):");
    eprintln!("    {} 1000000 logs 70 20 10", program);
    eprintln!("    Output: /home/anchalshivank/Documents/logs_70i_20w_10e_1000000.txt");
    eprintln!();
    eprintln!("  Space-delimited format:");
    eprintln!("    {} 1000000 logs 7 2 1 --format space", program);
    eprintln!("    Output: /home/anchalshivank/Documents/logs_7i_2w_1e_1000000_space.txt");
    eprintln!();
    eprintln!("  CSV format with custom output directory:");
    eprintln!(
        "    {} 1000000 logs 7 2 1 --format csv --output-dir /tmp",
        program
    );
    eprintln!("    Output: /tmp/logs_7i_2w_1e_1000000_csv.txt");
    eprintln!();
    eprintln!("  Custom delimiter:");
    eprintln!("    {} 1000000 logs 7 2 1 --delimiter ';'", program);
    eprintln!("    Output: /home/anchalshivank/Documents/logs_7i_2w_1e_1000000_custom.txt");
    eprintln!();
    eprintln!("Note: num_lines must be divisible by (info_ratio + warn_ratio + error_ratio)");
}

#[derive(Debug, Clone)]
enum LogFormat {
    Standard, // timestamp|level|service|message
    Space,    // timestamp level service message
    Csv,      // timestamp,level,service,message
    Custom { delimiter: char, level_pos: usize },
}

impl LogFormat {
    fn format_line(&self, timestamp: &str, level: &str, service: &str, message: &str) -> String {
        match self {
            LogFormat::Standard => {
                format!("{}|{}|{}|{}", timestamp, level, service, message)
            }
            LogFormat::Space => {
                format!("{} {} {} {}", timestamp, level, service, message)
            }
            LogFormat::Csv => {
                format!("{},{},{},{}", timestamp, level, service, message)
            }
            LogFormat::Custom {
                delimiter,
                level_pos,
            } => {
                let fields = ["timestamp", "service", "message"];
                let mut output = Vec::new();

                for (i, &field) in fields.iter().enumerate() {
                    if i == *level_pos {
                        output.push(level.to_string());
                    }

                    let value = match field {
                        "timestamp" => timestamp,
                        "service" => service,
                        "message" => message,
                        _ => "",
                    };
                    output.push(value.to_string());
                }

                // Handle case where level is at the end
                if *level_pos >= fields.len() {
                    output.push(level.to_string());
                }

                output.join(&delimiter.to_string())
            }
        }
    }

    fn suffix(&self) -> &str {
        match self {
            LogFormat::Standard => "",
            LogFormat::Space => "_space",
            LogFormat::Csv => "_csv",
            LogFormat::Custom { .. } => "_custom",
        }
    }

    fn delimiter(&self) -> char {
        match self {
            LogFormat::Standard => '|',
            LogFormat::Space => ' ',
            LogFormat::Csv => ',',
            LogFormat::Custom { delimiter, .. } => *delimiter,
        }
    }

    fn level_position(&self) -> usize {
        match self {
            LogFormat::Standard | LogFormat::Space | LogFormat::Csv => 1,
            LogFormat::Custom { level_pos, .. } => *level_pos,
        }
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 6 {
        print_usage(&args[0]);
        std::process::exit(1);
    }

    let num_lines: usize = args[1].parse().expect("Invalid number of lines");
    let base_name = &args[2];
    let info_ratio: usize = args[3].parse().expect("Invalid info ratio");
    let warn_ratio: usize = args[4].parse().expect("Invalid warn ratio");
    let error_ratio: usize = args[5].parse().expect("Invalid error ratio");

    // Default output directory
    let mut output_dir = PathBuf::from("/home/anchalshivank/Documents");

    // Parse format options
    let mut format = LogFormat::Standard;
    let mut i = 6;
    while i < args.len() {
        match args[i].as_str() {
            "--format" => {
                if i + 1 < args.len() {
                    format = match args[i + 1].as_str() {
                        "standard" => LogFormat::Standard,
                        "space" => LogFormat::Space,
                        "csv" => LogFormat::Csv,
                        other => {
                            eprintln!("Unknown format: {}", other);
                            print_usage(&args[0]);
                            std::process::exit(1);
                        }
                    };
                    i += 2;
                } else {
                    eprintln!("--format requires an argument");
                    std::process::exit(1);
                }
            }
            "--delimiter" => {
                if i + 1 < args.len() {
                    let delim_str = &args[i + 1];
                    if delim_str.len() == 1 {
                        let delimiter = delim_str.chars().next().unwrap();
                        format = LogFormat::Custom {
                            delimiter,
                            level_pos: 1, // default
                        };
                        i += 2;
                    } else {
                        eprintln!("Delimiter must be a single character");
                        std::process::exit(1);
                    }
                } else {
                    eprintln!("--delimiter requires an argument");
                    std::process::exit(1);
                }
            }
            "--level-pos" => {
                if i + 1 < args.len() {
                    let level_pos: usize = args[i + 1].parse().expect("Invalid level position");
                    if let LogFormat::Custom { delimiter, .. } = format {
                        format = LogFormat::Custom {
                            delimiter,
                            level_pos,
                        };
                    } else {
                        format = LogFormat::Custom {
                            delimiter: '|',
                            level_pos,
                        };
                    }
                    i += 2;
                } else {
                    eprintln!("--level-pos requires an argument");
                    std::process::exit(1);
                }
            }
            "--output-dir" => {
                if i + 1 < args.len() {
                    output_dir = PathBuf::from(&args[i + 1]);
                    i += 2;
                } else {
                    eprintln!("--output-dir requires an argument");
                    std::process::exit(1);
                }
            }
            "--help" | "-h" => {
                print_usage(&args[0]);
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                print_usage(&args[0]);
                std::process::exit(1);
            }
        }
    }

    // Ensure output directory exists
    if !output_dir.exists() {
        eprintln!(
            "ERROR: Output directory does not exist: {}",
            output_dir.display()
        );
        eprintln!("Please create it first or specify a different directory with --output-dir");
        std::process::exit(1);
    }

    let total_ratio = info_ratio + warn_ratio + error_ratio;

    // Validate that num_lines is divisible by total_ratio
    if !num_lines.is_multiple_of(total_ratio) {
        eprintln!(
            "ERROR: num_lines ({}) must be divisible by sum of ratios ({})",
            num_lines, total_ratio
        );
        eprintln!(
            "Suggestion: Use {} lines instead",
            (num_lines / total_ratio) * total_ratio
        );
        std::process::exit(1);
    }

    // Create output filename with format suffix
    let filename = format!(
        "{}_{}i_{}w_{}e_{}{}.txt",
        base_name,
        info_ratio,
        warn_ratio,
        error_ratio,
        num_lines,
        format.suffix()
    );

    let output_path = output_dir.join(&filename);

    // Calculate exact counts
    let chunk_size = num_lines / total_ratio;
    let info_count = chunk_size * info_ratio;
    let warn_count = chunk_size * warn_ratio;
    let error_count = chunk_size * error_ratio;

    println!("          DETERMINISTIC LOG GENERATOR");
    println!("\nOutput directory: {}", output_dir.display());
    println!("Output file: {}", filename);
    println!("Full path: {}", output_path.display());
    println!("Total lines: {}", num_lines);
    println!("\nFormat configuration:");
    println!("  Delimiter: '{}'", format.delimiter());
    println!("  Level position: {}", format.level_position());
    println!(
        "\nRatio configuration: {}:{}:{}",
        info_ratio, warn_ratio, error_ratio
    );
    println!("Percentages:");
    println!("  INFO:  {}%", info_ratio * 100 / total_ratio);
    println!("  WARN:  {}%", warn_ratio * 100 / total_ratio);
    println!("  ERROR: {}%", error_ratio * 100 / total_ratio);
    println!("\nExpected exact counts:");
    println!("  INFO:  {}", info_count);
    println!("  WARN:  {}", warn_count);
    println!("  ERROR: {}", error_count);
    println!("  TOTAL: {}", num_lines);
    println!("\nGenerating...\n");

    let file = File::create(&output_path)?;
    let mut writer = BufWriter::new(file);

    let services = ["auth", "api", "db", "cache", "worker"];
    let messages = [
        "request completed",
        "connection timeout",
        "invalid token",
        "query slow",
        "cache miss",
        "rate limit exceeded",
    ];

    let mut actual_info = 0;
    let mut actual_warn = 0;
    let mut actual_error = 0;

    // Generate logs in a repeating pattern based on ratios
    for i in 0..num_lines {
        let timestamp = format!(
            "2025-01-{:02}T{:02}:{:02}:{:02}Z",
            (i / 86400) % 28 + 1,
            (i / 3600) % 24,
            (i / 60) % 60,
            i % 60
        );

        // Deterministic level assignment based on position in pattern
        let position_in_pattern = i % total_ratio;
        let level = if position_in_pattern < info_ratio {
            actual_info += 1;
            "INFO"
        } else if position_in_pattern < info_ratio + warn_ratio {
            actual_warn += 1;
            "WARN"
        } else {
            actual_error += 1;
            "ERROR"
        };

        // Vary service and message based on line number (still deterministic)
        let service = services[i % services.len()];
        let message = messages[(i / services.len()) % messages.len()];

        // Format line according to selected format
        let line = format.format_line(&timestamp, level, service, message);
        writeln!(writer, "{}", line)?;

        if i > 0 && i % 1_000_000 == 0 {
            println!("  Progress: {}M lines written...", i / 1_000_000);
        }
    }

    writer.flush()?;

    // Get file size
    let metadata = std::fs::metadata(&output_path)?;
    let file_size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
    let file_size_gb = file_size_mb / 1024.0;

    println!("|=========================================================|");
    println!("|                  GENERATION COMPLETE                    |");
    println!("|=========================================================|");
    println!("\nFile: {}", output_path.display());
    if file_size_gb >= 1.0 {
        println!("Size: {:.2} GiB ({:.2} MB)", file_size_gb, file_size_mb);
    } else {
        println!("Size: {:.2} MB", file_size_mb);
    }
    println!(
        "Format: delimiter='{}', level_position={}",
        format.delimiter(),
        format.level_position()
    );
    println!("\nActual counts (for verification):");
    println!("  INFO:  {}", actual_info);
    println!("  WARN:  {}", actual_warn);
    println!("  ERROR: {}", actual_error);
    println!("  TOTAL: {}", actual_info + actual_warn + actual_error);

    // Verify counts match expected
    assert_eq!(actual_info, info_count, "INFO count mismatch!");
    assert_eq!(actual_warn, warn_count, "WARN count mismatch!");
    assert_eq!(actual_error, error_count, "ERROR count mismatch!");

    println!("\nVerification passed! All counts are exact.");

    // Print command to analyze the file
    println!("\nTo analyze this file:");
    let analyze_cmd = match format {
        LogFormat::Standard => {
            format!(
                "cargo run --release --bin log-analyzer {} --parallel \n or cargo run --release --bin bench {} 3",
                output_path.display(), output_path.display()
            )
        }
        LogFormat::Space => {
            format!(
                "cargo run --release --bin log-analyzer {} --format space --parallel \n or cargo run --release --bin bench {} 3",
                output_path.display(), output_path.display()
            )
        }
        LogFormat::Csv => {
            format!(
                "cargo run --release --bin log-analyzer {} --delimiter ',' --parallel \n or cargo run --release --bin bench {} 3",
                output_path.display(), output_path.display()
            )
        }
        LogFormat::Custom {
            delimiter,
            level_pos,
        } => {
            format!(
                "cargo run --release --bin log-analyzer {} --delimiter '{}' --level-pos {} --parallel \n or cargo run --release --bin bench {} 3",
                output_path.display(),
                delimiter,
                level_pos,
                output_path.display()
            )
        }
    };
    println!("  {}", analyze_cmd);

    Ok(())
}
