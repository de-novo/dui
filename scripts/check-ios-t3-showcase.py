"""Run the pinned app's two native showcase scenes on an isolated GitHub macOS runner."""

import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import time
import uuid


REVISION = "f274d47150dfe0df5ed96bb27cb490a023e75500"
DEVICE = "T3 Showcase iPhone 17 Pro Max"
DEVICE_TYPE = "com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro-Max"
SCENES = (
    ("thread", "Thread", "", "92fc17514fa134896f838741c755fe9b491b34201ea7b2174f108d25d4032a15"),
    ("review", "ThreadReview", "/review", "7e3c32693db85ff1348e90ad4e617a0f2164f19c0e8663ddb8bcff425fa53774"),
)


def output(*command):
    return subprocess.check_output(command, text=True).strip()


def simulator_keychain_groups(app):
    executable = app / plistlib.loads((app / "Info.plist").read_bytes())["CFBundleExecutable"]
    # Xcode places simulator entitlements in a Mach-O section of the arm64 executable.
    listing = output("xcrun", "otool", "-l", executable)
    section = re.search(r"sectname __entitlements\s+segname __TEXT\s+addr \S+\s+size (0x[0-9a-fA-F]+)\s+offset (\d+)", listing)
    if section is None:
        signature = subprocess.run(["codesign", "--display", "--entitlements", "-", "--xml", executable], capture_output=True)
        if not signature.stdout.startswith(b"<?xml"):
            return []
        entitlements = plistlib.loads(signature.stdout)
    else:
        size, offset = section.groups()
        with executable.open("rb") as data:
            data.seek(int(offset))
            entitlements = plistlib.loads(data.read(int(size, 16)).rstrip(b"\x00"))
    return entitlements.get("keychain-access-groups") or [entitlements.get("application-identifier")]


def prepare_simulator_keychain(app):
    if any(simulator_keychain_groups(app)):
        return
    # T3 persists the fixture's device ID and pairing in the simulator Keychain.
    # Let Xcode generate its simulated entitlements with a local ad-hoc identity.
    derived = app.parents[3]
    mobile = app.parents[5]
    with tempfile.TemporaryDirectory() as tmp:
        config = Path(tmp) / "simulator.xcconfig"
        config.write_text("CODE_SIGN_IDENTITY[sdk=iphonesimulator*] = -\n"
                          "CODE_SIGNING_ALLOWED[sdk=iphoneos*] = NO\n"
                          "PROVISIONING_PROFILE_SPECIFIER =\n")
        subprocess.run(["xcodebuild", "-quiet", "-workspace", mobile / "ios/T3Code.xcworkspace",
                        "-scheme", "T3Code", "-configuration", "Release", "-sdk", "iphonesimulator",
                        "-derivedDataPath", derived, "ARCHS=arm64", "ONLY_ACTIVE_ARCH=YES", "build"],
                       env=dict(os.environ, XCODE_XCCONFIG_FILE=str(config)), check=True)
    if not any(simulator_keychain_groups(app)):
        raise RuntimeError("The iOS simulator executable still lacks Keychain entitlements")
    # Only this constant can leave the private build log.
    print("Simulator Keychain ready via local ad-hoc identity", flush=True)


def install_route_observer(source):
    coordinator = source / "apps/mobile/src/features/showcase/ShowcaseCaptureCoordinator.tsx"
    code = coordinator.read_text()
    anchor = "    if (requestedScene !== null && hasFixture"
    if code.count(anchor) != 1:
        raise ValueError("The pinned showcase navigation observation point changed")
    # Observe existing runtime values. Upstream still handles pairing, navigation, and readiness.
    observation = """
    const evidence = JSON.stringify({ path: rootState ? pathnameFromState(rootState).split(/[?#]/)[0] : "",
      requestedScene, scene, launchRequestRead, orientationSettled, themeApplied, hasFixture,
      environments: appAtomRegistry.get(workspaceConnections.environmentsAtom).length,
      projects: projects.length, threads: threads.length });
    if (evidence !== duiPreviousObservation) {
      try {
        const name = scene === "thread" || scene === "review" ? scene : "startup";
        const file = new DuiFile(DuiPaths.document, "dui-showcase-" + name + ".json");
        file.create({ overwrite: true });
        file.writeSync(evidence);
        duiPreviousObservation = evidence;
      } catch {}
    }
"""
    imports = 'import { File as DuiFile, Paths as DuiPaths } from "expo-file-system";\n'
    coordinator.write_text(imports + 'let duiPreviousObservation = "";\n' + code.replace(anchor, observation + anchor))


