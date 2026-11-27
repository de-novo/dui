"""Capture only the named app window. Preserve wall-clock offsets for each sample."""
import json, subprocess, sys, time
from pathlib import Path
window, directory = sys.argv[1:]
directory = Path(directory)
directory.mkdir(exist_ok=True)
start = time.monotonic()
timestamps = []
for index in range(20):
    time.sleep(max(0, start + index * .12 - time.monotonic()))
    begin = time.monotonic() - start
    subprocess.run(["screencapture", "-x", "-o", "-l", window, str(directory / f"{index:02}.png")], check=True)
    timestamps.append([begin, time.monotonic() - start])
(directory / "times.json").write_text(json.dumps(timestamps))
