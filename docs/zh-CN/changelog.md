# 变更日志 (Changelog)

[English](../../CHANGELOG.md) | [한국어](../ko/changelog.md) | 简体中文

本文件按里程碑版本（`v0.0.1`–`v0.0.4`）记录 `dui` 的所有重要变更。
格式遵循 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)，每个发布版本对应 SemVer Git 标签（`vX.Y.Z`）。

## 变更日志维护规则

- 合并到 `main` 的用户可见改动（新增功能、行为变更、问题修复、移除项）记录在 `[Unreleased]` 下。
- 条目按 **新增（Added）**、**变更（Changed）**、**修复（Fixed）**、**移除（Removed）** 分组，并附带对应 PR（`#N`）与议题（`#N`）链接。
- 发布新标签 `vX.Y.Z` 时，将 `[Unreleased]` 条目移至 `## [vX.Y.Z] - YYYY-MM-DD`，并在同一 PR 中同步更新 `CHANGELOG.md`、`docs/ko/changelog.md` 与 `docs/zh-CN/changelog.md`。

---

## [Unreleased]

- 下一计划里程碑：`v0.0.5`（附着于页面/状态/宽度/转换的评审评论及 CLI/MCP/查看器闭环、`gpui-kit` 原生桌面查看器）。

---

## [v0.0.4] - 2026-11-21

**重点：** 动作驱动的转换重放、单二进制内嵌 Web 查看器、基于标签的安装与更新，以及 Figma 级画布可读性与触控板交互。

### 新增（Added）

