"""Observe only the GPUI window with macOS window capture."""

import hashlib
import subprocess
import sys
import time
from pathlib import Path

from PIL import Image

helper, pid, w, h, d = sys.argv[1:]
w, h = int(w), int(h)
target = subprocess.check_output([helper, pid, str(w), str(h)], text=True).strip()
path = Path(d) / "current.png"
last = None
sequence = 0
while True:
    result = subprocess.run(
        ["/usr/sbin/screencapture", "-x", "-o", "-l", target, str(path)], check=False
    )
    if result.returncode:
        time.sleep(0.02)
        continue
    received = time.monotonic_ns()
    with Image.open(path) as raw:
        if raw.width * h != raw.height * w:
            continue
        image = raw.resize((w, h), Image.Resampling.LANCZOS)
    region = image.crop((w // 2 - 160, h // 2 - 100, w // 2 + 160, h // 2 + 100))
    key = int.from_bytes(
        hashlib.blake2b(region.tobytes(), digest_size=8).digest(), "big"
    )
    if key != last:
        image.save(Path(d) / f"frame-{sequence}.png")
        print(sequence, key, received, flush=True)
        last = key
        sequence += 1
