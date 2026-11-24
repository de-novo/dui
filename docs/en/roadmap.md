# dui roadmap · Reviewing an app as a UI map

English · [한국어](../ko/roadmap.md) · [简体中文](../zh-CN/roadmap.md)

dui maps the screens an app actually renders so a reviewer can follow a task from start to finish. Screens are the landmarks. Each screen opens into its states and viewport widths. A connection records the action that leads to another screen or state.

**A review should answer a concrete question.** After cancelling an order on a phone, where does the user land? The reviewer follows the path from order details through confirmation, opens the result at mobile width and records what feels unclear. An agent fixes the code. The reviewer returns to the same point on the map and compares the two worktrees.

Capture a pickup board with `dui map .`, inspect its dialog, compare a heading change and open the same run on the native desktop. [Run the Dispatch example](../../examples/dispatch/README.md)

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

Baseline verified on `fixtures/gpui-kit` (`gpui-kit 0.10.0`, `gpui-pre 0.3.12`, `rustc 1.100.0`, macOS Metal offscreen renderer at 480 px, `elementTreeAvailable: true` via AccessKit / `TestQueryExt`; missing-render-env path reports `unrenderable` with `no-render-env`). Reproduce with `npm run check:gpui`. Live native session round-trip verified through the shared `dui live` / MCP `live` contract across 30 warm edits (**3.65 ms p50 / 4.23 ms p95**, 0 host rebuilds, 0 native restarts, independent second view instance state preserved, missing semantic evidence reported as `unverified` with `missing-semantic-evidence`). Reproduce with `npm run check:gpui-live`.

