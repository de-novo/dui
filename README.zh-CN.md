# dui (de-novo-ui)

[English](README.md) | 简体中文 | [한국어](README.ko.md)

> 本文是[英文原文](README.md)的简体中文翻译。如有出入，以英文为准。

> **愿景（2026-11-19）**：dui 只支持*构建良好的产品*。当状态用类型表达、API 用 DTO 表达时，所有界面、状态和宽度都会画在同一张画布上。全部画出来代表代码良好；`未验证` 越多，说明状态管理越分散。目标是成为 UI 代码质量的行业标准：① 以公开规则（+ lint）给出支持标准，② 用一个数字表示——"UI 状态覆盖率 %"，③ 状态图和快照采用开放格式。

> **定位**：设计生成类工具展示的是*将会怎样构建*：由 AI 想象尚不存在的界面。dui 展示的是*实际上是怎样构建的*：它通过官方编译器读取现有代码，把真实存在的每个状态都真正渲染出来。不编造任何东西；无法展示的部分标为 `未验证` 或 `无法渲染`。

> **原则 #1 —— AI 原生（2026-11-19）**：dui 是 AI 原生的。**一切由 AI 使用；人只看画布。** AI 代理通过 `dui` CLI 和 MCP 服务器驱动 dui（分析、注入、渲染、验证，并修复 `未验证` 格子）。人只看画布（界面 × 状态 × 宽度 × 转换）并在格子上留言；AI 读取留言、修复代码，画布随之重新渲染。见[设计 §0](docs/zh-CN/design-v0.md#0-核心原则ai-原生)。

> **原则 #2 —— 依赖官方编译器（2026-11-19）**：dui 依赖各语言的**官方编译器 / 工具链**（TypeScript：Microsoft `typescript` 包中的 `tsc`，仅 7.0+；Rust：`rustc` + rust-analyzer；以后 Swift：`swiftc` + SourceKit-LSP；Kotlin：`kotlinc`）。它使用你项目中已有的官方编译器，绝不重新实现类型检查。非官方或替代的编译器 / 工具链为 `不支持`。每个技术栈只支持当前最新稳定主版本。见[设计：技术栈](docs/zh-CN/design-v0.md#技术栈2026-11-19-决定)。

> 处于构想阶段的仓库，尚无代码。

设计文档：[docs/zh-CN/design-v0.md](docs/zh-CN/design-v0.md)（开发者工具设计 v0）
架构：[docs/zh-CN/architecture.md](docs/zh-CN/architecture.md)（函数式核心、端口与适配器、monorepo 和 crate 边界）
语言调研：[docs/zh-CN/language-survey.md](docs/zh-CN/language-survey.md)（目标语言与 UI 技术栈全面调研，推荐顺序）
分析方式调研：[docs/zh-CN/analysis-approaches.md](docs/zh-CN/analysis-approaches.md)（代码分析 AI 工具的做法，dui 分析器技术栈推荐）
工作流程：[docs/zh-CN/workflow.md](docs/zh-CN/workflow.md)（Discussions、issues、标签、milestones、项目看板、PR）
翻译规则与术语表：[docs/i18n.md](docs/i18n.md)

## 一句话
读取代码库，把应用的**所有界面 × 所有状态 × 所有响应式宽度**一目了然地铺在类似 Figma 的无限画布上的工具。同时覆盖 Web 和桌面应用（包括原生）。

## 为什么（需求）
- 像在 Figma 里看设计一样，一次性按状态查看**真实代码画出的界面**。
- QA 和评审时，不用逐个打开，就能直接确认"这个界面的空状态 / 加载 / 错误 / 无权限是什么样子"。
- 缺失的状态（从未画过的状态）必须一眼可见。

## 核心概念
- **节点 = 一个界面 + 该界面的一个状态**（例：审核界面 / 批准最后一页之后）。
- **边 = 状态转换**（在什么条件、什么操作下进入下一个状态）。条件跳转也用状态表示 → 整个应用成为一张状态图。
- 每个节点注入**模拟数据**来渲染该状态。不需要交互（内部处理），静态画面即可。
- 把一个节点横向展开就是**响应式宽度**（手机、平板、桌面），需要时可加入亮色/暗色、语言等维度。
- 无法确认的格子标记为 `未验证`。

