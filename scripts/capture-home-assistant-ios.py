"""Capture home-assistant/iOS screens on a fresh iPhone simulator and attach them to its dui map.

Run after `dui map` on the pinned checkout, with the same HOME. The app is rebuilt with local
signing (simulator builds embed their entitlements only then; HA needs its app group), installed
in English, and each screen is captured once its expected text is visible (OCR on the PNG).
The attached run keeps every cell `unverified`. Writes the attached run's files and
provenance.json into a new output directory.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import tempfile
import time
from ios_simulator import fresh_iphone

ROOT = Path(__file__).resolve().parents[1]
# Screens reachable without a Home Assistant server: (declaration name, OCR witness).
SCREENS = [("OnboardingWelcomeView", "Home Assistant Companion App")]


def read(*args, **kwargs):
    return subprocess.check_output([str(a) for a in args], text=True, **kwargs).strip()


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def screen_scale(udid):
    devices = json.loads(read("xcrun", "simctl", "list", "devices", "-j"))["devices"]
    kind = next(d["deviceTypeIdentifier"] for ds in devices.values() for d in ds if d["udid"] == udid)
    types = json.loads(read("xcrun", "simctl", "list", "devicetypes", "-j"))["devicetypes"]
    bundle = next(t["bundlePath"] for t in types if t["identifier"] == kind)
    profile = plistlib.loads((Path(bundle) / "Contents/Resources/profile.plist").read_bytes())
    return profile["mainScreenScale"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("checkout", type=Path, help="home-assistant/iOS checkout that was mapped")
    parser.add_argument("map", type=Path, help="JSON printed by `dui map`")
    parser.add_argument("output", type=Path, help="new output directory")
    args = parser.parse_args()
    checkout = args.checkout.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    home = Path(os.environ["HOME"])
    run = json.loads(args.map.read_text())
    source = Path(run["output"])
    graph = json.loads((source / "graph.json").read_text())
    by_name = {node["nativeSource"]["name"]: node for node in graph["nodes"]}
    derived = next((home / ".dui" / "swift-index").iterdir())
    scheme = json.loads((checkout / "dui.json").read_text())["scheme"]
    dui = os.environ.get("DUI_BIN", str(ROOT / "target/debug/dui"))

    subprocess.run(["xcodebuild", "-project", "HomeAssistant.xcodeproj", "-scheme", scheme,
                    "-destination", "generic/platform=iOS Simulator", "-derivedDataPath", str(derived),
                    "-quiet", "build", "CODE_SIGN_IDENTITY=-", "CODE_SIGN_STYLE=Manual",
                    "DEVELOPMENT_TEAM=", "PROVISIONING_PROFILE_SPECIFIER="], cwd=checkout, check=True)
    app = next((derived / "Build/Products/Debug-iphonesimulator").glob("*.app"))
    bundle = plistlib.loads((app / "Info.plist").read_bytes())["CFBundleIdentifier"]

    with tempfile.TemporaryDirectory() as temp_dir:
        temp = Path(temp_dir)
        checker = temp / "check-image"
        subprocess.run(["xcrun", "swiftc", str(ROOT / "scripts/check-ios-screenshot.swift"),
                        "-o", str(checker)], check=True)
        captures, evidence = [], []
        with fresh_iphone("home-assistant") as phone:
            scale = screen_scale(phone.udid)
            subprocess.run(["xcrun", "simctl", "install", phone.udid, str(app)], check=True)
            for name, witness in SCREENS:
                node = by_name[name]
                subprocess.run(["xcrun", "simctl", "launch", "--terminate-running-process", phone.udid,
                                bundle, "-AppleLanguages", "(en)", "-AppleLocale", "en_US"], check=True)
                image = temp / f"{name}.png"
                deadline = time.monotonic() + 90
                while True:
                    time.sleep(3)
                    subprocess.run(["xcrun", "simctl", "io", phone.udid, "screenshot", "--type=png",
                                    str(image)], check=True, capture_output=True)
                    found = subprocess.run([str(checker), str(image), witness], capture_output=True, text=True)
                    if found.returncode == 0:
                        width, height = map(int, found.stdout.split())
                        break
                    if time.monotonic() > deadline:
                        raise TimeoutError(f"{name}: `{witness}` never appeared")
                captures.append({"screenId": node["id"], "runtime": "ios", "observedPath": node["label"],
                                 "width": round(width / scale), "image": image.name})
                evidence.append({"name": name, "source": node["nativeSource"]["source"], "witness": witness,
                                 "imageWidth": width, "imageHeight": height, "scale": scale,
                                 "imageSha256": sha256(image)})
        manifest = temp / "recordings.json"
        manifest.write_text(json.dumps({"sourceSnapshot": source.name, "captures": captures}))
        attached = json.loads(read(dui, "map", "attach", source, manifest))
        for item in Path(attached["output"]).iterdir():
            shutil.copy2(item, output / item.name)
    shutil.copy2(checkout / "LICENSE.md", output / "LICENSE.md")

    xcode = read("xcodebuild", "-version").splitlines()[0]
    provenance = {
        "repository": "https://github.com/home-assistant/iOS",
        "commit": read("git", "-C", checkout, "rev-parse", "HEAD"),
        "scheme": scheme, "xcode": xcode, "device": phone.name, "runtime": phone.runtime,
        "declarations": len(graph["nodes"]), "captures": evidence,
        "workflowRun": "/".join(os.environ.get(k, "") for k in ("GITHUB_SERVER_URL", "GITHUB_REPOSITORY"))
        + f"/actions/runs/{os.environ['GITHUB_RUN_ID']}" if os.environ.get("GITHUB_RUN_ID") else None,
    }
    (output / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    summary = (f"home-assistant/iOS {provenance['commit'][:7]}: {provenance['declarations']} declarations, "
               f"{len(evidence)} simulator capture(s) attached as unverified on {phone.name} ({xcode}).")
    print(summary)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as stream:
            stream.write(summary + "\n")


if __name__ == "__main__":
    main()
