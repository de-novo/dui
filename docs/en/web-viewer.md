# Web viewer architecture

English | [简体中文](../zh-CN/web-viewer.md) | [한국어](../ko/web-viewer.md)

## 1. Scope and entry points

This is the implementation contract for v0.0.2. Today, `crates/dui` writes standalone HTML galleries and `dui open` opens the history file. The target viewer is `apps/canvas`, written in TypeScript 7 with React 19, TanStack Start in SPA mode, shadcn/ui, Tailwind CSS and React Flow. These dependencies belong to the viewer. Captured projects use their own stack.

Build the client during the dui build and embed the shell and hashed assets in the Rust binary. Keep the existing source installer and release build on the same asset build path. Opening an archive requires only dui and a browser. Keep old standalone galleries accessible. Live sessions also need the target toolchain and renderer. [Core architecture](architecture.md) · [Milestone](roadmap.md#v002)

**Commercial distribution.** Use the MIT-licensed `@xyflow/react` package and retain the default attribution on the map. A paid dui edition can use the library without a Pro subscription. Upstream asks projects that hide the on-screen attribution to subscribe. Separately, when distributing a binary with the viewer embedded, ship the copyright notice and MIT license from the exact package version. Check bundled dependencies when updating the lockfile. Implement dui map features using the public API and keep Pro examples and templates out of the repository. [MIT](https://github.com/xyflow/xyflow/blob/main/LICENSE) · [ProOptions](https://reactflow.dev/api-reference/types/pro-options) · [React Flow Pro](https://reactflow.dev/pro/content)

### Shared screen maps

Run `dui map . --url http://localhost:4200/` to map a running local web app. dui starts at that address and follows same-origin links using the existing Chromium capture and layout checks. This works across web frameworks, including Angular. Source analysis of framework-specific routes, guards, component inputs and states requires rules for the project's compiler and router.

Run `dui map PATH` on a React Navigation, Expo Router or Flutter app package to map its route declarations without a device. React Navigation and Expo Router use the project's TypeScript installation. Flutter uses the Dart SDK and a locked analyzer package which dui prepares in its own cache on first use. Point at the app package within a monorepo or select it with `appRoot` in `dui.json`.

Open the resulting archive with `dui open`. Source cards identify the declaration and its file and line. Named registrations can have a navigation key instead of a URL. Conditional and parameterized routes keep their expressions or patterns. Unresolved declarations appear in the discovery limitations. Source-only maps have zero attempted captures. Native appearance, transitions and runtime coverage require capture evidence.

When a recorder has opened a native route, save its PNG and write the following JSON alongside it. Copy the snapshot ID and the matching declaration ID from the source run that corresponds to the recorded build. The recorder supplies the actual route and logical viewport width. `runtime` accepts `android`, `ios`, `web` and `desktop`.

```json
{
  "sourceSnapshot": "<snapshot.id>",
  "captures": [{
    "screenId": "<graph.nodes[].id>",
    "runtime": "android",
    "observedPath": "/settings",
    "width": 393,
    "image": "settings.png"
  }]
}
```

Run `dui map attach SOURCE_RUN recordings.json` against the original local source archive, then open the new output with `dui open`. Each attached card keeps its source location and shows the recorded image. The archive records the PNG dimensions, runtime and observed route. An imported image is marked `unverified` because its association comes from the recorder. Automated coverage counts stay at zero. Use one capture per declaration, runtime and width in a recording set.

### React Flow validation

The development preview reads the entry captures and discovered links from a completed run. From the repository root, run the following with Node 26 and open the printed loopback URL. Replace the path with the output directory of a capture.

```sh
npm ci --prefix apps/canvas
npm --prefix apps/canvas run preview:archive -- /path/to/run
```

Build the viewer with [Vite](../../dependency-catalog.json) and the official Tailwind Vite plugin. `npm --prefix apps/canvas run build` writes browser assets to `apps/canvas/dist`. The build-only CSS tool has a narrow [license exception](architecture.md#62-automated-checks-ci-fails-on-any-violation).

Verified with the npmgraph archive: two screen nodes, one discovered link with its origin and selector evidence (marked not replay-verified), and four images at 375 and 1280 px. Chrome checks covered expanding states and widths, opening a live session from the selected screen, editing boolean and enum controls on the two-control inspect fixture, saving a verified revision, returning to overview with the selected screen preserved, keyboard focus return, reload, Back and visible attribution. The installed CLI still opens the standalone gallery.

For new maps, a directed link joins two captured screens when the source DOM contains a same-origin anchor to the target URL. Repeated anchors produce one edge. The viewer shows the edge at the widths where the anchor was observed. This records a navigation option; confirming that it works requires replay.

The preview accepts one or more run directories. With no arguments it lists saved screen maps under `~/.dui/projects`. Use **Select workspace** to choose a project, recorded branch and version. The rail always contains workspace selection, theme and collapse in that order. **Map options** contains screen lookup, link mode and capture width. Selecting a card opens its details with its states, widths, verification results, missing-image reasons, observed link evidence and **Open live session**. Returning to the overview preserves the selected screen. Cards count unverified and unrenderable cells across the available states and widths.

The native desktop overview draws a spanning tree for each connected group of screens. Selecting a screen highlights all recorded incoming and outgoing connections, including links outside that tree. Arrows retain their recorded direction and evidence type. Hover previews these connections when no screen is selected.

Use one display name per project ID across branches, worktrees and versions. The first successful capture records the primary project’s `package.json` name, falling back to its root folder name. `dui history` registers names for older local archives.

Use **Find screen** or `Cmd+K` / `Ctrl+K` to filter captured routes. The arrow keys move through matches and Enter opens the selected screen. The native desktop map also supports route and display-name search. Escape returns keyboard focus to Find screen. The desktop map controls also toggle a minimap. When a successful same-origin document at `/` changes its route during mounting, the inspector shows the requested path and the observed destination.

New Chromium captures include `screenEvidence` in CLI and MCP `inspect` and in both inspectors. Capture status records the capture checks. Screen verification additionally requires a successful document response, the requested route (or an accepted entry redirect), a settled page, and a visible element matching the project’s `readySelector`. A default readiness selector leaves screen identity unverified. A unique same-origin canonical link is automatically compared with the observed route. The observed heading is evidence the reviewer can read. Older archives retain their recorded evidence.

In the web viewer’s **Element inspector**, turn on **Element frames** to outline the rectangles recorded in the capture. **Alignment guides** shows the selected element’s edges or the measured offset between two controls. The switches work independently and start off. To find alignment changes, the viewer compares identifiable peer controls on the same screen, scenario, state and locale with their nearest narrower and wider captures. If an edge or centerline agreed at both reference widths (within 1 CSS pixel) and no shared alignment holds at the selected width, the inspector shows the pair, the offset and the reference widths. Intentional reflow can also appear here. With insufficient comparison evidence, you can still inspect recorded edges manually. For full-page Chromium captures, dui preserves the occupied vertical scrollbar gutter while taking the screenshot. Older archives can contain rectangles measured before the browser changed the layout for painting. Recapture the page if its PNG and frames disagree.

In **Map options → All map cards**, switch **Element frames** and **Alignment guides** on or off independently. Each card uses the coordinates recorded for the image it displays. Guides mark the largest measured responsive alignment change on that card. When no such change is available, they mark the row and column shared by the most non-overlapping sibling controls in the capture. A missing or ambiguous capture has no overlay.

From the project directory, run `dui recheck RUN` to retry cells whose capture was unverified or unrenderable, or whose recorded screen verification failed. Each retry uses the archived path, state inputs, width and locale with the current project and fresh rendering. Pass `dui recheck RUN PROJECT MANIFEST` for a custom manifest. The MCP tool `recheck` takes `run`, `project` and an optional `manifest`. A new archive contains only the selected cells, links back to the source in `recheck.source`, and reports `selected`, `skipped`, `resolved` and `remaining`. The CLI exits with `5` if a target still lacks evidence. When all source cells passed, it exits with `0` without launching a renderer or creating an archive.

For PR review, capture both branches and run `dui diff --base main --regressions-only --markdown`. The comparison uses the latest archived captures for the app on each branch, skipping selective rechecks and live edits. To pin the exact runs, use `dui diff BEFORE AFTER --regressions-only`. Capture and screen verification are compared independently for the same route, scenario, state, width and locale. A check counts as a regression only when it was verified on the base and has a recorded unsuccessful verdict on the head. The JSON lists affected cells under `verification.regressions` and counts compared checks and unmatched cells. Missing evidence stays unknown. The CLI exits with `5` for confirmed regressions and `0` for a completed comparison with none. If no checks can be compared, the verdict is `inconclusive` and the regression-only CLI exits with `2`. MCP `diff` accepts `regressionsOnly: true` and returns the same verdict as data.

The **Compare base and head captures** GitHub check runs when a PR opens or updates. It checks out both event commit SHAs, builds one dui binary from the base, and captures each shared web project in a fresh archive on each side. It discovers projects from tracked `index.html` files or a `dui.json` next to a `package.json`. Nested static pages belong to their enclosing project. npm apps install their locked dependencies. Routes, state inputs, widths, locales and readiness checks come from each revision’s project and dui discovery. The job summary includes both SHAs, unmatched projects or cells, and recorded evidence for each new failure. A regression or an inconclusive comparison fails the check. A repair commit triggers fresh captures.

The map initially estimates each frame from the capture. Once rendered, measured card heights, including captions, determine row placement and link clearance. Image loads and subsequent size changes update those measurements.

### Viewer design system

The preview uses Tailwind CSS 4.3.5 and the shadcn/ui Base UI preset (`base-nova`). The shared button and dialog were generated with shadcn CLI 4.23.0 and run on `@base-ui/react` 1.9.0. Tailwind reads the same theme tokens as the map.

Apply the saved theme before the first viewer paint. With the system setting, follow device appearance changes even while the archive is loading.

[tokens.css](../../apps/canvas/src/styles/tokens.css) · [Button](../../apps/canvas/src/components/ui/button.tsx) · [Dialog](../../apps/canvas/src/components/ui/dialog.tsx) · [CaptureCard](../../apps/canvas/src/features/map/CaptureCard.tsx)

- **Color and material** — Use solid monochrome canvas surfaces in both themes. Use orange `--primary` (#EF803F) for the dui mark and the primary action. Float the rail and inspector panels over the map with translucent frosted surfaces (light white glass in light mode, smoked charcoal glass in dark mode) using `--blur` (16 px), alongside frosted capture cards.
- **Type and spacing** — System sans for controls, system monospace for routes. At the default text size use 12 / 14 / 20 px for metadata, body and panel titles; space elements on a 4 / 8 / 12 / 16 / 24 px scale. Controls use a 6 px radius and panels use 12 px.
- **Controls** — Use `Button` for panel actions (default, primary or icon). Give icon-only controls accessible names. Reserve the focus outline for keyboard navigation. Selection uses placement and weight.
- **Evidence** — Use `CaptureCard` in both map views. Fit the original image inside the frame and name the route beneath it. Count reviewable issues on the card; read the recorded verdict and reason in the inspector.
- **Navigation** — Keep the rail order fixed: workspace, theme, collapse. Screen lookup stays on the map. Selecting a screen enlarges it and pages through its observed neighbors two at a time. Closing details restores the overview viewport.
- **Map controls** — Navigation and Map options share a bottom edge above the attribution. On phones, camera actions and Map options share one row with touch targets of at least 44 × 44 CSS pixels and at least 8 CSS pixels between them. Find screen sits next to the Cards/2D map switch. Deck controls wrap as needed. An overview fitted to the map follows viewport and rail size changes. Manual pan and zoom remain in place until the user chooses Fit view.

Check the actual viewer at 375 and 1280 px in both themes. Inspect an overview and a selected screen. Confirm all connected screens remain reachable, arrow directions match the archive, image colors are intact and keyboard focus returns on close.

## 2. Modules and dependencies

```text
apps/canvas/src/
  routes/                        # TanStack Start
  features/{history,map,inspector}/
  model/                         # selection, records, projections
  data/                          # HTTP, archive decoding, session client
  components/ui/                 # shadcn/ui primitives
  styles/                        # shared Tailwind tokens
packages/schemas/                # archive and session JSON Schemas
```

Routes compose features and validate URL parameters. Features import the shared model and UI primitives. Features communicate through selection or typed callbacks. The model contains pure functions and immutable values. It imports neither React nor I/O code. Only the map translates records into React Flow nodes and edges. Load the map and inspector when first opened. Scope generated shadcn/ui code and theme tokens to the viewer.

Rust owns verification, coverage and archive writes. CLI and MCP invoke the same application operations. The local HTTP adapter delivers viewer data. Keep these shell responsibilities in `crates/dui` until the workspace split described in the core architecture. Preserve the three existing ports. Implement a feature module when its first caller lands.

### 2.1 Clean Architecture boundaries

Organize each change around the policy it affects. Source dependencies point inward. Execution can call outward through a port owned by the inner contract.

| Boundary | Owns | Permitted dependencies |
|---|---|---|
| Domain | Typed observations, cell verdicts and session decisions. The same inputs produce the same result. | Pure values and functions in `dui-core`. |
| Application | The inspect, edit and save use cases. Order commands, invoke ports, feed observations back to the domain and return a typed outcome. | Domain and the `TypeOracle`, `Host` and `Renderer` contracts. |
| Adapters and wiring | CLI/MCP/HTTP translate inputs and errors. Compiler and Chromium adapters collect facts. The binary entry point supplies concrete implementations. | Application or port contracts, domain values and the adapter's own tools. |
| Browser presentation | URL selection, view models and controls. Route loaders read through `data/`; components receive model values and callbacks. | `data/` decodes wire types into `model/`; `features/` renders the model. The model imports neither layer. |

**Trace one edit.** CLI or MCP decodes a request into the same edit command. The use case asks the domain to decide. A stale revision produces a conflict and zero renderer calls. An accepted edit yields an effect. The application runs it through the renderer port, converts its observation into domain values and returns the verdict. Only the transport adapter maps the conflict to its protocol. Test the refusal with an in-memory renderer and the verdict with plain observations.

**Migration seam.** Today `ports.rs` passes JSON values, and `analysis.rs` adds `missing-screen-assertion` after the core verifier runs. When implementing live verification, move that decision into the shared verifier and convert the relevant JSON fields at the adapter boundary. Keep CLI and MCP results compatible. A React Flow node, CDP response or HTTP status must never become a domain input. Keep ordinary functions for pure work; introduce a trait only at an established port.

## 3. Data and navigation

Use the public JSON Schemas in `packages/schemas` as the wire contract. Validate Rust output against them and derive TypeScript wire types from them. Decode external JSON once into the viewer model. Accept additive fields. Read today's unversioned archives through a legacy decoder. Record a schema version on new archives. An unsupported version or invalid required field produces a data error for that run and leaves other runs available.

Treat a saved run as immutable. Read identity and Git metadata from `snapshot.json`, cell evidence from `report.json`, and topology from `graph.json`. Resolve opaque IDs within the selected run. Use branch and worktree as history filters. The route is `/projects/$project/runs/$run` with validated search keys `screen`, `state`, `width` and `view`. Optional keys allow a screen overview. Use the same selection for list, map and inspector. Back, reload and keyboard return restore that selection. A missing cell stays visibly unavailable.

The Rust listener binds to loopback. Serve finished runs under `/api/runs/$project/$run`, referenced images through lookup inside that run, and the static shell for viewer navigation. Missing API records and assets return their own errors. Scope live mutations to the owning local session. Keep control credentials and filesystem paths out of navigation URLs.

### Inspect captured state

Choose `state` and `width` from one cell in the run’s `report.json`.

```sh
dui inspect RUN_DIRECTORY '{"state":"STATE_ID","width":768}'
```

The MCP tool `inspect` accepts `directory` and the same `selection` object. Each control identifies its source, owner, instance, compiler type and captured value when available. Instance IDs are scoped to the cell. Add `slot` and `instance` to select one control. Add `value` to validate a proposed boolean or enum value without applying it. `editable` reports whether the captured control had a setter and a supported type. Older captures without instance evidence return `instance-evidence-unavailable`.

## 4. Live edits and failure handling

One local dui owner process holds a live session's host and renderer. CLI and MCP address that owner by session ID. The viewer reads the same session's revision and latest observed preview over loopback. A saved image is always labeled as an archive. The live inspector shows the running target’s last observed frame and edit status. Refresh on a revision change and discard superseded responses.

Use the core Decider (command → event/effect) and shell-owned resource guards. The operations are `open → inspect → set/reset → save → close`; inspection, edits and saves may repeat. Inspect returns owner, instance, source, type, current value and editability. Each edit carries the session, cell, instance, expected revision and request ID. Serialize edits per session. An invalid type, ambiguous instance or stale revision fails before mutation. A retry of the same request returns its recorded outcome. A reused ID with a different payload fails.

After mutation, read back the value and check the visible DOM or accessibility result. If validation fails after the write, return the observed value and a reason, advance the revision and mark the edit unverified. If the renderer disconnects, invalidate its session and allow reopening from the archived cell. Save publishes a new run atomically. Close releases the owned renderer and host. Navigation to another cell never retargets an in-flight edit.

The first client starts a background instance of the same binary. Publish its loopback address only after binding, under a user-private lock in `~/.dui`. Later clients reuse the instance. An expired session ID requires reopening the cell. Use same-origin JSON reads for the viewer and authenticated local commands for CLI/MCP. Poll a session revision while its inspector is open and stop polling when it closes.

```text
open(project, run, cell, requestId) -> session, revision
inspect(session, cell) -> revision, controls[], observation
set(session, cell, instance, slot, value, expectedRevision, requestId) -> editResult
reset(session, cell, instance, slot, expectedRevision, requestId) -> editResult
save(session, cell, expectedRevision, requestId) -> project, run, cell
close(session) -> closed
editResult = {session, cell, instance, slot, revision, observedValue, status, reason?, evidence}
```

Reset restores the selected control’s value at session open. Save writes the latest observed revision, including its verdict. Check request identity before the expected revision so a retry after a lost reply returns the first result. Retain request outcomes until close. After owner restart the old session expires. If the target revision differs from the archive, show the live provenance separately.

### Run a live CLI or MCP session

Pass a JSON object to `dui live '{…}'` or pass that object as arguments to the MCP tool `live`. Use an archived run directory and a cell from its report. Replace the placeholders below with the returned session ID, live instance ID and slot ID. For runs created by `dui map`, omit `manifest` to rediscover the project.

```jsonl
{"op":"open","project":"PROJECT","manifest":"MANIFEST","run":"RUN_DIRECTORY","cell":{"state":"STATE_ID","width":768},"requestId":"open-1"}
{"op":"inspect","session":"SESSION","cell":{"state":"STATE_ID","width":768}}
{"op":"set","session":"SESSION","cell":{"state":"STATE_ID","width":768},"instance":"INSTANCE","slot":"SLOT","value":true,"expectedRevision":0,"requestId":"edit-1","expect":{"selector":"#menu-toggle","text":"Close"}}
{"op":"reset","session":"SESSION","cell":{"state":"STATE_ID","width":768},"instance":"INSTANCE","slot":"SLOT","expectedRevision":1,"requestId":"reset-1","expect":{"selector":"#menu-toggle","text":"Open"}}
{"op":"save","session":"SESSION","cell":{"state":"STATE_ID","width":768},"expectedRevision":2,"requestId":"save-1"}
{"op":"close","session":"SESSION"}
```

Use a unique request ID per edit. Retry the same payload after a lost reply. For set and reset, `expect.selector` must match one visible HTML element whose trimmed inner text equals `expect.text`. A failed assertion after a write advances the revision and returns `unverified` with the observed value and text. Reset uses the value at open. Close releases the browser and host. An idle session expires after 30 minutes. Save returns the path of a new run under `~/.dui`. Run `dui open` (or `dui open RUN_DIRECTORY`) to open the embedded viewer, or pass the saved directory and cell to `dui inspect`. The archive contains the PNG, its SHA-256 digest, the observed control values and text assertion, the edit sequence and Git provenance. To edit again, open the original source cell.

### Warm edit baseline

On the pinned React inspect fixture, 30 alternating edits took **111.4 ms p50** and **112.9 ms p95** from owner acceptance to verified value and visible text. Opening a fresh session took **1.72 s** on an Apple M1 Pro, Darwin 25.5.0, Node 26.13.0, TypeScript 7.0.2 and Chrome 157.0.8103.79. Cold startup built one host and launched one browser. Both warm counts were zero. Reproduce with `npm ci`, `npm ci --prefix fixtures/replay` and `npm run check:inspect`. Each run prints its environment and fixture hashes.

## 5. Patterns and acceptance

| Pattern | Acceptance check |
|---|---|
| Boundary decoder and pure projection | Read a legacy run and a current run. The map and list resolve the same cells. Unknown statuses surface as data errors. Rust supplies all three verdicts and their reasons. |
| Feature-local state and shared selection | Navigate from a screen to its inspector using only the keyboard. Back restores focus and selection. Reload preserves the selected width. |
| Composition of accessible controls | Exercise verified, unverified and unrenderable cells. Labels and reasons remain readable at 375 and 1280 px. A captured placeholder retains its verdict. |
| One session with serialized commands | Edit a boolean and an enum through CLI and MCP. Check independent controls, duplicate requests, stale edits, disconnect and reset. Compare both read-back values and visible evidence. |
| Embedded client and immutable archive | Install from a clean checkout. Open a deep link with the target app stopped. Missing images show their error. Save a live state and reopen both the old run and the new run. |
| Multi-locale capture and viewer i18n ([#12](https://github.com/de-novo/dui/issues/12)) | Capture `screen × state × width × locale` from manifest `locales` or detected message files (`locales/*.json` / `messages/*.json`). Report per-locale summaries (`report.locales`), missing translation keys, overflow selectors and RTL mismatches. Switch the viewer UI language between `en`, `zh-CN` and `ko` from the workspace rail and filter archive cells by locale (`npm run check:i18n`). |
| Capture diff and pixel-difference overlay ([#11](https://github.com/de-novo/dui/issues/11)) | Compare two captures via `dui diff BEFORE AFTER [--overlay-dir DIR]`, MCP `diff` and the viewer's `?view=diff` view (`/api/diff`). Hide unchanged cells with identical content hashes, group added/removed/changed cells by screen, state and width, list changed source files per cell, render before/after + pixel-difference overlays for verified cells and report status changes without pixel diffs for unverified or unrenderable cells (`npm run check:diff`). |

Implement in this order: archive and schema fixtures → router and accessible list → map → live inspection and editing → packaging and benchmark. Use the real two-screen npmgraph case and a redistributable two-control fixture. Measure 30 warm edits with zero host rebuilds or browser restarts. Report cold startup separately from warm p50 and p95. [shadcn/ui](../../.agents/skills/shadcn/SKILL.md)
