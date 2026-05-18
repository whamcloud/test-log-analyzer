# Design & Reasoning: High Performance Log Analyzer

This document outlines the design, architecture, and performance characteristics of the `ddnn` log analyzer.

## Code Structure

`src/main.rs` is the entry point for the application.
`src/file_ops.rs` handles file operations and worker management.
`src/types.rs` defines the types used in the application.
`benches/benchmark.rs` is the benchmark code.

## Architecture

Given the constraints to use safe rust(eliminating mmap as an option) and to read a huge file, I am splitting the file into chunks and processing them in parellel using worker threads.

`ChunkWorker` - A worker having start_offset and end_offset of a file to process

** The execution is split into 3 parts: **

1. Chunking the file: `chunk_file` is used to calculate the offset range for each `ChunkWorker` to process
2. Splitting the task:
    - For each core id(logical core in the machine), we spawn a thread
    - ChunkWorkers calculated in step 1 are sent to a crossbeam unbounded channel Sender
    - Each thread will recv the ChunkWorker from the channel and process it
3. Aggregrating the result: After all the threads are done processing, we aggregate the results from each thread and return the final result

Using crossbeam channel since we need single producer multi consumer channel. Alternatively
we could use n workers for n threads for n cores, and assign a large chunk to each worker. In this case it would be sub optimal as that would require large mem allocation for buffer for each worker. (And block a possible optimization where a thread can process mutliple chunks while waiting for I/O)

## Low Overhead for Parsing

Parsing avoids all `String` allocations by operating entirely on byte slices (`&[u8]`).

1. **Buffered Reading:** Using a `BufReader` with a highly optimized buffer size (default 100MB).
2. **Byte Slicing:** The `parse_log_line` function scans the raw bytes. It uses `.split(|&b| b == b'|')` to step through the pipe delimiters.
3. **Enum Mapping:** The extracted level slice (e.g., `b"ERROR"`) is pattern matched directly to a `LogLevel` variant.
4. **No UTF-8 Overhead:**: Avoid using `String::from_utf8` checks entirely unless emitting an error message and use byte matching instead.

---

## Memory Allocations & Trade-offs

**Where allocations are unavoidable:**
- **The `BufReader` buffer:** A large contiguous block (e.g., 100MB) is allocated per worker. This is a deliberate trade-off: we trade a fixed, predictable amount of memory for a drastic reduction in I/O system calls.
- **The line buffer (`Vec<u8>`):** Each `ChunkedWorker` holds exactly one `Vec<u8>` to buffer the *current* line and reused for each iteration.

**Performance Trade-offs:**
- **Manual Buffer vs `mmap`:** Using buffered reads rather than memory-mapped files. Well, in rust, using mmap is unsafe, and since we are doing sequential reads, the OS page caching would work in our favour.
- **Thread Count:** spawning 4 workers per physical core (`core_ids.len() * 4`). This slightly oversubscribes the CPU to hide disk I/O latency.
- **Channel vs direct worker assignment:** Using a channel to distribute work to workers. This allows a worker to process multiple chunks if it finishes early, improving load balancing. Each thread is additionally pinned to a core, reducing context switches
- **Error Reporting:** Using a lightweight error struct and not `anyhow::anyhow!` to create an error when parsing fails. All workers have a `tx(mpsc::channel)` which is collected by an error handler (a dedicated thread) which consumes the message and writes to an output temp file.
 ---

## Performance & Benchmark Results

For measuring performance and throughput, I have generated a large file(~15 gb) with `gen.py` to simulate a large file processing, followed by `dtrace` to capture CPU profiles and `flamegraph.pl` to generate visuals. (checkout `profile.sh`).

We can run: `sh profile.sh` to generate a flamegraph.svg and checkout hotspots.

`dtrace` was useful to check for memory allocations that could have been avoided:
1. Identified String allocations when creating error messages using anyhow
2. Identified Redundant buffer allocations when in ChunkedWorker (later rectified by having a single buffer which is reused)
3. Identified writing errors to stderr in main thread is costly, moved to a dedicated error handler thread to write to a temp file

Additionally, I heavily used `time` command to check the time taken by user and kernel and cpu util
cmd:  `time cargo r --profile profiling`

Throughput was verified using Criterion:
cmd: `cargo bench` 


## Possible Improvement:
- Explore possiblity of using file mapped to user space memory directly , avoiding Disk -> Kernel copy and Kernel -> User space copy (probably io_uring)
- Allow multiple worker analyze function to be async, so that thread can work on other tasks while waiting for I/O