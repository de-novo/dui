"""Run only the named GPUI window; measure save to the first matching window capture."""

import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

here = Path(__file__).resolve().parent
root = here.parent.parent
stage = Path(sys.argv[1])
sf = Path(sys.argv[2])
modes = sys.argv[3:] or ["cosmic"]


def wait_for(fn, procs, deadline=8):
    end = time.monotonic() + deadline
    while time.monotonic() < end:
        if any(p.poll() is not None for p in procs):
            raise RuntimeError("study subprocess exited")
        value = fn()
        if value:
            return value
        time.sleep(0.004)
    return None


def frames(p):
    rows = []
    for line in p.read_text().splitlines():
        vals = line.split()
        if len(vals) == 3:
            rows.append(tuple(map(int, vals)))
    return rows


def run_case(mode, platform, width, height, name):
    d = stage / f"{platform}-{name}-{width}-{mode}"
    d.mkdir(exist_ok=True)
    if (d / "result.json").exists():
        return
    ext = "swift" if platform == "ios" else "kt"
    source = d / ("Source." + ext)
    original = (
        here / "fixtures" / ("Stack." + ext)
        if name == "Stack"
        else root
        / (
            "fixtures/swiftui-map/Sources/SwiftUIMapFixture/Screens.swift"
            if platform == "ios"
            else "experiments/screen-engines/android/Examples.kt"
        )
    ).read_text()
    title = "Settings" if name == "Stack" else name
    needle = f'Text("{title}"'
    assert original.count(needle) == 1
    symbol = (
        "StackExample"
        if name == "Stack"
        else name + ("View" if platform == "ios" else "Screen")
    )
    source.write_text(original)
    model = d / "model.json"
    model.unlink(missing_ok=True)
    processes = []
    files = []

    def launch(cmd, log):
        stdout = (d / log).open("w")
        err = (d / (log + ".err")).open("w")
        files.extend([stdout, err])
        p = subprocess.Popen(cmd, stdout=stdout, stderr=err)
        processes.append(p)
        return p

    try:
        launch(
            [
                sys.executable,
                str(here / "project.py"),
                str(source),
                symbol,
                str(model),
            ],
            "project.log",
        )
        assert wait_for(model.exists, processes)
        app = launch(
            [
                str(stage / "target/debug/dui-native-preview-study"),
                str(model),
                str(sf if platform == "ios" else stage / "Roboto.ttf"),
                platform,
                str(width),
                str(height),
                mode,
            ],
            "draw.log",
        )
        assert wait_for(lambda: (d / "draw.log").stat().st_size, processes)
        launch(
            [
                sys.executable,
                str(here / "capture.py"),
                str(stage / "capture"),
                str(app.pid),
                str(width),
                str(height),
                str(d),
            ],
            "frames",
        )
        stream = d / "frames"
        assert wait_for(lambda: frames(stream), processes, 25)
        time.sleep(0.18)
        initial = frames(stream)[-1]
        shutil.copyfile(d / f"frame-{initial[0]}.png", d / "baseline.png")
        expected = {False: initial[1]}
        observations = []
        for i in range(10):
            edited = i % 2 == 0
            content = (
                original.replace(needle, f'Text("{title} edited"')
                if edited
                else original
            )
            start = time.monotonic_ns()
            source.write_text(content)

            def observed(start=start, edited=edited):
                return next(
                    (
                        r
                        for r in frames(stream)
                        if r[2] >= start
                        and (
                            r[1] == expected[edited]
                            if edited in expected
                            else r[1] != expected[False]
                        )
                    ),
                    None,
                )

            row = wait_for(observed, processes, 3)
            duration = (row[2] - start) / 1e6 if row else None
            if i < 2 and not row:
                raise RuntimeError(f"calibration failed: {d}")
            if row and i == 0:
                expected[True] = row[1]
                shutil.copyfile(d / f"frame-{row[0]}.png", d / "edited.png")
            if i >= 2:
                observations.append(duration)
            time.sleep(0.04)
        (d / "result.json").write_text(json.dumps(observations))
        print(d.name, observations, flush=True)
    finally:
        for p in reversed(processes):
            p.terminate()
            try:
                p.wait(2)
            except subprocess.TimeoutExpired:
                p.kill()
                p.wait()
        for f in files:
            f.close()


for mode in modes:
    for platform in ["ios", "android"]:
        for width, height in [(390, 844), (1440, 920)]:
            for name in ["Detail", "Profile", "Stack"]:
                run_case(mode, platform, width, height, name)
