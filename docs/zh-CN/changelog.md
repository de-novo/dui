# 变更日志 (Changelog)

[English](../../CHANGELOG.md) | [한국어](../ko/changelog.md) | 简体中文

本文件记录项目的所有重要变更。
格式基于 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)，版本遵循 [Semantic Versioning](https://semver.org/spec/v2.0.0.html)。

## [Unreleased]

## [v0.0.4] - 2026-11-21

### Added

- 动作驱动转换重放（`dui transitions` / MCP `transitions`）并在查看器中区分 `extracted`、`inferred` 与 `verified` 边来源 ([#32](https://github.com/de-novo/dui/pull/32))。
- 通过 `dui open` / `dui serve` 直接启动的单二进制内嵌 Web 查看器 ([#29](https://github.com/de-novo/dui/pull/29))。
- 基于标签的版本（`dui --version`）、更新检查与自更新（`dui update --check` / `dui update`），以及自动配置 `PATH` 的标签源码安装器 ([#29](https://github.com/de-novo/dui/pull/29), [#30](https://github.com/de-novo/dui/pull/30))。
- 界面地图 Figma 风格双指触控板平移、捏合缩放与快捷键 ([#36](https://github.com/de-novo/dui/pull/36))。

### Changed

- 将界面地图节点放大至 1:1 捕获宽度，支持最高 3× 缩放及 `1:1` 视图按钮 ([#34](https://github.com/de-novo/dui/pull/34))。

### Fixed

- 支持在 monorepo 中向上级目录查找提升安装的 `node_modules/typescript` ([#30](https://github.com/de-novo/dui/pull/30))。

### Removed

- 移除已被内嵌查看器替代的旧版静态 `graph.html` 和 `history.html` 画廊 ([#29](https://github.com/de-novo/dui/pull/29))。

## [v0.0.3] - 2026-11-21

### Added

- 基于模块图的内容寻址捕获缓存、`--force` 与 `dui prune` ([#25](https://github.com/de-novo/dui/pull/25))。
- 捕获差异对比（`dui diff` / MCP `diff` / 查看器 diff 模式）、变更源文件列表与像素差异叠加层 ([#26](https://github.com/de-novo/dui/pull/26))。
- 原生 GPUI（`gpui-kit`）离屏渲染、AccessKit 元素树观测及缺失渲染环境报告 ([#27](https://github.com/de-novo/dui/pull/27))。
- 复用共享 `dui live` / MCP `live` 协议的常驻 `gpui-kit` 实时会话 ([#28](https://github.com/de-novo/dui/pull/28))。

## [v0.0.2] - 2026-11-21

### Added

- 基于 React、Tailwind CSS、shadcn/ui 与 React Flow 的归档界面地图查看器 ([#15](https://github.com/de-novo/dui/pull/15))。
- 通过 `dui inspect` 与 MCP `inspect` 查询类型化组件状态 ([#17](https://github.com/de-novo/dui/pull/17))。
- 常驻 Web 实时会话（`dui live` / MCP `live`）状态编辑、重置、快照保存与查看器集成 ([#18](https://github.com/de-novo/dui/pull/18), [#19](https://github.com/de-novo/dui/pull/19), [#20](https://github.com/de-novo/dui/pull/20))。
- `dui-core` 纯领域模型与存储端口 ([#21](https://github.com/de-novo/dui/pull/21))。
- 共享 Drizzle 迁移并支持无损历史导入的本地 SQLite 元数据存储 ([#22](https://github.com/de-novo/dui/pull/22))。
- 确定性规划与覆盖率验收门禁（`npm run check:m1`） ([#23](https://github.com/de-novo/dui/pull/23))。
- 多语言页面捕获、缺失翻译/文本溢出/RTL 审计及 `en` / `zh-CN` / `ko` 查看器界面切换 ([#24](https://github.com/de-novo/dui/pull/24))。

## [v0.0.1] - 2026-11-20

### Added

- 初始 `dui` CLI 与 MCP stdio 服务器（`scan`、`run`、`map`、`replay`、`history`、`mcp`）。
- 官方 TypeScript 6/7 编译器分析、类型化 GET 自动模拟、Vite 宿主与多宽度 Chromium CDP 捕获。
- 按项目、分支与工作树组织在 `~/.dui/` 下的本地归档与源码安装器。

[Unreleased]: https://github.com/de-novo/dui/compare/v0.0.4...HEAD
[v0.0.4]: https://github.com/de-novo/dui/compare/v0.0.3...v0.0.4
[v0.0.3]: https://github.com/de-novo/dui/compare/v0.0.2...v0.0.3
[v0.0.2]: https://github.com/de-novo/dui/compare/v0.0.1...v0.0.2
[v0.0.1]: https://github.com/de-novo/dui/releases/tag/v0.0.1
