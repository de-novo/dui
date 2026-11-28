"""Score logical pixels on white, preserving full-screen inputs and matched edit timing."""

import io
import json
import math
import statistics
import sys
import zipfile
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFont

stage = Path(sys.argv[1])
here = Path(__file__).resolve().parent
out = here / "evidence"
out.mkdir(exist_ok=True)
old = here.parent / "screen-engines/evidence"


def white(im):
    fg = im.convert("RGBA")
    bg = Image.new("RGBA", fg.size, "white")
    bg.alpha_composite(fg)
    return bg.convert("RGB")


def mask(im):
    r, g, b = im.split()
    return ImageChops.darker(r, ImageChops.darker(g, b)).point(
        lambda v: 255 if v < 235 else 0
    )


def score(ref, img):
    a, b = map(mask, (ref, img))
    # Strip only the native window border for ink metrics; exact compares the full frame.
    interior = Image.new("L", ref.size, 0)
    ImageDraw.Draw(interior).rectangle(
        (32, 32, ref.width - 33, ref.height - 33), fill=255
    )
    assert ImageChops.subtract(a, interior).getbbox() is None
    b = ImageChops.multiply(b, interior)
    union = ImageChops.lighter(a, b)
    diff = ImageChops.difference(ref, img)
    channels = diff.split()
    maximum = ImageChops.lighter(channels[0], ImageChops.lighter(*channels[1:]))
    bad = maximum.point(lambda v: 255 if v > 20 else 0)
    count = union.histogram()[255]
    changed = ImageChops.multiply(bad, union).histogram()[255]
    assert count and a.getbbox() and b.getbbox()
    return {
        "inkPixels": count,
        "changedInkPixels": changed,
        "changedPercent": 100 * changed / count,
        "exact": diff.getbbox() is None,
        "referenceBounds": a.getbbox(),
        "previewBounds": b.getbbox(),
    }


rows = []
font = ImageFont.truetype("/System/Library/Fonts/Supplemental/Arial.ttf", 19)
with zipfile.ZipFile(out / "screens.zip", "w", zipfile.ZIP_DEFLATED) as archive:
    for w, h in [(390, 844), (1440, 920)]:
        sheet = Image.new("RGB", (754, 6 * 177 + 42), "#f1f3f5")
        draw = ImageDraw.Draw(sheet)
        for x, label in [
            (8, "1x center crops"),
            (199, "Platform reference"),
            (480, "GPUI + cosmic-text"),
        ]:
            draw.text((x, 10), label, font=font, fill="black")
        for j, (plat, name) in enumerate(
            (p, n) for p in ["ios", "android"] for n in ["Detail", "Profile", "Stack"]
        ):
            if name == "Stack":
                ref = white(Image.open(stage / f"{plat}-stack-{w}.png"))
            else:
                file, component = (
                    ("full-screens.zip", "View")
                    if plat == "ios"
                    else ("native-device-captures.zip", "Screen")
                )
                with zipfile.ZipFile(old / file) as z:
                    ref = white(
                        Image.open(
                            io.BytesIO(
                                z.read(f"{plat}/{name}{component}-reference-{w}.png")
                            )
                        )
                    )
            imgs = [ref]
            assert ref.size == (w, h)
            for mode in ["cosmic"]:
                d = stage / f"{plat}-{name}-{w}-{mode}"
                img = white(Image.open(d / "baseline.png"))
                assert img.size == ref.size
                assert ImageChops.difference(
                    img, white(Image.open(d / "edited.png"))
                ).getbbox()
                rows.append(
                    dict(
                        platform=plat,
                        fixture=name,
                        width=w,
                        mode=mode,
                        **score(ref, img),
                        saveToObservedMs=json.loads((d / "result.json").read_text()),
                    )
                )
                imgs.append(img)
            y = 42 + j * 177
            draw.text(
                (8, y + 52),
                ("SwiftUI" if plat == "ios" else "Compose") + f"  {w}",
                font=font,
                fill="black",
            )
            draw.text((8, y + 78), name, font=font, fill="black")
            for i, im in enumerate(imgs):
                buf = io.BytesIO()
                im.save(buf, format="PNG")
                archive.writestr(
                    f"{plat}-{name}-{w}-{['reference', 'cosmic', 'host'][i]}.png",
                    buf.getvalue(),
                )
                sheet.paste(
                    im.crop((w // 2 - 136, h // 2 - 84, w // 2 + 136, h // 2 + 84)),
                    (192 + 282 * i, y),
                )
        sheet.save(out / f"compare-{w}.png")
(out / "results.json").write_text(json.dumps(rows, indent=2) + "\n")
for plat in ["ios", "android"]:
    for mode in ["cosmic"]:
        group = [r for r in rows if r["platform"] == plat and r["mode"] == mode]
        times = sorted(t for r in group for t in r["saveToObservedMs"] if t is not None)
        print(
            plat,
            mode,
            "exact",
            sum(r["exact"] for r in group),
            "within1%",
            sum(r["changedPercent"] <= 1 for r in group),
            "ink%",
            [(r["width"], r["fixture"], round(r["changedPercent"], 1)) for r in group],
            "median/p95",
            round(statistics.median(times), 1),
            round(times[math.ceil(0.95 * len(times)) - 1], 1),
            "observed",
            len(times),
        )
