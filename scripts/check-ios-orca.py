"""Instrument a disposable upstream checkout, or capture that build on a fresh iPhone."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


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
    const file = new CaptureFile(CapturePaths.document, 'dui-observation.json')
    file.create({ overwrite: true })
    file.writeSync(JSON.stringify({ pathname, width, height, scale, screen: Dimensions.get('screen') }))
  }, [pathname, width, height, scale])
"""
    layout.write_text(imports + source.replace(anchor, anchor + observation))


def capture(app, checker, output):
    udid = os.environ["DUI_IOS_UDID"]
    simctl = ["xcrun", "simctl"]
    subprocess.run([*simctl, "install", udid, app], check=True)
    subprocess.run([*simctl, "launch", udid, BUNDLE, "-AppleLanguages", "(en)", "-AppleLocale", "en_US"], check=True)
    container = subprocess.check_output([*simctl, "get_app_container", udid, BUNDLE, "data"], text=True)
    state_file = Path(container.strip()) / "Documents/dui-observation.json"
    output.mkdir(parents=True, exist_ok=False)
    recordings = []
    with tempfile.TemporaryDirectory() as work:
        frame = Path(work) / "candidate.png"
        for route, witness, filename in [
            ("/", "Connect your desktop", "orca-home.png"),
            ("/about", "Open-source agent IDE", "orca-about.png"),
        ]:
            if route != "/":
                subprocess.run([*simctl, "openurl", udid, "orca://" + route.lstrip("/")], check=True)
            deadline = time.monotonic() + 120
            state = None
            while time.monotonic() < deadline:
                time.sleep(1)
                try:
                    state = json.loads(state_file.read_text())
                except (FileNotFoundError, json.JSONDecodeError):
                    continue
                if state["pathname"] != route or state["width"] <= 0 or state["height"] <= 0:
                    continue
                subprocess.run([*simctl, "io", udid, "screenshot", "--type=png", frame], check=True)
                check = subprocess.run([checker, frame, witness], capture_output=True, text=True)
                if check.returncode != 0:
                    continue
                width, height = map(int, check.stdout.split())
                screen = state["screen"]
                if (width, height) != (round(screen["width"] * screen["scale"]), round(screen["height"] * screen["scale"])):
                    raise ValueError(f"PNG {width}×{height} disagrees with iOS screen {screen}")
                if json.loads(state_file.read_text()) != state:
                    continue
                pixels = frame.read_bytes()
                (output / filename).write_bytes(pixels)
                recordings.append({
                    "runtime": "ios", "observedPath": route, "width": state["width"],
                    "height": state["height"], "scale": state["scale"], "image": filename,
                    "imageWidth": width, "imageHeight": height, "visibleText": witness,
                    "sha256": hashlib.sha256(pixels).hexdigest(),
                })
                break
            else:
                raise RuntimeError(f"Timed out waiting for {route} to display {witness!r}; last observation: {state}")
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
        sys.exit("Use instrument ORCA_MOBILE or capture APP_BUNDLE SCREENSHOT_CHECKER OUTPUT_DIRECTORY")
