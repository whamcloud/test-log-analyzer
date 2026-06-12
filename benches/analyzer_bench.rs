use criterion::{Criterion, criterion_group, criterion_main};
use log_analyzer::parser::parse_log_level;

/// Benchmarks the hot-path parser on a single valid log line.
///
/// `memchr` uses SIMD intrinsics so the result should be in the single-digit
/// nanosecond range on modern hardware — effectively free per line.
fn bench_parse_valid_line(c: &mut Criterion) {
    let line = "2025-01-01T12:00:00Z|ERROR|auth|invalid token";
    c.bench_function("parse_log_level / valid line", |b| {
        b.iter(|| {
            let _ = parse_log_level(line);
        });
    });
}

/// Benchmarks the parser on a malformed line to confirm early-exit is fast.
fn bench_parse_malformed_line(c: &mut Criterion) {
    let line = "this-line-has-no-pipes-at-all";
    c.bench_function("parse_log_level / malformed line", |b| {
        b.iter(|| {
            let _ = parse_log_level(line);
        });
    });
}

criterion_group!(benches, bench_parse_valid_line, bench_parse_malformed_line);
criterion_main!(benches);
