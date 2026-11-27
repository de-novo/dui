"""Build a slowed, synchronized video from measured Chrome and model frames."""
import sys, subprocess
from pathlib import Path
from PIL import Image, ImageDraw
root = Path(sys.argv[1])
evidence = Path("evidence")
for i, ms in enumerate(range(0, 1001, 100)):
    frame = Image.new("RGB", (1000, 490), "white")
    draw = ImageDraw.Draw(frame)
    for row, width in enumerate((1440, 390)):
        for col, kind in enumerate(("chrome", "model")):
            x, y = 10 + 500 * col, 10 + 240 * row
            draw.text((x, y), f"{width}px | {kind.upper()} | {ms} ms", fill="#104c40", font_size=19)
            frame.paste(Image.open(root / f"{width}-{ms}-{kind}.png"), (x, y + 33))
    frame.save(root / f"frame-{i:02}.png")
    if ms == 500:
        frame.save(evidence / "half-second.png")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-framerate", "4", "-i", str(root / "frame-%02d.png"), "-vf", "fps=24,format=yuv420p", "-c:v", "libx264", "-movflags", "+faststart", str(evidence / "motion.mp4")], check=True)
