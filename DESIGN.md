# Log Analyzer Design

## Overview

This log analyzer processes large log files and counts log levels (`INFO`, `WARN`, `ERROR`, and malformed lines).

The primary goals are:

- High performance on large files
- Correct handling of line boundaries
- Scalable parallel processing
- Minimal and predictable memory usage

The solution is implemented in Rust using standard library primitives only.

---

## High-Level Approach

The file is processed in **byte-range chunks**, where each chunk is handled independently by a worker thread.

Each worker:
- Reads only its assigned byte range
- Parses complete log lines
- Produces local counts

The main thread aggregates all per-thread results.

**Key idea:**  
Parallelize by **file offsets**, not by lines, while carefully handling chunk boundaries.

---

## How the File Is Read and Processed

- The total file size is obtained using filesystem metadata.
- The file is divided into evenly sized byte ranges.
- Each thread:
  - Opens the file independently
  - Seeks to its assigned start offset
  - Reads sequentially using a buffered reader

There is **no shared file handle** and **no shared mutable state** during parsing.

---

## Threading Model

- The number of threads is determined by:
  - Available CPU cores
  - A hard cap of 4 threads
  - File size (small files fall back to single-threaded execution)

This avoids unnecessary thread overhead for small inputs and limits I/O contention on large files.

---

## File Chunking Strategy

- Each thread is assigned a `[start, end)` byte range.
- Chunk size is computed as:
chunk_size = file_size / thread_count

- The final thread consumes any remaining bytes.

Threads operate fully independently.

---

## Line Boundary Handling

Because chunk boundaries may fall in the middle of a log line, care is taken to ensure that each log line is processed exactly once.

For any chunk that does not start at the beginning of the file, the reader first advances to the next newline character before beginning parsing. This ensures that the thread always starts processing from a complete log line boundary.

---

## Parsing Logic (Zero-Copy)

- Each thread reads data using `BufReader::read_until(b'\n')`
- Parsing operates directly on the byte buffer (`&[u8]`)
- Log level extraction is done via byte scanning, not string splitting

This avoids:
- UTF-8 validation
- String allocations
- Intermediate parsing structures

### Log Classification

INFO → info += 1
WARN → warn += 1
ERROR → error += 1
else → malformed += 1


Malformed lines include:
- Invalid format
- Unknown log levels
- Empty or corrupted lines

---

## Allocations: Avoided vs Unavoidable

### Avoided
- No per-line string allocations
- No shared data structures between threads
- No intermediate parsing objects

### Unavoidable
- A reusable per-thread buffer to hold the current log line
- One `LogCounts` struct per thread

Memory usage is bounded per thread and depends only on the maximum line length, not on total file size.

---

## Performance Characteristics

- Single-threaded: Comparable to an optimized sequential parser
- Multi-threaded: Near-linear speedup for large files
- Memory usage: Bounded per thread, proportional only to the maximum line length
- I/O pattern: Sequential reads per thread, no random access during parsing

---

## Behavior With Very Large Files

- File size does not affect memory usage
- Threads process disjoint byte ranges independently
- Performance scales with available CPU cores until limited by disk throughput
- No global locks or synchronization during parsing

This makes the solution suitable for multi-gigabyte log files.

---

## Performance Trade-offs

- Thread count is capped to reduce disk contention
- Files below a size threshold are processed single-threaded
- The design favors sequential disk access over aggressive parallel reads

These choices prioritize real-world throughput and predictability over theoretical maximum parallelism.

---

## Limitations

- Assumes individual log lines are reasonably sized and newline-delimited, as is typical for production logs.

---

## Conclusion

This design provides a fast, safe, and scalable log analyzer with careful handling of edge cases.

It achieves parallelism without excessive memory usage, avoids unnecessary allocations, and maintains correctness even at chunk boundaries, making it suitable for production use on large log files.

---