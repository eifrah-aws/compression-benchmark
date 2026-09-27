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
| `data/reddit-comments.json` | 54,848 | 537 B | Reddit comments (JSON). Text-heavy, varied length (278 B - 10 KiB). |
| `data/taxi-trips.json` | 50,000 | 456 B | NYC yellow cab trips (JSON). Numeric, very uniform (449 - 469 B). |

## Results

100,000 iterations per scenario, 1,000 dictionary training samples, 16 KiB
zstd dictionary, 64 KiB lz4 dictionary. Measured on Apple M4 Pro.

### GitHub Events (avg 3,414 B)

| Algorithm | Ratio | Saved | Compress | Decompress | p50 comp | p99 comp |
|-----------|------:|------:|---------:|-----------:|---------:|---------:|
| zstd | 0.273 | 72.7% | 354 MiB/s | 1,076 MiB/s | 4.8 us | 48.1 us |
| zstd + dict | 0.169 | 83.1% | 475 MiB/s | 1,452 MiB/s | 2.0 us | 55.1 us |
| lz4 | 0.374 | 62.6% | 820 MiB/s | 3,412 MiB/s | 1.3 us | 30.8 us |
| lz4 + dict | 0.245 | 75.5% | 205 MiB/s | 2,665 MiB/s | 12.6 us | 49.5 us |

### Reddit Comments (avg 534 B)

| Algorithm | Ratio | Saved | Compress | Decompress | p50 comp | p99 comp |
|-----------|------:|------:|---------:|-----------:|---------:|---------:|
| zstd | 0.663 | 33.7% | 120 MiB/s | 285 MiB/s | 3.8 us | 10.1 us |
| zstd + dict | 0.352 | 64.8% | 261 MiB/s | 576 MiB/s | 1.5 us | 8.7 us |
| lz4 | 0.856 | 14.4% | 586 MiB/s | 2,485 MiB/s | 0.7 us | 3.7 us |
| lz4 + dict | 0.451 | 54.9% | 42 MiB/s | 1,950 MiB/s | 11.8 us | 19.5 us |

### NYC Taxi Trips (avg 456 B)

| Algorithm | Ratio | Saved | Compress | Decompress | p50 comp | p99 comp |
|-----------|------:|------:|---------:|-----------:|---------:|---------:|
| zstd | 0.630 | 37.0% | 117 MiB/s | 240 MiB/s | 3.6 us | 5.0 us |
| zstd + dict | 0.128 | 87.2% | 584 MiB/s | 985 MiB/s | 0.7 us | 1.1 us |
| lz4 | 0.776 | 22.4% | 618 MiB/s | 3,071 MiB/s | 0.7 us | 1.1 us |
| lz4 + dict | 0.188 | 81.2% | 36 MiB/s | 1,965 MiB/s | 12.0 us | 17.0 us |

### Key observations

**Dictionary compression helps the most on small, uniform records.** Taxi
trips (456 B, fixed schema) go from 0.630 to 0.128 with zstd + dict - a
4.9x improvement. Reddit comments (534 B, varied text) improve from 0.663
to 0.352 - still a large gain but less dramatic because the content is
less predictable.

**Zstd + dict wins on ratio.** It achieves the best compression in every
dataset: 83.1% savings on GitHub events, 64.8% on Reddit, 87.2% on taxi
trips.

**LZ4 without dictionary wins on speed.** Decompression reaches 2.5-3.4
GiB/s. Compression is 2-6x faster than zstd depending on value size.

**LZ4 + dict has a compression throughput problem.** The 64 KiB dictionary
hash table rebuild on every call drops compress throughput to 36-205
MiB/s. Decompression stays fast (1.9-2.7 GiB/s) because the dictionary
lookup is cheaper on the decode path.

**Without a dictionary, small values compress poorly.** LZ4 saves only
14.4% on Reddit comments and 22.4% on taxi trips. Plain zstd does better
(33.7% and 37.0%) but still leaves significant room for a dictionary.

**The crossover point is around 1-4 KiB.** GitHub events (avg 3.4 KiB) show
strong compression even without a dictionary (zstd 72.7%, lz4 62.6%).
For sub-1 KiB values, a dictionary is essential for meaningful savings.
