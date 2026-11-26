"""Capture home-assistant/iOS screens on a fresh iPhone simulator and attach them to its dui map.

Run after `dui map` on the pinned checkout, with the same HOME. The Tests-UI scheme is built with
local signing (simulator builds embed their entitlements only then; HA needs its app group) and
runs fixtures/home-assistant-ios-capture in English: it walks onboarding without a server and
captures each screen once an element with the app's own accessibility identifier exists.
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
from ios_simulator import fresh_iphone

ROOT = Path(__file__).resolve().parents[1]
# Screens reachable without a Home Assistant server, in the order the capture flow visits them:
# (declaration name, the app's own accessibility identifier that proves the screen is showing).
SCREENS = [
    ("OnboardingWelcomeView", "onboarding.welcome.continue"),
    ("OnboardingServersListView", "onboarding.servers.manualEntry"),
    ("ManualURLEntryView", "onboarding.manualEntry.connect"),
]


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

    # The capture flow replaces a test file that is already in the Tests-UI target.
    shutil.copy2(ROOT / "fixtures/home-assistant-ios-capture/HomeAssistantUITests.swift",
                 checkout / "Tests/UI/HomeAssistantUITests.swift")
    signing = ["CODE_SIGN_IDENTITY=-", "CODE_SIGN_STYLE=Manual", "DEVELOPMENT_TEAM=",
               "PROVISIONING_PROFILE_SPECIFIER="]

    with tempfile.TemporaryDirectory() as temp_dir:
        temp = Path(temp_dir)
        captures, evidence = [], []
        with fresh_iphone("home-assistant") as phone:
            scale = screen_scale(phone.udid)
            # A fresh simulator covers the keyboard with a one-time typing tip; it is not app UI.
            subprocess.run(["xcrun", "simctl", "spawn", phone.udid, "defaults", "write",
                            "com.apple.keyboard.preferences", "DidShowContinuousPathIntroduction",
                            "-bool", "true"], check=True)
            result = temp / "capture.xcresult"
            subprocess.run(["xcodebuild", "test", "-project", "HomeAssistant.xcodeproj", "-scheme", "Tests-UI",
                            "-destination", f"platform=iOS Simulator,id={phone.udid}",
                            "-derivedDataPath", str(derived), "-parallel-testing-enabled", "NO",
                            "-only-testing:Tests-UI/HomeAssistantUITests/testDuiCaptures",
                            "-resultBundlePath", str(result), "-quiet", *signing], cwd=checkout, check=True)
            attachments = temp / "attachments"
            subprocess.run(["xcrun", "xcresulttool", "export", "attachments", "--path", str(result),
                            "--output-path", str(attachments)], check=True)
            manifest_rows = json.loads((attachments / "manifest.json").read_text())
            exported = [entry for test in manifest_rows for entry in test["attachments"]
                        if entry["suggestedHumanReadableName"].startswith("dui-capture-")]
            for name, witness in SCREENS:
                matches = [e for e in exported
                           if e["suggestedHumanReadableName"].startswith(f"dui-capture-{name}_")]
                if len(matches) != 1:
                    raise ValueError(f"Expected one capture for {name}, found {len(matches)}")
                node = by_name[name]
                image = temp / f"{name}.png"
                shutil.copyfile(attachments / matches[0]["exportedFileName"], image)
                png = image.read_bytes()
                if png[:8] != bytes.fromhex("89504e470d0a1a0a"):
                    raise ValueError(f"{name}: capture is not a PNG")
                width, height = int.from_bytes(png[16:20], "big"), int.from_bytes(png[20:24], "big")
                captures.append({"screenId": node["id"], "runtime": "ios", "observedPath": node["label"],
                                 "width": round(width / scale), "image": image.name})
                evidence.append({"name": name, "source": node["nativeSource"]["source"],
                                 "witness": f"accessibility identifier {witness}",
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
