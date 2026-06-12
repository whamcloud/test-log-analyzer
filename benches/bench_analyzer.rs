use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use log_analyzer::analyzer::analyze;
use log_analyzer::format::LogFormat;

fn generate_log(n_lines: usize, format: LogFormat) -> Vec<u8> {
    let delim = format.delimiter as char;
    let templates = [
        format!("2025-01-01T12:00:00Z{d}INFO{d}auth{d}user logged in\n",  d = delim),
        format!("2025-01-01T12:00:01Z{d}WARN{d}storage{d}disk at 85%\n",  d = delim),
        format!("2025-01-01T12:00:02Z{d}ERROR{d}auth{d}invalid token\n",   d = delim),
    ];
    let mut buf = Vec::with_capacity(n_lines * 60);
    for i in 0..n_lines {
        buf.extend_from_slice(templates[i % 3].as_bytes());
    }
    buf
}

fn bench_standard(c: &mut Criterion) {
    let mut group = c.benchmark_group("analyze_standard");
    for &n in &[10_000usize, 100_000, 1_000_000] {
        let data = generate_log(n, LogFormat::standard());
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_with_input(BenchmarkId::new("parallel", n), &data, |b, d| {
            b.iter(|| analyze(black_box(d), LogFormat::standard()))
        });
    }
    group.finish();
}

fn bench_csv(c: &mut Criterion) {
    let mut group = c.benchmark_group("analyze_csv");
    for &n in &[100_000usize, 1_000_000] {
        let data = generate_log(n, LogFormat::csv());
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_with_input(BenchmarkId::new("parallel", n), &data, |b, d| {
            b.iter(|| analyze(black_box(d), LogFormat::csv()))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_standard, bench_csv);
criterion_main!(benches);
