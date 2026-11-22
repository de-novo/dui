# 变更日志 (Changelog)

[English](../../CHANGELOG.md) | [한국어](../ko/changelog.md) | 简体中文

本文件记录项目的所有重要变更。
格式基于 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)，版本遵循 [Semantic Versioning](https://semver.org/spec/v2.0.0.html)。

## [Unreleased]

## [v0.0.6] - 2026-11-22

### Added

- 99% UI 状态覆盖率审计（`dui audit` / MCP `audit` / 查看器 `/api/audit`）及未解决评审评论徽章 ([#45](https://github.com/de-novo/dui/pull/45))。
- 基于标准 `URLSearchParams` 与声明式 `kind: "search"` 规则的框架无关 URL 查询参数追踪及 `normalizedQuery` 记录 ([#58](https://github.com/de-novo/dui/pull/58))。
- GPUI 桌面查看器集成 `dui live` 实时状态编辑、未验证 HTTP 错误诊断卡片与 Figma 快捷键 ([#60](https://github.com/de-novo/dui/pull/60))。

### Changed

- 将 GPUI 桌面查看器的设计令牌、布局与交互与 Web 画布 UI 保持 1:1 一致 ([#47](https://github.com/de-novo/dui/pull/47), [#49](https://github.com/de-novo/dui/pull/49))。
- 在 Web 与 GPUI 查看器中选中节点时保留完整地图，并将非关联节点和边淡化显示 ([#51](https://github.com/de-novo/dui/pull/51))。

### Fixed

- 修复 GPUI 桌面查看器对 SQLite `cells.status_json`（`status` 标签）的解析以及检查器宽度同步问题 ([#53](https://github.com/de-novo/dui/pull/53), [#55](https://github.com/de-novo/dui/pull/55))。

## [v0.0.5] - 2026-11-21

### Added

- 覆盖 SQLite、CLI（`dui comment`）、MCP（`comment_*`）和查看器（`/api/comments`）的页面/状态/宽度/跳转评审评论 ([#39](https://github.com/de-novo/dui/pull/39))。
- 直接读取 `~/.dui/dui.db` 的 GPUI + `gpui-kit 0.10.0` 原生桌面查看器（`apps/desktop`、`dui desktop`）([#41](https://github.com/de-novo/dui/pull/41))。
- 建立 Open-Core 许可证边界（核心 `Apache-2.0` + 商业企业功能 `ee/LICENSE`）([#43](https://github.com/de-novo/dui/pull/43))。

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

[Unreleased]: https://github.com/de-novo/dui/compare/v0.0.6...HEAD
[v0.0.6]: https://github.com/de-novo/dui/compare/v0.0.5...v0.0.6
[v0.0.5]: https://github.com/de-novo/dui/compare/v0.0.4...v0.0.5
[v0.0.4]: https://github.com/de-novo/dui/compare/v0.0.3...v0.0.4
[v0.0.3]: https://github.com/de-novo/dui/compare/v0.0.2...v0.0.3
[v0.0.2]: https://github.com/de-novo/dui/compare/v0.0.1...v0.0.2
[v0.0.1]: https://github.com/de-novo/dui/releases/tag/v0.0.1
