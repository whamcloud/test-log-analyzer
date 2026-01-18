use std::env;
use std::path::PathBuf;
use std::time::Instant;

use log_analyzer::error::LogError;
use log_analyzer::log_format::LogFormat;
use log_analyzer::log_summary::LogSummary;
use log_analyzer::processor::LogProcessor;
use log_analyzer::processor::buffered_line_reader::BufferedLineReader;
use log_analyzer::processor::parallel_reader::ParallelReader;

struct BenchConfig {
    chunk_size_mb: u64,
    read_buffer_kb: usize,
    line_capacity: usize,
}

struct BenchResult {
    config: BenchConfig,
    time_ms: u128,
    threads_used: usize,
}

#[derive(Debug)]
struct ExpectedCounts {
    info: u64,
    warn: u64,
    error: u64,
    total: u64,
}

/// Parse filename to extract expected counts
/// Format: basename_<info>i_<warn>w_<error>e_<lines>[_format].txt
/// Example: test_7i_2w_1e_100000000.txt or logs_70i_20w_10e_1000000_space.txt
fn parse_filename_for_counts(filename: &str) -> Option<ExpectedCounts> {
    // Remove .txt extension
    let name = filename.strip_suffix(".txt")?;

    // Split by underscores
    let parts: Vec<&str> = name.split('_').collect();

    // Need at least: basename, Xi, Yw, Ze, lines
    if parts.len() < 5 {
        return None;
    }

    // Find the pattern: Xi_Yw_Ze_lines
    let mut info_ratio = None;
    let mut warn_ratio = None;
    let mut error_ratio = None;
    let mut total_lines = None;

    for part in parts {
        if part.ends_with('i') && info_ratio.is_none() {
            info_ratio = part.strip_suffix('i')?.parse::<u64>().ok();
        } else if part.ends_with('w') && warn_ratio.is_none() {
            warn_ratio = part.strip_suffix('w')?.parse::<u64>().ok();
        } else if part.ends_with('e') && error_ratio.is_none() {
            error_ratio = part.strip_suffix('e')?.parse::<u64>().ok();
        } else if info_ratio.is_some() && warn_ratio.is_some() && error_ratio.is_some() {
            // Next numeric part should be total lines
            if let Ok(lines) = part.parse::<u64>() {
                total_lines = Some(lines);
                break;
            }
        }
    }

    let info_ratio = info_ratio?;
    let warn_ratio = warn_ratio?;
    let error_ratio = error_ratio?;
    let total_lines = total_lines?;

    let total_ratio = info_ratio + warn_ratio + error_ratio;
    let chunk_size = total_lines / total_ratio;

    Some(ExpectedCounts {
        info: chunk_size * info_ratio,
        warn: chunk_size * warn_ratio,
        error: chunk_size * error_ratio,
        total: total_lines,
    })
}

/// Detect format from filename
/// Looks for _space, _csv, _custom suffix before .txt
fn detect_format_from_filename(filename: &str) -> LogFormat {
    if filename.contains("_space.txt") {
        LogFormat::space_delimited()
    } else if filename.contains("_csv.txt") {
        LogFormat::custom(',', 1)
    } else if filename.contains("_custom.txt") {
        // Default to pipe, user should specify via CLI if different
        LogFormat::standard()
    } else {
        LogFormat::standard()
    }
}

fn verify_counts(summary: &LogSummary, expected: &ExpectedCounts) {
    println!("===============================================================");
    println!("                     COUNT VERIFICATION                        ");
    println!("===============================================================");

    let info_match = summary.info == expected.info;
    let warn_match = summary.warn == expected.warn;
    let error_match = summary.error == expected.error;
    let total_match = summary.total() == expected.total;

    println!("  Level  |  Expected   |   Actual    |  Status                |");
    println!("===============================================================");
    println!(
        "| INFO   │ {:>11} │ {:>11} │ {:^22} |",
        expected.info,
        summary.info,
        if info_match { "PASS" } else { "FAIL" }
    );
    println!(
        "| WARN   | {:>11} | {:>11} | {:^22} |",
        expected.warn,
        summary.warn,
        if warn_match { "PASS" } else { "FAIL" }
    );
    println!(
        "| ERROR  | {:>11} | {:>11} | {:^22} |",
        expected.error,
        summary.error,
        if error_match { "PASS" } else { "FAIL" }
    );
    println!("===============================================================");
    println!(
        "| TOTAL  | {:>11} | {:>11} | {:^22} |",
        expected.total,
        summary.total(),
        if total_match { "PASS" } else { "FAIL" }
    );
    println!("===============================================================");

    if info_match && warn_match && error_match && total_match {
        println!("ALL COUNTS VERIFIED! Analyzer is working correctly.");
    } else {
        eprintln!("COUNT MISMATCH! Analyzer has bugs or wrong format used.");
        std::process::exit(1);
    }
}

