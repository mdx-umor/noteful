#!/bin/sh
# Runs the app under a virtual X display with simulated Apple Pencil input and
# writes screenshots plus PASS/FAIL checks. Linux only; needs Xvfb and Mesa.
#   scripts/autotest.sh [output-dir]
set -eu
cd "$(dirname "$0")/.."
OUT="${1:-target/autotest}"
rm -rf "$OUT" && mkdir -p "$OUT/shots"
cargo build -p notebook_app --features autotest
NOTEBOOK_DATA_DIR="$OUT/data" NOTEBOOK_AUTOTEST="$OUT/shots" \
    xvfb-run -a -s "-screen 0 1366x1024x24" target/debug/notebook | grep -E "CHECK|screenshot"
