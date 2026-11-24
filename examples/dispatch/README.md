# Dispatch pickup board

[English](README.md) · [한국어](README.ko.md) · [简体中文](README.zh-CN.md)

Follow a collection from the pickup board to its driver instructions. The example uses plain HTML, CSS and a native dialog. [Watch the captured walkthrough (MP4, 20 s)](../recordings/dispatch.mp4)

## Capture and inspect

Start at the repository root with dui, Node.js and Chromium installed. Work in a temporary copy so the two captures share a project history.

```sh
work=$(mktemp -d)
cp -R examples/dispatch "$work/dispatch"
cd "$work/dispatch"
dui map . > "$work/before.json"
before=$(node -p 'require(process.argv[1]).output' "$work/before.json")
dui open "$before"
```

On the map, open the dialog card to see the recorded handoff instructions. Select the pickup board, open **Rendered elements** in **Element Inspector** and choose the heading to see its measured box.

## Compare a change

Stop the viewer with Ctrl+C. In the same shell, change the heading and capture the second run.

```sh
node -e 'const fs = require("node:fs"); const p = "index.html"; fs.writeFileSync(p, fs.readFileSync(p, "utf8").replace("<h1>Pickup board</h1>", "<h1>Pickup board · revised</h1>"))'
dui map . > "$work/after.json"
after=$(node -p 'require(process.argv[1]).output' "$work/after.json")
dui diff "$before" "$after"
dui open "$before" "$after"
```

In **Map options**, choose **Compare captures** and select the original and revised runs. The CLI reports the heading text change at each captured width.

## Open on desktop

Stop the web viewer and open the original run with a GPUI desktop build. Select the collection card to see its recorded neighbors.

```sh
dui desktop --run "$(node -p 'require(process.argv[1]).snapshot.id' "$work/before.json")"
```

## Run the automatic check

From the repository root with Rust, Node.js 26 and Chromium installed:

```sh
npm ci
npm ci --prefix apps/canvas
npm run check:example
```

CI follows the links and opens the dialog in Chromium at each captured width, then checks the archive, the Inspector selection and the before/after Diff. To exercise native rendering locally, build `apps/desktop/Cargo.toml` with Cargo and run `npm run check:example -- --desktop`. The video ends with overview and detail snapshots from macOS Metal.
