#!/usr/bin/env bash
set -euo pipefail

BIN="./target/release/compression-benchmark"
ITERS="${BENCH_ITERATIONS:-100000}"
TRAIN="${BENCH_TRAIN_SAMPLES:-1000}"

DATASETS=(
    "data/github-events.json"
    "data/reddit-comments.json"
    "data/taxi-trips.json"
)

ALGORITHMS=("zstd" "lz4")
if [ ! -f "$BIN" ]; then
    echo "Binary not found. Building..."
    cargo build --release
fi

run_bench() {
    local dataset="$1" alg="$2" use_dict="$3"
    local label="$alg"
    local cmd=("$BIN" --alg "$alg" --input-file "$dataset"
               --iterations "$ITERS" --train-samples "$TRAIN")
    if [ "$use_dict" = "yes" ]; then
        cmd+=(--dict)
        label="$alg + dict"
    fi
    echo ""
    echo "--- $label ---"
    "${cmd[@]}"
}

for dataset in "${DATASETS[@]}"; do
    if [ ! -f "$dataset" ]; then
        echo "ERROR: $dataset not found. Skipping."
        continue
    fi
    echo ""
    echo "================================================================"
    echo "Dataset: $dataset"
    echo "================================================================"
    for alg in "${ALGORITHMS[@]}"; do
        run_bench "$dataset" "$alg" "no"
        run_bench "$dataset" "$alg" "yes"
    done
done
