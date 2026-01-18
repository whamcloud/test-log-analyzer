# Log Analyzer Design

## Overview
This program analyzes structured log files of arbitrary size and counts
the number of INFO, WARN, and ERROR log entries. The solution is optimized
for large (multi-GB) files and minimal memory usage.

## File Reading Strategy
The program uses `BufRead::read_until(b'\n')` to stream the input one line
at a time. This ensures:
- No full-file buffering
- Constant memory usage
- Efficient disk I/O

A single reusable buffer is allocated and cleared between reads.

## Parsing Strategy
Each log line is parsed at the byte level (`&[u8]`) instead of converting
to `String`. Fields are extracted by scanning for the `|` delimiter.

This avoids:
- Per-line heap allocation
- Temporary vectors or string copies

Only byte slices referencing the original buffer are used.

## Error Handling
Malformed lines (wrong delimiter count or invalid log level) are counted
and skipped. The analyzer never panics on invalid input.

## Performance Considerations
- O(n) time complexity where n is file size
- Constant memory usage
- Zero-copy parsing
- Suitable for very large log files

## Safety
The implementation uses only safe Rust. No unsafe code is required.