Implementation · [#5](https://github.com/de-novo/dui/issues/5) · [#6](https://github.com/de-novo/dui/issues/6)

**Render only what changed.** Reuse cells whose inputs are unchanged and store each image once ([cache](https://github.com/de-novo/dui/issues/8)). Compare two captures by listing only the changed cells ([diff](https://github.com/de-novo/dui/issues/11)).

## Later milestones

| Version | Outcome |
| --- | --- |
| v0.0.4 | Follow an action from one screen or state to the next. Each edge shows whether it was extracted from code, inferred, or verified by replay. Replay from the initial state and check the destination before marking the edge verified ([#31](https://github.com/de-novo/dui/issues/31), `dui transitions` / MCP `transitions`, `npm run check:transitions`). |
| v0.0.5 | Open the project map in the GPUI and gpui-kit desktop viewer (`apps/desktop`, `dui desktop`, [#40](https://github.com/de-novo/dui/issues/40), `npm run check:desktop`). Attach a comment to a screen, state, width or transition. An agent reads the location through MCP. The reviewer reopens it on the updated worktree and compares before and after ([#38](https://github.com/de-novo/dui/issues/38), `dui comment` / MCP `comment_*` / viewer `/api/comments`, `npm run check:comments`). |
| v0.0.6 | Audit the map on the reference screens against the 99% coverage goal. Publish verified and renderable cell counts, missing branches and reasons for gaps. Keep unresolved review comments visible alongside the evidence ([#44](https://github.com/de-novo/dui/issues/44), `dui audit` / MCP `audit` / viewer `/api/audit`, `npm run check:audit`). |
| v0.0.7 | Priority 2 declarative rule packs (Zustand 5, Redux Toolkit 2, Jotai 3, XState 5, React Hook Form 7, valibot 1, React Router 8, TanStack Start 1), optional `hybrid`/`live` real-API mode with read-only mutation guard, branch-name diff (`dui diff --base main`), loop-free multi-hop BFS with dynamic ID collapsing (`/orders/:id`), greedy pairwise covering array, `<select>` real option discovery, visual-change witness verification, 2D canvas inline diff (`Elements` render-tree diff with lazy `Pixel` fallback), and `linksTo` edges on all captures ([#56](https://github.com/de-novo/dui/issues/56), [#63](https://github.com/de-novo/dui/issues/63), [#66](https://github.com/de-novo/dui/issues/66), [#70](https://github.com/de-novo/dui/issues/70), [#74](https://github.com/de-novo/dui/issues/74), [#76](https://github.com/de-novo/dui/issues/76), [#80](https://github.com/de-novo/dui/issues/80), [#82](https://github.com/de-novo/dui/issues/82), [#84](https://github.com/de-novo/dui/issues/84), [#86](https://github.com/de-novo/dui/issues/86), [#88](https://github.com/de-novo/dui/issues/88)). |
| v0.0.8 | **Multi-surface platform architecture & Universal Web + TUI.** Formalize `SurfaceRegistry`, `NoopOracle`, and the unified `RenderNodeSnapshot` IR so every surface shares one `CaptureEngine` pipeline without duplicating caching, SQLite, diffing, or canvas review ([#99](https://github.com/de-novo/dui/issues/99)). Ship the Universal Web Host (`static` / `url` / `command`, [#100](https://github.com/de-novo/dui/issues/100)) for any HTML/JS app and the Universal TUI Adapter (`pty` + VT100 cell grid -> `RenderNodeSnapshot` + monospace PNG, [#101](https://github.com/de-novo/dui/issues/101)). |
| v0.0.9 | **Android, Desktop, Design-Defect Detection & DevTools Element Inspector.** Add the Android adapter (`adb screencap` + `uiautomator`/Semantics -> `RenderNodeSnapshot`, [#102](https://github.com/de-novo/dui/issues/102)), Electron 45 host + GPUI AccessKit render-tree diff ([#103](https://github.com/de-novo/dui/issues/103)), Radix UI / shadcn rule packs + Next.js 16 host ([#104](https://github.com/de-novo/dui/issues/104)), multi-tab parallel capture + automatic design-defect & a11y lint over `renderTree` (overflow, overlap, clipping, contrast, touch target size, spacing/alignment drift — shown visually on the canvas for humans and as structured JSON via CLI/MCP for AI, [#105](https://github.com/de-novo/dui/issues/105)), and the DevTools-style element inspector with box-model measurements (width, height, margin, padding, gap, spacing redlines, [#106](https://github.com/de-novo/dui/issues/106)). |
| v0.1.0 | **Cross-platform Mobile Capture & Compiler-Oracle Foundations.** Add iOS Simulator (`xcrun simctl` + XCUI) capture, universal authenticated sessions (`storageState.json` / `POST authFlow`), PR Markdown diff output, React Native / Expo Router / Ink `tsc` rule packs, shared compiler-oracle IR, and `dui toolchains` probing 6 official compilers (`tsc`, `rustc`, `go`, `swiftc`, `kotlinc`, `dart`). |
| v0.2.0 | **Universal Routing Tree, Full-Page True-Aspect Nodes, Modal Auto-Capture & Fact-Only Core.** Capture full-page screen heights (including nested app-shell scroll containers) and size canvas/desktop nodes to each screen's real aspect ratio without cropping or overlap ([#147](https://github.com/de-novo/dui/issues/147)), discover framework-agnostic navigation trees via WAI-ARIA disclosure expansion + official compiler route registries with zero-config `dui scan` & multi-host `dui map` ([#149](https://github.com/de-novo/dui/issues/149), [#153](https://github.com/de-novo/dui/issues/153)), align GPUI desktop UX with native macOS `TitleBar` and obstacle-free gutter edge routing ([#155](https://github.com/de-novo/dui/issues/155), [#159](https://github.com/de-novo/dui/issues/159)), auto-discover and capture modals/dialogs/drawers via WAI-ARIA as child overlay nodes ([#157](https://github.com/de-novo/dui/issues/157)), and remove hardcoded global design-defect heuristics and premature iframe live-mount so `dui` reports strictly objective rendering evidence ([#163](https://github.com/de-novo/dui/issues/163), [#169](https://github.com/de-novo/dui/issues/169)). |
| v0.3.0 | **Pluggable Design-System Conformance (`DESIGN.md` / Plugins) & Code-Based Canvas Rendering.** Keep `dui-core` strictly fact-only while adding an opt-in plugin architecture in `dui.json` so projects can attach their own `DESIGN.md`, W3C Design Tokens, or custom checker plugins ([#167](https://github.com/de-novo/dui/issues/167)), and evolve canvas rendering from bitmap captures toward direct code-based component rendering. |

## Multi-surface architecture & evolution

dui targets four surfaces—**Web**, **Desktop App**, **Mobile App (Android & iOS)**, and **TUI**—using one shared architecture designed to be equally effortless for **both humans and AI agents**, built on **objective rendering evidence** with pluggable design-system conformance:

1. **Three ports (`TypeOracle`, `Host`, `Renderer`)**: official compiler analysis when available (Level 2), or `NoopOracle` for universal capture on any app (Level 1).
2. **Unified `RenderNodeSnapshot` IR**: DOM, AccessKit, VT100 terminal cells, Android `uiautomator`/Semantics, and iOS `XCUI` trees all normalize into one platform-neutral scene graph carrying geometry, box model (`margin`, `padding`, `border`, `gap`), typography, colors, and attributes.
3. **Single template-method `CaptureEngine`**: caching, SQLite persistence, render-level diff, lazy pixel diff, visual-change witness verification, coverage audit, and CLI/MCP parity live once in the shared core/shell.
4. **Dual human + AI ergonomics**: humans scan the 2D canvas with instant visual highlights, box-model measurements, and diffs; AI reads the exact same `renderTree`, box metrics, diffs, and comment anchors as structured JSON via CLI/MCP without guessing from raw screenshots.
5. **Fact-only core → Pluggable design-system specs & code-based rendering**: `dui-core` never imposes hardcoded aesthetic opinions; teams opt into conformance checks against their own `DESIGN.md`, token definitions, or custom audit plugins (`v0.3.0`, [#167](https://github.com/de-novo/dui/issues/167)), while future canvas milestones move from bitmap screenshots to direct code-based rendering.

[GitHub milestones](https://github.com/de-novo/dui/milestones) · [Changelog](../../CHANGELOG.md)

The [Compose fixture](../../fixtures/android-compose) mounts two independent instances of the same settings row. The [emulator check](../../scripts/check-compose.mts) reads their names, bounds and checked states from fresh dui captures at 720 and 1080 px. It taps one row, checks its sibling, then taps again to verify reset. To reproduce, build dui, install the fixture debug APK on a disposable emulator, set `ANDROID_SERIAL`, and run `node scripts/check-compose.mts`. The [CI workflow](../../.github/workflows/android-compose.yml) includes the build and emulator commands.
