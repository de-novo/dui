"""Build the fixture-only font and WASM size variant from a staged probe bundle."""
import argparse
import gzip
import json
from pathlib import Path
import shutil
import subprocess

from fontTools import subset

parser = argparse.ArgumentParser()
parser.add_argument("source", type=Path)
parser.add_argument("output", type=Path)
parser.add_argument("--wasm-opt", default="wasm-opt")
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
model = json.loads((args.source / "model.json").read_bytes())
label = model["components"][0]["root"]["children"][0]["children"][0]["parts"][0]["value"]
sizes = {}
for name in ["index.html", "model.json", "IBMPlexSans-Regular.ttf", "dui_gpui_web_probe.js", "dui_gpui_web_probe_bg.wasm"]:
    source, output = args.source / name, args.output / name
    if name.endswith(".ttf"):
        subset.main([str(source), "--text=" + label + " ", "--output-file=" + str(output)])
    elif name.endswith(".wasm"):
        subprocess.run([args.wasm_opt, "-Oz", source, "-o", output], check=True)
    else:
        shutil.copyfile(source, output)
    sizes[name] = [[p.stat().st_size, len(gzip.compress(p.read_bytes(), compresslevel=9, mtime=0))]
                   for p in (source, output)]
print(json.dumps(sizes, indent=2))
