#!/usr/bin/env bash
# Run from the research checkout with an explicit Chrome reference binary.
set -euo pipefail
: "${1:?usage: reproduce.sh /absolute/path/to/chrome}"
dir=$(cd "$(dirname "$0")" && pwd)
repo=$(git -C "$dir" rev-parse --show-toplevel)
baseline=0ae1b928641c0fc09bb6cf70434bd3d52444a1e8
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
git -C "$repo" archive "$baseline" | tar -x -C "$work"
mkdir -p "$work/crates/dui-lang-typescript/examples"
cp "$dir/../renderer-geometry/src/bin/fixture-model.rs" "$work/crates/dui-lang-typescript/examples/ratio_measure.rs"
cargo build --locked --manifest-path "$work/Cargo.toml" -p dui-lang-typescript --example ratio_measure --release --target-dir "$work/target"
npm ci --prefix "$dir"
node "$dir/capture.mjs" --capture "$1" "$work" "$work/target/release/examples/ratio_measure"
