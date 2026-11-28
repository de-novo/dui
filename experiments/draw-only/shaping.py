"""Shape route text with the captured variable font and produce identical glyph paths for both rasterizers."""

import json, subprocess, sys
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen

w = Path(sys.argv[1])
data = json.loads((w / "raw.json").read_text())
cache = w / "cache"
(woff,) = cache.glob("*.woff2")
f = TTFont(woff)
f.flavor = None
f.save(w / "Inter.ttf")
upem = f["head"].unitsPerEm
runs = {}
for screen, case in data["screens"].items():
    for id, r in case["modes"]["reference"]["geom"]["runs"].items():
        text = r["text"].strip()
        key = json.dumps(
            [text, r["weight"], r["size"], r["spacing"]], ensure_ascii=False
        )
        if key in runs:
            continue
        size = r["size"]
        opsz = max(14, min(32, size))
        weight = int(r["weight"])
        coords = {"wght": weight, "opsz": opsz}
        gset = f.getGlyphSet(location=coords)
        adv = {}
        paths = []
        for engine in ["ot", "coretext"]:
            cmd = [
                "hb-shape",
                str(w / "Inter.ttf"),
                "--text=" + text,
                "--shapers=" + engine,
                "--output-format=json",
                f"--variations=wght={weight},opsz={opsz}",
            ]
            glyphs = json.loads(subprocess.check_output(cmd))
            total = 0
            for g in glyphs:
                if engine == "ot":
                    pen = SVGPathPen(gset)
                    gset[g["g"]].draw(pen)
                    paths.append([pen.getCommands(), total + g["dx"], g["dy"]])
                total += g["ax"]
            adv[engine] = total * size / upem
        runs[key] = dict(
            text=text,
            size=size,
            weight=weight,
            spacing=r["spacing"],
            opsz=opsz,
            upem=upem,
            paths=paths,
            advance=adv,
        )
(w / "paths.json").write_text(json.dumps(runs))
