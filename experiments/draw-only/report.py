from pathlib import Path
from PIL import Image, ImageChops, ImageDraw, ImageFont
import json, statistics, subprocess, sys, zipfile

w = Path(sys.argv[1])
out = Path(__file__).parent / "evidence"
out.mkdir(exist_ok=True)
raw = json.loads((w / "raw.json").read_text())
raster = json.loads((w / "raster.json").read_text())
assert len(raw["screens"]) == 4
summary = {}
tf = {}


def getwidth(text, style):
    return next(
        v["ot"]
        for k, v in raster.items()
        if (
            lambda a: (
                a[0] == text.strip()
                and a[1] == style["font-weight"]
                and float(a[2]) == float(style["font-size"].rstrip("px"))
            )
        )(json.loads(k))
    )


def flow(n):
    u = n["used"]
    css = {**u, **n["typed"], "line-height": u["line-height"]}
    lines = []
    children = []
    for c in n["children"]:
        if "text" in c:
            if c["text"].strip():
                lines.append(getwidth(c["text"], u))
        elif c["tag"] != "br":
            children.append(flow(c))
    return dict(id=n["id"], css=css, lines=lines, children=children)


def breaks(run):
    return [
        i
        for i in range(1, len(run["chars"]))
        if run["chars"][i][1] != run["chars"][i - 1][1] and run["chars"][i][2] > 0
    ]


for name, item in raw["screens"].items():
    mode = item["modes"]
    ref = mode["reference"]
    tree = json.loads((w / (name + "-tree.json")).read_text())
    assert all(
        mode["identity"][k] == 0
        for k in [
            "usedDiff",
            "geomTypedDiff",
            "textMismatch",
            "attrMismatch",
            "pseudoMismatch",
        ]
    )
    assert ref["fonts"] == mode["identity"]["fonts"]
    width = int(name.split("-")[1])
    height = 920 if width == 1440 else 844
    tf[name] = json.loads(
        subprocess.check_output(
            [str(w / "target/release/dui-draw-only-taffy")],
            input=json.dumps(
                dict(tree=flow(tree), width=width, height=height)
            ).encode(),
        )
    )
    refim = Image.open(w / (name + "-reference.png")).convert("RGB")
    summary[name] = {}
    for label in ["identity", "stock", "shadow-only", "typed", "used", "yoga", "taffy"]:
        cand = mode.get(label)
        boxes = cand["geom"]["boxes"] if cand else tf[name]
        errors = [
            [abs(v - b) for v, b in zip(box, ref["geom"]["boxes"][id])]
            for id, box in boxes.items()
        ]
        row = {
            "boxes": len(errors),
            "positionMax": max(max(e[:2]) for e in errors),
            "sizeMax": max(max(e[2:]) for e in errors),
            "boxMeanAbs": statistics.mean(v for e in errors for v in e),
        }
        if cand:
            delta = ImageChops.difference(
                refim, Image.open(w / (name + "-" + label + ".png")).convert("RGB")
            )
            delta = ImageChops.lighter(
                ImageChops.lighter(*delta.split()[:2]), delta.split()[2]
            )
            hist = delta.histogram()
            row.update(
                changedPixels=sum(hist[1:]),
                over20=sum(hist[21:]),
                total=width * height,
                runCount=len(ref["geom"]["runs"]),
                breakMismatch=0,
                charAdvanceMax=0,
                fontMismatches=0,
            )
            for id, r in ref["geom"]["runs"].items():
                nxt = cand["geom"]["runs"][id]
                assert nxt["text"] == r["text"]
                row["breakMismatch"] += breaks(r) != breaks(nxt)
                row["charAdvanceMax"] = max(
                    row["charAdvanceMax"],
                    max(abs(a[2] - b[2]) for a, b in zip(r["chars"], nxt["chars"])),
                )
                row["fontMismatches"] += ref["fonts"][id] != cand["fonts"][id]
            if label == "identity":
                assert (
                    row["changedPixels"]
                    == row["positionMax"]
                    == row["sizeMax"]
                    == row["charAdvanceMax"]
                    == 0
                )
        summary[name][label] = row
