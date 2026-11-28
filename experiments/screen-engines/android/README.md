# Compose simulator probe

Use a disposable, booted arm64 AVD with the Android SDK, build-tools, API 37.1 platform, JDK 17 and Gradle 9.9.3. This experiment uses a separate APK and adb serial. Run from the repository root with an absolute temporary stage directory.

```sh
cp -R fixtures/android-compose "$stage/android-probe"
src="$stage/android-probe/src/main/java/sh/dui/fixture"
cp experiments/screen-engines/android/Examples.kt "$src"
cp experiments/screen-engines/android/ProbeActivity.kt "$src/SettingsActivity.kt"
gradle --max-workers=2 -p "$stage/android-probe" assembleDebug
python3 -m venv "$stage/py"
"$stage/py/bin/pip" install Pillow==13.0.0 tree-sitter==0.27.0 tree-sitter-kotlin==1.1.0
"$stage/py/bin/python" experiments/screen-engines/android/run.py "$stage" "$serial"
```

Confirm the Android first-use fullscreen overlay on the disposable device before the scored run. The initial trial captured this OS overlay and was excluded. Disable system transition/window/animator animations on the disposable AVD. The driver sets density to 160, reads whole emulator framebuffers at 1440×920 and 390×844, and resets the size and density. The two reference functions compile directly to Compose. The candidate projects a single literal Text call using the Kotlin tree-sitter grammar, then materializes the record as Compose Text. This tests the same narrow surface scope as the SwiftUI probe. The separate real settings fixtures use additional widgets and layout.

Eight alternating text edits per screen/width. The host measures save, fresh parser process, adb model publication and native acknowledgement after two frame callbacks. The subsequent framebuffer read independently verifies the visible output. Analysis times include the Python process startup; RSS is from Darwin time. Device memory uses Android dumpsys after the scenario. APK bytes cover the entire debug test application including reference functions and runtime.
