"""Build fixtures/source-preview/Settings.swift into a SwiftUI app and capture it on a fresh iPhone simulator.

Writes ios.png and ios.json (provenance) into a new output directory. Requires macOS with Xcode.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import subprocess
import tempfile
import time
from ios_simulator import fresh_iphone

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "fixtures/source-preview/Settings.swift"
HOST = ROOT / "fixtures/source-preview-native/ios/Host.swift"
BUNDLE_ID = "sh.dui.sourcepreview"
DEPLOYMENT = "17.0"
# Text the component must show in its initial state; checked by OCR on the PNG itself.
WITNESS = "Notifications off"


def read(*args, **kwargs):
    return subprocess.check_output(args, text=True, **kwargs).strip()


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def build_app(temp):
    sdk = read("xcrun", "--sdk", "iphonesimulator", "--show-sdk-path")
    arch = read("uname", "-m")
    app = temp / "DuiSourcePreview.app"
    app.mkdir()
    subprocess.run(["xcrun", "--sdk", "iphonesimulator", "swiftc", "-parse-as-library",
                    "-target", f"{arch}-apple-ios{DEPLOYMENT}-simulator", "-sdk", sdk,
                    str(SOURCE), str(HOST), "-o", str(app / "DuiSourcePreview")], check=True)
    plist = {
        "CFBundleIdentifier": BUNDLE_ID,
        "CFBundleExecutable": "DuiSourcePreview",
        "CFBundleName": "DuiSourcePreview",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": "1.0",
        "CFBundleVersion": "1",
        "CFBundleSupportedPlatforms": ["iPhoneSimulator"],
        "LSRequiresIPhoneOS": True,
        "MinimumOSVersion": DEPLOYMENT,
        "UIDeviceFamily": [1],
        # Without a launch screen iOS runs the app in a letterboxed legacy size.
        "UILaunchScreen": {},
        "UIApplicationSceneManifest": {"UIApplicationSupportsMultipleScenes": True},
    }
    (app / "Info.plist").write_bytes(plistlib.dumps(plist))
    subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True)
    return app, read("xcrun", "swiftc", "--version").splitlines()[0]


def wait_for_screen(log):
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        if log.exists():
            for line in log.read_text().splitlines():
                if line.startswith("DUI_SCREEN "):
                    return json.loads(line[len("DUI_SCREEN "):])
        time.sleep(0.5)
    raise TimeoutError("The host app did not report its screen")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="new output directory")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory() as temp_dir:
        temp = Path(temp_dir)
        app, swift = build_app(temp)
        checker = temp / "check-image"
        subprocess.run(["xcrun", "swiftc", str(ROOT / "scripts/check-ios-screenshot.swift"),
                        "-o", str(checker)], check=True)
        with fresh_iphone("source-preview") as phone:
            subprocess.run(["xcrun", "simctl", "install", phone.udid, str(app)], check=True)
            log = temp / "stdout.log"
            subprocess.run(["xcrun", "simctl", "launch", f"--stdout={log}", phone.udid, BUNDLE_ID],
                           check=True)
            screen = wait_for_screen(log)
            # Let the first frame settle before reading pixels.
            time.sleep(2)
            image = output / "ios.png"
            subprocess.run(["xcrun", "simctl", "io", phone.udid, "screenshot", "--type=png", str(image)],
                           check=True)
            w, h = map(int, read(str(checker), str(image), WITNESS).split())
            scale = screen["scale"]
            if abs(w / scale - screen["width"]) > 1 or abs(h / scale - screen["height"]) > 1:
                raise ValueError("Screenshot size differs from the reported iOS screen")
            # com.apple.CoreSimulator.SimRuntime.iOS-26-1 -> 26.1
            os_version = phone.runtime.rsplit(".", 1)[-1].removeprefix("iOS-").replace("-", ".")
            device = phone.name
    record = {
        "runtime": "ios",
        "source": "ios/Settings.swift",
        "entry": "Settings",
        "fixture": "fixtures/source-preview/Settings.swift",
        "sourceSha256": sha256(SOURCE),
        "host": "fixtures/source-preview-native/ios/Host.swift",
        "hostSha256": sha256(HOST),
        "image": "ios.png",
        "imageWidth": w,
        "imageHeight": h,
        "imageSha256": sha256(image),
        "width": round(screen["width"]),
        "scale": scale,
        "device": device,
        "os": f"iOS {os_version}",
        "toolchain": swift,
        "witness": WITNESS,
        "commit": os.environ.get("GITHUB_SHA", read("git", "-C", str(ROOT), "rev-parse", "HEAD")),
        "workflowRun": run_url(),
    }
    (output / "ios.json").write_text(json.dumps(record, indent=2) + "\n")
    summary(record)


def run_url():
    server, repo, run = (os.environ.get(k) for k in ("GITHUB_SERVER_URL", "GITHUB_REPOSITORY", "GITHUB_RUN_ID"))
    return f"{server}/{repo}/actions/runs/{run}" if server and repo and run else None


def summary(record):
    text = (f"Native SwiftUI capture on {record['device']} ({record['os']}): "
            f"{record['imageWidth']} × {record['imageHeight']} px, {record['width']} pt wide, "
            f"OCR witness `{record['witness']}`, image SHA-256 `{record['imageSha256']}`.")
    print(text)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as stream:
            stream.write(text + "\n")


if __name__ == "__main__":
    main()