def observe_startup(udid, temporary, device_data):
    image = temporary / "startup.png"
    capture = subprocess.run(["xcrun", "simctl", "io", udid, "screenshot", image], capture_output=True)
    if capture.returncode == 0:
        seen = subprocess.run(["tesseract", image, "stdout", "-l", "eng", "--psm", "11"],
                              capture_output=True, text=True, errors="replace", timeout=20)
        labels = ("development servers", "add environment", "connect", "remote-first", "review changes",
                  "uncaught error", "failed to load", "welcome", "home screen")
        recognized = [label for label in labels if label in seen.stdout.lower()]
        print(f"Simulator startup witnesses: {recognized}", flush=True)
    else:
        print(f"simctl screenshot exited with {capture.returncode}", flush=True)
    for observation in device_data.glob("Containers/Data/Application/*/Documents/dui-showcase-*.json"):
        try:
            info = json.loads(observation.read_text())
        except (FileNotFoundError, ValueError):
            continue
        # A diagnostic contains counts and booleans only. Successful routes are published below.
        print("Showcase state:", {key: value for key, value in info.items() if key != "path"}, flush=True)
    for path in Path.home().glob("Library/Logs/DiagnosticReports/T3Code*.ips"):
        try:
            crash = json.loads(path.read_text().split("\n", 1)[1])
            print("App crash:", crash.get("exception", {}).get("type"), crash.get("termination", {}).get("namespace"))
        except (ValueError, IndexError):
            pass


def run_capture(source, temporary, udid, device_data):
    pnpm_version = json.loads((source / "package.json").read_text())["packageManager"].split("@")[1]
    pnpm_dir = Path.home() / ".vite-plus/package_manager/pnpm" / pnpm_version / "pnpm/bin"
    xcconfig = temporary / "unsigned.xcconfig"
    xcconfig.write_text("CODE_SIGNING_ALLOWED = NO\nCODE_SIGNING_REQUIRED = NO\n")
    env = dict(os.environ, PATH=f"{pnpm_dir}:{os.environ['PATH']}", TMPDIR=str(temporary),
               XCODE_XCCONFIG_FILE=str(xcconfig))
    # Build the Release simulator product so the app uses its checked-out JS bundle.
    # Its local fixture servers, scene selection, readiness and screenshot steps stay upstream.
    original = source / "scripts/mobile-showcase.ts"
    driver = original.with_name(".dui-ios-showcase.ts")
    harness = original.read_text()
    product_hook = "return IOS_APP_PATH;"
    if harness.count('"Debug"') != 1 or harness.count("Debug-iphonesimulator") != 1 or harness.count(product_hook) != 1:
        raise ValueError("The pinned driver's iOS build configuration changed")
    helper = json.dumps(str(Path(__file__).resolve()))
    inspect = f'await runCommand("python3", [{helper}, "--check-simulator-keychain", IOS_APP_PATH], {{ env: MOBILE_BUILD_ENV }});'
    harness = harness.replace(product_hook, inspect + "\n  " + product_hook)
    driver.write_text(harness.replace('"Debug"', '"Release"').replace("Debug-iphonesimulator", "Release-iphonesimulator"))
    install_route_observer(source)
    log = temporary / "showcase.log"
    environment = None
    observe_at = None
    with log.open("w") as stream, subprocess.Popen(
        ["node", driver, "--skip-metro", "--platform", "ios", "--device", "iphone-6.9",
         "--appearance", "light", "--theme", "t3-code", "--scene", "thread", "--scene", "review"],
        cwd=source, env=env, stdout=stream, stderr=subprocess.STDOUT,
    ) as process:
        # The app and fixture share this ID. Its server removes the file on successful cleanup.
        while process.poll() is None:
            if observe_at is None and any(device_data.glob("Containers/Data/Application/*/Library/Caches/T3ShowcaseScene")):
                observe_at = time.monotonic() + 45
            if observe_at is not None and time.monotonic() >= observe_at:
                try:
                    observe_startup(udid, temporary, device_data)
                except (OSError, ValueError, subprocess.SubprocessError) as exc:
                    print(f"Optional launch diagnostic unavailable ({type(exc).__name__})", flush=True)
                observe_at = time.monotonic() + 90
            if environment is None:
                for path in temporary.glob("t3-mobile-showcase-*/environments/moonbase-terminal/userdata/environment-id"):
                    try:
                        environment = uuid.UUID(path.read_text().strip())
                    except (FileNotFoundError, ValueError):
                        pass
            time.sleep(1)
    log_text = log.read_text(errors="replace")
    if "Simulator Keychain ready via local ad-hoc identity" in log_text:
        print("Simulator Keychain ready via local ad-hoc identity", flush=True)
    if process.returncode:
        # Only emit diagnostics. The full harness log can contain ephemeral pairing URLs.
        lines = [line[:750] for line in log_text.splitlines()
                 if re.search(r"error:|failed|timed out|ENOSPC|not found|\[!\]", line, re.I)
                 and not re.search(r"warning:|NSError|token|pairing|credential|://|authorization|password", line, re.I)]
        print("\n".join(lines[-30:]), file=sys.stderr)
        raise RuntimeError(f"T3 Code showcase exited with {process.returncode}")
    if environment is None:
        raise RuntimeError("The Moonbase Terminal fixture did not report its environment ID")
    return f"/threads/{environment}/remote-command-center"


