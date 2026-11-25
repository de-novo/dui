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
SCREENS = (("/", "home", "Connect your desktop"), ("/about", "about", "Open-source agent IDE"))


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


def wait_for_visible_frame(app_containers, process, name, witness):
    previous = None
    while process.poll() is None:
        for candidate in app_containers.glob(f"*/Documents/orca-{name}.candidate.png"):
            pixels = candidate.read_bytes()
            if pixels == previous:
                continue
            previous = pixels
            for mode in ("11", "6"):
                result = subprocess.run(["tesseract", "stdin", "stdout", "-l", "eng", "--psm", mode],
                                        input=pixels, stdout=subprocess.PIPE, check=True, timeout=25)
                if normalized_text(witness) in normalized_text(result.stdout.decode("utf-8")):
                    candidate.with_name(f"orca-{name}.accepted").touch()
                    print(f"Confirmed {witness!r} in the {name} screenshot", flush=True)
                    return pixels
        time.sleep(0.4)
    raise RuntimeError(f"XCTest exited with {process.returncode} before OCR confirmed {witness!r}")


def capture(products, checker, output):
    udid = os.environ["DUI_IOS_UDID"]
    xctestrun, = products.glob("*.xctestrun")
    output.mkdir(parents=True, exist_ok=False)
    recordings = []
    devices = json.loads(subprocess.check_output(["xcrun", "simctl", "list", "devices", "-j"]))["devices"]
    device = next(device for group in devices.values() for device in group if device["udid"] == udid)
    app_containers = Path(device["dataPath"]) / "Containers/Data/Application"
    with tempfile.TemporaryDirectory() as work:
        result = Path(work) / "Capture.xcresult"
        frames = {}
        with subprocess.Popen([
            "xcodebuild", "test-without-building", "-xctestrun", xctestrun,
            "-destination", f"platform=iOS Simulator,id={udid}",
            "-parallel-testing-enabled", "NO", "-resultBundlePath", result,
        ]) as process:
            try:
                for _, name, witness in SCREENS:
                    frames[name] = wait_for_visible_frame(app_containers, process, name, witness)
                if process.wait() != 0:
                    raise subprocess.CalledProcessError(process.returncode, process.args)
            finally:
                if process.poll() is None:
                    process.terminate()
        container = subprocess.check_output(
            ["xcrun", "simctl", "get_app_container", udid, BUNDLE, "data"], text=True
        )
        for route, name, witness in SCREENS:
            state_file = Path(container.strip()) / f"Documents/dui-observation-{name}.json"
            state = json.loads(state_file.read_text())
            if state["pathname"] != route or state["width"] <= 0 or state["height"] <= 0:
                raise ValueError(f"Invalid route observation {state}")
            filename = f"orca-{name}.png"
            pixels = frames[name]
            frame = Path(work) / filename
            frame.write_bytes(pixels)
            geometry = subprocess.check_output([checker, frame], text=True)
            width, height = map(int, geometry.split())
            screen = state["screen"]
            if (width, height) != (round(screen["width"] * screen["scale"]), round(screen["height"] * screen["scale"])):
                raise ValueError(f"PNG {width}×{height} disagrees with iOS screen {screen}")
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
