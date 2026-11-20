# Web viewer architecture

English | [简体中文](../zh-CN/web-viewer.md) | [한국어](../ko/web-viewer.md)

## 1. Scope and entry points

This is the implementation contract for v0.0.2. Today, `crates/dui` writes standalone HTML galleries and `dui open` opens the history file. The target viewer is `apps/canvas`, written in TypeScript 7 with React 19, TanStack Start in SPA mode, shadcn/ui, Tailwind CSS and React Flow. These dependencies belong to the viewer. Captured projects use their own stack.

Build the client during the dui build and embed the shell and hashed assets in the Rust binary. Keep the existing source installer and release build on the same asset build path. Opening an archive requires only dui and a browser. Keep old standalone galleries accessible. Live sessions also need the target toolchain and renderer. [Core architecture](architecture.md) · [Milestone](roadmap.md#v002)

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

Implement in this order: archive and schema fixtures → router and accessible list → map → live inspection and editing → packaging and benchmark. Use the real two-screen npmgraph case and a redistributable two-control fixture. Measure 30 warm edits with zero host rebuilds or browser restarts. Report cold startup separately from warm p50 and p95. [Viewer review skill](../../.agents/skills/dui-web-viewer/SKILL.md)
