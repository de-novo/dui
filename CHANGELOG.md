# Changelog

English | [한국어](docs/ko/changelog.md) | [简体中文](docs/zh-CN/changelog.md)

All notable changes to `dui` are documented in this file by release milestone (`v0.0.1`–`v0.0.4`).
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and release versions correspond to SemVer Git tags (`vX.Y.Z`).

## Changelog policy

- Record user-visible additions, behavioral changes, bug fixes, and removals under `[Unreleased]` as PRs land on `main`.
- Group entries under **Added**, **Changed**, **Fixed**, and **Removed**, citing the relevant PR (`#N`) and issue (`#N`) links.
- When cutting a release tag `vX.Y.Z`, move the `[Unreleased]` entries under `## [vX.Y.Z] - YYYY-MM-DD` and update `docs/ko/changelog.md` and `docs/zh-CN/changelog.md` in the same PR.

---

## [Unreleased]

- Planned next milestone: `v0.0.5` (review comments attached to screen/state/width/transition via CLI/MCP/viewer and native `gpui-kit` desktop viewer).

---

## [v0.0.4] - 2026-11-21

**Focus:** Action-driven transition replay, embedded single-binary web viewer, tag-based installation/updates, and Figma-grade canvas readability and navigation.

### Added

- **Action-driven transition replay & edge provenance** ([#31](https://github.com/de-novo/dui/issues/31), [PR #32](https://github.com/de-novo/dui/pull/32)):
  - Added pure transition verdict and provenance rules in `dui-core::transition` (`extracted` / `inferred` / `verified`).
  - Added `Browser::replay_transition`, `dui transitions PROJECT MANIFEST [RUN_DIR]`, and MCP `transitions` returning identical idempotent JSON and writing replay provenance, confirmed widths, and per-width failure reasons to `graph.json` and `transitions.json`.
  - Updated `apps/canvas` to visually distinguish replay-verified (solid orange), extracted (dashed), and inferred (dotted) edges with provenance badges and failure evidence in `en`, `zh-CN`, and `ko`.
  - Added `npm run check:transitions` to CI.
- **Embedded single-binary web viewer (`dui open` / `dui serve`)** ([PR #29](https://github.com/de-novo/dui/pull/29)):
  - Built and embedded the `apps/canvas` Vite bundle inside `dui` via `crates/dui/build.rs`.
  - Added an in-process loopback HTTP server (`crates/dui/src/viewer.rs`) serving `apps/canvas`, `/archive/*`, `/api/diff` + `/api/diff/overlay`, and `/api/live` + `/api/live/frame` directly from the `dui` binary.
  - Added `npm run check:viewer` to CI.
- **Tag-based version, self-update, and source installer** ([PR #29](https://github.com/de-novo/dui/pull/29), [PR #30](https://github.com/de-novo/dui/pull/30)):
  - Added pure `dui-core::version::SemVerTag` parsing and numeric comparison for `vX.Y.Z` tags.
  - Embedded build-time `DUI_VERSION` in `dui --version` (`{ name, version, tag }`) and added `dui update [--check] [--tag TAG] [--repo URL] [--install-dir DIR]`.
  - Updated `scripts/install` to resolve and build the latest `vX.Y.Z` Git tag (including `apps/canvas`), automatically add the install directory to shell `PATH` (`zsh` / `bash` / `fish` / `sh`, opt-out via `DUI_NO_MODIFY_PATH=1`), and symlink into a user-writable `PATH` bin directory when available.
- **Figma-style trackpad navigation** ([#35](https://github.com/de-novo/dui/issues/35), [PR #36](https://github.com/de-novo/dui/pull/36)):
  - Enabled two-finger 2D trackpad panning (`panOnScroll`, `PanOnScrollMode.Free`, `zoomOnScroll={false}`), pinch-to-zoom (`zoomOnPinch`), `Cmd`/`Ctrl`+wheel zoom, and keyboard zoom shortcuts (`+`, `-`, `0` for `1:1`, `Shift+1` for fit view).

### Changed

- **1:1 capture-width map nodes & 3× zoom** ([#33](https://github.com/de-novo/dui/issues/33), [PR #34](https://github.com/de-novo/dui/pull/34)):
  - Replaced hardcoded `280px` map cards and `16:9` cropping with dynamic `nodeMetrics(width)` matching real capture dimensions (`1280×800` desktop, `375×600` mobile) at `zoom = 1.0`, raised `maxZoom` to `3`, lowered `minZoom` to `0.02`, and added a centered `1:1` control button.

### Fixed

- **Hoisted monorepo TypeScript resolution** ([PR #30](https://github.com/de-novo/dui/pull/30)):
  - Updated `scan()` in `crates/dui/src/main.rs` to walk ancestor directories for `node_modules/typescript/package.json`, enabling `dui map` / `dui run` inside hoisted pnpm/npm monorepo packages.

### Removed

- **Legacy static HTML galleries** ([PR #29](https://github.com/de-novo/dui/pull/29)):
  - Removed `crates/dui/src/graph.html`, `crates/dui/src/history.html`, `adapters/browser/graph.ts`, and `adapters/browser/history.ts`; replaced `dui open` gallery files with the embedded `apps/canvas` viewer and made `dui history` return pure JSON snapshots.

---

## [v0.0.3] - 2026-11-21

**Focus:** Content-addressed incremental capture cache, capture-to-capture visual diffs, and real native GPUI (`gpui-kit`) rendering and live sessions.

### Added

- **Content-addressed capture cache & `dui prune`** ([#8](https://github.com/de-novo/dui/issues/8), [PR #25](https://github.com/de-novo/dui/pull/25)):
  - Extracted compiler-resolved `moduleGraph` in `adapters/typescript/facts.mts` to scope per-cell `CacheInputs` by dependent source files, facts, mocks, state, width, locale, rule packs, `dui` version, and renderer environment.
  - Stored PNGs once under `~/.dui/blobs/<sha256>.png` and hard-linked across runs, branches, and worktrees; skipped starting Vite/Chromium on full cache hits (`14,385 ms -> 520 ms`, `27.64×` on `fixtures/toggle`).
  - Added per-cell hit/miss and changed-input attribution across CLI/MCP, `--force` / MCP `force: true`, `dui prune [--max-bytes BYTES]` / MCP `prune`, and `npm run check:cache`.
- **Capture diff & pixel-difference overlays** ([#11](https://github.com/de-novo/dui/issues/11), [PR #26](https://github.com/de-novo/dui/pull/26)):
  - Added pure diffing and RGBA pixel-diff computation in `dui-core::diff` hiding unchanged cells, listing per-cell `changedFiles`, computing `diffPixels` / `diffRatio` / `changedBounds` + `#EF803F` overlays, and reporting status changes without pixel diffs for `unverified`/`unrenderable` cells.
  - Added `dui diff BEFORE AFTER [--overlay-dir DIR]`, MCP `diff`, viewer `/api/diff` + `/api/diff/overlay`, `CaptureDiffView` (`en` / `zh-CN` / `ko`), and `npm run check:diff`.
- **Real GPUI (`gpui-kit`) offscreen rendering** ([#5](https://github.com/de-novo/dui/issues/5), [PR #27](https://github.com/de-novo/dui/pull/27)):
  - Added standalone `fixtures/gpui-kit` pinned to `gpui-kit 0.10.0` (`gpui-pre 0.3.12`, Apache-2.0) rendering a real two-instance `SettingsScreen` via `HeadlessAppContext` offscreen Metal/WGPU rendering and AccessKit / `TestQueryExt` element-tree observations.
  - Added `GpuiRenderer` implementing the shared `Renderer` port in `crates/dui/src/gpui.rs`, recording OS, toolchain, `gpui-kit`/`gpui-pre` versions, render path, and `elementTreeAvailable`, plus explicit `unrenderable` (`no-render-env`) reporting when no render environment is present (`npm run check:gpui`).
- **Live native `gpui-kit` sessions** ([#6](https://github.com/de-novo/dui/issues/6), [PR #28](https://github.com/de-novo/dui/pull/28)):
  - Added persistent `--session` JSON-lines mode to `fixtures/gpui-kit` and `Backend::Gpui` to `crates/dui/src/live/session.rs`, reusing the same `dui live` / MCP `live` contract and `dui-core::edit::decide` Decider.
  - Verified independent second-instance state retention, AccessKit semantic evidence gating (`missing-semantic-evidence` / `semantic-mismatch`), saved PNGs under `~/.dui/`, and 30 warm edits (`4.22 ms p50 / 4.44 ms p95`, 0 rebuilds, 0 restarts; `npm run check:gpui-live`).

---

## [v0.0.2] - 2026-11-21

**Focus:** Archived screen map viewer, live typed state inspection/editing, DDD domain model, SQLite + Drizzle storage, M1 coverage gates, and multi-locale capture/i18n.

### Added

- **Archived screen map & `apps/canvas` viewer** ([#14](https://github.com/de-novo/dui/issues/14), [PR #15](https://github.com/de-novo/dui/pull/15)):
  - Built React 19 + Tailwind CSS 4 + shadcn/ui + React Flow (`@xyflow/react`) viewer browsing archived screen maps, discovered links, and project/branch/version history.
- **Typed element state inspection** ([#1](https://github.com/de-novo/dui/issues/1), [PR #17](https://github.com/de-novo/dui/pull/17)):
  - Added `dui-core::inspect` and `dui inspect` / MCP `inspect` returning owner, component instance, source location, compiler type, current value, options, and editability for boolean and enum controls.
- **Persistent live web sessions, warm edit benchmark & save** ([#2](https://github.com/de-novo/dui/issues/2), [#3](https://github.com/de-novo/dui/issues/3), [#7](https://github.com/de-novo/dui/issues/7), [PR #18](https://github.com/de-novo/dui/pull/18), [PR #19](https://github.com/de-novo/dui/pull/19), [PR #20](https://github.com/de-novo/dui/pull/20)):
  - Added loopback live session owner (`dui live` / MCP `live`) with `dui-core::edit` Decider (`open -> inspect -> set/reset -> save -> close`), visible DOM assertion verification, revision/request-ID idempotency, 30-edit warm benchmark (`npm run check:inspect`), and live session UI integrated into the screen map.
- **DDD domain model & storage ports** ([#9](https://github.com/de-novo/dui/issues/9), [PR #21](https://github.com/de-novo/dui/pull/21)):
  - Added pure aggregates, value objects, invariants, and split repository traits in `dui-core::model` and `dui-core::storage` (`MemoryStore`), documented in `docs/{en,ko,zh-CN}/domain-model.md`.
- **SQLite + Drizzle metadata store** ([#10](https://github.com/de-novo/dui/issues/10), [PR #22](https://github.com/de-novo/dui/pull/22)):
  - Added `crates/dui/src/sqlite_store.rs` (`rusqlite` bundled SQLite in WAL mode) and `packages/db/` (`drizzle-orm` `1.0.0-rc.5` SQLite dialect) sharing forward-only SQL migrations in `packages/db/migrations/` with lossless import/rebuild from `~/.dui/projects/`.
- **M1 deterministic planning & coverage gates** ([#4](https://github.com/de-novo/dui/issues/4), [PR #23](https://github.com/de-novo/dui/pull/23)):
  - Hardened fact-order invariance, recorded fill-in termination reasons (`exhausted`, `budget-limit`, `no-improvement`), distinguished runtime-only branch hits (`runtimeOutcome`), and added `npm run check:m1` verifying CLI/MCP parity and coverage gates.
- **Multi-locale capture, i18n audit & viewer UI localization** ([#12](https://github.com/de-novo/dui/issues/12), [PR #24](https://github.com/de-novo/dui/pull/24)):
  - Added `screen × state × width × locale` capture via CDP locale emulation, missing translation key diffs, DOM text-overflow selector detection, RTL direction checks (`dui-core::locale`), viewer UI language switching (`en` / `zh-CN` / `ko`) + archive locale filter, and `npm run check:i18n`.

---

## [v0.0.1] - 2026-11-20

**Focus:** M0 capture baseline — official compiler analysis, Vite + Chromium capture, typed auto-mocks, local archiving, and source installer.

### Added

- **Functional core & ports architecture (`dui-core`, `crates/dui`)**:
  - Established pure Rust `dui-core` (zero I/O, zero async, deterministic `BTreeMap`/`BTreeSet`) and three port contracts (`TypeOracle`, `Host`, `Renderer`) plus three-language architecture, design, workflow, and supported-frameworks docs.
- **Official TypeScript 6/7 compiler analysis & typed auto-mocks**:
  - Extracted React state slots, branches, component renders, and typed `fetch().json()` response shapes directly from the project's installed official `typescript` 6.x or 7.x compiler (`adapters/typescript/facts.mts`), generating deterministic mock responses (`dui-core::mock`).
- **Vite host, Chromium CDP capture & screen discovery**:
  - Implemented `dui scan`, `dui run`, `dui map`, `dui replay`, `dui replay-report`, `dui history`, and MCP stdio server (`dui mcp`) with TanStack file-route and same-origin anchor link discovery across widths (`375`, `768`, `1280` px).
  - Archived runs under `~/.dui/projects/` with Git project/branch/worktree provenance, verified on the `npmgraph` example and M0 fixtures (`fixtures/toggle`, `fixtures/typed-api`, `fixtures/replay`).
- **Source installer & release packaging**:
  - Added `scripts/install`, `scripts/package-release.mts`, and `dependency-catalog.json` license/version enforcement (`npm run deps:check`).
