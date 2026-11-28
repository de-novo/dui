# Stage 3 deterministic data/state benchmark harness

This directory contains the reusable harness used for stage 3 measurements. It never copies a capture or another project's model into a target: every candidate is extracted with `dui model` from the target checkout and entry below, then mounted by the source-model renderer with explicit JSON slot values.

Public target paths are environment-driven so private local checkouts and identifiers stay out of the repository:

```sh
CALCOM_WEB_ROOT=/path/to/calcom/apps/web \
PLANE_WEB_ROOT=/path/to/plane/apps/web \
PRIVATE_TARGET_WEB_ROOT=/path/to/private/app \
npm install --prefix experiments/app-native-benchmark/stage3
STAGE3_FIXTURE_OVERLAY=/secure/local/private-fixtures.json \
node experiments/app-native-benchmark/stage3/run.mjs --out /tmp/dui-stage3-results
```

The harness writes `results.json`, mounted-card PNGs for 1440×920 and 390×844, pixel diffs when a reference PNG is configured, and a browser trace/video directory for behavior evidence. `fixtures.json` is the exact shared public state input. Private targets are supplied only through a local overlay file named by `STAGE3_FIXTURE_OVERLAY`; that overlay and private evidence are not committed. Query slots use the ordinary query-result object shape (`data`, `isPending`, `isError`) rather than executing application code.

`measure-live.mjs` uses the production input path for viewer evidence: it extracts the target once to resolve configured fixture aliases to exact slot ids, writes those explicit values as `dui.source-inputs.json` in the temporary copied project, points `dui.json` at that file, and then archives and opens the project with `dui map --entry` and `dui serve`. It does not intercept viewer routes or rewrite network/browser payloads. Its visible assertions inspect the selected approximate-model card's shadow root in each viewport.
