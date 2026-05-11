#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$REPO_ROOT/perf/baseline/idle.json.gz"
DURATION="${DURATION:-30}"

cd "$REPO_ROOT"
cargo build --release >/dev/null

BIN="$REPO_ROOT/target/release/oversee"

# Launch oversee under a PTY so the TUI can initialise.
SESSION_LOG=$(mktemp)
script -q "$SESSION_LOG" "$BIN" >/dev/null 2>&1 &
APP_PID=$!

# Wait for the TUI to settle.
sleep 2
OVERSEE_PID=$(pgrep -n oversee || true)
if [[ -z "$OVERSEE_PID" ]]; then
  echo "Failed to start oversee" >&2
  kill "$APP_PID" 2>/dev/null || true
  exit 1
fi

mkdir -p "$(dirname "$OUT")"
echo "Recording ${DURATION}s flamegraph for pid=$OVERSEE_PID -> $OUT"
samply record --pid "$OVERSEE_PID" -d "$DURATION" --save-only --no-open -o "$OUT"

kill "$OVERSEE_PID" 2>/dev/null || true
kill "$APP_PID" 2>/dev/null || true
rm -f "$SESSION_LOG"

echo "Open with: samply load $OUT"
