"""Pair native window captures by elapsed recording time. OS animation phases are independent."""
from PIL import Image, ImageDraw
from pathlib import Path
import sys, json, subprocess
root, dest = map(Path, sys.argv[1:])
dest.mkdir(exist_ok=True)
streams = []
report = {}
for name in ["swift", "gpui"]:
    folder = root / (name + "-film")
    times = json.loads((folder / "times.json").read_text())
    frames = []
    for i, (start, end) in enumerate(times):
        image = Image.open(folder / f"{i:02}.png").convert("RGB")
        # Skip system title bar. A 2x backing scale was observed on this machine.
        content = image.crop((0, 64, 640, 304))
        mask = Image.new("1", content.size)
        mask.putdata([g > r + 28 and g > b + 4 for r,g,b in content.getdata()])
        bounds = mask.getbbox()
        if bounds is None:
            raise ValueError("no colored marker")
        frames.append(((start + end) / 2, content, bounds[0] / 2))
    streams.append(frames)
    report[name] = [{"start":s, "end":e, "markerLeftPt":v[2]} for (s,e),v in zip(times,frames)]
(dest/"native.json").write_text(json.dumps(report,indent=2))
duration = min(stream[-1][0] for stream in streams)
for i in range(int(duration * 10)):
    time = i / 10
    frame = Image.new("RGB",(660,190),"white")
    draw = ImageDraw.Draw(frame)
    for j, (name,stream) in enumerate(zip(["SwiftUI","GPUI"],streams)):
        stamp, im, left = min(stream,key=lambda f:abs(f[0]-time))
        x = 330*j+4
        draw.text((x,8), f"{name}    sample {stamp:.2f}s",font_size=18,fill="#244c42")
        frame.paste(im.resize((320,120)),(x,37))
        draw.text((x,158), f"Marker x = {left:g} pt",font_size=15,fill="#244c42")
    frame.save(root/f"native-{i:03}.png")
subprocess.run(["ffmpeg","-loglevel","error","-y","-framerate","10","-i",str(root/"native-%03d.png"),"-pix_fmt","yuv420p","-movflags","+faststart",str(dest/"native.mp4")],check=True)
