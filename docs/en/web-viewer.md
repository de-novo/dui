# Web viewer architecture

English | [简体中文](../zh-CN/web-viewer.md) | [한국어](../ko/web-viewer.md)

## 1. Scope and entry points

This is the implementation contract for v0.0.2. Today, `crates/dui` writes standalone HTML galleries and `dui open` opens the history file. The target viewer is `apps/canvas`, written in TypeScript 7 with React 19, TanStack Start in SPA mode, shadcn/ui, Tailwind CSS and React Flow. These dependencies belong to the viewer. Captured projects use their own stack.

Build the client during the dui build and embed the shell and hashed assets in the Rust binary. Keep the existing source installer and release build on the same asset build path. Opening an archive requires only dui and a browser. Keep old standalone galleries accessible. Live sessions also need the target toolchain and renderer. [Core architecture](architecture.md) · [Milestone](roadmap.md#v002)

**Commercial distribution.** Use the MIT-licensed `@xyflow/react` package and retain the default attribution on the map. A paid dui edition can use the library without a Pro subscription. Upstream asks projects that hide the on-screen attribution to subscribe. Separately, when distributing a binary with the viewer embedded, ship the copyright notice and MIT license from the exact package version. Check bundled dependencies when updating the lockfile. Implement dui map features using the public API and keep Pro examples and templates out of the repository. [MIT](https://github.com/xyflow/xyflow/blob/main/LICENSE) · [ProOptions](https://reactflow.dev/api-reference/types/pro-options) · [React Flow Pro](https://reactflow.dev/pro/content)

### React Flow validation

The development preview reads the entry captures and discovered links from a completed run. From the repository root, run the following with Node 26 and open the printed loopback URL. Replace the path with the output directory of a capture.

```sh
npm ci --prefix apps/canvas
npm --prefix apps/canvas run preview:archive -- /path/to/run
```

Verified with the npmgraph archive: two screen nodes, one discovered link and four images at 375 and 1280 px. Chrome checks covered keyboard selection, focus return, reload, Back and visible attribution. This preview exercises the map component independently of the planned Start shell and live inspector. The installed CLI still opens the standalone gallery.

For new maps, a directed link joins two captured screens when the source DOM contains a same-origin anchor to the target URL. Repeated anchors produce one edge. The viewer shows the edge at the widths where the anchor was observed. This records a navigation option; confirming that it works requires replay.

The preview accepts one or more run directories. With no arguments it lists saved screen maps under `~/.dui/projects`. Use **Select workspace** to choose a project, recorded branch and version. The rail always contains workspace selection, theme and collapse in that order. **Map options** contains screen lookup, link mode and capture width. Selecting a card opens its details and shows its observed links. Cards count unverified and unrenderable cells across the available states and widths.

### Viewer design system

The preview uses Tailwind CSS 4.3.5 and the shadcn/ui Base UI preset (`base-nova`). The shared button and dialog were generated with shadcn CLI 4.23.0 and run on `@base-ui/react` 1.9.0. Tailwind reads the same theme tokens as the map.

[tokens.css](../../apps/canvas/src/styles/tokens.css) · [Button](../../apps/canvas/src/components/ui/button.tsx) · [Dialog](../../apps/canvas/src/components/ui/dialog.tsx) · [CaptureCard](../../apps/canvas/src/features/map/CaptureCard.tsx)

- **Color and material** — Use solid monochrome canvas and panel surfaces in both themes. Use orange `--primary` (#EF803F) for the dui mark and the primary action. Float the rail over the map with a frosted white surface in light mode and a smoked charcoal surface in dark mode. Only the fixed rail and the capture card use `--blur` (10 px).
- **Type and spacing** — System sans for controls, system monospace for routes. At the default text size use 12 / 14 / 20 px for metadata, body and panel titles; space elements on a 4 / 8 / 12 / 16 / 24 px scale. Controls use a 6 px radius and panels use 12 px.
- **Controls** — Use `Button` for panel actions (default, primary or icon). Give icon-only controls accessible names. Reserve the focus outline for keyboard navigation. Selection uses placement and weight.
- **Evidence** — Use `CaptureCard` in both map views. Fit the original image inside the frame and name the route beneath it. Count reviewable issues on the card; read the recorded verdict and reason in the inspector.
- **Navigation** — Keep the rail order fixed: workspace, theme, collapse. Screen lookup stays on the map. Selecting a screen enlarges it and pages through its observed neighbors two at a time. Closing details restores the overview viewport.

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

## 5. Patterns and acceptance

| Pattern | Acceptance check |
|---|---|
| Boundary decoder and pure projection | Read a legacy run and a current run. The map and list resolve the same cells. Unknown statuses surface as data errors. Rust supplies all three verdicts and their reasons. |
| Feature-local state and shared selection | Navigate from a screen to its inspector using only the keyboard. Back restores focus and selection. Reload preserves the selected width. |
| Composition of accessible controls | Exercise verified, unverified and unrenderable cells. Labels and reasons remain readable at 375 and 1280 px. A captured placeholder retains its verdict. |
| One session with serialized commands | Edit a boolean and an enum through CLI and MCP. Check independent controls, duplicate requests, stale edits, disconnect and reset. Compare both read-back values and visible evidence. |
| Embedded client and immutable archive | Install from a clean checkout. Open a deep link with the target app stopped. Missing images show their error. Save a live state and reopen both the old run and the new run. |

Implement in this order: archive and schema fixtures → router and accessible list → map → live inspection and editing → packaging and benchmark. Use the real two-screen npmgraph case and a redistributable two-control fixture. Measure 30 warm edits with zero host rebuilds or browser restarts. Report cold startup separately from warm p50 and p95. [shadcn/ui](../../.agents/skills/shadcn/SKILL.md)