fn run_benchmark(
    path: &PathBuf,
    config: BenchConfig,
    format: LogFormat,
    runs: usize,
) -> Result<BenchResult, LogError> {
    let mut times = Vec::new();
    let mut threads_used = 0;

    for run in 0..runs {
        let mut processor = ParallelReader::with_format(
            path.clone(),
            config.chunk_size_mb,
            config.read_buffer_kb * 1024,
            config.line_capacity,
            format,
        );

        let start = Instant::now();
        let _ = processor.process()?;
        let elapsed = start.elapsed();

        times.push(elapsed.as_millis());

        // Extract metadata from first run
        if run == 0 {
            let file = std::fs::File::open(path)?;
            let total_size = file.metadata()?.len();
            let chunk_size = config.chunk_size_mb*1024*1024;
            let num_chunks = total_size.div_ceil(chunk_size);
            let chunks_created = num_chunks as usize;
            let num_cpus = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(12);
            threads_used = chunks_created.min(num_cpus);
        }
    }

    // Use median to avoid outliers
    times.sort();
    let median_time = times[times.len() / 2];

    Ok(BenchResult {
        config,
        time_ms: median_time,
        threads_used,
    })
}

fn main() -> Result<(), LogError> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!(
            "Usage: {} <log_file> [runs_per_config] [--delimiter CHAR] [--level-pos POS]",
            args[0]
        );
        eprintln!("\nExamples:");
        eprintln!("  {} test_7i_2w_1e_100000000.txt 3", args[0]);
        eprintln!("  {} logs_70i_20w_10e_1000000_csv.txt 3", args[0]);
        eprintln!(
            "  {} custom_7i_2w_1e_5000000_custom.txt 3 --delimiter ';'",
            args[0]
        );
        std::process::exit(1);
    }

    let file_path = PathBuf::from(&args[1]);
    let runs = args
        .get(2)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(3);

    if !file_path.exists() {
        eprintln!("Error: File '{}' does not exist", file_path.display());
        std::process::exit(1);
    }

    // Parse format options from CLI
    let mut custom_delimiter: Option<char> = None;
    let mut custom_level_pos: Option<usize> = None;

    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--delimiter" => {
                if i + 1 < args.len() && args[i + 1].len() == 1 {
                    custom_delimiter = args[i + 1].chars().next();
                    i += 2;
                } else {
                    eprintln!("--delimiter requires a single character");
                    std::process::exit(1);
                }
            }
            "--level-pos" => {
                if i + 1 < args.len() {
                    custom_level_pos = args[i + 1].parse().ok();
                    i += 2;
                } else {
                    eprintln!("--level-pos requires a number");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                std::process::exit(1);
            }
        }
    }

    let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    let format = if let Some(delim) = custom_delimiter {
        let level_pos = custom_level_pos.unwrap_or(1);
        LogFormat::custom(delim, level_pos)
    } else {
        detect_format_from_filename(filename)
    };

    // Try to parse expected counts from filename
    let expected_counts = parse_filename_for_counts(filename);

    println!("________________________________________________________________");
    println!("|              LOG ANALYZER BENCHMARK SUITE                     |");
    println!("|_______________________________________________________________|");
    println!("File: {}", file_path.display());
    println!("Runs per config: {}", runs);
    println!(
        "Format: delimiter='{}', level_position={}",
        format.delimiter, format.level_position
    );

    if let Some(ref expected) = expected_counts {
        println!("\nExpected counts (from filename):");
        println!("  INFO:  {}", expected.info);
        println!("  WARN:  {}", expected.warn);
        println!("  ERROR: {}", expected.error);
        println!("  TOTAL: {}", expected.total);
    }
    println!();

    // Baseline: Sequential with format
    println!("Running sequential baseline...");
    let seq_times: Vec<u128> = (0..runs)
        .map(|_| {
            let mut processor =
                BufferedLineReader::with_format(file_path.clone(), 768 * 1024, format);
            let start = Instant::now();
            let _ = processor.process().unwrap();
            start.elapsed().as_millis()
        })
        .collect();
    let mut seq_sorted = seq_times.clone();
    seq_sorted.sort();
    let seq_median = seq_sorted[seq_sorted.len() / 2];
    println!("Sequential: {:.3}s (median)\n", seq_median as f64 / 1000.0);

    let configs = vec![
        BenchConfig {
            chunk_size_mb: 256,
            read_buffer_kb: 704,
            line_capacity: 256,
        },
        BenchConfig {
            chunk_size_mb: 256,
            read_buffer_kb: 768,
            line_capacity: 256,
        },
        BenchConfig {
            chunk_size_mb: 512,
            read_buffer_kb: 768,
            line_capacity: 256,
        },
        BenchConfig {
            chunk_size_mb: 256,
            read_buffer_kb: 512,
            line_capacity: 256,
        },
        BenchConfig {
            chunk_size_mb: 1024,
            read_buffer_kb: 768,
            line_capacity: 256,
        },
        BenchConfig {
            chunk_size_mb: 2*1024,
            read_buffer_kb: 768,
            line_capacity: 256,
        },
    ];

    println!("Running parallel benchmarks...\n");

    let mut results = Vec::new();
    let mut verification_summary: Option<LogSummary> = None;

    for (i, config) in configs.iter().enumerate() {
        print!(
            "[{}/{}] Testing chunk={}MB, buf={}KB, line={}... ",
            i + 1,
            configs.len(),
            config.chunk_size_mb,
            config.read_buffer_kb,
            config.line_capacity
        );

        match run_benchmark(
            &file_path,
            BenchConfig {
                chunk_size_mb: config.chunk_size_mb,
                read_buffer_kb: config.read_buffer_kb,
                line_capacity: config.line_capacity,
            },
            format,
            runs,
        ) {
            Ok(result) => {
                println!("{:.3}s", result.time_ms as f64 / 1000.0);
                results.push(result);
            }
            Err(e) => {
                println!("ERROR: {:?}", e);
            }
        }

        if i == 0 && verification_summary.is_none() {
            let mut processor = ParallelReader::with_format(
                file_path.clone(),
                config.chunk_size_mb,
                config.read_buffer_kb * 1024,
                config.line_capacity,
                format,
            );
            if let Ok(summary) = processor.process() {
                verification_summary = Some(summary);
            }
        }
    }

    // Sort by performance
    results.sort_by_key(|r| r.time_ms);

    println!("===============================================================");
    println!("|                      BENCHMARK RESULTS                       |");
    println!("|==============================================================|");
    println!("| Rank | Chunk  | Buffer | Line | Threads | Time   | Speedup   |");
    println!("|==============================================================|");

    for (rank, result) in results.iter().enumerate() {
        let speedup = seq_median as f64 / result.time_ms as f64;
        println!(
            "| {:^4} | {:>5}M | {:>5}K | {:>4} | {:>7} | {:>5.2}s | {:>8.2}x |",
            rank + 1,
            result.config.chunk_size_mb,
            result.config.read_buffer_kb,
            result.config.line_capacity,
            result.threads_used,
            result.time_ms as f64 / 1000.0,
            speedup
        );
    }

    println!("|==============================================================|");
    println!(
        "| Sequential baseline: {:.3}s                                  |",
        seq_median as f64 / 1000.0
    );
    println!("|==============================================================|");

    // Show optimal configuration
    if let Some(best) = results.first() {
        println!("   OPTIMAL CONFIGURATION:");
        println!("   Chunk size:     {} MiB", best.config.chunk_size_mb);
        println!("   Read buffer:    {} KiB", best.config.read_buffer_kb);
        println!("   Line capacity:  {}", best.config.line_capacity);
        println!("   Threads used:   {}", best.threads_used);
        println!(
            "   Performance:    {:.3}s ({:.2}x speedup)",
            best.time_ms as f64 / 1000.0,
            seq_median as f64 / best.time_ms as f64
        );
    }

    // Verify counts if we have expected values
    if let (Some(expected), Some(actual)) = (expected_counts, verification_summary) {
        verify_counts(&actual, &expected);
    } else {
        println!(" No expected counts found in filename - skipping verification");
        println!("  Use filename format: basename_Xi_Yw_Ze_lines.txt");
        println!("  Example: test_7i_2w_1e_100000000.txt");
    }

    Ok(())
}
