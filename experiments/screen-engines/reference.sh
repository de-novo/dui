#!/bin/bash
set -eu
# Opt-in reference capture. The simulator must be available and selected by the caller.
out=$1
device=$2
root=$(git rev-parse --show-toplevel)
sdk=$(xcrun --sdk iphonesimulator --show-sdk-path)
xcrun simctl bootstatus "$device" -b
for view in DetailView ProfileView; do
  app="$out/$view.app"
  mkdir -p "$app"
  echo "import SwiftUI
@main struct StudyApp: App { var body: some Scene { WindowGroup { $view() } } }" > "$out/App.swift"
  /usr/bin/time -l xcrun --sdk iphonesimulator swiftc -parse-as-library -target arm64-apple-ios18.0-simulator -sdk "$sdk" "$out/App.swift" "$root/fixtures/swiftui-map/Sources/SwiftUIMapFixture/Screens.swift" "$root/fixtures/swiftui-map/Sources/SwiftUIMapFixture/HomeView.swift" -o "$app/Screen" 2>"$out/$view-build.time"
  cat > "$app/Info.plist" <<PLIST
<plist version="1.0"><dict><key>CFBundleExecutable</key><string>Screen</string><key>CFBundleIdentifier</key><string>sh.dui.study.$view</string><key>CFBundleVersion</key><string>1</string><key>CFBundlePackageType</key><string>APPL</string><key>UILaunchScreen</key><dict/></dict></plist>
PLIST
  codesign -s - --force "$app" >/dev/null
  xcrun simctl install "$device" "$app"
  xcrun simctl launch "$device" "sh.dui.study.$view"
  sleep 2
  xcrun simctl io "$device" screenshot "$out/$view.png"
  xcrun simctl terminate "$device" "sh.dui.study.$view"
  xcrun simctl uninstall "$device" "sh.dui.study.$view"
done
