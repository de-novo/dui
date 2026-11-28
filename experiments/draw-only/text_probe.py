"""Pin axes for cosmic-text; check the outline instance against variable-font shaping."""

from pathlib import Path
import sys, json, subprocess
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont
from PIL import Image, ImageChops

w = Path(sys.argv[1])
runs = json.loads((w / "raster.json").read_text())
result = {}
for index, (k, row) in enumerate(runs.items()):
    text, weight, size, spacing = json.loads(k)
    opsz = max(14, min(32, size))
    assert not spacing
    inst = w / f"inter-{weight}-{opsz}.ttf"
    if not inst.exists():
        f = instantiateVariableFont(
            TTFont(w / "Inter.ttf"), {"wght": int(weight), "opsz": opsz}
        )
        f.save(inst)
    hb = json.loads(
        subprocess.check_output(
            [
                "hb-shape",
                str(inst),
                "--text=" + text,
                "--shapers=ot",
                "--output-format=json",
            ]
        )
    )
    adv = sum(g["ax"] for g in hb) * size / TTFont(inst)["head"].unitsPerEm
    gray = w / "swash.gray"
    args = dict(
        text=text, weight=int(weight), size=size, font=str(inst), gray=str(gray)
    )
    lines = json.loads(
        subprocess.check_output(
            [w / "target/release/text"], input=json.dumps(args).encode()
        )
    )
    actual = [[a["text"] for a in line] for line in lines[1:]]
    swash = Image.frombytes("L", (660, 160), gray.read_bytes())
    dom = Image.open(w / f"text-{index}.png").convert("L")
    d = ImageChops.difference(dom, swash).histogram()
    if index == 0:
        swash.save(w / "glyph-Swash.png")
    result[k] = dict(
        staticHB=adv,
        cosmic=lines[0][0]["line_w"] if "line_w" in lines[0][0] else 0,
        lines=actual,
        chrome=row["wrap"],
        matches=[x == y for x, y in zip(actual, row["wrap"])],
        swashChanged=sum(d[1:]),
    )
(w / "cosmic.json").write_text(json.dumps(result, indent=2))