def main():
    source = Path(sys.argv[1]).resolve()
    destination = Path(sys.argv[2]).resolve()
    if output("git", "-C", source, "rev-parse", "HEAD") != REVISION:
        raise ValueError("Unexpected showcase source revision")
    simctl = ["xcrun", "simctl"]
    devices = json.loads(output(*simctl, "list", "devices", "available", "-j"))["devices"]
    if any(device["name"] == DEVICE for group in devices.values() for device in group):
        raise RuntimeError("A showcase device already exists. Run on a clean macOS runner.")
    udid = output(*simctl, "create", DEVICE, DEVICE_TYPE)
    try:
        subprocess.run([*simctl, "boot", udid], check=True)
        subprocess.run([*simctl, "bootstatus", udid, "-b"], check=True)
        runtime = output(*simctl, "getenv", udid, "SIMULATOR_RUNTIME_VERSION")
        scale = float(output(*simctl, "getenv", udid, "SIMULATOR_MAINSCREEN_SCALE"))
        inventory = json.loads(output(*simctl, "list", "devices", "-j"))["devices"]
        device_data = next(Path(d["dataPath"]) for group in inventory.values() for d in group if d["udid"] == udid)
        if not runtime or scale <= 0:
            raise ValueError("CoreSimulator did not report the runtime and display scale")
        subprocess.run([*simctl, "shutdown", udid], check=True)
        with tempfile.TemporaryDirectory(prefix="dui-t3-ios-", dir="/tmp") as tmp:
            temporary = Path(tmp)
            checker = temporary / "check-screenshot"
            subprocess.run(["xcrun", "swiftc", Path(__file__).with_name("check-ios-screenshot.swift"),
                            "-o", checker], check=True)
            thread_path = run_capture(source, temporary, udid, device_data)
            screenshots = source / "artifacts/app-store/screenshots/apple/iphone-6.9/light/t3-code"
            summary = [f"T3 Code `{REVISION}` · {DEVICE} · iOS {runtime} · {scale:g}×",
                       "The upstream harness waited for each native scene to report its route and content ready.",
                       "", "| File | Registration | Fixture route | Screen ID | Points | Pixels | SHA-256 |",
                       "|---|---|---|---|---|---|---|"]
            hashes = set()
            destination.mkdir(parents=True, exist_ok=True)
            for scene, name, suffix, screen_id in SCENES:
                observation, = device_data.glob(f"Containers/Data/Application/*/Documents/dui-showcase-{scene}.json")
                observed = json.loads(observation.read_text())
                if observed["path"] != thread_path + suffix or observed["scene"] != scene or not observed["hasFixture"]:
                    raise ValueError(f"Native navigation did not confirm {name}")
                image = screenshots / f"{scene}.png"
                width, height = map(int, output(checker, image).split())
                digest = hashlib.sha256(image.read_bytes()).hexdigest()
                if digest in hashes:
                    raise ValueError("Both routes produced identical pixels")
                hashes.add(digest)
                summary.append(f"| {image.name} | {name} | `{thread_path}{suffix}` | `{screen_id}` | "
                               f"{width / scale:g} × {height / scale:g} | {width} × {height} | `{digest}` |")
                shutil.copyfile(image, destination / image.name)
            report = "\n".join(summary) + "\n"
            print(report)
            if "GITHUB_STEP_SUMMARY" in os.environ:
                with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as stream:
                    stream.write(report)
    finally:
        subprocess.run([*simctl, "delete", udid], check=True)


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--check-simulator-keychain":
        prepare_simulator_keychain(Path(sys.argv[2]).resolve())
    else:
        main()
