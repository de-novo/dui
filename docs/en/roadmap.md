# dui roadmap · Reviewing an app as a UI map

English · [한국어](../ko/roadmap.md) · [简体中文](../zh-CN/roadmap.md)

dui maps the screens an app actually renders so a reviewer can follow a task from start to finish. Screens are the landmarks. Each screen opens into its states and viewport widths. A connection records the action that leads to another screen or state.

**A review should answer a concrete question.** After cancelling an order on a phone, where does the user land? The reviewer follows the path from order details through confirmation, opens the result at mobile width and records what feels unclear. An agent fixes the code. The reviewer returns to the same point on the map and compares the two worktrees.

## v0.0.1 · Capture baseline complete

Chromium renders the app with its project types and mock data. The local gallery shows captured states at each width, and the history viewer compares snapshots under `~/.dui/` by project, branch and worktree. The npmgraph example starts at one URL, discovers the search screen and produces four verified cells across two widths. [Reproduce the example](../../examples/npmgraph/README.md)

The M0 review screen passed at 375, 768 and 1280 px. Five states produced 15 verified cells and 15 distinct PNGs. The checks covered a local dialog state, the intercepted response body and the production client and server bundles. All 15 cell IDs remained in the report when Chromium disconnected.

A verified cell establishes that the observed state passed its checks. The reviewer judges whether the layout, wording and next action make sense. The roadmap below builds the map and the review workflow on that evidence.

## v0.0.2

**Open the map, then inspect a live state.** Start with one node per discovered screen. Expand a node to see its states and widths, then open the one that needs attention. On a screen with two menus, an agent can open one, check its contents and close it while the other menu keeps its state.

[Web viewer stack and interaction rules](design-v0.md#web-viewer) · [Web viewer architecture](web-viewer.md)

| Work | Acceptance evidence |
| --- | --- |
| [Screen map](https://github.com/de-novo/dui/issues/7) | On npmgraph, show two screens with two widths each. Preserve the selection when moving between overview and live inspection. Mark the discovered home link with its source. |
| [Typed element states](https://github.com/de-novo/dui/issues/1) | Test a boolean and an enum. CLI and MCP return the owner, instance, source, type, current value and editability. An ambiguous element or invalid value returns a reason and leaves state intact. |
| [Live rendering](https://github.com/de-novo/dui/issues/2) | Start the host and browser once. Set and reset a value in the running app. Check both the value read back and the visible DOM or accessibility result. |
| [Edit speed and capture](https://github.com/de-novo/dui/issues/3) | Measure 30 warm edits from request acceptance to a verified visible update. Report p50 and p95 in milliseconds, cold startup separately, and zero host rebuilds or browser restarts during the edits. Save a chosen state with its evidence under `~/.dui/`. |
| [Domain model](https://github.com/de-novo/dui/issues/9) | Document each entity's ID, fields and mutability with a diagram. Core types live in `dui-core` with no storage dependency and cover the map, saved captures, cache and comments. |
| [SQLite storage](https://github.com/de-novo/dui/issues/10) | Store metadata in SQLite through Drizzle migrations, keep images as content-addressed files and import existing `~/.dui/` history without loss. Keep the schema portable to a future remote Postgres server; sync through the API. |
| [Locale capture](https://github.com/de-novo/dui/issues/12) | Capture each screen in every supported language and flag missing translations and overflowing text. |
| [M1 coverage](https://github.com/de-novo/dui/issues/4) | Run the existing deterministic planning and coverage gates. Compare CLI and MCP results and report missing evidence for each unverified cell. [M1 gates](architecture.md#7-m0-and-m1-gates) |

The first benchmark establishes the latency baseline on a recorded machine and fixture. A cross-project latency target remains open.

## v0.0.3

**Native screens on the map.** Render one real GPUI screen at a known width with two typed states. Reuse the same inspect, set, read-back and reset contract. Record the OS, renderer and toolchain with the images. A matching value and independent semantic evidence are required for verification. Missing semantic evidence leaves the cell unverified. A missing render environment produces an unrenderable cell with its reason.

Baseline verified on `fixtures/gpui-kit` (`gpui-kit 0.10.0`, `gpui-pre 0.3.12`, `rustc 1.100.0`, macOS Metal offscreen renderer at 480 px, `elementTreeAvailable: true` via AccessKit / `TestQueryExt`; missing-render-env path reports `unrenderable` with `no-render-env`). Reproduce with `npm run check:gpui`.

Implementation · [#5](https://github.com/de-novo/dui/issues/5) · [#6](https://github.com/de-novo/dui/issues/6)

**Render only what changed.** Reuse cells whose inputs are unchanged and store each image once ([cache](https://github.com/de-novo/dui/issues/8)). Compare two captures by listing only the changed cells ([diff](https://github.com/de-novo/dui/issues/11)).

## Later milestones

| Version | Outcome |
| --- | --- |
| v0.0.4 | Follow an action from one screen or state to the next. Each edge shows whether it was extracted from code, inferred, or verified by replay. Replay from the initial state and check the destination before marking the edge verified. |
| v0.0.5 | Open the project map in the GPUI and gpui-kit desktop viewer. Attach a comment to a screen, state, width or transition. An agent reads the location through MCP. The reviewer reopens it on the updated worktree and compares before and after. |
| v0.0.6 | Audit the map on the reference screens against the 99% coverage goal. Publish verified and renderable cell counts, missing branches and reasons for gaps. Keep unresolved review comments visible alongside the evidence. |

[GitHub milestones](https://github.com/de-novo/dui/milestones)
