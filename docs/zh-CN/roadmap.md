# dui 路线图 · 用 UI 地图评审应用

[English](../en/roadmap.md) · [한국어](../ko/roadmap.md) · 简体中文

## 目标

如今，做产品的事实来源分散在策划文档、Figma 设计和代码里。dui 把开发与 UI 工作统一到同一个来源：代码本身。界面地图和设计视图由代码生成，UI 修改再写回同一份代码，因此不会再有一份单独的设计文件逐渐偏离。

**这意味着**

- 对照代码实际渲染出的界面进行评审和修改。
- 在地图上所做的改动会进入最终发布的同一代码库。

**这不是**

- 一个拥有独立文件格式的设计工具。
- 同时支持所有平台。优先完善实现上述目标所需的捕获和编辑能力。

**当前重点。** 先闭环：从代码生成地图，再把 UI 修改写回同一份代码（v0.3.0 规划的基于代码的画布）。更广的平台支持和打包工作可以稍后进行。

dui 把应用实际渲染的页面连成一张地图，便于从头到尾评审一项任务。节点代表页面，展开后可查看状态和视口宽度。连接线记录哪个操作会进入下一个页面或状态。

**在手机上取消订单后，用户会回到哪里？** 评审者从订单详情进入确认页，再查看操作后的页面。如果完成提示或下一步操作不够清楚，就在对应位置留言。AI 修改代码后，评审者回到地图上的同一位置，对比两个工作树。这是 dui 要实现的评审流程。

用 `dui map .` 捕获提货看板，检查对话框、比较标题改动，并在原生桌面端打开同一次运行。[运行 Dispatch 示例](../../examples/dispatch/README.zh-CN.md)

## v0.0.1 · 截图基线已完成

Chromium 结合项目类型和模拟数据渲染应用。本地画廊按状态和宽度展示截图，历史查看器按项目、分支和工作树比较 `~/.dui/` 中的记录。npmgraph 示例从一个入口地址发现搜索页，在两个宽度下验证两个页面，共四个格子。 [运行示例](../../examples/npmgraph/README.md)

M0 审核页面的五种状态在 375、768 和 1280 px 下全部通过验证，共生成 15 张不同的 PNG。检查涵盖对话框的实际打开状态、拦截的响应正文以及生产构建的客户端和服务端产物。Chromium 断开连接后，报告仍保留全部 15 个格子的 ID。

“已验证”表示观测到的状态通过了相应检查。布局和文案是否自然、下一步是否清晰，由评审者判断。后续版本将这些页面和证据组织成一张可评审的地图。

## v0.0.2

**从地图打开页面，检查运行中的状态。** 每个已发现的页面对应一个节点，展开即可选择状态和宽度并进入实时网页。同一页面有两个菜单时，AI 应能打开其中一个、查看内容并关闭它，另一个菜单保持原状。

