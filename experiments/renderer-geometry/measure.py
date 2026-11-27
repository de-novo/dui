"""Compare a model-only Taffy border box with an opt-in Chromium reference."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tempfile

from translate import translation

parser = argparse.ArgumentParser()
parser.add_argument("--dui", required=False, type=Path)
parser.add_argument("--capture", required=True, metavar="CHROME", type=Path)
args = parser.parse_args()
renderer = Path(__file__).resolve().parent / "target/release/renderer-geometry"
cases = {
    "fixed": "width:80px;height:24px",
    "translated": "width:80px;height:24px;transform:translate(12px,6px)",
    "constrained": "width:80px;height:24px;max-width:50px",
}
with tempfile.TemporaryDirectory() as tmp:
    project = Path(tmp)
    (project / "App.tsx").write_text('export function App(){return <div id="box"/>}')
    for name, css in cases.items():
        (project / "built.css").write_text("#box{" + css + "}")
        command = [args.dui, "model", project, "App.tsx", "--built-css", "built.css"] if args.dui else [renderer.with_name("fixture-model"), project]
        model = subprocess.check_output(command)
        extracted = json.loads(model)
        offset = translation(extracted, (project / "built.css").read_bytes())
        if offset is not None:
            model = json.dumps({"model": extracted, "translatePx": offset}).encode()
        predicted = json.loads(subprocess.check_output([renderer], input=model))
        page = project / "index.html"
        page.write_text('<!doctype html><style>body{margin:0}#box{' + css +
                        '}</style><div id="box"></div><output></output><script>'
                        'const r=document.querySelector("#box").getBoundingClientRect();'
                        'document.querySelector("output").textContent=JSON.stringify([r.x,r.y,r.width,r.height])</script>')
        html = subprocess.check_output([args.capture, "--headless", "--disable-gpu", "--no-first-run",
                                        "--window-size=390,844", f"--user-data-dir={project / 'profile'}",
                                        "--dump-dom", page.as_uri()], stderr=subprocess.DEVNULL, text=True, timeout=30)
        truth = json.loads(re.search(r"<output>(\[.*?\])</output>", html)[1])
        assert predicted["id"] == "box"
        errors = [abs(x-y) for x,y in zip(predicted["rect"], truth, strict=True)]
        print(json.dumps(dict(case=name, reference=truth, predicted=predicted, absoluteError=errors)))
