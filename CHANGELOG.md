# Changelog

English | [한국어](docs/ko/changelog.md) | [简体中文](docs/zh-CN/changelog.md)

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/de-novo/dui/compare/v0.0.4...HEAD
[v0.0.4]: https://github.com/de-novo/dui/compare/v0.0.3...v0.0.4
[v0.0.3]: https://github.com/de-novo/dui/compare/v0.0.2...v0.0.3
[v0.0.2]: https://github.com/de-novo/dui/compare/v0.0.1...v0.0.2
[v0.0.1]: https://github.com/de-novo/dui/releases/tag/v0.0.1