(out / "screen-metrics.json").write_text(json.dumps(summary, indent=2))
(out / "raster-metrics.json").write_text(json.dumps(raster, indent=2))
(out / "manifest.json").write_text(
    json.dumps(
        {
            "chrome": raw["chrome"],
            "resources": raw["resources"],
            "inputSHA256": {k: v["input"] for k, v in raw["screens"].items()},
        },
        indent=2,
    )
)
with zipfile.ZipFile(out / "oracle-and-captures.zip", "w", zipfile.ZIP_DEFLATED) as z:
    for n in raw["screens"]:
        for p in w.glob(n + "-*"):
            z.write(p, p.name)
    for p in [
        w / "raw.json",
        w / "raster.json",
        w / "cosmic.json",
        w / "cache/fonts.css",
    ]:
        z.write(p, p.name)
    for p in w.glob("glyph-*.png"):
        z.write(p, p.name)
    for p in w.glob("text-*.png"):
        z.write(p, p.name)
font = ImageFont.truetype("/System/Library/Fonts/Helvetica.ttc", 16)
for width in [1440, 390]:
    thumb = 310 if width == 1440 else 185
    h = round(thumb * (920 if width == 1440 else 844) / width)
    img = Image.new("RGB", ((thumb + 8) * 5, 36 + 2 * (h + 34)), "#dbe2ea")
    d = ImageDraw.Draw(img)
    for i, label in enumerate(
        ["Chrome", "dui · same CSS", "Stock card", "Style sidecar", "Yoga adapter"]
    ):
        d.text((8 + i * (thumb + 8), 8), label, font=font, fill="#192332")
    for j, route in enumerate(["403", "503"]):
        y = 36 + j * (h + 34)
        d.text((8, y), "/" + route, font=font, fill="#192332")
        for i, mode in enumerate(["reference", "identity", "stock", "typed", "yoga"]):
            p = w / f"{route}-{width}-{mode}.png"
            img.paste(
                Image.open(p)
                .convert("RGB")
                .resize((thumb, h), Image.Resampling.LANCZOS),
                (5 + i * (thumb + 8), y + 25),
            )
    img.save(out / f"compare-{width}.png")
print(
    json.dumps(
        {
            m: [
                round(max(summary[n][m][k] for n in summary), 4)
                for k in ["positionMax", "sizeMax"]
            ]
            for m in summary["403-390"]
        },
        indent=2,
    )
)

(out / "cosmic-metrics.json").write_bytes((w / "cosmic.json").read_bytes())
(out / "taffy-boxes.json").write_text(json.dumps(tf, indent=2))
sheet = Image.new("RGB", (860, 280), "#dbe2ea")
draw = ImageDraw.Draw(sheet)
for index, mode in enumerate(["DOM", "Skia", "ChromePath", "SkiaPath"]):
    im = Image.open(w / f"glyph-{mode}.png").convert("RGB")
    ink = ImageChops.difference(im, Image.new("RGB", im.size, "white")).getbbox()
    draw.text(
        (8 + (index % 2) * 430, 8 + (index // 2) * 135),
        [
            "Chrome DOM text",
            "Skia text",
            "Chrome · identical outline",
            "Skia · identical outline",
        ][index],
        font=font,
        fill="#192332",
    )
    sheet.paste(
        im.crop((0, 35, 400, 131)), (8 + (index % 2) * 430, 33 + (index // 2) * 135)
    )
sheet.save(out / "glyphs.png")
# Make the sub-20 RGB differences on the buttons visible at normal reading size.
name = "403-390"
ref = Image.open(w / (name + "-reference.png")).convert("RGB")
stock = Image.open(w / (name + "-stock.png")).convert("RGB")
shadow = Image.open(w / (name + "-shadow-only.png")).convert("RGB")
diff = ImageChops.difference(ref, stock)
box = diff.getbbox()
assert box
x, y, xx, yy = box
roi = (x - 5, y - 5, xx + 5, yy + 5)
tiles = [
    ("Chrome", ref),
    ("Stock card", stock),
    ("Absolute difference ×16", diff.point(lambda k: min(255, k * 16))),
    ("Restore box-shadow only", shadow),
]
width = max(255, xx - x + 16)
sheet = Image.new("RGB", (width * 2, 2 * (yy - y + 52)), "#dbe2ea")
draw = ImageDraw.Draw(sheet)
for i, (label, im) in enumerate(tiles):
    x = i % 2 * width
    y = i // 2 * (yy - roi[1] + 47)
    draw.text((x + 5, y + 3), label, fill="#192332", font=font)
    sheet.paste(im.crop(roi), (x + 5, y + 29))
sheet.save(out / "button-ablation.png")
