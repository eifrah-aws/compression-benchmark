use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};
use rand::Rng;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, ValueEnum)]
enum Algorithm {
    Zstd,
    Lz4,
}

#[derive(Parser, Debug)]
#[command(name = "compression-benchmark")]
struct Args {
    /// Compression algorithm to benchmark.
    #[arg(long)]
    alg: Algorithm,

    /// Enable dictionary-assisted compression.
    #[arg(long, default_value_t = false)]
    dict: bool,

    /// Size of each generated value in bytes.
    /// Ignored when --input-file is provided.
    #[arg(long)]
    value_size: Option<usize>,

    /// Read values from a file (one value per line) instead of generating
    /// synthetic data.
    #[arg(long)]
    input_file: Option<PathBuf>,

    /// Number of samples used to train the dictionary.
    #[arg(long, default_value_t = 1_000)]
    train_samples: usize,

    /// Number of measured iterations.
    #[arg(long, default_value_t = 100_000)]
    iterations: usize,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Number of distinct values generated in synthetic mode.
const NUM_VALUES: usize = 10_000;

/// Maximum dictionary size in bytes.
const DICT_MAX_SIZE: usize = 16_384;

/// Zstd compression level (3 is the default).
const ZSTD_LEVEL: i32 = 3;

/// Number of warm-up iterations before measurement.
const WARMUP_ITERS: usize = 500;

// ---------------------------------------------------------------------------
// Data loading / generation
// ---------------------------------------------------------------------------

fn load_values_from_file(path: &PathBuf) -> Vec<Vec<u8>> {
    let file = File::open(path).expect("failed to open input file");
    let reader = BufReader::new(file);
    let mut values = Vec::new();
    for line in reader.lines() {
        let line = line.expect("failed to read line");
        if !line.is_empty() {
            values.push(line.into_bytes());
        }
    }
    values
}

/// Build values that look like small JSON objects so a dictionary has
/// something useful to learn. Structure is fixed; field values vary.
fn generate_values(count: usize, size: usize) -> Vec<Vec<u8>> {
    let mut rng = rand::thread_rng();
    let mut values = Vec::with_capacity(count);

    for i in 0..count {
        let mut buf = Vec::with_capacity(size);

        let prefix = format!(
            "{{\"id\":{},\"ts\":{},\"flag\":{},\"tag\":\"item-{:04x}\",\"data\":\"",
            i,
            1_700_000_000 + i,
            i % 2 == 0,
            i % 0xFFFF,
        );
        buf.extend_from_slice(prefix.as_bytes());

        while buf.len() < size.saturating_sub(2) {
            let ch: u8 = rng.gen_range(0x20..0x7F);
            buf.push(ch);
        }
        if buf.len() < size.saturating_sub(1) {
            buf.push(b'"');
        }
        if buf.len() < size {
            buf.push(b'}');
        }
        buf.truncate(size);
        values.push(buf);
    }
    values
}

// ---------------------------------------------------------------------------
// Zstd helpers
// ---------------------------------------------------------------------------

fn train_zstd_dict(samples: &[Vec<u8>]) -> Vec<u8> {
    zstd::dict::from_samples(samples, DICT_MAX_SIZE).expect("zstd dictionary training failed")
}

fn zstd_compress(data: &[u8]) -> Vec<u8> {
    zstd::bulk::compress(data, ZSTD_LEVEL).expect("zstd compress failed")
}

fn zstd_decompress(data: &[u8], capacity: usize) -> Vec<u8> {
    zstd::bulk::decompress(data, capacity).expect("zstd decompress failed")
}

fn zstd_compress_dict(compressor: &mut zstd::bulk::Compressor<'_>, data: &[u8]) -> Vec<u8> {
    compressor
        .compress(data)
        .expect("zstd dict compress failed")
}

fn zstd_decompress_dict(
    decompressor: &mut zstd::bulk::Decompressor<'_>,
    data: &[u8],
    capacity: usize,
) -> Vec<u8> {
    decompressor
        .decompress(data, capacity)
        .expect("zstd dict decompress failed")
}

// ---------------------------------------------------------------------------
// LZ4 helpers
// ---------------------------------------------------------------------------

fn lz4_compress(data: &[u8]) -> Vec<u8> {
    lz4_flex::compress_prepend_size(data)
}

fn lz4_decompress(data: &[u8]) -> Vec<u8> {
    lz4_flex::decompress_size_prepended(data).expect("lz4 decompress failed")
}

fn lz4_compress_dict(data: &[u8], dict: &[u8]) -> Vec<u8> {
    lz4_flex::block::compress_prepend_size_with_dict(data, dict)
}

fn lz4_decompress_dict(data: &[u8], dict: &[u8]) -> Vec<u8> {
    lz4_flex::block::decompress_size_prepended_with_dict(data, dict)
        .expect("lz4 dict decompress failed")
}

// ---------------------------------------------------------------------------
// Timing helpers
// ---------------------------------------------------------------------------

struct TimingStats {
    total: Duration,
    min: Duration,
    max: Duration,
    p50: Duration,
    p99: Duration,
}

fn collect_timings(durations: &mut Vec<Duration>) -> TimingStats {
    durations.sort();
    let n = durations.len();
    let total: Duration = durations.iter().sum();
    TimingStats {
        total,
        min: durations[0],
        max: durations[n - 1],
        p50: durations[n / 2],
        p99: durations[(n as f64 * 0.99) as usize],
    }
}

fn fmt_duration(d: Duration) -> String {
    let ns = d.as_nanos();
    if ns < 1_000 {
        format!("{} ns", ns)
    } else if ns < 1_000_000 {
        format!("{:.1} us", ns as f64 / 1_000.0)
    } else {
        format!("{:.2} ms", ns as f64 / 1_000_000.0)
    }
}

// ---------------------------------------------------------------------------
// Benchmark runners
// ---------------------------------------------------------------------------

fn bench_zstd(values: &[Vec<u8>], iterations: usize) {
    for v in values.iter().take(WARMUP_ITERS) {
        let c = zstd_compress(v);
        let _ = zstd_decompress(&c, v.len());
    }

    let mut compress_times = Vec::with_capacity(iterations);
    let mut decompress_times = Vec::with_capacity(iterations);
    let mut total_raw: usize = 0;
    let mut total_compressed: usize = 0;

    for v in values.iter().cycle().take(iterations) {
        let t0 = Instant::now();
        let compressed = zstd_compress(v);
        compress_times.push(t0.elapsed());

        total_raw += v.len();
        total_compressed += compressed.len();

        let t1 = Instant::now();
        let _ = zstd_decompress(&compressed, v.len());
        decompress_times.push(t1.elapsed());
    }

    print_results(
        "zstd (no dictionary)",
        total_raw,
        iterations,
        total_compressed,
        &mut compress_times,
        &mut decompress_times,
    );
}

fn bench_zstd_dict(values: &[Vec<u8>], train_samples: usize, iterations: usize) {
    let samples: Vec<Vec<u8>> = values.iter().take(train_samples).cloned().collect();
    let dict_bytes = train_zstd_dict(&samples);
    println!("  dictionary size: {} bytes", dict_bytes.len());

    let enc_dict = zstd::dict::EncoderDictionary::copy(&dict_bytes, ZSTD_LEVEL);
    let dec_dict = zstd::dict::DecoderDictionary::copy(&dict_bytes);

    let mut compressor = zstd::bulk::Compressor::with_prepared_dictionary(&enc_dict)
        .expect("failed to create zstd compressor with dict");
    let mut decompressor = zstd::bulk::Decompressor::with_prepared_dictionary(&dec_dict)
        .expect("failed to create zstd decompressor with dict");

    for v in values.iter().take(WARMUP_ITERS) {
        let c = zstd_compress_dict(&mut compressor, v);
        let _ = zstd_decompress_dict(&mut decompressor, &c, v.len());
    }

    let mut compress_times = Vec::with_capacity(iterations);
    let mut decompress_times = Vec::with_capacity(iterations);
    let mut total_raw: usize = 0;
    let mut total_compressed: usize = 0;

    for v in values.iter().cycle().take(iterations) {
        let t0 = Instant::now();
        let compressed = zstd_compress_dict(&mut compressor, v);
        compress_times.push(t0.elapsed());

        total_raw += v.len();
        total_compressed += compressed.len();

        let t1 = Instant::now();
        let _ = zstd_decompress_dict(&mut decompressor, &compressed, v.len());
        decompress_times.push(t1.elapsed());
    }

    print_results(
        "zstd (with dictionary)",
        total_raw,
        iterations,
        total_compressed,
        &mut compress_times,
        &mut decompress_times,
    );
}

fn bench_lz4(values: &[Vec<u8>], iterations: usize) {
    for v in values.iter().take(WARMUP_ITERS) {
        let c = lz4_compress(v);
        let _ = lz4_decompress(&c);
    }

    let mut compress_times = Vec::with_capacity(iterations);
    let mut decompress_times = Vec::with_capacity(iterations);
    let mut total_raw: usize = 0;
    let mut total_compressed: usize = 0;

    for v in values.iter().cycle().take(iterations) {
        let t0 = Instant::now();
        let compressed = lz4_compress(v);
        compress_times.push(t0.elapsed());

        total_raw += v.len();
        total_compressed += compressed.len();

        let t1 = Instant::now();
        let _ = lz4_decompress(&compressed);
        decompress_times.push(t1.elapsed());
    }

    print_results(
        "lz4 (no dictionary)",
        total_raw,
        iterations,
        total_compressed,
        &mut compress_times,
        &mut decompress_times,
    );
}

fn bench_lz4_dict(values: &[Vec<u8>], train_samples: usize, iterations: usize) {
    // LZ4 uses a raw byte prefix as dictionary (up to 64 KiB window).
    // Concatenate training samples and keep the last 64 KiB.
    let mut raw_dict: Vec<u8> = Vec::new();
    for s in values.iter().take(train_samples) {
        raw_dict.extend_from_slice(s);
    }
    let max_window = 64 * 1024;
    if raw_dict.len() > max_window {
        let start = raw_dict.len() - max_window;
        raw_dict = raw_dict[start..].to_vec();
    }
    println!("  dictionary size: {} bytes", raw_dict.len());

    for v in values.iter().take(WARMUP_ITERS) {
        let c = lz4_compress_dict(v, &raw_dict);
        let _ = lz4_decompress_dict(&c, &raw_dict);
    }

    let mut compress_times = Vec::with_capacity(iterations);
    let mut decompress_times = Vec::with_capacity(iterations);
    let mut total_raw: usize = 0;
    let mut total_compressed: usize = 0;

    for v in values.iter().cycle().take(iterations) {
        let t0 = Instant::now();
        let compressed = lz4_compress_dict(v, &raw_dict);
        compress_times.push(t0.elapsed());

        total_raw += v.len();
        total_compressed += compressed.len();

        let t1 = Instant::now();
        let _ = lz4_decompress_dict(&compressed, &raw_dict);
        decompress_times.push(t1.elapsed());
    }

    print_results(
        "lz4 (with dictionary)",
        total_raw,
        iterations,
        total_compressed,
        &mut compress_times,
        &mut decompress_times,
    );
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn print_results(
    label: &str,
    total_raw: usize,
    iterations: usize,
    total_compressed: usize,
    compress_times: &mut Vec<Duration>,
    decompress_times: &mut Vec<Duration>,
) {
    let avg_raw = total_raw as f64 / iterations as f64;
    let avg_compressed = total_compressed as f64 / iterations as f64;
    let ratio = avg_compressed / avg_raw;

    let ct = collect_timings(compress_times);
    let dt = collect_timings(decompress_times);

    let compress_throughput_mbps = total_raw as f64 / ct.total.as_secs_f64() / (1024.0 * 1024.0);
    let decompress_throughput_mbps = total_raw as f64 / dt.total.as_secs_f64() / (1024.0 * 1024.0);

    println!();
    println!("=== {} ===", label);
    println!("  avg value size:     {:.0} bytes", avg_raw);
    println!("  avg compressed:     {:.0} bytes", avg_compressed);
    println!("  ratio:              {:.3} (lower is better)", ratio);
    println!("  space saved:        {:.1}%", (1.0 - ratio) * 100.0);
    println!("  iterations:         {}", iterations);
    println!();
    println!("  compress:");
    println!(
        "    throughput:       {:.1} MiB/s",
        compress_throughput_mbps
    );
    println!("    min:              {}", fmt_duration(ct.min));
    println!("    p50:              {}", fmt_duration(ct.p50));
    println!("    p99:              {}", fmt_duration(ct.p99));
    println!("    max:              {}", fmt_duration(ct.max));
    println!();
    println!("  decompress:");
    println!(
        "    throughput:       {:.1} MiB/s",
        decompress_throughput_mbps
    );
    println!("    min:              {}", fmt_duration(dt.min));
    println!("    p50:              {}", fmt_duration(dt.p50));
    println!("    p99:              {}", fmt_duration(dt.p99));
    println!("    max:              {}", fmt_duration(dt.max));
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args = Args::parse();

    let values = if let Some(ref path) = args.input_file {
        println!("Loading values from {}...", path.display());
        let v = load_values_from_file(path);
        println!("  loaded {} values", v.len());
        if v.is_empty() {
            eprintln!("error: input file is empty");
            std::process::exit(1);
        }
        let sizes: Vec<usize> = v.iter().map(|x| x.len()).collect();
        let min = *sizes.iter().min().unwrap();
        let max = *sizes.iter().max().unwrap();
        let avg = sizes.iter().sum::<usize>() / sizes.len();
        println!("  size range: {} - {} bytes (avg {})", min, max, avg);
        v
    } else {
        let size = args.value_size.unwrap_or_else(|| {
            eprintln!("error: --value-size is required when --input-file is not provided");
            std::process::exit(1);
        });
        println!("Generating {} values of {} bytes each...", NUM_VALUES, size);
        generate_values(NUM_VALUES, size)
    };

    let iters = args.iterations;
    let train = args.train_samples;

    match (&args.alg, args.dict) {
        (Algorithm::Zstd, false) => bench_zstd(&values, iters),
        (Algorithm::Zstd, true) => bench_zstd_dict(&values, train, iters),
        (Algorithm::Lz4, false) => bench_lz4(&values, iters),
        (Algorithm::Lz4, true) => bench_lz4_dict(&values, train, iters),
    }
}
