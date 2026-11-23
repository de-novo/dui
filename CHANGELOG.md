# Changelog

English | [한국어](docs/ko/changelog.md) | [简体中文](docs/zh-CN/changelog.md)

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [v0.2.0] - 2026-11-23

### Added

- Full-page screen capture (expanding nested app-shell scroll containers) with true-aspect card layout on both Web Canvas and GPUI Desktop ([#148](https://github.com/de-novo/dui/pull/148), [#150](https://github.com/de-novo/dui/pull/150)).
- Framework-agnostic WAI-ARIA navigation disclosure expansion, official compiler route-registry lookup (`routeTree.gen.ts` `FileRoutesByTo`, React Router generated types), zero-config `dui scan .`, and multi-host `dui map .` auto-detection for Vite, Next.js, and static HTML apps ([#150](https://github.com/de-novo/dui/pull/150), [#154](https://github.com/de-novo/dui/pull/154)).
- Framework-agnostic HTML/WAI-ARIA modal, dialog, drawer, and popover auto-capture promoted into verified child overlay nodes (`<path>#overlay:<slug>`) on the 2D canvas with CSS animation settling and content-addressed blob caching ([#158](https://github.com/de-novo/dui/pull/158), [#162](https://github.com/de-novo/dui/pull/162)).
- Native GPUI desktop UX parity with Web Canvas: full-bleed canvas, floating glass rail & inspector, cursor-anchored continuous zoom, native transparent macOS `TitleBar`, and obstacle-free gutter edge routing with hover/selection focus highlighting across both viewers ([#156](https://github.com/de-novo/dui/pull/156), [#160](https://github.com/de-novo/dui/pull/160)).
- Framework-level React controlled-prop and native HTML/WAI-ARIA control extraction (`package: "react"` and `package: "html-aria"`, independent of any specific UI component library) ([#140](https://github.com/de-novo/dui/pull/140)).
- Full Level 2 Vite 7 support alongside Vite 8+ via an official TypeScript 6/7 AST fallback (`adapters/vite/parser.mts`, `npm run check:vite7`) ([#133](https://github.com/de-novo/dui/pull/133)).
- Static React Router 7/8 route discovery (`createBrowserRouter`, `createHashRouter`, `createMemoryRouter`, and nested JSX `<Route>` trees with `index`, `:param`, and `*` splats) across Vite 7 and 8 ([#134](https://github.com/de-novo/dui/pull/134), [#135](https://github.com/de-novo/dui/pull/135)).
- Interactive browser login session capture (`dui auth login <URL> --out FILE`) saving Playwright-compatible `storageState.json` (`HttpOnly` cookies, `localStorage`, `sessionStorage`) with `0600` permissions ([#137](https://github.com/de-novo/dui/pull/137)).
- Shared compiler-oracle IR, Ink 5 / React Native / Expo Router 5 `tsc` rule packs, and `dui toolchains` probing 6 official compilers ([#124](https://github.com/de-novo/dui/pull/124)).
- Automated SemVer and 3-language `CHANGELOG` version-sync guard (`npm run check:version`) enforced in `check:style` CI ([#131](https://github.com/de-novo/dui/pull/131)).
- Viewer UI Design/Review agent skills (`anti-ui-slop`, `web-design-guidelines`) alongside `shadcn/ui` ([#132](https://github.com/de-novo/dui/pull/132)).

### Changed

- Simplified `dui map` / `dui run` CLI defaults, accelerated canvas card rendering with immutable PNG caching and removal of per-card GPU backdrop blur ([#144](https://github.com/de-novo/dui/pull/144), [#145](https://github.com/de-novo/dui/pull/145), [#152](https://github.com/de-novo/dui/pull/152)).
- Decoupled ecosystem / rule-pack library versions from hard toolchain gates: hard `>=` baseline checks apply only to core compiler/host/UI runtime layers (`tsc`, `vite` 7+, `react` 18+, `next` 16+ when `host: "next"`), while newer majors are always accepted and rule-pack libraries emit non-fatal `versionNotes` instead of aborting scans ([#136](https://github.com/de-novo/dui/pull/136)).
- Synced workspace and desktop crate versions to `0.2.0`.

### Removed

- Removed hardcoded global design-defect heuristics (`design_lint.rs`, `designAudit`, live lint, and unused synthetic DOM attributes) in favor of objective rendering evidence and future opt-in `DESIGN.md` / plugin conformance (`v0.3.0`, [#167](https://github.com/de-novo/dui/issues/167)) ([#164](https://github.com/de-novo/dui/pull/164), [#166](https://github.com/de-novo/dui/pull/166)).
- Removed premature iframe live-mount (`dui open --live` / `--live-url`, `/live-surface/*`) and viewer `Open live` buttons until true code-based canvas rendering is designed ([#169](https://github.com/de-novo/dui/issues/169)).

## [v0.1.0] - 2026-11-23

### Added

- Universal authenticated session support (`storageState.json`, `localStorage`, `sessionStorage`, `extraHeaders`, `POST` `authFlow` with chunked HTTP decoding, `env:` secret indirection & manifest redaction, and auth-isolated cache keys) plus `dui diff ... --markdown` GitHub PR comment output ([#128](https://github.com/de-novo/dui/pull/128)).
- iOS Simulator surface adapter (`host: "ios"`, `xcrun simctl` screenshots + XCUI accessibility tree → `RenderNodeSnapshot`) with honest `macos-required` / `no-ios-simulator` reporting ([#123](https://github.com/de-novo/dui/pull/123)).
- GPUI desktop viewer (`apps/desktop`) parity for the interactive Element Inspector, nested Box-Model diagram, and Design Audit overlays ([#122](https://github.com/de-novo/dui/pull/122)).

### Changed

- Relaxed TypeScript oracle runtime requirement from hardcoded Node 26 to Node 22+ LTS (`22` / `24` / `26+`) for real-world project compatibility ([#126](https://github.com/de-novo/dui/pull/126)).
- Deduplicated `designAudit` findings by `(kind, path, width, nodeId)` with `occurrences` & `rawTotal` counts and tightened overlap/touch-target/spacing-drift heuristics ([#126](https://github.com/de-novo/dui/pull/126)).
- Synced workspace package version to `0.1.0` matching `apps/desktop`.

### Fixed

- Rejected HTTP `4xx`/`5xx` primary document responses (e.g., 404 Not Found, 500 Internal Server Error) as `unverified` (`http-error`) while ignoring ancillary `/favicon.ico` 404s ([#126](https://github.com/de-novo/dui/pull/126)).
- Added VT100 `CSI b` (`REP`) / `CSI X` (`ECH`), block-element glyphs (`▌` `▏` `█`), lowercase `p`/`q` alignment, and text-cell contrast sampling for TUI captures (`htop`, `fzf`) ([#117](https://github.com/de-novo/dui/pull/117)).

## [v0.0.9] - 2026-11-23

### Added

- Android mobile surface adapter (`host: "android"`, `adb screencap` + `uiautomator` XML → `RenderNodeSnapshot`) ([#111](https://github.com/de-novo/dui/pull/111)).
- Electron 45 desktop adapter (`host: "electron"` + `ipcMocks`) and GPUI AccessKit → `RenderNodeSnapshot` render-tree diffing ([#112](https://github.com/de-novo/dui/pull/112)).
- Radix UI 1 / shadcn/ui 4 declarative `control` rule pack and Next.js 16 App Router host adapter with Server Component boundary reporting ([#113](https://github.com/de-novo/dui/pull/113)).
- Automatic 6-rule Design & Accessibility lint engine (AI structured JSON + 2D canvas overlay) and parallel multi-worker viewport capture (`concurrency`) ([#114](https://github.com/de-novo/dui/pull/114)).
- DevTools-style Element Inspector, nested Box-Model overlay (`margin`, `padding`, `border`, `gap`, `borderRadius`, attributes), and element-to-element spacing redline guides ([#115](https://github.com/de-novo/dui/pull/115)).

## [v0.0.8] - 2026-11-23

### Added

- Multi-surface architecture (`SurfaceRegistry`, `NoopOracle`, box-model & attribute-extended `RenderNodeSnapshot` IR) ([#107](https://github.com/de-novo/dui/pull/107), [#108](https://github.com/de-novo/dui/pull/108)).
- Universal Web host (`static` / `url` / `command`) for any HTML/JS app without bundler lock-in ([#109](https://github.com/de-novo/dui/pull/109)).
- Universal TUI adapter (`host: "pty"`, pure VT100/ANSI emulator → `RenderNodeSnapshot` + monospace PNG) ([#110](https://github.com/de-novo/dui/pull/110)).

## [v0.0.7] - 2026-11-22

### Added

- Live API proxy mode (`apiMode: "live"` / `"hybrid"`) with default mutation blocking, real-time branch comparison (`dui diff --base`), element-level `renderDiff` + lazy pixel diff hydration, and 2D canvas inline branch diff highlights ([#81](https://github.com/de-novo/dui/pull/81), [#85](https://github.com/de-novo/dui/pull/85), [#88](https://github.com/de-novo/dui/pull/88), [#92](https://github.com/de-novo/dui/pull/92)).
- Expanded declarative P2 rule packs (Redux Toolkit 2, Jotai 3, XState 5, React Hook Form 7, valibot 1) and removed arbitrary `stateBudget` caps in favor of exhaustive deterministic convergence ([#90](https://github.com/de-novo/dui/pull/90), [#94](https://github.com/de-novo/dui/pull/94)).

## [v0.0.6] - 2026-11-22

### Added

- 99% UI state coverage audit (`dui audit` / MCP `audit` / viewer `/api/audit`) with verified/renderable counts, gap reasons, and unresolved review comment badges ([#45](https://github.com/de-novo/dui/pull/45)).
- Framework-agnostic URL query parameter tracking from standard `URLSearchParams` and declarative `kind: "search"` rules, plus `normalizedQuery` tracking ([#58](https://github.com/de-novo/dui/pull/58)).
- Live state editing (`dui live`), unverified HTTP failure diagnostics, and Figma keyboard shortcuts in the GPUI desktop viewer ([#60](https://github.com/de-novo/dui/pull/60)).

### Changed

- Brought the GPUI desktop viewer to 1:1 visual, token, and interaction parity with the web canvas UI ([#47](https://github.com/de-novo/dui/pull/47), [#49](https://github.com/de-novo/dui/pull/49)).
- Kept the full screen map visible on node selection and dimmed unrelated nodes and edges in both Web and GPUI viewers ([#51](https://github.com/de-novo/dui/pull/51)).

### Fixed

- Parsed SQLite `cells.status_json` using the `status` tag and synchronized inspector cell width with the active map width in the GPUI desktop viewer ([#53](https://github.com/de-novo/dui/pull/53), [#55](https://github.com/de-novo/dui/pull/55)).

## [v0.0.5] - 2026-11-21

### Added

- Review comments anchored to screens, states, widths, and transitions across SQLite, CLI (`dui comment`), MCP (`comment_*`), and viewer `/api/comments` ([#39](https://github.com/de-novo/dui/pull/39)).
- Native GPUI + `gpui-kit 0.10.0` desktop viewer (`apps/desktop`, `dui desktop`) reading `~/.dui/dui.db` directly ([#41](https://github.com/de-novo/dui/pull/41)).
- Open-Core licensing boundary (`Apache-2.0` core + `ee/LICENSE` for commercial enterprise features) ([#43](https://github.com/de-novo/dui/pull/43)).

## [v0.0.4] - 2026-11-21

### Added

- Action-driven transition replay (`dui transitions` / MCP `transitions`) with `extracted`, `inferred`, and `verified` edge provenance in the viewer ([#32](https://github.com/de-novo/dui/pull/32)).
- Embedded single-binary web viewer served directly by `dui open` and `dui serve` ([#29](https://github.com/de-novo/dui/pull/29)).
- Tag-based versioning (`dui --version`), release update checks (`dui update --check` / `dui update`), and tag-based source installs with automatic `PATH` setup ([#29](https://github.com/de-novo/dui/pull/29), [#30](https://github.com/de-novo/dui/pull/30)).
- Figma-style two-finger trackpad panning, pinch-to-zoom, and zoom shortcuts on the screen map ([#36](https://github.com/de-novo/dui/pull/36)).

### Changed

- Sized screen map nodes to 1:1 capture width with zoom up to 3× and a `1:1` view control ([#34](https://github.com/de-novo/dui/pull/34)).

### Fixed

- Resolved hoisted `node_modules/typescript` from ancestor directories in monorepos ([#30](https://github.com/de-novo/dui/pull/30)).

### Removed

- Legacy static `graph.html` and `history.html` gallery files in favor of the embedded viewer ([#29](https://github.com/de-novo/dui/pull/29)).

## [v0.0.3] - 2026-11-21

### Added

- Content-addressed capture cache with module-graph invalidation, `--force`, and `dui prune` ([#25](https://github.com/de-novo/dui/pull/25)).
- Capture diff (`dui diff` / MCP `diff` / viewer diff mode) with changed source files and pixel-difference overlays ([#26](https://github.com/de-novo/dui/pull/26)).
- Offscreen native GPUI (`gpui-kit`) rendering with AccessKit element-tree observations and missing-environment reporting ([#27](https://github.com/de-novo/dui/pull/27)).
- Persistent live `gpui-kit` sessions through the shared `dui live` and MCP `live` contract ([#28](https://github.com/de-novo/dui/pull/28)).

## [v0.0.2] - 2026-11-21

### Added

- Archived screen map web viewer built with React, Tailwind CSS, shadcn/ui, and React Flow ([#15](https://github.com/de-novo/dui/pull/15)).
- Typed component state inspection via `dui inspect` and MCP `inspect` ([#17](https://github.com/de-novo/dui/pull/17)).
- Persistent live web sessions (`dui live` / MCP `live`) with state edits, resets, snapshot saving, and viewer integration ([#18](https://github.com/de-novo/dui/pull/18), [#19](https://github.com/de-novo/dui/pull/19), [#20](https://github.com/de-novo/dui/pull/20)).
- Pure domain model and storage ports in `dui-core` ([#21](https://github.com/de-novo/dui/pull/21)).
- Local SQLite metadata store with shared Drizzle migrations and lossless history import ([#22](https://github.com/de-novo/dui/pull/22)).
- Deterministic planning and coverage verification gates (`npm run check:m1`) ([#23](https://github.com/de-novo/dui/pull/23)).
- Multi-locale screen capture, translation/overflow/RTL audit, and `en` / `zh-CN` / `ko` viewer UI switching ([#24](https://github.com/de-novo/dui/pull/24)).

## [v0.0.1] - 2026-11-20

### Added

- Initial `dui` CLI and MCP stdio server (`scan`, `run`, `map`, `replay`, `history`, `mcp`).
- Official TypeScript 6/7 compiler analysis, typed GET auto-mocks, Vite host, and Chromium CDP capture across viewport widths.
- Local capture archiving under `~/.dui/` grouped by project, branch, and worktree, plus the source installer.

[Unreleased]: https://github.com/de-novo/dui/compare/v0.2.0...HEAD
[v0.2.0]: https://github.com/de-novo/dui/compare/v0.1.0...v0.2.0
[v0.1.0]: https://github.com/de-novo/dui/releases/tag/v0.1.0
[v0.0.9]: https://github.com/de-novo/dui/compare/v0.0.8...v0.0.9
[v0.0.8]: https://github.com/de-novo/dui/compare/v0.0.7...v0.0.8
[v0.0.7]: https://github.com/de-novo/dui/compare/v0.0.6...v0.0.7
[v0.0.6]: https://github.com/de-novo/dui/compare/v0.0.5...v0.0.6
[v0.0.5]: https://github.com/de-novo/dui/compare/v0.0.4...v0.0.5
[v0.0.4]: https://github.com/de-novo/dui/compare/v0.0.3...v0.0.4
[v0.0.3]: https://github.com/de-novo/dui/compare/v0.0.2...v0.0.3
[v0.0.2]: https://github.com/de-novo/dui/compare/v0.0.1...v0.0.2
[v0.0.1]: https://github.com/de-novo/dui/releases/tag/v0.0.1
