# GPUI native preview comparison

This macOS-only research executable evaluates native previews. Open each width's contact sheet in `evidence/` to compare the platform reference with the captured GPUI window. Crops are shown at one logical pixel per image pixel. The ZIP contains the entire logical-resolution screens, and results.json contains individual save observations and ink bounds.

The three fixtures per platform cover centered Detail and Profile labels and a horizontally and vertically nested Stack with a constrained paragraph. The projector watches source files, reads literal Text nodes from Swift's parse tree or the Kotlin syntax tree, and emits a private experimental model. The layout scaffold and typography settings are encoded for these fixtures. Changing layout source, arbitrary native controls, interaction, and native app rebuild time are outside this measurement.

Both GPUI modes use Taffy 0.16.0 for layout and cosmic-text 0.19.0 for intrinsic sizes. The cosmic mode draws Swash glyphs into a logical-resolution image inside GPUI. The host control uses GPUI text on macOS with its own tracking and display-scale rasterization. The iOS control selects the host system font and the Android control registers the device Roboto font. GPUI's own dependency graph also includes Taffy 0.15.1.

## Reproduction

Use an Apple Silicon Mac with a desktop session and window capture permission. The run used macOS 26.5.2, a 2x display, Xcode's iOS 26.5 simulator, and the Android API 37 emulator at 160 dpi. Install Python Pillow, watchfiles, tree-sitter and tree-sitter-kotlin. Run the following from this directory with a new temporary directory each time. Set SF to the simulator runtime's System/Library/Fonts/Core/SFUI.ttf, IOS_DEVICE to a booted simulator and ANDROID_SERIAL to the test emulator.

```sh
STAGE=$(mktemp -d)
export CARGO_TARGET_DIR="$STAGE/target"
cargo +nightly-2026-10-31 build --locked
xcrun swiftc -parse-as-library -O Capture.swift -o "$STAGE/capture"
adb -s "$ANDROID_SERIAL" pull /system/fonts/Roboto-Regular.ttf "$STAGE/Roboto.ttf"
xcrun swiftc -parse-as-library -O -target arm64-apple-ios18.0-simulator -sdk "$(xcrun --sdk iphonesimulator --show-sdk-path)" fixtures/Stack.swift -o "$STAGE/Stack"
xcrun simctl spawn "$IOS_DEVICE" "$STAGE/Stack" "$STAGE"
cp -R ../../fixtures/android-compose "$STAGE/android"
cp fixtures/Stack.kt "$(find "$STAGE/android" -name SettingsActivity.kt | head -1)"
gradle -p "$STAGE/android" assembleDebug
adb -s "$ANDROID_SERIAL" install -r "$(find "$STAGE/android/build" -name '*.apk' | head -1)"
adb -s "$ANDROID_SERIAL" shell wm density 160
adb -s "$ANDROID_SERIAL" shell am start -n sh.dui.fixture/.SettingsActivity
```

Set the emulator display with `adb shell wm size` to 390x844 and 1440x920 in turn. Check that the activity displays all four Stack labels, then capture each to `$STAGE/android-stack-390.png` or `$STAGE/android-stack-1440.png` via `adb exec-out screencap -p`. Detail and Profile references come from the committed archives in the adjacent screen-engines experiment. The iOS references are actual SwiftUI ImageRenderer surfaces. The Android references are emulator screenshots.

```sh
python run.py "$STAGE" "$SF"
python report.py "$STAGE"
```

The observer identifies the GPUI window by process and exact logical size using the system window list. It samples that window with screencapture, downsamples Retina output using Lanczos, and records the time the screenshot command returns. Two calibration transitions establish baseline and edited pixel hashes. Each of eight timed source writes must produce its expected hash within three seconds. These timings include watcher debounce, source projection, rendering and observer sampling/capture overhead. They are upper bounds on first visibility. The GPUI window reads the experimental model on animation frames. The tests measure active editing, not idle resource use.

The strict score requires full RGB pixel equality on white. The second score counts pixels with maximum channel difference greater than 20 inside the union of both images' ink (any channel below 235). A screen passes that score at up to 1% changed ink. The per-case JSON also gives bounding rectangles. PNGs and raw timings support other tolerances.

## Licenses

GPUI 0.3.14 and its macOS platform crate declare Apache-2.0. Taffy declares MIT. cosmic-text and Swash offer MIT or Apache-2.0. The target-specific normal/build inventory in evidence/licenses.json includes BSD-3-Clause, Zlib-only, Unicode-3.0 and build-time cbindgen (MPL-2.0) entries. The repository's dependency list permits Zlib but does not list Unicode-3.0 or a cbindgen MPL exception. Resolve this before adopting the full dependency graph. The pulled Roboto font identifies SIL OFL 1.1 in its embedded name table. Simulator SFUI identifies Apple's OS software license and was used locally. This experiment distributes neither font file.

The default stable compiler on this machine (1.93.1) failed on experimental GPUI's transitive compiler requirements. The pinned nightly above built this research executable. The streaming capture observer did not reliably deliver initial frames in this session, so the reported measurements use window screenshot polling throughout.

The optional host-text control (pass `host` to run.py) displayed a blank content region on the initial iOS Detail fixture even though the model revision advanced. Testing the documented GPUI system font alias also yielded a blank capture. Calibration failed, so host scores and timings were excluded. See evidence/host-failed.png.

The pixel score uses the union of reference and preview ink (any RGB channel below 235), counting a mismatch where any channel differs by more than 20. The 32-pixel outer window margin is excluded only from ink counts and ink bounds. Strict equality compares the entire frame. The acceptance cutoff is at most 1% mismatched ink.

The initial Android single-label run passed 0.5 logical pixels directly to cosmic-text’s em-based letter-spacing API. The corrected run divides by the 16-pixel font size. The four affected cases were rerun with the same sources and reference images. The initial observations are included in evidence/initial-tracking.json.
