#!/usr/bin/env bash
# stage.sh NEW_OUTPUT_DIR DUI_BINARY IBM_PLEX_SANS_SUBSET_TTF
set -euo pipefail
crate_dir=$(cd "$(dirname "$0")" && pwd)
out=$1
mkdir "$out"
cargo +nightly-2026-10-31 build --manifest-path "$crate_dir/Cargo.toml" --locked --target wasm32-unknown-unknown --release
wasm-bindgen "$crate_dir/target/wasm32-unknown-unknown/release/dui_gpui_web_probe.wasm" --target web --no-typescript --out-dir "$out"
"$2" model "$crate_dir" Probe.tsx > "$out/model.json"
cp "$3" "$out/IBMPlexSans-Regular.ttf"
cp "$crate_dir/index.html" "$out/"