[Web 查看器技术栈与交互规则](design-v0.md#web-viewer) · [Web 查看器架构](web-viewer.md)

| 工作 | 验收依据 |
| --- | --- |
| [页面地图](https://github.com/de-novo/dui/issues/7) | 在 npmgraph 上显示两个页面节点，各有两个宽度。在概览与实时查看之间切换时保留所选页面。主页链接标明发现依据。 |
| [查询元素状态](https://github.com/de-novo/dui/issues/1) | 用布尔值和枚举进行验证。CLI 和 MCP 返回所属组件、实例、源码位置、类型、当前值以及是否可编辑。目标不明确或值无效时返回原因并保留原状态。 |
| [实时渲染](https://github.com/de-novo/dui/issues/2) | 仅启动一次宿主和浏览器。设置和恢复状态后，同时检查读回的值以及 DOM 或无障碍树中的可见结果。 |
| [修改延迟和捕获](https://github.com/de-novo/dui/issues/3) | 在同一会话中连续修改 30 次，以毫秒记录从收到请求到确认画面变化的 p50 和 p95。单独记录冷启动时间。修改期间宿主重建和浏览器重启均为 0 次。将选定状态及其证据保存到 `~/.dui/`。 |
| [领域模型](https://github.com/de-novo/dui/issues/9) | 用图表记录每个实体的 ID、字段和可变性。核心类型放在 `dui-core`，不依赖存储，覆盖地图、已保存捕获、缓存和评论。 |
| [SQLite 存储](https://github.com/de-novo/dui/issues/10) | 通过 Drizzle 迁移将元数据存入 SQLite，图片按内容哈希存为文件，并无损导入现有 `~/.dui/` 历史。表结构需便于日后用于远程 Postgres 服务器，同步走 API。 |
| [多语言捕获](https://github.com/de-novo/dui/issues/12) | 按应用支持的每种语言捕获页面，标出缺失翻译和溢出文本。 |
| [M1 覆盖率](https://github.com/de-novo/dui/issues/4) | 通过已有的确定性规划和覆盖率验收。比较 CLI 和 MCP 的结果，为每个未验证的格子记录缺失的证据。 [M1 验收条件](architecture.md) |

首次基准测试会记录设备和测试项目，得出延迟基线。跨项目的延迟目标在测量后确定。

## v0.0.3

**在地图中评审原生页面。** 在给定宽度下，用两种有类型的状态渲染一个真实的 GPUI 页面。复用网页会话的查询、设置、读回和恢复操作。随图片记录操作系统、渲染器和工具链。值一致且有独立的界面语义证据才能标记为已验证。缺少语义证据则保留为未验证。缺少渲染环境则标记为无法渲染并说明原因。

基线已在 `fixtures/gpui-kit` 上验证（`gpui-kit 0.10.0`、`gpui-pre 0.3.12`、`rustc 1.100.0`、macOS Metal 离屏渲染 480 px，通过 AccessKit / `TestQueryExt` 提供 `elementTreeAvailable: true`，缺少渲染环境时报告 `unrenderable` 与 `no-render-env`）。运行 `npm run check:gpui` 可复现。共享 `dui live` / MCP `live` 协议下的原生实时会话 30 次热编辑也已验证（**p50 3.65 ms / p95 4.23 ms**，宿主重建 0 次，原生重启 0 次，第二个视图实例独立状态保持不变，缺少语义证据时报告 `unverified` 与 `missing-semantic-evidence`）。运行 `npm run check:gpui-live` 可复现。

实施任务 · [#5](https://github.com/de-novo/dui/issues/5) · [#6](https://github.com/de-novo/dui/issues/6)

**只渲染有变化的部分。** 输入未变的格子复用已存结果，每张图片只存一次（[缓存](https://github.com/de-novo/dui/issues/8)）。比较两次捕获时只列出有变化的格子（[差异](https://github.com/de-novo/dui/issues/11)）。

## 后续里程碑

当前已有本地网页 URL 捕获、原生路由发现及部分运行时的界面捕获，各项范围见[能力表](supported-frameworks.md)。后续平台和打包工作根据上述目标安排。预编译二进制和 Homebrew（[#176](https://github.com/de-novo/dui/issues/176)）等打包工作等到产品稳定后再做。符合目标的下一步，是把 UI 修改写回代码（v0.3.0）。

| 版本 | 目标 |
| --- | --- |
| v0.0.4 | 沿操作在页面或状态之间移动。每条边标明依据来自代码提取、推断或重放验证。从初始状态重放并确认到达目标后，才将边标记为已验证（[#31](https://github.com/de-novo/dui/issues/31)，`dui transitions` / MCP `transitions`，`npm run check:transitions`）。 |
| v0.0.5 | 在 GPUI 和 gpui-kit 桌面查看器（`apps/desktop`、`dui desktop`，[#40](https://github.com/de-novo/dui/issues/40)，`npm run check:desktop`）中打开项目地图。评审者可在页面、状态、宽度或转换上留言。AI 通过 MCP 读取位置，评审者在修改后的工作树中回到同一位置，对比修改前后（[#38](https://github.com/de-novo/dui/issues/38)，`dui comment` / MCP `comment_*` / 查看器 `/api/comments`，`npm run check:comments`）。 |
| v0.0.6 | 在参考页面上测量 99% 覆盖率目标，公布已验证和可渲染格子数、遗漏分支及原因。在对应页面保留尚未解决的评审意见（[#44](https://github.com/de-novo/dui/issues/44)，`dui audit` / MCP `audit` / 查看器 `/api/audit`，`npm run check:audit`）。 |
| v0.0.7 | Priority 2 声明式规则包（Zustand 5、Redux Toolkit 2、Jotai 3、XState 5、React Hook Form 7、valibot 1、React Router 8、TanStack Start 1）、可选 `hybrid`/`live` 真实 API 模式（只读保护）、分支对比（`dui diff --base main`）、带动态 ID 折叠（`/orders/:id`）的无环多跳 BFS、贪心成对覆盖数组、`<select>` 真实选项发现、视觉变化佐证（`visualWitness`）、2D 画布内联对比（`元素` 渲染树 Diff + 惰性 `像素` 回退）以及常规运行 `linksTo` 连线（[#56](https://github.com/de-novo/dui/issues/56)、[#63](https://github.com/de-novo/dui/issues/63)、[#66](https://github.com/de-novo/dui/issues/66)、[#70](https://github.com/de-novo/dui/issues/70)、[#74](https://github.com/de-novo/dui/issues/74)、[#76](https://github.com/de-novo/dui/issues/76)、[#80](https://github.com/de-novo/dui/issues/80)、[#82](https://github.com/de-novo/dui/issues/82)、[#84](https://github.com/de-novo/dui/issues/84)、[#86](https://github.com/de-novo/dui/issues/86)、[#88](https://github.com/de-novo/dui/issues/88)）。 |
| v0.0.8 | **多平台抽象层与通用 Web · TUI。** 固定 `SurfaceRegistry`、`NoopOracle` 与统一的 `RenderNodeSnapshot` 中间表示，使新增平台只需实现单个适配器而不重复缓存、SQLite、Diff 或画布代码（[#99](https://github.com/de-novo/dui/issues/99)）。交付支持任意 HTML/JS 应用的 Universal Web 宿主（`static` / `url` / `command`，[#100](https://github.com/de-novo/dui/issues/100)）与 Universal TUI 适配器（`pty` + VT100 单元网格 → `RenderNodeSnapshot` + 等宽 PNG，[#101](https://github.com/de-novo/dui/issues/101)）。 |
| v0.0.9 | **Android · 桌面 · 设计缺陷检测与开发者工具级元素检查器。** 新增 Android 适配器（`adb screencap` + `uiautomator`/Semantics → `RenderNodeSnapshot`，[#102](https://github.com/de-novo/dui/issues/102)）、Electron 45 宿主 + GPUI AccessKit 渲染树 Diff（[#103](https://github.com/de-novo/dui/issues/103)）、Radix UI / shadcn 规则包 + Next.js 16 宿主（[#104](https://github.com/de-novo/dui/issues/104)）、多标签页并行捕获 + 基于 `renderTree` 的自动设计缺陷与无障碍诊断（溢出、重叠、文本截断、对比度不足、触控目标过小、间距与对齐偏差——人类在画布上直观查看，AI 通过 CLI/MCP 获取结构化 JSON，[#105](https://github.com/de-novo/dui/issues/105)），以及在画布上查看任意元素属性、宽高、外边距、内边距与间距标尺的开发者工具级元素检查器（[#106](https://github.com/de-novo/dui/issues/106)）。 |
| v0.1.0 | **跨平台移动端捕获与编译器预言机基础。** 新增 iOS Simulator（`xcrun simctl` + XCUI）捕获、通用认证会话（`storageState.json` / `POST authFlow`）、PR Markdown Diff 输出、React Native / Expo Router / Ink `tsc` 规则包、共享编译器预言机 IR 与 6 个官方工具链探测（`dui toolchains`：`tsc`、`rustc`、`go`、`swiftc`、`kotlinc`、`dart`）。 |
| v0.2.0 | **框架无关路由树、全页真实比例节点、模态框自动捕获与客观事实核心。** 按页面完整高度（含嵌套滚动容器）捕获并按真实宽高比排列画布与桌面节点（[#147](https://github.com/de-novo/dui/issues/147)），通过 WAI-ARIA 导航展开与官方编译器路由注册表实现框架无关路由树发现、零配置 `dui scan` 与多宿主 `dui map`（[#149](https://github.com/de-novo/dui/issues/149)、[#153](https://github.com/de-novo/dui/issues/153)），对齐 GPUI 桌面原生 macOS `TitleBar` 与避障走廊连线（[#155](https://github.com/de-novo/dui/issues/155)、[#159](https://github.com/de-novo/dui/issues/159)），基于 WAI-ARIA 自动捕获模态框/抽屉并以子叠加层节点展示（[#157](https://github.com/de-novo/dui/issues/157)），并移除硬编码的全局设计缺陷启发式规则与过早的 iframe 实时挂载，使 `dui` 严格报告客观渲染证据（[#163](https://github.com/de-novo/dui/issues/163)、[#169](https://github.com/de-novo/dui/issues/169)）。 |
| v0.3.0 | **插件化设计系统一致性校验（`DESIGN.md` / 插件）与基于代码的画布渲染。** 保持 `dui-core` 严格输出客观事实，在 `dui.json` 中提供可选插件架构接入自有 `DESIGN.md`、W3C Design Tokens 或自定义审计插件（[#167](https://github.com/de-novo/dui/issues/167)），并推动画布渲染从位图截图演进为直接基于代码的组件渲染。 |

**源码预览（实验性）。** 在装有 Node 26 和 Swift 6 或 Kotlin 2/JDK 的检出目录中，对受信任的本地源码运行以下任一命令。导出器将所选组件与布局、文本和布尔开关的可移植预览实现一起编译并运行。独立 HTML 可离线回放实际观察到的状态转换，并标识为源码预览。原生外观、平台服务和运行验证需要单独运行原生应用。

原型接受一个独立文件和一个无参入口。状态必须是确定性的布尔值，声明顺序须稳定。最多探索 32 个可达状态，每个状态最多 16 个开关。遇到不支持的 API 或超过上限时导出失败。当前产物独立于地图，在浏览器中打开。

```sh
node scripts/source-preview.mts fixtures/source-preview/Settings.swift Settings swift.html
node scripts/source-preview.mts fixtures/source-preview/Settings.kt example.Settings compose.html
```

## 多平台架构与演进路线

dui 面向四大界面形态——**Web**、**Desktop App**、**Mobile App（Android 与 iOS）**、**TUI**——以**客观渲染证据**与**插件化设计系统校验**为核心，兼顾**人类与 AI 双方的高可读性**：

1. **三端口设计（`TypeOracle` · `Host` · `Renderer`）**：具备官方编译器类型时提供 Level 2 深度状态覆盖；任意应用也可通过 `NoopOracle` 立即获得 Level 1 通用捕获与对比。
2. **统一 `RenderNodeSnapshot` 中间表示**：DOM、AccessKit、VT100 终端网格、Android `uiautomator`/Semantics 与 iOS `XCUI` 树均归一化为同一平台无关场景图，记录坐标、盒模型（`margin`、`padding`、`border`、`gap`）、排版、颜色与属性。
3. **单一模板方法流水线（`CaptureEngine`）**：缓存、SQLite 存储、渲染级 Diff、惰性像素 Diff、视觉变化佐证、覆盖率审计、CLI/MCP 一致性仅在共享核心与外壳实现一次。
4. **人类与 AI 双向友好**：人类在 2D 画布上通过视觉高亮、盒模型测量与 Diff 快速审阅；AI 通过 CLI/MCP 直接读取相同的 `renderTree`、尺寸指标、Diff 与评论锚点，无需从像素猜测。
5. **客观事实核心 → 插件化设计系统规范与代码级画布渲染**：核心不强加硬编码审美规则，团队可按需接入自有 `DESIGN.md`、Token 规范或自定义审计插件检测设计偏离（`v0.3.0`，[#167](https://github.com/de-novo/dui/issues/167)），后续里程碑将从位图截图演进到直接基于代码的画布渲染。

[GitHub 里程碑](https://github.com/de-novo/dui/milestones) · [变更日志](changelog.md)

[Compose 示例](../../fixtures/android-compose) 挂载同一设置行的两个独立实例。[模拟器检查](../../scripts/check-compose.mts) 在 720 和 1080 px 两种宽度下，从 dui 新采集的数据读取两个控件的名称、边界和选中状态。点击其中一行后检查另一行，再次点击以验证状态恢复。要复现，请构建 dui，在专用模拟器中安装示例的调试 APK，设置 `ANDROID_SERIAL`，然后运行 `node scripts/check-compose.mts`。[CI 工作流](../../.github/workflows/android-compose.yml) 包含构建和启动模拟器的命令。
