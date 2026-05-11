#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$REPO_ROOT/perf/baseline"
mkdir -p "$OUT_DIR"

cd "$REPO_ROOT"
# shellcheck disable=SC2086
cargo build --release ${EXTRA_FEATURES:-} >/dev/null

BIN="$REPO_ROOT/target/release/oversee"

# Clear the profile log if the profile feature produced one previously.
: > /tmp/oversee-profile.log 2>/dev/null || true

# Launch oversee detached. We need a TTY; use `script` to allocate one on macOS.
SESSION_LOG=$(mktemp)
script -q "$SESSION_LOG" "$BIN" >/dev/null 2>&1 &
APP_PID=$!

# Wait for the process to settle.
sleep 2

# Find the actual oversee pid (script forks).
OVERSEE_PID=$(pgrep -n oversee || true)
if [[ -z "$OVERSEE_PID" ]]; then
  echo "Failed to start oversee" >&2
  kill "$APP_PID" 2>/dev/null || true
  exit 1
fi

echo "oversee pid=$OVERSEE_PID. Sampling 10s of top..."

# 10 samples, 1s apart. Column 3 of `top -stats pid,command,cpu` is CPU%.
top -l 10 -s 1 -pid "$OVERSEE_PID" -stats cpu \
  | awk '/^[0-9]+\.[0-9]+/ {print $1}' \
  | tee "$OUT_DIR/idle-cpu-samples.txt" \
  | awk '{s+=$1; n++} END {if (n>0) printf("avg_cpu_pct=%.2f\nsamples=%d\n", s/n, n)}' \
  | tee "$OUT_DIR/idle-cpu.txt"

# Summarise profile log if it exists (only populated with --features profile).
if [[ -s /tmp/oversee-profile.log ]]; then
  awk -F': ' '
    { label=$1; ms=$2+0; sum[label]+=ms; n[label]++;
      if (ms>max[label]) max[label]=ms }
    END { for (l in sum) printf("%-20s mean=%.2fms max=%dms n=%d\n",
                                 l, sum[l]/n[l], max[l], n[l]) }
  ' /tmp/oversee-profile.log | sort > "$OUT_DIR/profile-summary.txt"
  echo "Profile summary -> $OUT_DIR/profile-summary.txt"
fi

kill "$OVERSEE_PID" 2>/dev/null || true
kill "$APP_PID" 2>/dev/null || true
rm -f "$SESSION_LOG"

echo "Wrote $OUT_DIR/idle-cpu.txt"
