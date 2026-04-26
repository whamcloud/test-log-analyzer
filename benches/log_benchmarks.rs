use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::fs::File;
use std::io::Write;
use tempfile::NamedTempFile;
use test_log_analyzer::config::{Config, Target};
use test_log_analyzer::log_processor::{
    LogProcessor, ParallelLogProcessor, SequentialLogProcessor,
};

/// Generates a dummy log file for benchmarking
fn create_mock_log_file(lines: usize) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    for i in 0..lines {
        let level = match i % 3 {
            0 => "INFO",
            1 => "WARN",
            _ => "ERROR",
        };
        writeln!(
            file,
            "2026-04-26T15:00:00Z|{}|service_{}|This is a sample log message number {}",
            level,
            i % 5,
            i
        )
        .unwrap();
    }
    file
}

fn bench_log_processors(c: &mut Criterion) {
    let line_count = 100_000;
    let temp_file = create_mock_log_file(line_count);
    let path = temp_file.path().to_str().unwrap().to_string();

    let cfg = Config {
        delimiter: "|".to_string(),
        levels: vec!["INFO".to_string(), "WARN".to_string(), "ERROR".to_string()],
        parallel: None,
        target: Target::Level,
    };

    let mut group = c.benchmark_group("Log Analysis");
    group.sample_size(10); // Processing 100k lines can be slow; reduce sample size if needed

    // Benchmark Sequential Processor
    group.bench_function("Sequential", |b| {
        b.iter(|| {
            let processor = SequentialLogProcessor {
                file_path: &path,
                cfg: &cfg,
            };
            let _ = black_box(processor.process());
        })
    });

    // Benchmark Parallel Processor
    group.bench_function("Parallel", |b| {
        b.iter(|| {
            let processor = ParallelLogProcessor {
                file_path: &path,
                cfg: &cfg,
            };
            let _ = black_box(processor.process());
        })
    });

    group.finish();
}

criterion_group!(benches, bench_log_processors);
criterion_main!(benches);
