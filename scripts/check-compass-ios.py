"""Render the pinned Compass screens on a fresh iPhone simulator with the pinned Flutter SDK on PATH."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from ios_simulator import fresh_iphone

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "fixtures/compass-ios"
SAMPLES = "a5bccd247163b1017d2eb9320cf3b94a95ccfb5e"
FLUTTER = "ae9b586f34a8443573f82ef57c3bf914a908cbf7"


def read(*args):
    return subprocess.check_output(args, text=True).strip()


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
            driver = "test_driver/dui_capture_driver.dart"
            (app / driver).parent.mkdir(exist_ok=True)
            shutil.copyfile(FIXTURE / "capture_test.dart", app / target)
            shutil.copyfile(FIXTURE / "capture_driver.dart", app / driver)
            subprocess.run(["flutter", "drive", "--debug", "-d", phone.udid,
                            "--target", target, "--driver", driver], cwd=app, check=True,
                           env={**os.environ, "COMPASS_CAPTURE_OUTPUT": str(output)})
            checker = Path(temp) / "check-image"
            subprocess.run(["xcrun", "swiftc", ROOT / "scripts/check-ios-screenshot.swift", "-o", checker], check=True)
            rows = json.loads((output / "screens.json").read_text())
            expected = {"/", "/search", "/results", "/activities", "/booking", "/booking/:id", "/login"}
            if len(rows) != len(expected) or {row["route"] for row in rows} != expected:
                raise ValueError("Expected all seven source routes")
            summary = [f"Native Compass capture on {phone.name} ({phone.runtime.split('.')[-1]}).",
                       f"flutter/samples `{SAMPLES}`, Flutter `{sdk['frameworkVersion']}`, source `lib/routing/router.dart`.",
                       "", "| Declaration | Observed route | PNG | Viewport (pt) | Image (px) | SHA-256 |",
                       "|---|---|---|---|---|---|"]
            for row in rows:
                image = output / (row["name"] + ".png")
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
