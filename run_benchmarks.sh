#!/usr/bin/env bash
set -euo pipefail

BIN="./target/release/compression-benchmark"
ITERS="${BENCH_ITERATIONS:-100000}"
TRAIN="${BENCH_TRAIN_SAMPLES:-1000}"

DATASETS=(
    "data/github-events.json"
    "data/github-events-50k.json"
    "data/reddit-comments.json"
    "data/taxi-trips.json"
)

ALGORITHMS=("zstd" "lz4")

if [ ! -f "$BIN" ]; then
    echo "Binary not found. Building..."
    cargo build --release
fi

# Decompress .gz files if the uncompressed version is missing.
for dataset in "${DATASETS[@]}"; do
    if [ ! -f "$dataset" ] && [ -f "${dataset}.gz" ]; then
        echo "Decompressing ${dataset}.gz..."
        gunzip -k "${dataset}.gz"
    fi
done

# parse_output <output>
# Extracts fields from benchmark output into shell variables.
parse_output() {
    local out="$1"
    P_AVG_SIZE=$(echo "$out" | grep "avg value size:" | awk '{print $4}')
    P_RATIO=$(echo "$out" | grep "ratio:" | awk '{print $2}')
    P_SAVED=$(echo "$out" | grep "space saved:" | awk '{print $3}')
    # compress section: first occurrence of throughput/p50/p99
    P_COMP_TP=$(echo "$out" | awk '/compress:/{found=1} found && /throughput:/{print $2; exit}')
    P_COMP_P50=$(echo "$out" | awk '/compress:/{found=1} found && /p50:/{print $2, $3; exit}')
    P_COMP_P99=$(echo "$out" | awk '/compress:/{found=1} found && /p99:/{print $2, $3; exit}')
    # decompress section
    P_DECOMP_TP=$(echo "$out" | awk '/decompress:/{found=1} found && /throughput:/{print $2; exit}')
    P_DECOMP_P50=$(echo "$out" | awk '/decompress:/{found=1} found && /p50:/{print $2, $3; exit}')
    P_DECOMP_P99=$(echo "$out" | awk '/decompress:/{found=1} found && /p99:/{print $2, $3; exit}')
}

run_bench() {
    local dataset="$1" alg="$2" use_dict="$3"
    local cmd=("$BIN" --alg "$alg" --input-file "$dataset"
               --iterations "$ITERS" --train-samples "$TRAIN")
    if [ "$use_dict" = "yes" ]; then
        cmd+=(--dict)
    fi
    "${cmd[@]}" 2>&1
}

# Collect all results per dataset: rows[dataset_index] is a newline-separated
# list of "label|ratio|saved|comp_tp|decomp_tp|p50|p99" strings.
declare -a ALL_ROWS
declare -a ALL_AVG_SIZES
declare -a ALL_DATASET_NAMES

ds_idx=0
for dataset in "${DATASETS[@]}"; do
    if [ ! -f "$dataset" ]; then
        echo "ERROR: $dataset not found. Skipping."
        continue
    fi

    name=$(basename "$dataset" .json)
    ALL_DATASET_NAMES[$ds_idx]="$name"
    rows=""

    for alg in "${ALGORITHMS[@]}"; do
        for use_dict in "no" "yes"; do
            label="$alg"
            if [ "$use_dict" = "yes" ]; then
                label="$alg + dict"
            fi

            echo "Running: $name / $label ..." >&2
            out=$(run_bench "$dataset" "$alg" "$use_dict")
            parse_output "$out"
            ALL_AVG_SIZES[$ds_idx]="$P_AVG_SIZE"

            row="${label}|${P_RATIO}|${P_SAVED}|${P_COMP_TP}|${P_DECOMP_TP}|${P_COMP_P50}|${P_COMP_P99}|${P_DECOMP_P50}|${P_DECOMP_P99}"
            if [ -z "$rows" ]; then
                rows="$row"
            else
                rows="${rows}
${row}"
            fi
        done
    done

    ALL_ROWS[$ds_idx]="$rows"
    ds_idx=$((ds_idx + 1))
done

# Print summary tables.
echo ""
echo "================================================================"
echo "  Benchmark Summary"
echo "  iterations: $ITERS  |  train samples: $TRAIN"
echo "================================================================"

for i in $(seq 0 $((ds_idx - 1))); do
    name="${ALL_DATASET_NAMES[$i]}"
    avg="${ALL_AVG_SIZES[$i]}"
    echo ""
    echo "### ${name} (avg ${avg} B)"
    echo ""
    printf "%-14s %7s %7s %12s %12s %10s %10s %10s %10s\n" \
        "Algorithm" "Ratio" "Saved" "Compress" "Decompress" "p50 comp" "p99 comp" "p50 decomp" "p99 decomp"
    printf "%-14s %7s %7s %12s %12s %10s %10s %10s %10s\n" \
        "--------------" "-------" "-------" "------------" "------------" "----------" "----------" "----------" "----------"

    while IFS= read -r row; do
        IFS='|' read -r label ratio saved comp_tp decomp_tp cp50 cp99 dp50 dp99 <<< "$row"
        printf "%-14s %7s %7s %12s %12s %10s %10s %10s %10s\n" \
            "$label" "$ratio" "$saved" "${comp_tp} MiB/s" "${decomp_tp} MiB/s" "$cp50" "$cp99" "$dp50" "$dp99"
    done <<< "${ALL_ROWS[$i]}"
done

echo ""
