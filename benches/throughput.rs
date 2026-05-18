use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use ddnn::{Config, LogLine, run_analyzer};
use std::io::Write;

fn bench_parse(c: &mut Criterion) {
    let valid = b"2025-01-01T12:00:00Z|ERROR|auth|invalid token";
    let invalid = b"CORRUPT_BYTES_CRASH_DUMP";

    let mut g = c.benchmark_group("parse_log_line");
    g.bench_function("valid_line", |b| {
        b.iter(|| LogLine::parse(std::hint::black_box(valid)))
    });
    g.bench_function("invalid_line", |b| {
        b.iter(|| LogLine::parse(std::hint::black_box(invalid)))
    });
    g.finish();
}

fn bench_run_analyzer(c: &mut Criterion) {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let line = b"2025-01-01T12:00:00Z|INFO|auth|hello world\n";
    let target = 64 * 1024 * 1024usize;
    let mut written = 0usize;
    while written < target {
        tmp.as_file().write_all(line).unwrap();
        written += line.len();
    }
    let path = tmp.path().to_path_buf();

    let mut g = c.benchmark_group("run_analyzer");
    // it measure throughput in terms of bytes per second
    g.throughput(Throughput::Bytes(written as u64));

    for multiplier in [1usize, 2, 4] {
        g.bench_with_input(
            BenchmarkId::new("core_multiplier", multiplier),
            &multiplier,
            |b, &m| {
                b.iter(|| {
                    run_analyzer(Config {
                        path: path.clone(),
                        core_multiplier: m,
                        buffer_size: 1024 * 1024 * 16,
                        enable_error_reporting: false,
                    })
                    .unwrap()
                })
            },
        );
    }
    g.finish();
}

criterion_group!(benches, bench_parse, bench_run_analyzer);
criterion_main!(benches);
