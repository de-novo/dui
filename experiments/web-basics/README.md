# Web basics calibration

Compare the default source map and its production card against a locally compiled React fixture. The 37 inventory IDs in #441 have 49 scenarios. Seven extra paint and transform IDs bring the total to 56 scenarios. Router discovery is outside this measurement.

## Reproduce on macOS

Run from this directory with Git, Cargo, Node, Chrome and Python with Pillow installed. The benchmark uses Darwin time -l. Use an empty output directory and run the cost and live measurements separately.

```sh
work=$(mktemp -d)
src="$work/main"
mkdir "$src"
git archive 9f622bae075cf669ad82e859fa9567556eed86dd | tar -x -C "$src"
(cd "$src" && npm ci && npm ci --prefix apps/canvas && cargo build --release --locked -p dui)
npm ci
dui="$src/target/release/dui"
chrome='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
node measure.mjs --capture "$src" "$dui" "$chrome" "$work/run"
cp "$work/run/results.json" "$work/run/all.json"
node live.mjs "$dui" "$chrome" "$work/run" "$work/live"
cp "$work/live/live.json" "$work/run/live.json"
python3 bench.py "$work/run" "$dui"
python3 report.py "$work/run"
```

## Reading the evidence

The default command maps a source directory with a missing Chrome executable. Offline compilation prepares the React reference and built CSS input for experiments. Chrome verification measures the reference and the archived model drawn by the main viewer. Both logical card sizes use DPR 1. The images show the top-left 360×116 area. Probes read full geometry and line rectangles. The live trial instead uses two actual viewer windows on the same built-in dui server. No project dev server is used.

Geometry tolerance is 1 CSS pixel, or 2 for native control geometry. Interior RGB channels allow a difference of 2/255. Other mandatory probes use exact equality. A passing item satisfies all its scenarios. List identity additionally requires a declared model key. Native form probes inspect visible state and DOM attributes. Each edit has a source replacement and separately declared expected value. Timing starts immediately before writing the file and ends after the expected DOM change and two animation frames. It includes observation overhead and measures a browser paint opportunity. The embedded viewer chooses its own card size. One trial per item and width supports a baseline observation, not a per-item latency percentile.

### Candidate interpretation and failure attribution

- **sheet** uses the existing CLI built-CSS and viewport options, with zero new extractor or renderer lines. Seven failures recover. Compiled module names regress the CSS Modules control. Ordinary import discovery and module binding preservation are tracked in #444. The valid font-face input is rejected in this mode and in lower (#423).
- **lower** resolves a restricted syntax tree to literal JSX before the existing extractor. The projection is 102 lines and serialization is 8 lines, excluding CLI/measurement glue. Six JSX failures recover, including ordered spreads and default props. Key identity is lost after list expansion. Serializing inline styles into quoted object keys regresses passing layout and paint controls. Track the narrowly demonstrated binding behavior in #445.
- **sidecar** uses the same 102-line projection with a 26-line DOM materializer, compiled CSS and local fixture assets. Chrome supplies CSS layout, text and native controls. This isolates information loss across translation and drawing. Its passing scores are a diagnostic upper bound, and its process timings exclude browser rendering.
- **Layout and paint**. Archived grid, positioned, overflow, border and transform nodes retain partial styles such as gap or explicit size and mark other declarations unresolved. Competing selectors, conditional classes, Tailwind and inherited variables still fail in the built-sheet path. See #404 and #440. The table and raw JSON distinguish computed-style errors from visible geometry.
- **Images, fonts and controls**. Literal raster/SVG nodes do not deliver the required drawable surface in the default card. The font probe checks the named loaded face, size, line height and line boxes. Form metadata can survive in v1 while label, value or checked state is lost on presentation. The button retains its tested state but its default font metrics differ. The sidecar tests the combined asset/DOM path, so it cannot assign the entire improvement to the extractor or the drawer alone.

The paired PNG archive contains all captured candidates and references. Missing experimental images correspond to extraction rejection. The model archive contains each baseline model, source, built stylesheet, binding map and experimental scene. Raw per-probe values are in results.json.
