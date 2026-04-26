# DESIGN: Zero-Copy Log Analyzer

## Overview
The **Zero-Copy Log Analyzer** is a high-performance tool designed to process multi-gigabyte log files with minimal memory overhead. It achieves this by using buffered I/O, streaming processing, and a multi-threaded chunking strategy.

---

## 1. Architectural Strategy

### Streaming Processing
To satisfy the requirement of not loading the entire file into memory, the analyzer uses `std::io::BufReader`. This ensures that only small chunks of the file reside in RAM at any given time, regardless of whether the file is 100MB or 100GB.

### Parallel vs. Sequential Execution
The tool dynamically chooses an execution strategy based on file size:
* **Sequential:** Used for small files to avoid the overhead of thread spawning.
* **Parallel:** Triggered for files exceeding a threshold (default **100 MB**). It uses `std::thread::scope` to divide the file into segments processed by all available CPU cores.

---

## 2. Flexible Configuration & Targeting

The analyzer is driven by a `config.toml` file that allows users to change the aggregation logic without recompiling the code.
Dynamic Counting Targets

The `target` field in the configuration determines the "Key" used for the final summary:
- `target = "level"`: The analyzer aggregates counts based on the importance of the log (e.g., how many `ERRORs` vs `INFOs`).
- `target = "service"`: The analyzer aggregates counts based on the originating source (e.g., which microservice is generating the most logs).

Configuration Schema
- `delimiter`: Defines the character separating fields (default: |).
- `levels`: A whitelist of valid log levels. Anything not in this list is categorized as `UNKNOWN`.
- `parallel`: A boolean toggle to force or disable multi-threading.

---

## 3. Parsing Logic & Memory Efficiency

### Zero-Copy Mechanics
Parsing is handled by the `process_log_line` function. 
* **String Slices (`&str`):** Instead of creating new `String` objects for the timestamp, level, or message, the parser returns a `LogEntry` containing references to the original buffer.
* **Buffer Reuse:** In the `SequentialLogProcessor`, a single `String` buffer is allocated once and cleared (`line.clear()`) for every line. This prevents $O(n)$ allocations where $n$ is the number of log lines.

### Allocation Boundaries
While parsing is zero-copy, some allocations are unavoidable:
1.  **Summary Map:** A `HashMap<String, u64>` is used to store the final counts. The keys (log levels or service names) are owned `String`s to allow them to persist after the line buffer is cleared.
2.  **Thread Results:** In parallel mode, each thread maintains its own local `HashMap`, which is merged into the main map upon completion.

---

## 4. Parallel Chunking Logic
Parallel processing of a text file is non-trivial because a simple byte-offset split could cut a log line in half. 

**The Solution:**
1.  Calculate `chunk_size = file_size / num_threads`.
2.  Seek to the "rough" end of a chunk.
3.  Read until the next newline (`\n`) to find the **true boundary**.
4.  Each thread starts at its designated offset and processes exactly until the end of its logical boundary.

---

## 5. Performance Trade-offs

| Feature | Choice | Trade-off |
| :--- | :--- | :--- |
| **I/O** | `BufReader` | Slower than memory-mapped files (`mmap`) but safer and more portable across different OS environments. |
| **Concurrency** | `std::thread::scope` | Requires the file path to be available to all threads, but avoids the complexity of `Arc` or mutexes during the high-speed parsing phase. |
| **Error Handling** | `MALFORMED` counter | Instead of panicking or stopping, malformed lines are counted separately to provide a robust report without compromising speed. |

---

## 6. Scalability
* **Memory Complexity:** $O(K)$, where $K$ is the number of unique log levels/services. Memory usage does not increase with the number of log lines.
* **Time Complexity:** $O(N/P)$, where $N$ is the number of lines and $P$ is the number of CPU cores (in parallel mode).

---

## 7. How to Run
1.  **Configure:** Modify `config.toml` to set your desired delimiter and target (Level or Service).
2.  **Execute:**
    ```bash
    cargo run --release -- <path_to_log_file> [path_to_config]
    ```
3.  **Test:**
    ```bash
    cargo test
    ```
