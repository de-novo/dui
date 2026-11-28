#!/bin/bash
set -eu
stage=$1
device=$2
here=$(cd "$(dirname "$0")" && pwd)
src=fixtures/swiftui-map/Sources/SwiftUIMapFixture
sdk=$(xcrun --sdk iphonesimulator --show-sdk-path)
mkdir -p "$stage/surface/Documents"
echo "$stage/surface" > "$stage/surface-container"
xcrun --sdk iphonesimulator swiftc -parse-as-library -O -target arm64-apple-ios18.0-simulator -sdk "$sdk" "$here/SurfaceProbe.swift" "$here/SceneText.swift" "$src/Screens.swift" "$src/HomeView.swift" -o "$stage/SurfaceProbe"
xcrun simctl spawn "$device" "$stage/SurfaceProbe" "$stage/surface/Documents" &
pid=$!
python3 "$here/native.py" "$stage" surface-container surface-matrix.json
wait "$pid"
