# Design Document - Log Analyzer

## Overview

This program analyzes large log files and counts occurrences of INFO, WARN, and ERROR messages. It supports two modes:
1. **Sequential mode** - single-threaded, simple
2. **Parallel mode** - multi-threaded, faster for large files

## Quick Start

### Generate Test Files
```bash
# Small file (1M lines, ~45MB)
cargo run --bin generate_logs 1000000 small 4 4 2

# Large file (300M lines, ~13GB)
cargo run --bin generate_logs 300000000 large 4 4 2
```

### Analyze Files
```bash
# Sequential mode
cargo run --release --bin log-analyzer myfile.txt

# Parallel mode
cargo run --release --bin log-analyzer myfile.txt --parallel

# Custom format (CSV)
cargo run --release --bin log-analyzer myfile.txt --delimiter ',' --parallel
```

### Run Benchmarks
```bash
cargo run --release --bin bench myfile.txt 3
```

### Run Tests
```bash
cargo test
```

## Architecture

### Sequential Processing

Simple line-by-line reading:

```
Open file → Read line → Parse level → Count → Repeat until EOF
```

**Key optimization**: Reuses a single String buffer instead of allocating per line:

```rust
let mut line = String::with_capacity(256);  // Allocate once
loop {
    line.clear();                            // Clear, don't allocate
    reader.read_line(&mut line)?;            // Reuse buffer
    // parse and count...
}
```

### Parallel Processing

Splits file into chunks processed by multiple threads:

```
1. Calculate file size
2. Divide into chunks (~256MB each)
3. Align chunk boundaries to line endings
4. Worker threads pull chunks from shared queue
5. Each thread processes chunks independently
6. Aggregate counts using atomic operations
```

**Critical challenge solved**: Chunk boundaries must align with line endings to avoid splitting lines. The `find_next_line_start()` function searches forward from approximate boundaries to find the next newline.

## Zero-Copy Parsing

Log format: `timestamp|level|service|message`

Instead of copying substrings, we use string slices:

```rust
let parts = line.split('|');     // No allocation, just slices
let level = parts.nth(1)?;       // Points to "INFO" in original line
```

This avoids allocating new Strings for each field.

## Memory Allocations

### Unavoidable Allocations
1. **Line buffer** - one per thread (~256 bytes)
2. **BufReader buffer** - for file I/O (~512KB per thread)
3. **Thread coordination** - Arc/Mutex wrappers, atomic counters

### Memory Usage
- Sequential: ~1MB RAM
- Parallel (12 threads): ~10MB RAM (512KB × 12 + overhead)
- **Constant regardless of file size** - only chunks in memory, not entire file

## Benchmark Results

Tested on: 12-core CPU, SSD storage

### Configuration Tested
```
File: 300M lines (~13GB)
Format: timestamp|level|service|message
Ratios: 40% INFO, 40% WARN, 20% ERROR
Runs: 1 (median of multiple runs)
```

### Performance Results

| Configuration | Time | Speedup vs Sequential |
|---------------|------|----------------------|
| Sequential | 35.743s | 1.00x |
| Parallel (256MB chunks, 512KB buffer) | 23.306s | **1.53x** |
| Parallel (256MB chunks, 768KB buffer) | 23.494s | 1.52x |
| Parallel (512MB chunks, 768KB buffer) | 23.667s | 1.51x |

**Optimal settings**:
- Chunk size: 256 MB
- Read buffer: 512 KB
- Line capacity: 256 bytes
- Threads used: 12

### Why Not 12x Speedup?

With 12 CPU cores, you might expect 12x speedup. We only got 1.53x because:

1. **I/O bottleneck**: Even with SSD, reading 13GB takes time. Disk I/O is slower than CPU processing.
2. **Thread overhead**: Creating threads, coordinating work, and aggregating results adds cost.
3. **Sequential portions**: File metadata, chunk boundary calculation, result aggregation can't be parallelized.

On traditional HDDs, speedup would be even lower due to slower disk speeds.

## Error Handling

The program handles malformed input gracefully:

- **Empty lines** -> counted as malformed
- **Missing fields** -> counted as malformed
- **Unknown log levels** (e.g., DEBUG, TRACE) -> counted as malformed
- **File not found** -> error message, clean exit
- **I/O errors** -> propagated via Result types

No panics on invalid input.

## Testing Strategy

### 1. Unit Tests
Test individual functions with `Cursor` (in-memory):
```rust
let content = b"line1\nline2\n";
let mut cursor = Cursor::new(content);
// Test without file I/O
```

### 2. Integration Tests
Generate files with known counts, verify accuracy:
```bash
cargo run --bin generate_logs 1000000 test 7 2 1
# Creates test_7i_2w_1e_1000000.txt with exact 70% INFO, 20% WARN, 10% ERROR
```

### 3. Boundary Testing
- Empty files
- Files without trailing newlines
- Lines longer than buffer size
- Chunk boundaries at various positions
- Single-line files
- Files with only newlines

### 4. Benchmarking
Measure real performance with various configurations.

## Design Trade-offs

### Sequential vs Parallel

| Aspect | Sequential | Parallel |
|--------|-----------|----------|
| Performance | Slower | 1.5x faster (13GB file) |
| Memory | ~1MB | ~10MB |
| Complexity | Simple | More complex |
| Best for | Files < 100MB | Files > 1GB |

### Buffer Size Selection

Testing showed 512KB is optimal:
- **Too small (< 64KB)**: Excessive system calls
- **Too large (> 2MB)**: Wastes memory, no performance gain
- **Sweet spot (512-768KB)**: Minimizes syscalls, reasonable memory usage

### Work-Stealing Pattern

Threads pull chunks from shared queue rather than static assignment:
- **Better load balancing**: Fast threads get more work
- **Handles variable chunk sizes**: Some chunks may have different processing times
- **Simple synchronization**: `Arc<Mutex<Iterator>>` with lock held <10ns

## Scalability
In my case it was 2MB , but for safety I am considering it to be 10MB.

Memory usage remains **constant** regardless of file size:

| File Size | Memory Used | Reason |
|-----------|-------------|--------|
| 1 MB | ~10 MB | Base + buffers |
| 1 GB | ~10 MB | Only chunks in memory |
| 100 GB | ~10 MB | Same - streaming processing |

Processing time scales linearly with file size (assuming I/O bandwidth remains constant).

## Design Decisions

### Why Standard Library Only?

I used only `std::thread`, `Arc`, `Mutex`, and `AtomicU64` instead of libraries like `rayon` or `crossbeam` to demonstrate understanding of parallel programming fundamentals.

### Why Not Async/Tokio?

This is CPU + I/O intensive work best suited for thread-based parallelism. Async frameworks excel at I/O-bound tasks with many concurrent connections (e.g., web servers), not single-file processing.

### Custom Chunk Boundaries

Aligning chunks to line boundaries prevents counting errors. The `find_next_line_start()` function ensures no line is processed twice or missed.

## Future Improvements

With more time, I would add:

1. **Progress indicator** - show percentage complete for large files
2. **Cached chunk boundaries** - save/reuse boundaries for repeated analysis
3. **Compressed file support** - handle .gz files directly
4. **Memory-mapped I/O** - potentially faster than buffered I/O (out of scope for this assignment)

## Verification

All counts are verified against expected values:

```
Expected: INFO=120000000, WARN=120000000, ERROR=60000000, TOTAL=300000000
Actual:   INFO=120000000, WARN=120000000, ERROR=60000000, TOTAL=300000000
Status:    ALL COUNTS VERIFIED
```

The deterministic log generator ensures test files have exact, known counts for validation.