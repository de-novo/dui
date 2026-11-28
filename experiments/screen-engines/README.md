# Whole-screen engine comparison

Web reference: shadcn-admin at 76b0248cb6a1fa6ab60c6de80a2f242cf2b3878f (MIT), the production-built /403 and /503 routes, the EmptyWorkspace feature from dui, and the inline and grid scenes in #441. The two error-page inputs are feature slices with their UI component directory and TS configs. The full production application supplies their reference. The workspace comparison uses the component with explicit label props and its preview stylesheet. Input slicing limits attribution of whole-application styles. All projects and assets are local during capture. No development server participates.

Both web candidates receive the same model from the same release build of dui at 9f622bae. DOM uses the production source-model drawer. Yoga feeds its inert element tree and projected styles to Yoga 3.2.1 WASM, shapes text with Canvas 2D and paints the resulting boxes. Its 82-line adapter supports column and row flow, a subset of alignment, gaps, margins, padding and simple definite sizes. Grid, inline formatting and mixed text/element runs need more work. Neither candidate reads the reference during drawing. Compare these implementations as delivered. The single-box probes in the adjacent engine-comparison directory do not establish whole-screen Taffy performance.

The reference and both candidates use the same logical size and device scale factor 1. Score a whole screen when at most 1% of foreground pixels differ by more than 20 in any RGB channel. Foreground is the union of pixels with at least one channel below 235 in either image. A blank pair cannot pass. The fixed rule prevents the white background from drowning out missing text. All viewports and screens are weighted equally. Full-resolution PNGs, raw ratios and text are in evidence.

Map analysis uses five fresh CLI processes per screen after one discarded run, measuring wall time and peak process RSS on macOS. Both web candidates share that archive. Viewer memory is live JS heap after garbage collection on the same embedded viewer page with the same offscreen viewer present. Browser process RSS, GPU and WASM allocation are outside this measure. Bundle bytes include the minified model drawer and experimental mounting bridge, plus the layout dependency for Yoga. They exclude Chrome and the common viewer shell.

Edit runs alternate a literal change and revert four times on each screen, engine and viewport. Saves go through the real dui server's source watcher. Both experimental mounts long-poll the revision, fetch a full source-model snapshot, render, observe expected visible text, then wait two animation frames. The measurement includes that extra snapshot and offers a browser paint opportunity. Deadline is 1.5 seconds. Report nearest-rank p95 among successful edits and missed count separately. DOM then Yoga order is fixed. The sample does not establish a timing advantage for Yoga.

On iOS the Swift parser extracts only a single literal Text call from the two selected declarations. The simulator prototype constructs SwiftUI Text from that experimental record. Actual DetailView and ProfileView supply the reference using ImageRenderer at logical scale 1. Every edit must change both output PNGs and every revert must reproduce the reference. Timing includes parsing, atomic model publication and writing both raster sizes to disk. This measures save to completed raster, separately from presentation. This is an isolated probe, not production native drawing support.

## Reproduce

Use macOS arm64, local Chrome, Xcode with an iOS 26.5 simulator, Python and Pillow. From the repository root set a new absolute stage directory, then run:

```sh
mkdir -p "$stage/main" "$stage/home"
git archive 9f622bae075cf669ad82e859fa9567556eed86dd | tar -x -C "$stage/main"
(cd "$stage/main" && npm ci && npm ci --prefix apps/canvas && CARGO_TARGET_DIR="$stage/target" cargo build --locked --release -p dui)
npm ci --prefix experiments/web-basics
npm ci --prefix experiments/engine-comparison
git clone https://github.com/satnaing/shadcn-admin "$stage/admin"
(cd "$stage/admin" && git checkout 76b0248cb6a1fa6ab60c6de80a2f242cf2b3878f && corepack pnpm install --frozen-lockfile --ignore-scripts && corepack pnpm exec vite build)
node experiments/screen-engines/run.mjs --capture "$stage" '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
bash experiments/screen-engines/ios.sh "$stage/repeat" "$device"
python3 experiments/screen-engines/figure.py "$stage"
```

The first shadcn-admin install reported pnpm's ignored dependency build scripts. The installed graph built the full static production app successfully. The native default CLI produced route metadata without a drawable model for SwiftUI. Initial Android default attempts returned exit 5 before SDK provisioning. Records in native-default.json include exit status and output filenames; these are diagnostic attempts, not failed image comparisons.
