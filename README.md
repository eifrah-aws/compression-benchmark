# Compression Benchmark

Compares zstd and lz4 compression with and without dictionary training.
Measures compression ratio, throughput, and latency percentiles on
real-world datasets.

## Build

Requires Rust 1.70+.

```
cargo build --release
```

The binary is `./target/release/compression-benchmark`.

## Usage

```
# Synthetic data (fixed-size JSON-like values)
./target/release/compression-benchmark --alg zstd --value-size 512
./target/release/compression-benchmark --alg lz4 --dict --value-size 1024

# Real data (one value per line)
./target/release/compression-benchmark --alg zstd --input-file data/github-events.json
./target/release/compression-benchmark --alg zstd --dict --input-file data/taxi-trips.json
```

### Flags

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--alg <zstd\|lz4>` | yes | - | Compression algorithm |
| `--dict` | no | off | Enable dictionary-assisted compression |
| `--value-size <N>` | * | - | Byte size of each generated value |
| `--input-file <PATH>` | * | - | Read values from file (one per line) |
| `--iterations <N>` | no | 100000 | Number of measured iterations |
| `--train-samples <N>` | no | 1000 | Samples used for dictionary training |

(*) One of `--value-size` or `--input-file` is required.

### Run full matrix

```
./run_benchmarks.sh
```

Override defaults with environment variables:

```
BENCH_ITERATIONS=50000 BENCH_TRAIN_SAMPLES=2000 ./run_benchmarks.sh
```

## Datasets

| File | Records | Avg size | Description |
|------|--------:|---------:|-------------|
| `data/github-events.json` | 153,619 | 3,274 B | GitHub Archive API events (JSON). Wide size range (388 B - 155 KiB). |
| `data/github-events-50k.json` | 3,622 | 47 KiB | Batched GitHub events (JSON arrays). Uniform size (46 - 56 KiB). |
| `data/reddit-comments.json` | 54,848 | 537 B | Reddit comments (JSON). Text-heavy, varied length (278 B - 10 KiB). |
| `data/taxi-trips.json` | 50,000 | 456 B | NYC yellow cab trips (JSON). Numeric, very uniform (449 - 469 B). |

## Results

100,000 iterations per scenario, 1,000 dictionary training samples, 16 KiB
zstd dictionary, 64 KiB lz4 dictionary. Measured on Apple M4 Pro.

### GitHub Events (avg 3,414 B)

| Algorithm | Ratio | Saved | Compress | Decompress | p50 comp | p99 comp | p50 decomp | p99 decomp |
|-----------|------:|------:|---------:|-----------:|---------:|---------:|-----------:|-----------:|
| zstd | 0.273 | 72.7% | 362 MiB/s | 1,095 MiB/s | 4.8 us | 45.9 us | 1.8 us | 13.0 us |
| zstd + dict | 0.169 | 83.1% | 476 MiB/s | 1,492 MiB/s | 2.0 us | 55.3 us | 0.9 us | 13.7 us |
| lz4 | 0.374 | 62.6% | 829 MiB/s | 3,207 MiB/s | 1.2 us | 30.3 us | 0.3 us | 8.0 us |
| lz4 + dict | 0.245 | 75.5% | 590 MiB/s | 2,527 MiB/s | 2.8 us | 32.4 us | 0.5 us | 8.4 us |

### GitHub Events 50 KiB (avg 47,338 B)

| Algorithm | Ratio | Saved | Compress | Decompress | p50 comp | p99 comp | p50 decomp | p99 decomp |
|-----------|------:|------:|---------:|-----------:|---------:|---------:|-----------:|-----------:|
| zstd | 0.180 | 82.0% | 698 MiB/s | 2,232 MiB/s | 63.3 us | 87.1 us | 19.7 us | 29.6 us |
| zstd + dict | 0.146 | 85.4% | 505 MiB/s | 2,070 MiB/s | 88.5 us | 118.1 us | 21.5 us | 30.4 us |
| lz4 | 0.274 | 72.6% | 973 MiB/s | 2,972 MiB/s | 46.0 us | 64.0 us | 14.9 us | 22.2 us |
| lz4 + dict | 0.243 | 75.7% | 953 MiB/s | 3,590 MiB/s | 46.5 us | 64.7 us | 12.2 us | 19.0 us |

### Reddit Comments (avg 534 B)

| Algorithm | Ratio | Saved | Compress | Decompress | p50 comp | p99 comp | p50 decomp | p99 decomp |
|-----------|------:|------:|---------:|-----------:|---------:|---------:|-----------:|-----------:|
| zstd | 0.663 | 33.7% | 118 MiB/s | 286 MiB/s | 3.8 us | 11.0 us | 1.6 us | 4.7 us |
| zstd + dict | 0.352 | 64.8% | 254 MiB/s | 584 MiB/s | 1.5 us | 8.9 us | 0.7 us | 3.7 us |
| lz4 | 0.856 | 14.4% | 568 MiB/s | 2,189 MiB/s | 0.7 us | 3.8 us | 0.2 us | 0.8 us |
| lz4 + dict | 0.449 | 55.1% | 198 MiB/s | 1,267 MiB/s | 2.3 us | 7.0 us | 0.3 us | 1.5 us |

### NYC Taxi Trips (avg 456 B)

| Algorithm | Ratio | Saved | Compress | Decompress | p50 comp | p99 comp | p50 decomp | p99 decomp |
|-----------|------:|------:|---------:|-----------:|---------:|---------:|-----------:|-----------:|
| zstd | 0.630 | 37.0% | 118 MiB/s | 243 MiB/s | 3.6 us | 5.0 us | 1.9 us | 2.6 us |
| zstd + dict | 0.128 | 87.2% | 570 MiB/s | 1,239 MiB/s | 0.7 us | 1.1 us | 0.3 us | 0.7 us |
| lz4 | 0.776 | 22.4% | 634 MiB/s | 2,807 MiB/s | 0.7 us | 0.9 us | 0.2 us | 0.2 us |
| lz4 + dict | 0.183 | 81.7% | 217 MiB/s | 1,447 MiB/s | 1.9 us | 3.4 us | 0.3 us | 0.5 us |

### Key observations

**Dictionary compression helps the most on small, uniform records.** Taxi
trips (456 B, fixed schema) go from 0.630 to 0.128 with zstd + dict - a
4.9x improvement. Reddit comments (534 B, varied text) improve from 0.663
to 0.352 - still a large gain but less dramatic because the content is
less predictable.

**At 50 KiB, dictionary benefit is small.** The GitHub Events 50 KiB dataset
shows zstd at 0.180 without a dictionary and 0.146 with one - only a 19%
relative improvement. At this size the input contains enough internal
repetition for the compressor to exploit without external help.

**Zstd + dict wins on ratio for small values.** It achieves the best
compression on every small-value dataset: 83.1% savings on GitHub events,
64.8% on Reddit, 87.2% on taxi trips.

**LZ4 without dictionary wins on speed.** Decompression reaches 2.2-3.2
GiB/s. Compression is 2-5x faster than zstd depending on value size.

**LZ4 + dict is now competitive on throughput.** Using the C liblz4 binding
(lzzzz crate) with `attach_dict` for preloaded dictionary reuse, compress
throughput reaches 198-953 MiB/s. The previous pure-Rust lz4_flex crate
rebuilt the 64 KiB hash table on every call, limiting throughput to
36-205 MiB/s.

**Without a dictionary, small values compress poorly.** LZ4 saves only
14.4% on Reddit comments and 22.4% on taxi trips. Plain zstd does better
(33.7% and 37.0%) but still leaves significant room for a dictionary.

**The crossover point is around 1-4 KiB.** GitHub events (avg 3.4 KiB) show
strong compression even without a dictionary (zstd 72.7%, lz4 62.6%).
For sub-1 KiB values, a dictionary is essential for meaningful savings.
By 50 KiB, dictionaries add little.
