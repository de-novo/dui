# 变更日志 (Changelog)

[English](../../CHANGELOG.md) | [한국어](../ko/changelog.md) | 简体中文

本文件记录项目的所有重要变更。
格式基于 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)，版本遵循 [Semantic Versioning](https://semver.org/spec/v2.0.0.html)。

## [Unreleased] — v0.2.0

### Added

- 带版本的源码模型，schema 版本 1：界面、组件、槽位、分支、列表、列表项 schema、跳转边、中立样式属性和设计 token，附 JSON Schema、检查引用的 Rust 读取器和 TSX 参考示例（`npm run check:source-model`）([#353](https://github.com/de-novo/dui/issues/353))。
- 从本地 URL 或静态构建产物绘制网页地图，从原生源码发现路由并关联模拟器截图。 ([#238](https://github.com/de-novo/dui/pull/238), [#241](https://github.com/de-novo/dui/pull/241), [#243](https://github.com/de-novo/dui/pull/243)).
- 检查捕获依据，重试失败的格子，并在 PR 中比较界面。 ([#199](https://github.com/de-novo/dui/pull/199), [#201](https://github.com/de-novo/dui/pull/201), [#205](https://github.com/de-novo/dui/pull/205)).

- 全页完整高度捕获（含嵌套应用滚动容器展开）及 Web 画布与 GPUI 桌面端真实宽高比卡片布局 ([#148](https://github.com/de-novo/dui/pull/148), [#150](https://github.com/de-novo/dui/pull/150))。
- 框架无关的 WAI-ARIA 导航展开、官方编译器路由注册表读取（`routeTree.gen.ts` `FileRoutesByTo`、React Router 生成类型）、零配置 `dui scan .` 及 Vite / Next.js / 静态 HTML 自动检测 `dui map .` ([#150](https://github.com/de-novo/dui/pull/150), [#154](https://github.com/de-novo/dui/pull/154))。
- 框架无关的 HTML/WAI-ARIA 模态框、对话框、抽屉与弹出层自动捕获，以已验证子叠加层节点（`<path>#overlay:<slug>`）呈现在 2D 画布上，并支持 CSS 动画稳定等待与内容寻址缓存 ([#158](https://github.com/de-novo/dui/pull/158), [#162](https://github.com/de-novo/dui/pull/162))。
- GPUI 桌面端与 Web 画布体验 1:1 对齐：全出血画布、浮动毛玻璃侧栏与检查器、光标锚定连续缩放、原生透明 macOS `TitleBar`，以及两端统一的避障走廊连线与悬停/选中高亮 ([#156](https://github.com/de-novo/dui/pull/156), [#160](https://github.com/de-novo/dui/pull/160))。
- 框架级 React 受控属性与原生 HTML/WAI-ARIA 控件状态提取（`package: "react"` 与 `package: "html-aria"`，不绑定特定 UI 组件库） ([#140](https://github.com/de-novo/dui/pull/140))。
- 通过官方 TypeScript 6/7 AST 解析回退（`adapters/vite/parser.mts`、`npm run check:vite7`）完整支持 Vite 7 Level 2 插桩与捕获 ([#133](https://github.com/de-novo/dui/pull/133))。
- 在 Vite 7 与 8 中新增 React Router 7/8 静态路由自动发现（`createBrowserRouter`、`createHashRouter`、`createMemoryRouter`、嵌套 JSX `<Route>`、`index`、`:param` 与 `*` 通配路由） ([#134](https://github.com/de-novo/dui/pull/134), [#135](https://github.com/de-novo/dui/pull/135))。
- 交互式浏览器登录会话捕获（`dui auth login <URL> --out FILE`），将 `HttpOnly` Cookie、`localStorage` 与 `sessionStorage` 以 `0600` 权限保存为 Playwright 兼容的 `storageState.json` ([#137](https://github.com/de-novo/dui/pull/137))。
- 共享编译器预言机 IR、Ink 5 / React Native / Expo Router 5 `tsc` 规则包与 6 个官方工具链探测命令（`dui toolchains`） ([#124](https://github.com/de-novo/dui/pull/124))。
- 在 `check:style` CI 中强制校验 SemVer 与三语 `CHANGELOG` 版本同步（`npm run check:version`） ([#131](https://github.com/de-novo/dui/pull/131))。
- 注册查看器 UI 设计与评审技能（`anti-ui-slop`、`web-design-guidelines`） ([#132](https://github.com/de-novo/dui/pull/132))。

### Changed

- CLI 区分开发构建和稳定发行版，网站显示安装程序默认选择的版本。

- 简化 `dui map` / `dui run` CLI 默认行为，通过不可变 PNG 缓存与移除逐卡片 GPU 模糊显著提升画布首屏渲染速度 ([#144](https://github.com/de-novo/dui/pull/144), [#145](https://github.com/de-novo/dui/pull/145), [#152](https://github.com/de-novo/dui/pull/152))。
- 将生态/规则包库版本从硬性工具链门禁中解耦：硬性 `>=` 基线校验仅适用于核心编译器、宿主与 UI 运行时（`tsc`、`vite` 7+、`react` 18+、`host: "next"` 时的 `next` 16+），始终接受更新的稳定主版本，并对规则包库版本差异以非致命 `versionNotes` 报告而不中断扫描 ([#136](https://github.com/de-novo/dui/pull/136))。
- 将工作区与桌面端 crate 版本同步为 `0.2.0`。

### Removed

- 移除硬编码的全局设计缺陷启发式规则（`design_lint.rs`、`designAudit`、实时 lint 与未使用的合成 DOM 属性），转向客观渲染证据及未来可选 `DESIGN.md` / 插件校验（`v0.3.0`，[#167](https://github.com/de-novo/dui/issues/167)） ([#164](https://github.com/de-novo/dui/pull/164), [#166](https://github.com/de-novo/dui/pull/166))。
- 移除过早引入的 iframe 实时挂载（`dui open --live` / `--live-url`、`/live-surface/*`）与查看器 `打开实时会话` 按钮，待后续设计真正基于代码的画布渲染方案 ([#169](https://github.com/de-novo/dui/issues/169))。

## [v0.1.0] - 2026-11-23

### Added

- 通用认证会话支持（`storageState.json`、`localStorage`、`sessionStorage`、`extraHeaders`、支持 chunked 响应的 `POST authFlow`、`env:` 密钥间接引用与脱敏、按认证隔离缓存键）以及 GitHub PR 评论用 `dui diff ... --markdown` 输出 ([#128](https://github.com/de-novo/dui/pull/128))。
- iOS Simulator 表面适配器（`host: "ios"`，`xcrun simctl` 截图 + XCUI 无障碍树 → `RenderNodeSnapshot`），无环境时如实报告 `macos-required` / `no-ios-simulator` ([#123](https://github.com/de-novo/dui/pull/123))。
- GPUI 桌面查看器（`apps/desktop`）补齐交互式元素检查器、嵌套盒模型图与设计缺陷叠加层 ([#122](https://github.com/de-novo/dui/pull/122))。

### Changed

- 将 TypeScript 编译器预言机的 Node 运行时要求从硬编码 Node 26 放宽至 Node 22+ LTS（`22` / `24` / `26+`） ([#126](https://github.com/de-novo/dui/pull/126))。
- 将 `designAudit` 缺陷按 `(kind, path, width, nodeId)` 去重并附带 `occurrences` 与 `rawTotal`，同时收紧重叠、触控目标与间距偏差启发式以减少误报 ([#126](https://github.com/de-novo/dui/pull/126))。
- 将工作区包版本同步为 `0.1.0`，与 `apps/desktop` 保持一致。

### Fixed

- 将主页面文档 HTTP `4xx`/`5xx`（如 404、500）响应明确判为 `unverified`（`http-error`），同时忽略附带 `/favicon.ico` 404 ([#126](https://github.com/de-novo/dui/pull/126))。
- 修复 TUI 捕获（`htop`、`fzf`）中的 VT100 `CSI b`（`REP`）/ `CSI X`（`ECH`）、块状字符（`▌` `▏` `█`）、小写 `p`/`q` 字形对齐与文本单元对比度采样 ([#117](https://github.com/de-novo/dui/pull/117))。

## [v0.0.9] - 2026-11-23

### Added

- Android 移动表面适配器（`host: "android"`，`adb screencap` + `uiautomator` XML → `RenderNodeSnapshot`） ([#111](https://github.com/de-novo/dui/pull/111))。
- Electron 45 桌面适配器（`host: "electron"` + `ipcMocks`）以及 GPUI AccessKit → `RenderNodeSnapshot` 渲染树差异对比 ([#112](https://github.com/de-novo/dui/pull/112))。
- Radix UI 1 / shadcn/ui 4 声明式 `control` 规则包与 Next.js 16 App Router 宿主（含 Server Component 边界报告） ([#113](https://github.com/de-novo/dui/pull/113))。
- 6 类设计与无障碍缺陷自动检测引擎（AI 结构化 JSON + 2D 画布叠加层）与多工作线程并行宽度捕获（`concurrency`） ([#114](https://github.com/de-novo/dui/pull/114))。
- 开发者工具风格元素检查器、嵌套盒模型叠加层（`margin`、`padding`、`border`、`gap`、`borderRadius`、属性）与元素间距红线标尺 ([#115](https://github.com/de-novo/dui/pull/115))。

## [v0.0.8] - 2026-11-23

### Added

- 多表面抽象基础（`SurfaceRegistry`、`NoopOracle`、含盒模型与属性的统一 `RenderNodeSnapshot` IR） ([#107](https://github.com/de-novo/dui/pull/107), [#108](https://github.com/de-novo/dui/pull/108))。
- 不依赖特定打包器的 Universal Web 宿主（`static` / `url` / `command`） ([#109](https://github.com/de-novo/dui/pull/109))。
- Universal TUI 适配器（`host: "pty"`，纯 VT100/ANSI 仿真器 → `RenderNodeSnapshot` + 等宽 PNG） ([#110](https://github.com/de-novo/dui/pull/110))。

## [v0.0.7] - 2026-11-22

### Added

- 真实 API 代理模式（`apiMode: "live"` / `"hybrid"`，默认拦截写操作）、分支对比（`dui diff --base`）、元素级 `renderDiff` + 延迟像素差异与 2D 画布内联分支变更高亮 ([#81](https://github.com/de-novo/dui/pull/81), [#85](https://github.com/de-novo/dui/pull/85), [#88](https://github.com/de-novo/dui/pull/88), [#92](https://github.com/de-novo/dui/pull/92))。
- 扩展声明式 P2 规则包（Redux Toolkit 2、Jotai 3、XState 5、React Hook Form 7、valibot 1），移除人为 `stateBudget` 上限改用确定性收敛 ([#90](https://github.com/de-novo/dui/pull/90), [#94](https://github.com/de-novo/dui/pull/94))。

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

[Unreleased]: https://github.com/de-novo/dui/compare/v0.1.0...HEAD
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
