#!/bin/sh
# Builds the web version into a single self-contained web/dist/index.htm.
set -e
cd "$(dirname "$0")/.."
cargo build -p notebook_app --target wasm32-unknown-unknown --release
rm -rf web/pkg && wasm-bindgen --target no-modules --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/notebook.wasm
python3 web/bundle.py web/pkg web/dist/index.htm
cp web/dist/index.htm web/public/index.html
