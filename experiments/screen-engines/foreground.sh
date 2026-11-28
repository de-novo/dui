#!/bin/bash
set -eu
stage=$1
device=$2
here=$(cd "$(dirname "$0")" && pwd)
src=fixtures/swiftui-map/Sources/SwiftUIMapFixture
app="$stage/probe.app"
mkdir -p "$app"
sdk=$(xcrun --sdk iphonesimulator --show-sdk-path)
xcrun --sdk iphonesimulator swiftc -parse-as-library -O -target arm64-apple-ios18.0-simulator -sdk "$sdk" "$here/SceneText.swift" "$here/NativeProbe.swift" "$src/HomeView.swift" "$src/Screens.swift" -o "$app/Screen"
cat > "$app/Info.plist" <<'PLIST'
<plist version="1.0"><dict><key>CFBundleExecutable</key><string>Screen</string><key>CFBundleIdentifier</key><string>sh.dui.study.engineprobe</string><key>CFBundleVersion</key><string>1</string><key>CFBundlePackageType</key><string>APPL</string><key>UILaunchScreen</key><dict/></dict></plist>
PLIST
codesign -s - --force "$app"
xcrun simctl install "$device" "$app"
xcrun simctl get_app_container "$device" sh.dui.study.engineprobe data > "$stage/container"
xcrun simctl launch "$device" sh.dui.study.engineprobe
for width in 1440 390; do
  DUI_VIEWPORT=$width DUI_PROBE_SIMULATOR=$device "$stage/py/bin/python" "$here/native.py" "$stage" container "foreground-$width.json"
done
