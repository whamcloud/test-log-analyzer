# Design notes — simple streaming log analyzer

Overview
--------
This program reads a structured log file line-by-line and counts INFO / WARN / ERROR occurrences.
Goal: keep the code simple, safe, and able to handle large files without loading them into memory.

What the code does
------------------
- Opens the file and uses `BufReader::with_capacity(128 * 1024, file)` for larger buffered IO.
- Uses `read_until(b'\n', &mut buf)` with a reusable `Vec<u8>` to avoid allocating a new `String` per line.
- Converts the byte slice to `&str` with `std::str::from_utf8` (invalid UTF-8 is counted as malformed).
- `parse_level(&str)` does simple parsing with `splitn(4, '|')`, trims the level, rejects control/non-ASCII chars, and matches levels case-insensitively.
- `LineErr<'a>` borrows the offending level `&str` (no allocation) for `UnknownLevel`.
- Counts valid levels and malformed lines; prints a summary at the end.

Why this approach
-----------------
- Streaming with a single reusable buffer keeps memory usage small even for multi-GB files.
- Using `&str` slices where possible avoids extra allocations and keeps code readable.
- The design favors correctness and clarity 

Testing
-----------
- Created a large file using python scrips & tested the code for 1 GB file size. 

How to run
----------
- Build: `cargo build --release`
- Run: `cargo run --release`
- Tests: `cargo test`