- **动作驱动转换重放与边来源验证**（[#31](https://github.com/de-novo/dui/issues/31)，[PR #32](https://github.com/de-novo/dui/pull/32)）：
  - 在 `dui-core::transition` 中添加纯函数转换判定与来源规则（`extracted` / `inferred` / `verified`）。
  - 新增 `Browser::replay_transition`、`dui transitions PROJECT MANIFEST [RUN_DIR]` 与 MCP `transitions`，返回一致的幂等 JSON 并将来源、确认宽度及各宽度失败原因写入 `graph.json` 和 `transitions.json`。
  - 更新 `apps/canvas` 以实线橙色（回放已验证）、虚线（代码提取）、点线（推断）区分边来源，并以 `en`/`zh-CN`/`ko` 显示徽章与失败证据。
  - 将 `npm run check:transitions` 接入 CI。
- **单二进制内嵌 Web 查看器（`dui open` / `dui serve`）**（[PR #29](https://github.com/de-novo/dui/pull/29)）：
  - 通过 `crates/dui/build.rs` 在构建时编译并内嵌 `apps/canvas` Vite 产物至 `dui` 二进制文件。
  - 新增进程内回环 HTTP 服务器（`crates/dui/src/viewer.rs`），直接提供 `apps/canvas`、`/archive/*`、`/api/diff` + `/api/diff/overlay` 与 `/api/live` + `/api/live/frame`。
  - 将 `npm run check:viewer` 接入 CI。
- **基于标签的版本、自更新与源码安装器**（[PR #29](https://github.com/de-novo/dui/pull/29)，[PR #30](https://github.com/de-novo/dui/pull/30)）：
  - 在 `dui-core::version::SemVerTag` 中实现 `vX.Y.Z` 发布标签解析与数值比较。
  - `dui --version` 输出构建期嵌入的 `DUI_VERSION`（`{ name, version, tag }`）；新增 `dui update [--check] [--tag TAG] [--repo URL] [--install-dir DIR]`。
  - 更新 `scripts/install` 按最新 `vX.Y.Z` Git 标签拉取并构建（含 `apps/canvas`），自动将安装目录写入 shell `PATH`（`zsh` / `bash` / `fish` / `sh`，支持 `DUI_NO_MODIFY_PATH=1` 跳过），并在可用时创建符号链接使 `dui` 立即可用。
- **Figma 风格触控板导航**（[#35](https://github.com/de-novo/dui/issues/35)，[PR #36](https://github.com/de-novo/dui/pull/36)）：
  - 启用触控板双指 2D 平移（`panOnScroll`、`PanOnScrollMode.Free`、`zoomOnScroll={false}`）、捏合缩放（`zoomOnPinch`）、`Cmd`/`Ctrl`+滚轮缩放及快捷键（`+`、`-`、`0` 跳转 `1:1`、`Shift+1` 适配全图）。

### 变更（Changed）

- **1:1 捕获宽度地图节点与 3× 缩放**（[#33](https://github.com/de-novo/dui/issues/33)，[PR #34](https://github.com/de-novo/dui/pull/34)）：
  - 将固定 `280px` 卡片和 `16:9` 裁剪替换为按实际捕获宽度计算的 `nodeMetrics(width)`（桌面 `1280×800`、移动 `375×600`），将 `maxZoom` 提高至 `3`、`minZoom` 降至 `0.02`，并新增居中 `1:1` 控制按钮。

### 修复（Fixed）

- **Monorepo 提升依赖中的 TypeScript 查找**（[PR #30](https://github.com/de-novo/dui/pull/30)）：
  - 更新 `crates/dui/src/main.rs` 的 `scan()` 向上级目录查找 `node_modules/typescript/package.json`，支持在 pnpm/npm hoisted monorepo 子包中直接运行 `dui map` / `dui run`。

### 移除（Removed）

- **旧版静态 HTML 画廊**（[PR #29](https://github.com/de-novo/dui/pull/29)）：
  - 移除 `crates/dui/src/graph.html`、`crates/dui/src/history.html`、`adapters/browser/graph.ts` 和 `adapters/browser/history.ts`；`dui open` 改为启动内嵌 `apps/canvas` 查看器，`dui history` 返回纯 JSON 快照列表。

---

## [v0.0.3] - 2026-11-21

**重点：** 内容寻址增量捕获缓存、两次捕获差异与像素差异叠加层，以及真实原生 GPUI（`gpui-kit`）离屏渲染与实时会话。

### 新增（Added）

- **内容寻址捕获缓存与 `dui prune`**（[#8](https://github.com/de-novo/dui/issues/8)，[PR #25](https://github.com/de-novo/dui/pull/25)）：
  - 在 `adapters/typescript/facts.mts` 中提取官方 TypeScript 编译器 `moduleGraph`，按单页依赖源文件、类型事实、模拟数据、状态、宽度、语言区域、规则包、`dui` 版本与渲染环境计算每格 `CacheInputs`。
  - PNG 按哈希仅存于 `~/.dui/blobs/<sha256>.png` 并在不同运行、分支与工作树间硬链接复用；全命中时跳过启动 Vite/Chromium（`fixtures/toggle` 上 `14,385 ms -> 520 ms`，提速 `27.64×`）。
  - CLI/MCP 返回一致的命中/未命中与变更输入归因，支持 `--force` / MCP `force: true`、`dui prune [--max-bytes BYTES]` / MCP `prune` 与 `npm run check:cache`。
- **捕获差异与像素差异叠加层**（[#11](https://github.com/de-novo/dui/issues/11)，[PR #26](https://github.com/de-novo/dui/pull/26)）：
  - 在 `dui-core::diff` 中实现纯函数差异对比与 RGBA 像素差异计算（隐藏未变更格子、列出每格 `changedFiles`、计算 `diffPixels`/`diffRatio`/`changedBounds` 与 `#EF803F` 叠加层，`unverified`/`unrenderable` 仅报告状态变化而不生成像素差异）。
  - 新增 `dui diff BEFORE AFTER [--overlay-dir DIR]`、MCP `diff`、查看器 `/api/diff` + `/api/diff/overlay`、`CaptureDiffView`（`en` / `zh-CN` / `ko`）及 `npm run check:diff`。
- **真实 GPUI（`gpui-kit`）离屏渲染**（[#5](https://github.com/de-novo/dui/issues/5)，[PR #27](https://github.com/de-novo/dui/pull/27)）：
  - 新增锁定 `gpui-kit 0.10.0`（`gpui-pre 0.3.12`，Apache-2.0）的独立夹具 `fixtures/gpui-kit`，通过 `HeadlessAppContext` Metal/WGPU 离屏渲染双实例 `SettingsScreen` 并以 AccessKit / `TestQueryExt` 采集元素树证据。
  - 在 `crates/dui/src/gpui.rs` 中实现共享 `Renderer` 端口 `GpuiRenderer`，记录 OS、工具链、版本、渲染路径与 `elementTreeAvailable`，缺少渲染环境时返回 `unrenderable`（`no-render-env`）（`npm run check:gpui`）。
- **原生 `gpui-kit` 实时会话**（[#6](https://github.com/de-novo/dui/issues/6)，[PR #28](https://github.com/de-novo/dui/pull/28)）：
  - 为 `fixtures/gpui-kit` 增加常驻 `--session` JSON-lines 模式，并在 `crates/dui/src/live/session.rs` 中集成 `Backend::Gpui`，复用与 Web 相同的 `dui live` / MCP `live` 协议与 `dui-core::edit::decide` 决策器。
  - 验证第二个视图实例状态独立保持、AccessKit 语义证据门禁（`missing-semantic-evidence` / `semantic-mismatch`）、`~/.dui/` 归档保存以及 30 次热编辑基准（`p50 4.22 ms / p95 4.44 ms`，0 次重建与重启；`npm run check:gpui-live`）。

---

## [v0.0.2] - 2026-11-21

**重点：** 归档界面地图查看器、实时类型化状态查询/编辑、DDD 领域模型、SQLite + Drizzle 存储、M1 覆盖率门禁与多语言捕获/i18n。

### 新增（Added）

- **归档界面地图与 `apps/canvas` 查看器**（[#14](https://github.com/de-novo/dui/issues/14)，[PR #15](https://github.com/de-novo/dui/pull/15)）：
  - 构建基于 React 19 + Tailwind CSS 4 + shadcn/ui + React Flow（`@xyflow/react`）的查看器，浏览归档界面地图、发现链接与项目/分支/版本历史。
- **类型化元素状态查询**（[#1](https://github.com/de-novo/dui/issues/1)，[PR #17](https://github.com/de-novo/dui/pull/17)）：
  - 新增 `dui-core::inspect` 与 `dui inspect` / MCP `inspect`，返回布尔和枚举控件的所属组件、实例、源码位置、编译器类型、当前值、可选项与可编辑性。
- **常驻 Web 实时会话、热编辑延迟基准与保存**（[#2](https://github.com/de-novo/dui/issues/2)，[#3](https://github.com/de-novo/dui/issues/3)，[#7](https://github.com/de-novo/dui/issues/7)，[PR #18](https://github.com/de-novo/dui/pull/18)，[PR #19](https://github.com/de-novo/dui/pull/19)，[PR #20](https://github.com/de-novo/dui/pull/20)）：
  - 实现回环实时会话所有者（`dui live` / MCP `live`）、`dui-core::edit` 决策器（`open -> inspect -> set/reset -> save -> close`）、可见 DOM 断言校验、修订号与请求 ID 幂等、30 次热编辑基准（`npm run check:inspect`）及地图内实时编辑界面。
- **DDD 领域模型与存储端口**（[#9](https://github.com/de-novo/dui/issues/9)，[PR #21](https://github.com/de-novo/dui/pull/21)）：
  - 在 `dui-core::model` 与 `dui-core::storage`（`MemoryStore`）中定义纯领域聚合、值对象、不变式与拆分仓储端口，并编写三语 `domain-model.md`。
- **SQLite + Drizzle 元数据存储**（[#10](https://github.com/de-novo/dui/issues/10)，[PR #22](https://github.com/de-novo/dui/pull/22)）：
  - 新增 `crates/dui/src/sqlite_store.rs`（`rusqlite` 内置 SQLite WAL 模式）与 `packages/db/`（`drizzle-orm` `1.0.0-rc.5` SQLite 方言），共享 `packages/db/migrations/` 单向 SQL 迁移并支持从 `~/.dui/projects/` 无损导入与重建。
- **M1 确定性规划与覆盖率门禁**（[#4](https://github.com/de-novo/dui/issues/4)，[PR #23](https://github.com/de-novo/dui/pull/23)）：
  - 强化事实顺序不变性，记录补全循环终止原因（`exhausted`、`budget-limit`、`no-improvement`），区分仅运行时分支节点（`runtimeOutcome`）并新增 `npm run check:m1`。
- **多语言捕获、i18n 审计与查看器界面多语言**（[#12](https://github.com/de-novo/dui/issues/12)，[PR #24](https://github.com/de-novo/dui/pull/24)）：
  - 支持基于 CDP 语言区域模拟的 `screen × state × width × locale` 捕获、缺失翻译键对比、DOM 文本溢出选择器检测、RTL 方向检查（`dui-core::locale`）、查看器界面语言切换（`en` / `zh-CN` / `ko`）与归档语言筛选及 `npm run check:i18n`。

---

## [v0.0.1] - 2026-11-20

**重点：** M0 捕获基线 — 官方编译器分析、Vite + Chromium 捕获、类型化自动模拟数据、本地归档与源码安装器。

### 新增（Added）

- **函数式核心与端口架构（`dui-core`、`crates/dui`）**：
  - 建立零 I/O、零异步、确定性的纯 Rust `dui-core` 与三大端口契约（`TypeOracle`、`Host`、`Renderer`），以及三语架构、设计、工作流与支持框架文档。
- **官方 TypeScript 6/7 编译器分析与类型化自动模拟**：
  - 直接从项目安装的官方 `typescript` 6.x 或 7.x 编译器提取 React 状态槽位、分支、组件渲染与 `fetch().json()` 响应类型（`adapters/typescript/facts.mts`），并生成确定性模拟响应（`dui-core::mock`）。
- **Vite 宿主、Chromium CDP 捕获与页面发现**：
  - 实现 `dui scan`、`dui run`、`dui map`、`dui replay`、`dui replay-report`、`dui history` 与 MCP stdio 服务器（`dui mcp`），支持 TanStack 文件路由及同源链接发现在 `375`、`768`、`1280` px 下捕获。
  - 将结果连同 Git 项目、分支与工作树来源归档至 `~/.dui/projects/`，并通过 `npmgraph` 示例及 M0 夹具验证。
- **源码安装器与发布打包**：
  - 新增 `scripts/install`、`scripts/package-release.mts` 与 `dependency-catalog.json` 依赖版本及许可证校验（`npm run deps:check`）。