## 目标：99% 覆盖率（靠测量，不靠感觉）
1. 用**代码覆盖率测量**界面代码分支（if、加载 / 错误 / 空状态、按权限）→ "界面分支中实际画出了百分之几"。
2. 模拟数据**从类型 / API 模式（OpenAPI、zod 等）自动生成** → 包括空值、超长值、错误响应等边界值。
3. 出现未画出的分支时，由 AI 生成进入该分支的模拟数据并重新渲染 → 反复直到 99%。
4. 我们自己的代码按规则用类型和状态机**显式**表达界面状态 → 状态列表本身做到 100% 准确。

## 图的提取
- 静态分析：路由、链接、`navigate()` / `router.push()`、菜单项 → 骨架。
- AI 推断：条件跳转、动态地址 → 标记为 `推断`。
- 运行记录：与实际浏览记录叠加进行验证。

## 支持的框架

优先级 1–3 中的全部 38 个技术栈都是正式支持目标；优先级只表示实现顺序。暂不支持的 11 个技术栈在阻碍消除后立即加入（多数情况：其官方工具接受 TypeScript 7）。版本、原因和待定问题：[docs/zh-CN/supported-frameworks.md](docs/zh-CN/supported-frameworks.md)。

| 优先级 | 数量 | 技术栈 |
|---|---:|---|
| 优先级 1（MVP） | 8 | TypeScript, Node.js, React, Vite, TanStack Router, TanStack Query, zod, Chromium (Chrome stable) |
| 优先级 2 | 15 | Rust, GPUI (`gpui-pre`), gpui-component / gpui-kit, Zustand, Redux Toolkit, Jotai, XState, React Hook Form, valibot, Next.js, React Router, TanStack Start, Electron, Radix UI, shadcn/ui |
| 优先级 3 | 15 | Solid, SolidStart, Preact, Rspack / Rsbuild, Tauri, Dioxus, Leptos, egui, Iced, React Native, Expo, Flutter / Dart, SwiftUI / Swift, Jetpack Compose / Kotlin, Compose Multiplatform |
| 暂不支持 | 11 | Vue, Nuxt, Svelte, SvelteKit, Astro, Angular, Qwik, Remix, Turbopack, Slint, Playwright |

## 各类别的渲染方式
| 类别 | 例子 | 渲染方式 | 难度 |
|---|---|---|---|
| Web | React、Vite、Next.js、Solid、Preact | 通过 CDP 在 Chromium 中渲染，注入状态并拦截请求 | 容易 |
| 基于 Web 的桌面 | Electron、Tauri | 与 Web 相同（Tauri 模拟 invoke） | 容易 |
| 原生 | GPUI（pippip） | 在应用内的画廊模式中按状态渲染 | 费工夫 |

Web 类：一个通用引擎 + 很薄的、按框架读取界面列表的部分。原生：在框架内接入画廊模式。

## 在哪里看（画布）
- 方案 1：自动排布到 **Figma**（每个界面一行，状态横向排列，宽度放在旁边）。可以直接用 Figma 连接器做原型。
- 方案 2：用 React Flow（MIT）自建无限画布。代码变化时自动更新。tldraw 用于生产环境需要另行购买商业许可，暂缓（[设计文档](docs/zh-CN/design-v0.md) §11）。

## 类似的东西（调研于 2026-11-19）
- Storybook（+ AI 工作流、MCP）：按状态编写 story，真实代码。但呈列表形式，且仅限 Web。
- Figma：是画布，但它是设计文件，不是真实代码。
- Selfloop：读取代码，生成界面图和流程测试。仅限 Web 和移动端，未提及桌面。
- UI Verify、Chromatic、Percy：界面对比（视觉回归）。不是画布，也不是状态图。
- XState：状态机可视化。需人工编写，没有真实界面。
- AgentController、Peekaboo：让 AI 操作并截图 macOS 应用的 MCP。

**空白**：目前还没找到一个工具，能把真实代码画出的界面——不分 Web 还是桌面——连同状态图和响应式宽度一起铺在无限画布上。

## 第一个原型候选
把 obora Web 的几个界面（审核、导出、设置）按状态、按宽度渲染，并自动排布到一个 Figma 文件中。之后是 obora 桌面（已有的状态画廊），再之后是 pippip（GPUI）。

