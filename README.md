# High-Performance Log Analyzer

A production-ready Rust log analyzer built for multi-gigabyte files.

## Architecture

| Concern | Solution |
|---|---|
| Streaming IO | `BufReader` (64 KB OS buffer) + `ChunkReader` iterator (8 MB chunks) |
| Zero-copy parsing | `memchr` SIMD search on `&str` slices — no `String` allocation per line |
| Parallelism | `rayon` `par_bridge` Map-Reduce over chunks |
| Error handling | `thiserror` typed errors; malformed lines counted, never panic |
| CLI | `clap` derive — supports `--help`, `--version`, env-var fallback |
| Logging | `env_logger` — level controlled by `RUST_LOG` |

## Log Format

```
<timestamp>|<level>|<service>|<message>
```

Example:
```
2025-01-01T12:00:00Z|ERROR|auth|invalid token
```

- `timestamp`: ISO-8601
- `level`: `INFO` | `WARN` | `ERROR`
- `service`: alphanumeric
- `message`: free-form text

## Output

```
INFO: 120394
WARN: 23941
ERROR: 4821
MALFORMED: 12
```

## Commands

```bash
# Build release binary
make build

# Analyze a log file
make run FILE=path/to/file.log

# Run all tests
make test

# Run benchmarks
make bench

# Clean build artifacts
make clean
```

## Direct Usage

```bash
# Via argument
RUST_LOG=info ./target/release/log_analyzer path/to/file.log

# Via environment variable
RUST_LOG=info LOG_FILE_PATH=path/to/file.log ./target/release/log_analyzer
```

## Configuration

The analyzer can be configured via command-line arguments, environment variables, or a `.env` file.

| Variable | Purpose | Default |
|---|---|---|
| `LOG_FILE_PATH` | File to analyze (alternative to positional argument) | None |
| `CHUNK_SIZE` | Bytes per parallel worker chunk | `8388608` (8 MB) |
| `READ_BUFFER_SIZE` | Bytes for the OS read buffer | `65536` (64 KB) |
| `RUST_LOG` | Log verbosity (`error`, `warn`, `info`, `off`) | `info` |

**Tuning Performance:** The default `CHUNK_SIZE` of 8 MB is tuned for large files (> 100 MB). For very small files, consider lowering it to improve parallelism. `READ_BUFFER_SIZE` dictates the chunking IO read size.
