# Design Notes — Zero-Copy Log Analyzer

## Overview

This document explains every architectural decision: how the file is read,
how parsing avoids allocation, where allocations are unavoidable, performance
trade-offs, configurable format support, and behavior with very large files.

---

## How the file is read

The file is read using `memmap2::Mmap`, which asks the OS to map the file's
pages directly into the process's virtual address space via `mmap(2)`.

No `read(2)` syscall copies bytes from kernel buffers into a userspace
allocation. The entire file appears to the program as a `&[u8]` slice. Pages
are faulted in on demand and never fully loaded into RAM at once.

### Why not BufReader?

`BufReader<File>` copies bytes into an 8 KB heap buffer, then into a `String`
per line — two copies and one allocation per line. `memmap2` lets us inspect
bytes in-place. For a log analyzer that only needs ~5 bytes per line (the
level field), copying the entire line is wasteful.

### Why not tokio::fs?

`tokio::fs` is for concurrent I/O across many sockets. This program has a
single file and a CPU-bound workload. The OS page cache handles read-ahead;
`rayon` handles parallelism. Adding tokio would spin up a second thread pool
with zero benefit — measurably slower, and architecturally wrong.

### The one unsafe block

`Mmap::map` requires `unsafe` because another process could truncate the file
mid-read, causing a bus error. For log analysis this risk is accepted and
documented here. All code downstream operates on a plain `&[u8]`.

---

## Configurable log format

`LogFormat` carries two fields:

| Field | Type | Meaning |
|---|---|---|
| `delimiter` | `u8` | The byte separating fields (`|`, `,`, ` `, …) |
| `level_position` | `usize` | 0-indexed position of the level field |

The delimiter is stored as `u8` (not `char`) so it can be passed directly to
`memchr::memchr` without conversion — keeping the hot path allocation-free.

Presets: `standard` (`|`, pos 1), `space` (` `, pos 1), `csv` (`,`, pos 1).
Custom formats via `--delimiter` and `--level-pos` CLI flags.

---

## How parsing avoids allocation

All parsing is in `parser::parse_line`, which accepts `&[u8]` and a
`LogFormat` and returns a `ParseOutcome` — no lifetime tying needed since we
only return the level enum, not a slice.

### Delimiter scanning

`memchr::memchr(delimiter, slice)` uses SSE2/AVX2 on x86-64 and NEON on
AArch64 — roughly 5–8× faster than a scalar byte loop. It handles arbitrary
`u8` delimiters, so CSV and space-delimited formats get the same SIMD
acceleration as pipe-delimited ones.

### Level matching

```rust
match level_bytes {
    b"INFO"  => LogLevel::Info,
    b"WARN"  => LogLevel::Warn,
    b"ERROR" => LogLevel::Error,
    _        => return ParseOutcome::Malformed,
}
```

Integer comparisons on `&[u8]`. No `String`, no UTF-8 decode, no allocation.

### Structural validation

A line is only accepted if it has at least `level_position + 2` delimiters —
one before the level, one after (service), and one more (message). A line
like `timestamp|INFO|no-message` is correctly rejected as malformed.

---

## Parallel processing

The mapped buffer is split into `rayon::current_num_threads()` chunks, each
snapped forward to the next `\n` so no line is divided between workers.

Each rayon worker calls `process_chunk`, building a `LogCounts` entirely on
its own stack — zero shared memory during the parallel phase. `reduce()` merges
all per-worker counts via `impl Add for LogCounts`, called O(thread_count)
times at the end.

This is strictly better than `AtomicU64::fetch_add` for this workload:
atomics still cause cache-line sharing at high line rates; reduce does not.

### --threads flag

The optional `--threads N` flag calls `rayon::ThreadPoolBuilder` before
mapping the file. Useful for benchmarking or resource-constrained deployments.

---

## Where allocations are unavoidable

1. `Mmap` struct — fixed-size handle, created once.
2. `Vec<&[u8]>` of chunk pointers — one pointer per thread, created once.
3. Rayon thread pool — created once at startup, reused.
4. `LogCounts` per worker — four `u64`s on each worker's stack (not heap).

**Zero per-line heap allocations in the hot path.**

---

## Performance trade-offs

| Decision | Benefit | Trade-off |
|---|---|---|
| `memmap2` | Zero userspace copies | File truncation mid-read is UB; acceptable for log analysis |
| `rayon` over `tokio` | Correct tool for CPU-bound work | Global thread pool shared with other rayon users |
| `memchr` | SIMD acceleration for any `u8` delimiter | Extra crate dependency |
| `reduce()` over atomics | Zero cache-line contention | O(P) additions at the end — negligible |
| `u8` delimiter in `LogFormat` | Passed directly to `memchr`, no conversion | Non-ASCII delimiters unsupported |
| Empty lines silently skipped | Malformed count reflects real parse errors | Blank lines in log files are invisible in output |

---

## Behavior with very large files

`memmap2` maps virtual address space, not physical RAM. A 64-bit process can
map files larger than available RAM; the OS pages data in and out transparently.

Memory usage stays constant regardless of file size:
- Page cache churn: OS-managed
- Chunk pointer `Vec`: at most `CPU_COUNT` pointers
- `LogCounts` per worker: 32 bytes per thread

Time complexity: **O(N)** in file size, **O(N/P)** wall-clock for P threads.

---

## Assumptions

- LF or CRLF line endings. Both handled.
- Level field is exactly `INFO`, `WARN`, or `ERROR` (case-sensitive).
- Lines may contain extra delimiters in the message field; only the first
  `level_position + 2` delimiters are significant.
- 64-bit platform. On 32-bit, `mmap` is limited to ~2 GB files.

---

## Running

```sh
# Build (optimised)
cargo build --release

# Standard pipe-delimited log
./target/release/log-analyzer server.log

# CSV format
./target/release/log-analyzer server.log --format csv

# Custom delimiter and level position
./target/release/log-analyzer server.log --delimiter ',' --level-pos 2

# Override thread count
./target/release/log-analyzer server.log --threads 4

# Tests
cargo test

# Benchmarks (standard + csv formats)
cargo bench
```
