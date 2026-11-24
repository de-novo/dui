"""Instrument a disposable upstream checkout, or capture that build on a fresh iPhone."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


REVISION = "55049f5745a7a6c1755e6d9c6641bb02f158fcc5"
BUNDLE = "com.stably.orca.mobile"


def instrument(mobile):
    revision = subprocess.check_output(["git", "-C", mobile, "rev-parse", "HEAD"], text=True).strip()
    if revision != REVISION:
        raise ValueError(f"expected Orca {REVISION}, got {revision}")
    layout = mobile / "app/_layout.tsx"
    subprocess.run(["git", "-C", mobile, "diff", "--quiet", "HEAD", "--", "app/_layout.tsx"], check=True)
    source = layout.read_text()
    anchor = "  const pathname = usePathname()"
    if source.count(anchor) != 1:
        raise ValueError("expected one Expo Router pathname observation point")
    imports = """import { File as CaptureFile, Paths as CapturePaths } from 'expo-file-system'
import { Dimensions, useWindowDimensions } from 'react-native'
"""
    # This effect records route and viewport only. The app renders and navigates normally.
    observation = """
  const { width, height, scale } = useWindowDimensions()
  useEffect(() => {
    if (pathname !== '/' && pathname !== '/about') return
    const name = pathname === '/' ? 'home' : 'about'
    const file = new CaptureFile(CapturePaths.document, `dui-observation-${name}.json`)
    file.create({ overwrite: true })
    file.writeSync(JSON.stringify({ pathname, width, height, scale, screen: Dimensions.get('screen') }))
  }, [pathname, width, height, scale])
"""
    layout.write_text(imports + source.replace(anchor, anchor + observation))


def normalized_text(text):
    return "".join(character for character in text.casefold() if character.isalnum())


def capture(products, checker, output):
    udid = os.environ["DUI_IOS_UDID"]
    xctestrun, = products.glob("*.xctestrun")
    output.mkdir(parents=True, exist_ok=False)
    recordings = []
    with tempfile.TemporaryDirectory() as work:
        result = Path(work) / "Capture.xcresult"
        attachments = Path(work) / "attachments"
        subprocess.run([
            "xcodebuild", "test-without-building", "-xctestrun", xctestrun,
            "-destination", f"platform=iOS Simulator,id={udid}",
            "-parallel-testing-enabled", "NO", "-resultBundlePath", result,
        ], check=True)
        subprocess.run(["xcrun", "xcresulttool", "export", "attachments",
                        "--path", result, "--output-path", attachments], check=True)
        manifest = json.loads((attachments / "manifest.json").read_text())
        container = subprocess.check_output(
            ["xcrun", "simctl", "get_app_container", udid, BUNDLE, "data"], text=True
        )
        for route, name, witness in [
            ("/", "home", "Connect your desktop"),
            ("/about", "about", "Open-source agent IDE"),
        ]:
            state_file = Path(container.strip()) / f"Documents/dui-observation-{name}.json"
            state = json.loads(state_file.read_text())
            if state["pathname"] != route or state["width"] <= 0 or state["height"] <= 0:
                raise ValueError(f"Invalid route observation {state}")
            filename = f"orca-{name}.png"
            match, = [item for test in manifest for item in test["attachments"]
                      if item["suggestedHumanReadableName"].startswith(f"orca-{name}")]
            frame = attachments / match["exportedFileName"]
            # The virtual Xcode 27 host lacks the Vision recognition service.
            # ImageIO still validates the PNG; CPU OCR checks its visible content.
            geometry = subprocess.check_output([checker, frame], text=True)
            lines = []
            for page_segmentation in ["11", "6"]:
                text = subprocess.check_output([
                    "tesseract", frame, "stdout", "-l", "eng", "--psm", page_segmentation
                ], text=True)
                lines.append(text)
                if normalized_text(witness) in normalized_text(text):
                    break
            else:
                raise ValueError(f"Missing {witness!r} in {filename}. OCR read {lines!r}")
            width, height = map(int, geometry.split())
            screen = state["screen"]
            if (width, height) != (round(screen["width"] * screen["scale"]), round(screen["height"] * screen["scale"])):
                raise ValueError(f"PNG {width}×{height} disagrees with iOS screen {screen}")
            pixels = frame.read_bytes()
            (output / filename).write_bytes(pixels)
            recordings.append({
                "runtime": "ios", "observedPath": route, "width": state["width"],
                "height": state["height"], "scale": state["scale"], "image": filename,
                "imageWidth": width, "imageHeight": height, "visibleText": witness,
                "sha256": hashlib.sha256(pixels).hexdigest(),
            })
    evidence = {"revision": REVISION, "device": os.environ["DUI_IOS_DEVICE"],
                "simulatorRuntime": os.environ["DUI_IOS_RUNTIME"], "captures": recordings}
    print(json.dumps(evidence, indent=2))
    (output / "evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "instrument":
        instrument(Path(sys.argv[2]))
    elif len(sys.argv) == 5 and sys.argv[1] == "capture":
        capture(*(Path(arg).resolve() for arg in sys.argv[2:]))
    else:
        sys.exit("Use instrument ORCA_MOBILE or capture BUILD_PRODUCTS SCREENSHOT_CHECKER OUTPUT_DIRECTORY")
