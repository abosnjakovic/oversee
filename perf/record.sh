#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$REPO_ROOT/perf/baseline/idle.json"

cd "$REPO_ROOT"
cargo build --release

mkdir -p "$(dirname "$OUT")"
echo "Recording 30s samply profile -> $OUT"
echo "Quit oversee with 'q' when done (or it will be killed after 30s)."

samply record --save-only -o "$OUT" -- "$REPO_ROOT/target/release/oversee" &
SAMPLY_PID=$!
sleep 30
kill "$SAMPLY_PID" 2>/dev/null || true
wait "$SAMPLY_PID" 2>/dev/null || true

echo "Done. Open with: samply load $OUT"
