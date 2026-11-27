"""Compose the measured screenshot strips; each column uses the same spatial scale."""
import json
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

base = Path(__file__).resolve().parent
out = base / "out"
rows = json.loads((out / "results.json").read_text())
labels = ["8rem width", "3em / nested font", "50% width", "10% padding", "20vw width",
          "clamp() width", "4em / inherited", "5% 10% padding"]
def font(n):
    return ImageFont.truetype("Arial.ttf", n)
for w, values in rows.items():
    width = int(w)
    frame = Image.new("RGB", (1240, 654), "white")
    draw = ImageDraw.Draw(frame)
    draw.text((18,15), f"Relative units at {w} CSS px", font=font(29), fill="#183a31")
    draw.text((18,56), "Captured rows at equal scale. Labels report original CSS pixels.", font=font(19), fill="#384f47")
    for j,(mode,title) in enumerate([("baseline","Current renderer"),("prototype","Research prototype"),("reference","Chrome reference")]):
        shot = Image.open(out / f"{w}-{mode}.png").convert("RGB")
        assert shot.width == width
        left = 210+j*342
        draw.text((left,94),title,font=font(22),fill="#163a31")
        for i,label in enumerate(labels):
            top = 140+i*62
            if j==0:
                draw.text((18,top),label,font=font(18),fill="#243c33")
            x,y,boxwidth,_ = values[mode]["rows"][i]
            txt = f"x {x}, y {y}" if "padding" in label else f"{boxwidth} px"
            draw.text((left,top),txt,font=font(17),fill="#1a352d")
            crop = shot.crop((0,i*72,width,i*72+50))
            thumb = crop.resize((326,round(50*326/width)),Image.Resampling.LANCZOS)
            frame.paste(thumb,(left,top+20))
    dest = base/"evidence"
    dest.mkdir(exist_ok=True)
    frame.save(dest/f"relative-{w}.png")
