"""Render a representative wrapped reference and its Taffy boxes on one contact sheet."""

import csv
import subprocess
from pathlib import Path

rows = list(csv.reader(Path("taffy.csv").read_text().splitlines()))
for width, height in [(390, 844), (1440, 920)]:
    panels = []
    for label, mode in [
        ("Chrome CSS", None),
        ("Taffy, fractional", 0),
        ("Taffy, rounded", 1),
    ]:
        if mode is None:
            body = (
                '<div style="width:265px;display:flex;flex-wrap:wrap;gap:7.5px;justify-content:space-between;align-items:start;margin:auto">'
                + "".join(
                    f'<i style="width:{w}px;height:{h}px"></i>'
                    for w, h in [(94.5, 26), (76.25, 32), (110, 24), (58.75, 28)]
                )
                + "</div>"
            )
        else:
            row = next(
                r
                for r in rows
                if list(map(float, r[:7])) == [width, 265, 7.5, 1, 2, 0, mode]
            )
            vals = row[7:]
            body = "".join(
                f'<i style="position:absolute;left:{x}px;top:{y}px;width:{w}px;height:{h}px"></i>'
                for x, y, w, h in zip(*[iter(vals)] * 4)
            )
        panels.append(f"<h2>{label}</h2><section>{body}</section>")
    page = Path(f"compare-{width}.html").resolve()
    page.write_text(
        '<meta charset="utf-8"><style>body{margin:0;color:#203144;font:16px sans-serif}h2{padding:16px;font-size:18px;background:#f1f4f6}section{position:relative;height:90px}i{display:block;background:#224e86}i:nth-child(even){background:#b65534}</style>'
        + "".join(panels)
    )
    subprocess.run(
        [
            "google-chrome",
            "--headless",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--hide-scrollbars",
            f"--window-size={width},{height}",
            f"--screenshot=compare-{width}.png",
            page.as_uri(),
        ],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