## 底层引擎候选：开源代码知识图谱（2026-11-19）
已有开源工具能很好地构建函数调用图和依赖图。dui 以它们为底层，只在上面叠加一层**状态转换层**。
- [Graphify](https://github.com/Graphify-Labs/graphify/)（★ 约 15 万，Apache-2.0）：通过 tree-sitter 支持约 40 种语言，`graph.html` 可视化，每条边标记 `EXTRACTED` / `INFERRED`。**第一候选**。
- [CodeGraph](https://github.com/colbymchenry/codegraph/)（★ 约 7.8 万，MIT）：自动更新，连接 Web 路由 ↔ 处理函数，追踪 Tauri Web ↔ Rust 调用。
- ~~[GitNexus](https://github.com/abhigyanpatwari/GitNexus/)~~（★ 约 4.9 万）：因 PolyForm Noncommercial 许可证而**排除**。

### 状态转换层（dui 要做的部分）
调用图 = "函数 A 调用 B"（代码结构）。dui 想要的 = "在这个状态下做这个操作，会变成这个状态 / 界面"（界面行为）。
1. 以界面事件（onClick、快捷键等）作为起点。
2. 沿调用图找到状态变更点（setState、store action、reducer、GPUI `cx.notify()`）。
3. 转换成 `状态 + 事件 → 下一个状态` 规则。
4. 从初始状态开始自动展开规则，生成所有可达的状态和顺序（模型检查方式）。无限展开用代表值（0、1、最后一个、最后一个的下一个）截断。
5. 回放路径，验证该状态确实能渲染出来。

像 Graphify 一样，每条边都标记 `已提取`（代码中明确）/ `已推断`（AI 推断）/ `已验证`（回放确认）。

## 状态能否提取：小结
| 类型 | 例子 | 方法 | 准确度 |
|---|---|---|---|
| 数据形态 | 空列表、1 个、非常多、长文本、无值 | 从类型 / API 模式自动生成模拟数据 | 高 |
| 代码分支 | loading、error、权限、`length === 0` | 静态分析 + 覆盖率测量 | 高（可以以 99% 为目标） |
| 由顺序产生的状态 | 上传中、批准最后一页之后、填到一半的弹窗 | 追踪事件 → 状态变更 + 展开 + 回放 | 中 → 通过"把状态集中到一个类型 / reducer"的规则提高 |

## 许可证与参考政策（2026-11-19，按开源前提更新）
dui 计划以**开源**方式发布（发布时间待 준혁 批准，目前为私有）。为避免因参考的项目引发问题，遵守以下规则。

1. **默认禁止复制代码。** 其他项目只参考设计和思路，代码自己写。文档中标为"参考"的项目（React Compiler、GATOR、CodeGraph、react-docgen-typescript、eslint-plugin-react-hooks、XState 等）都只是思路参考，不是代码复制。
2. **复用只通过依赖。** 依赖只允许 MIT、Apache-2.0、BSD、ISC。如果必须复制源码片段，只能来自同样的宽松许可证，并把原始路径、提交和许可证记录到 `THIRD_PARTY_NOTICES`。
3. **受限许可证的代码不看也不抄。** GPL、AGPL、LGPL、PolyForm Noncommercial、SSPL、BSL、tldraw 许可证等受限制的代码（GitNexus、Serena、Semgrep 引擎、FlowDroid、CodeQL 等），在实现时不阅读其源码（净室）。只使用公开文档和论文层面的概念。
4. **AI 写的代码也遵守同样标准。** 给 AI 代理的工作规则是"不得引入特定项目的代码，依赖只能是宽松许可证"。依赖许可证在 CI 中自动检查（例如 `license-checker`、`cargo-deny`）。
5. **名称与商标。** Storybook、Figma、Graphify 等他人的名称不用于产品名或包名，只用于"与 ~ 兼容"这类说明。
6. **dui 自身的许可证：Apache-2.0（已确定，2026-11-19）。** 它带有专利授权条款，企业使用方便，有利于争取成为行业标准。贡献者需通过 DCO 签署提交（`git commit -s`）。全文见 [LICENSE](LICENSE)，声明见 [NOTICE](NOTICE)，贡献规则见 [CONTRIBUTING.md](CONTRIBUTING.md)。
7. **多语言（2026-11-19 决定）。** 项目文档使用英文、简体中文、韩文三种语言。**以英文为准**；修改时先改英文，再在同一个 PR 中同步中文和韩文，或加上 "outdated" 横幅。规则与术语表：[docs/i18n.md](docs/i18n.md)。
8. **发布前检查：** 检查整个仓库中是否有外部代码片段、密钥和内部路径（obora / pippip 的内部信息）。
