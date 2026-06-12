# Design Notes

## File Reading & Parallelism
- **Streaming IO:** `BufReader` (64 KB buffer) feeds a custom `ChunkReader` iterator.
- **Chunking:** Reads complete lines into 8 MB chunks to avoid breaking log entries mid-way.
- **Map-Reduce:** Rayon's `par_bridge` processes chunks in parallel. Workers build independent counters which are reduced into a single total.

## Parsing & Memory Efficiency
- **Zero-Copy Parsing:** Operates strictly on `&str` references; no string allocations per line or field.
- **SIMD Optimized:** Uses `memchr` for blazing-fast SIMD vector scanning of pipe (`|`) delimiters.
- **Oversize Guard:** Immediately skips lines exceeding 64 KB to prevent pathological memory growth without panicking.

## Minimal Allocations
Strictly limited to three startup/buffer allocations:
1. **Worker Buffers:** One `String` buffer per active worker (8 MB + overflow margin).
2. **IO Buffer:** Internal `BufReader` buffer (64 KB).
3. **CLI Arguments:** `PathBuf` for the file path.

## Trade-offs
- **Chunk Size (8 MB):** Tuned to balance thread-dispatch overhead vs. memory usage. Tunable via `.env`.
- **Sequential Reader:** One thread reads while others process. Simpler and safer than byte-offset splitting, but can become a bottleneck on high-core machines with extremely fast NVMe drives.

