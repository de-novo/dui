"""Render the pinned Compass screens on a fresh iPhone simulator with the pinned Flutter SDK on PATH."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tempfile
import time
from ios_simulator import fresh_iphone

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "fixtures/compass-ios"
SAMPLES = "a5bccd247163b1017d2eb9320cf3b94a95ccfb5e"
FLUTTER = "ae9b586f34a8443573f82ef57c3bf914a908cbf7"


def read(*args):
    return subprocess.check_output(args, text=True).strip()


def capture_on_simulator(app_bundle, phone):
    with (app_bundle / "Info.plist").open("rb") as source:
        bundle_id = plistlib.load(source)["CFBundleIdentifier"]
    simctl = ["xcrun", "simctl"]
    print(f"Installing {bundle_id} on {phone.name}", flush=True)
    subprocess.run([*simctl, "install", phone.udid, app_bundle], check=True, timeout=180)
    container = subprocess.check_output(
        [*simctl, "get_app_container", phone.udid, bundle_id, "data"], text=True, timeout=30
    ).strip()
    capture = Path(container) / "Documents/dui-capture"
    if not container or capture.exists():
        raise RuntimeError("Expected a fresh Compass app data container")
    print(f"Launching {bundle_id} and waiting for the integration test", flush=True)
    subprocess.run([*simctl, "launch", phone.udid, bundle_id], check=True, timeout=90,
                   env={**os.environ, "SIMCTL_CHILD_COMPASS_CAPTURE_OUTPUT": str(capture)})
    result = capture / "result.json"
    deadline = time.monotonic() + 300
    reported = set()
    while not result.is_file():
        frames = {p.name for p in capture.glob("*.png")}
        if new_frames := frames - reported:
            print(f"Captured {', '.join(sorted(new_frames))}", flush=True)
            reported.update(new_frames)
        if time.monotonic() >= deadline:
            raise TimeoutError(f"Compass did not finish. Captured frames: {sorted(reported)}")
        time.sleep(1)
    report = json.loads(result.read_text())
    if report["passed"] is not True:
        raise RuntimeError(f"Compass integration test failed: {report['failures']}")
    return capture, report["screens"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("samples", type=Path, help="clean flutter/samples checkout")
    parser.add_argument("output", type=Path, help="new output directory")
    args = parser.parse_args()
    samples = args.samples.resolve()
    if read("git", "-C", samples, "rev-parse", "HEAD") != SAMPLES:
        raise ValueError(f"Expected flutter/samples at {SAMPLES}")
    subprocess.run(["git", "-C", samples, "diff", "--exit-code", "HEAD"], check=True)
    # Flutter can print first-run bootstrap progress before its machine-readable response.
    subprocess.run(["flutter", "--no-version-check", "precache", "--ios"], check=True)
    sdk = json.loads(read("flutter", "--no-version-check", "--version", "--machine"))
    if sdk["frameworkRevision"] != FLUTTER:
        raise ValueError(f"Expected Flutter 3.50.1 at {FLUTTER}")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)

    with fresh_iphone("compass") as phone:
        with tempfile.TemporaryDirectory() as temp:
            checkout = Path(temp) / "samples"
            subprocess.run(["git", "clone", "--quiet", "--no-local", "--no-checkout",
                            str(samples), str(checkout)], check=True)
            subprocess.run(["git", "-C", checkout, "checkout", "--quiet", "--detach", SAMPLES], check=True)
            app = checkout / "compass_app/app"
            target = "integration_test/dui_capture_test.dart"
            shutil.copyfile(FIXTURE / "capture_test.dart", app / target)
            subprocess.run(["flutter", "build", "ios", "--debug", "--simulator", "--target", target],
                           cwd=app, check=True)
            bundles = list((app / "build/ios/iphonesimulator").glob("*.app"))
            if len(bundles) != 1:
                raise RuntimeError(f"Expected one simulator app, found {bundles}")
            capture, rows = capture_on_simulator(bundles[0], phone)
            checker = Path(temp) / "check-image"
            subprocess.run(["xcrun", "swiftc", ROOT / "scripts/check-ios-screenshot.swift", "-o", checker], check=True)
            expected = {"/", "/search", "/results", "/activities", "/booking", "/booking/:id", "/login"}
            if len(rows) != len(expected) or {row["route"] for row in rows} != expected:
                raise ValueError("Expected all seven source routes")
            names = {row["name"] for row in rows}
            if len(names) != len(rows) or not all(re.fullmatch(r"[a-z-]+", name) for name in names):
                raise ValueError("Expected unique screenshot names")
            summary = [f"Native Compass capture on {phone.name} ({phone.runtime.split('.')[-1]}).",
                       f"flutter/samples `{SAMPLES}`, Flutter `{sdk['frameworkVersion']}`, source `lib/routing/router.dart`.",
                       "", "| Declaration | Observed route | PNG | Viewport (pt) | Image (px) | SHA-256 |",
                       "|---|---|---|---|---|---|"]
            for row in rows:
                image = output / (row["name"] + ".png")
                shutil.copyfile(capture / image.name, image)
                witness = [row["witness"]] if row["route"] == "/" else []
                w, h = map(int, read(checker, image, *witness).split())
                ratio = row["devicePixelRatio"]
                if abs(w / ratio - row["width"]) > 1 or abs(h / ratio - row["height"]) > 1:
                    raise ValueError(f"Screenshot size differs from the iOS view for {image.name}")
                digest = hashlib.sha256(image.read_bytes()).hexdigest()
                row.update(runtime="ios", image=image.name, imageWidth=w, imageHeight=h,
                           sha256=digest, source="lib/routing/router.dart")
                summary.append(f"| `{row['route']}` | `{row['observedPath']}` | `{image.name}` | "
                               f"{row['width']} × {row['height']} | {w} × {h} | `{digest}` |")
            evidence = dict(revision=SAMPLES, flutterRevision=FLUTTER, simulator=phone.name,
                            runtime=phone.runtime, screens=rows)
            (output / "screens.json").write_text(json.dumps(evidence, indent=2) + "\n")
            print("\n".join(summary))
            if os.environ.get("GITHUB_STEP_SUMMARY"):
                with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as stream:
                    stream.write("\n".join(summary) + "\n")


if __name__ == "__main__":
    main()
