"""Run the pinned app's two native showcase scenes on an isolated GitHub macOS runner."""

import hashlib
import json
import os
from pathlib import Path
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


def run_capture(source, temporary):
    pnpm_version = json.loads((source / "package.json").read_text())["packageManager"].split("@")[1]
    pnpm_dir = Path.home() / ".vite-plus/package_manager/pnpm" / pnpm_version / "pnpm/bin"
    xcconfig = temporary / "unsigned.xcconfig"
    xcconfig.write_text("CODE_SIGNING_ALLOWED = NO\nCODE_SIGNING_REQUIRED = NO\n")
    env = dict(os.environ, PATH=f"{pnpm_dir}:{os.environ['PATH']}", TMPDIR=str(temporary),
               XCODE_XCCONFIG_FILE=str(xcconfig))
    log = temporary / "showcase.log"
    environment = None
    with log.open("w") as stream, subprocess.Popen(
        ["pnpm", "screenshots:mobile", "--platform", "ios", "--device", "iphone-6.9",
         "--appearance", "light", "--theme", "t3-code", "--scene", "thread", "--scene", "review"],
        cwd=source, env=env, stdout=stream, stderr=subprocess.STDOUT,
    ) as process:
        # The app and fixture share this ID. Its server removes the file on successful cleanup.
        while process.poll() is None:
            if environment is None:
                for path in temporary.glob("t3-mobile-showcase-*/environments/moonbase-terminal/userdata/environment-id"):
                    try:
                        environment = uuid.UUID(path.read_text().strip())
                    except (FileNotFoundError, ValueError):
                        pass
            time.sleep(1)
    if process.returncode:
        # Only emit diagnostics. The full harness log can contain ephemeral pairing URLs.
        lines = [line[:750] for line in log.read_text(errors="replace").splitlines()
                 if re.search(r"error|failed|timed out|ENOSPC|not found|\[!\]", line, re.I)
                 and not re.search(r"token|pairing|credential|://|authorization|password", line, re.I)]
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
        if not runtime or scale <= 0:
            raise ValueError("CoreSimulator did not report the runtime and display scale")
        subprocess.run([*simctl, "shutdown", udid], check=True)
        with tempfile.TemporaryDirectory(prefix="dui-t3-ios-", dir="/tmp") as tmp:
            temporary = Path(tmp)
            checker = temporary / "check-screenshot"
            subprocess.run(["xcrun", "swiftc", Path(__file__).with_name("check-ios-screenshot.swift"),
                            "-o", checker], check=True)
            thread_path = run_capture(source, temporary)
            screenshots = source / "artifacts/app-store/screenshots/apple/iphone-6.9/light/t3-code"
            summary = [f"T3 Code `{REVISION}` · {DEVICE} · iOS {runtime} · {scale:g}×",
                       "The upstream harness waited for each native scene to report its route and content ready.",
                       "", "| File | Registration | Fixture route | Screen ID | Points | Pixels | SHA-256 |",
                       "|---|---|---|---|---|---|---|"]
            hashes = set()
            destination.mkdir(parents=True, exist_ok=True)
            for scene, name, suffix, screen_id in SCENES:
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
    main()
