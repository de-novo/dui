# 代码分析 AI 工具如何理解代码库（设计 dui 分析器之前的调研）

[English](../en/analysis-approaches.md) | 简体中文 | [한국어](../ko/analysis-approaches.md)

> 本文是[英文原文](../en/analysis-approaches.md)的简体中文翻译。如有出入，以英文为准。

> 编写于 2026-11-19。直接阅读各工具的 README、源码和官方文档整理而成。无法确认的标记为 `未确认`。
> ★ 和许可证为 2026-11-19 的 GitHub API 数值（`spdx_id` 为空时，通过 LICENSE 原文或包元数据确认）。
> 前提（준혁 的结论）：状态位置、类型、分支、`事件 → setState` 规则大多可以从 **AST + 类型信息**中得出。AI 只填补空白，组合由确定性算法生成。

## 0. 术语

| 术语 | 含义 |
|---|---|
| AST | 把代码读成语法结构树。不含类型和名称绑定 |
| tree-sitter | 能以同样方式快速把几十种语言解析成 AST 的解析器。代码有错也能产出部分树。**没有类型信息** |
| 编译器 API | 语言官方工具（TS compiler API、rust-analyzer 等）提供的**类型与名称解析** |
| LSP | 面向编辑器的语言服务器协议（跳转到定义、查找引用）。查询粒度很小 |
| SCIP | Sourcegraph 制定的语言中立索引格式。把定义、引用、实现以**已解析类型的形式**存在一个文件里 |
| 数据流 | 追踪一个值从哪里来、到哪里去（例：state → JSX） |
| 嵌入 / RAG | 把代码片段转成向量以查找相似内容的方法。结果是概率性的 |

## 1. 代码图与 AI 编程工具

| 工具 | ★ / 许可证 | 解析器 | 图 | LLM 用在哪里 | 嵌入 | 增量更新 | 语言 |
|---|---|---|---|---|---|---|---|
| [Graphify](https://github.com/Graphify-Labs/graphify) | 150,438 / Apache-2.0 | tree-sitter（"deterministic, no LLM"） | calls、imports、inherits，每条边带 EXTRACTED/INFERRED，Leiden 社区 | 不用于代码。只用于文档、PDF、媒体的语义提取和社区命名（可选） | **无**（"Not a vector index"） | `graphify watch` | 约 40 种 |
| [CodeGraph](https://github.com/colbymchenry/codegraph) | 78,088 / MIT | tree-sitter（Rust 内核，`codegraph-kernel/Cargo.toml`） | 符号、调用、引用 + **框架路由和 `navigates` 边**（React Router、TanStack Router、Next、Vue Router 等），追踪 Tauri `invoke` | 据 README，构建图时不用 LLM | 无，SQLite FTS5 全文检索 | 监听文件变化自动同步 | 20 种以上 |
| [GitNexus](https://github.com/abhigyanpatwari/GitNexus) | 48,658 / **PolyForm Noncommercial** | tree-sitter 查询（`ARCHITECTURE.md`） | 19 阶段流水线：符号、MRO、DI、Leiden 社区、执行流（process），存于 LadybugDB | "CLI 在本地索引，不用 LLM"，生成 wiki 等为可选 | 可选（远程嵌入） | `--watch` | 多种 |
| [Axon](https://github.com/harshkedia177/axon) | 820 / MIT（pyproject） | tree-sitter（Python、TS、JS） | 调用、类型关系、社区、执行流，KuzuDB | 无（"Zero cloud"） | 本地 384 维（bge-small）+ BM25 混合 | `--watch` | 3 种 |
| [Descry](https://github.com/06chaynes/descry) | 1 / MIT | 正则（+ast-grep）+ **用 SCIP 叠加类型解析** | 调用、import、定义，解析结果分为 resolved / unresolved / external | 无 | 可选（sentence-transformers） | 缓存 | SCIP 支持的全部语言 |
| [SCIP](https://github.com/scip-code/scip)（+ scip-typescript 等） | 892 / Apache-2.0 | 各语言编译器：scip-typescript 使用 TS compiler API（`ts.createProgram`、`getTypeChecker`），Rust 使用 `rust-analyzer scip` | 定义、引用、实现（精确） | 无 | 无 | 取决于索引器 | TS/JS、Rust、Java/Kotlin、C#、C/C++、Dart、Go 等 |
| Sourcegraph Cody | 未确认（仓库公开状态未确认） | 关键词搜索 + SCIP 代码图 | SCIP 定义、引用 | 生成回答、候选重排序 | 有（嵌入搜索） | 未确认 | 多种 |
| [Aider repo map](https://aider.chat/docs/repomap.html) | 50,182 / Apache-2.0 | tree-sitter + 各语言 `*-tags.scm` 查询（def/ref 标签），没有引用的语言用 Pygments 补充 | **文件图**（引用文件 → 定义文件）+ **个性化 PageRank** 排序 | 该图用来为 LLM 汇总上下文 | 无 | 按文件 mtime 缓存 | tree-sitter 支持的语言 |
| [Continue](https://github.com/continuedev/continue) | 36,519 / Apache-2.0 | 用 tree-sitter 按函数、类切块（`core/indexing/chunk/code.ts`） | 无（以检索为主） | 回答 | **有**，LanceDB 向量存储 | 基于缓存键 | 多种 |
| Cursor | 闭源 | 据公开文档：Instant Grep（本地正则索引）+ 用于探索的子代理 | 未确认 | 汇总搜索结果、探索 | 当前文档明确写着"不存储代码库的嵌入用于搜索"（[文档](https://cursor.com/docs/context/semantic-search)） | 本地索引 | 与语言无关 |
| Windsurf（Codeium） | 闭源 | "利用代码结构把代码解析成块"（解析器类型未确认） | 未确认 | 重排序、回答 | **有**（本地索引：片段发送到服务器生成嵌入，存储在本地） | 远程索引定期重建 | 与语言无关 |
| GitHub 代码导航 | [stack-graphs](https://github.com/github/stack-graphs) 877 / Apache·MIT，**2025-09-09 已归档** | tree-sitter + 名称绑定规则 DSL（stack graphs） | 定义、引用（按文件分别构建，查询时合并 → 增量） | 无 | 无 | 按文件增量 | Python、TS 等 |
| [Serena](https://github.com/oraios/serena) | 31,530 / **SolidLSP 为 MIT，Serena 应用为 GPL-3.0+** | **LSP**（40 多个语言服务器）或 JetBrains 插件 | 不存储图，按需查询 LSP（定义、引用、符号） | 代理把它当工具使用 | 无 | 由 LSP 处理 | 40 种以上 |
| [Joern](https://github.com/joernio/joern) | 3,655 / Apache-2.0 | 各语言前端（含模糊解析器） | **Code Property Graph**：AST + CFG + PDG（数据流）合成一张图 | 无 | 无 | 未确认 | C/C++、Java、JS、Python、Go、Kotlin、PHP、Ruby、Swift |
| [CodeQL](https://github.com/github/codeql) | 10,385 / 查询为 MIT，**CLI 另有条款**：分析非公开代码需要付费的 GitHub Code Security | 编译器级提取器（观察构建） | 关系数据库 + 数据流 / taint 库 | 无 | 无 | 否（重新提取） | 十余种 |
| [Semgrep](https://github.com/semgrep/semgrep) | 17,822 / LGPL-2.1（引擎） | 各语言解析器 → 通用 AST | 模式匹配 + 函数内数据流（跨文件分析在付费版，未确认） | 无（开源引擎） | 无 | 未确认 | 30 种以上（未确认） |
| [Kythe](https://github.com/kythe/kythe) | 2,171 / Apache-2.0 | 各语言索引器（与编译器集成） | 定义、引用、类型图 | 无 | 无 | 未确认 | C++、Java、Go、TS 等 |

## 2. UI 与状态专用的静态分析

| 工具 | 做法 | 能给 dui 什么 |
|---|---|---|
| [React Compiler](https://github.com/react/react/blob/main/compiler/docs/DESIGN_GOALS.md)（MIT） | Babel 插件。组件、hook 的 AST → **HIR**（基本块控制流图）→ **SSA** →（保守的）类型推断 → 变更与别名效果推断 → reactive scope → 重新生成代码。不用 LLM | **仅凭 AST 精确求解 hook、state、渲染之间数据流**的真实案例。注入转换也在同一个 Babel 阶段 |
| eslint-plugin-react-hooks | `rules-of-hooks` 使用 ESLint AST + 代码路径分析（不用 TS 类型）。6.x 的 `recommended-latest` 包含基于 React Compiler 的诊断 | 参考如何定位 hook 调用、判断条件调用 |
| [react-docgen-typescript](https://github.com/styleguidist/react-docgen-typescript)（MIT，★1,266） | 用 `ts.createProgram` + `getTypeChecker()` 提取 props 类型 | **props 类型 → 生成代表值**的基础材料 |
| [ts-morph](https://github.com/dsherret/ts-morph)（MIT，★6,231） | TS compiler API 的封装（AST 遍历与修改、类型检查器） | 用一个工具同时完成分析 + 源码转换。但 TS 7 支持仍在进行中（见下方风险） |
| XState / Stately（MIT，★30,520） | 由人编写状态机。[statelyai/agent](https://github.com/statelyai/agent) **用 LLM 生成状态机**。未找到从现有代码静态推断状态机的官方工具 | 参考输出格式（状态、事件、转换）。推断部分必须由 dui 自己做 |
| [Storybook 11 `write-stories`](https://storybook.js.org/docs/11/ai/skills/write-stories) | 由**代理（LLM）**为每个组件编写按状态划分的 story（默认、加载、空、错误、禁用）+ MSW 模拟 | 把状态列举交给 LLM 的例子。结果不确定，也没有覆盖率数字 → 这正是 dui 的差异点 |
| rust-analyzer（MIT/Apache，★17,008） | 可通过 `ra_ap_*` crate 作为库使用（`ra_ap_hir` 等）。**不保证向后兼容** | Rust 类型与名称解析。也能用 `rust-analyzer scip` 输出 SCIP |
| syn（Apache/MIT，★3,448） | Rust 语法解析器。不做类型解析 | 用于编写 proc-macro（注入）。作为主要分析手段不够 |
| Swift：SwiftSyntax、SourceKit-LSP | 语法树 + LSP（细节未确认） | 以后支持 Swift 时使用 |
| Kotlin：Kotlin Analysis API（K2）、PSI | 基于编译器的分析（细节未确认） | 以后支持 Kotlin 时使用 |

## 3. 学术界：从代码中提取 UI 转换图

| 研究 | 方法 | 能给 dui 什么 |
|---|---|---|
| GATOR / [Static Window Transition Graphs for Android (ASE 2015)](https://dacongy.github.io/papers/yang-ase15.pdf) | 用静态分析构建**窗口 = 节点、由事件引起的转换 = 边**的图（WTG）。边上标注窗口栈 push/pop 和回调顺序 | 与 dui 的 `状态 + 事件 → 下一个状态` 形状相同。证明仅靠静态分析即可做到 |
| [On the unsoundness of static analysis for Android GUIs](https://doi.org/10.1145/2931021.2931026) | 分析静态 GUI 分析遗漏的情况 | 仅靠静态分析无法达到 100% → **需要回放验证**的依据 |
| [FlowDroid](https://github.com/secure-software-engineering/FlowDroid)（LGPL-2.1，★1,275） | Android 生命周期建模 + taint 数据流 | 教训：必须对框架回调（事件）建模，数据流才能接上 |
| 基于模型的 GUI 测试（边运行应用边积累状态模型，例如 DroidBot / Stoat 一类） | 实际运行应用，收集界面状态图 | 用**运行来补充**静态分析遗漏的状态。各工具细节和许可证未确认 |

## 4. 共同模式：哪一层用什么技术

| 层 | 几乎所有工具都在用的 | 是否使用 LLM |
|---|---|---|
| 1. 解析 | tree-sitter（语言多、快、无类型） | 否 |
| 2. 名称与类型解析 | 需要精度时用**语言编译器**（TS compiler API、rust-analyzer）或其产物 **SCIP / LSP** | 否 |
| 3. 图 | 调用、引用图。安全工具用数据流（CPG、CodeQL） | 否 |
| 4. 排序与分组 | PageRank（Aider）、Leiden 社区（Graphify、GitNexus） | 否（仅命名可选） |
| 5. 检索 | grep / FTS（Cursor、CodeGraph），嵌入（Continue、Windsurf、Cody） | 嵌入模型 |
| 6. 解释与生成 | 摘要、回答、重排序、编写 story（Storybook） | **只有这里用 LLM** |

观察
- 构建图的工具（Graphify、CodeGraph、Aider、stack-graphs、Joern）**提取结构时不用 LLM**，因为必须确定且可复现。
- 需要精度的地方（SCIP、CodeQL、React Compiler）使用的不是 tree-sitter，而是**编译器级的类型解析**。
- 嵌入用于"查找相似代码"，而不是提取状态和分支。Cursor 最近的公开文档反而以 grep 为中心。
- 通用工具中没有（UI）状态转换提取。最接近的是学术界（GATOR WTG）和 React Compiler 的内部分析。
→ 준혁 的结论（以 AST + 类型为主，AI 为辅）与业界做法一致。

## 5. dui 分析器推荐

> **2026-11-19 更新**：dui 的核心现在是 Rust，并且只依赖官方编译器。下文中的 ts-morph、Babel / SWC 以及把 Graphify 作为引擎的方案已被[设计：技术栈](design-v0.md#技术栈2026-11-19-决定)取代。下面的表格作为调研记录保留。

### 5.1 TypeScript

| 候选 | 优点 | 缺点 | 判断 |
|---|---|---|---|
| **TS compiler API（用 ts-morph 封装）** | 通过类型检查器精确解析类型和符号。能解析可辨识联合和泛型。同一个工具还能做转换 | 慢（加载整个程序）。TS 7（Go 原生）中 API 有变化 | **主力** |
| tree-sitter | 非常快，容忍错误代码，增量 | 无类型 → 无法判断代表值和不可能的组合 | 只用于快速初扫（界面、文件列表） |
| SCIP（scip-typescript） | 定义、引用精确，语言中立格式 | 没有类型**结构**（字段、联合分支）和数据流 | 不使用（必要时作为交叉引用辅助） |
| Babel/SWC AST | 放置注入转换的地方 | 无类型 | **仅用于注入器**（Vite 插件） |

TS 版本风险：据 InfoQ（2026-11-06），TS 7.0（2026-07，Go 原生）移除了编译器 API，7.1 beta 通过 `typescript/sync`、`typescript/async`、`typescript/ast` 等**新入口恢复了 API**（与旧的 `ts.createProgram` 不兼容；已在[官方 TS 7.1 beta 博客](https://devblogs.microsoft.com/typescript/announcing-typescript-7-1-beta/)中确认，其中还列出了 `typescript/fs`）。obora 在工具中保留 `typescript ^5.9.3`，TS7 用 tsgo 单独运行（已在 pnpm-workspace.yaml 中确认）。→ **dui 把分析层放在 `TypeOracle` 接口之后，并且只支持 TS 7.0+（2026-11-19 决定）；obora 将迁移到 TS 7。类型查询使用 TS 7.1 API。**

数据流（state → JSX，处理函数 → setState）像 React Compiler 一样，以函数为单位的控制流 + 简单数据流**由 dui 自己实现**（React 的 hook 规则让范围很窄）。能否把 React Compiler 内部作为库复用 `未确认`（不是公开 API）→ 在 M1 中确认。

### 5.2 Rust

| 候选 | 优点 | 缺点 | 判断 |
|---|---|---|---|
| **rust-analyzer 库（`ra_ap_hir` 等）** | 能解析类型、trait，甚至宏展开 | 不保证向后兼容 → 需要固定版本，构建较重 | **主力**（固定版本） |
| `rust-analyzer scip` | 以稳定格式提供定义、引用 | 没有类型结构和数据流 | 交叉引用辅助 |
| syn | 轻量稳定，proc-macro 标准 | 无类型解析，看不到宏展开 | **注入器（proc-macro）、快速扫描用** |
| tree-sitter-rust | 快 | 无类型 | Graphify 已在使用 → 复用 Graphify 的图 |

GPUI 的状态集中在 `Entity<T>` 模型字段中，所以"找到模型类型 → 字段类型 → 代表值"可以借助 rust-analyzer 类型信息确定性地完成。查找 `cx.notify()`、`update` 调用点用 AST 模式就足够。

### 5.3 复用还是自建

| 项目 | 复用 | 自建 |
|---|---|---|
| 解析 | tree-sitter（MIT）、Babel/SWC（MIT/Apache）、syn | — |
| 类型解析 | TS compiler API、ts-morph（MIT/Apache）、rust-analyzer（MIT/Apache） | `TypeOracle` 接口 |
| 调用 / import 图 | Graphify（Apache-2.0）优先；路由和 `navigates` 边参考或使用 CodeGraph（MIT）的做法 | `CodeGraphPort` |
| props 类型提取 | 参考 react-docgen-typescript（MIT） | — |
| **状态位置判定、状态 → 渲染数据流、事件 → 状态变更规则、代表值与组合生成、覆盖率分母** | 无（仅参考 GATOR WTG 和 React Compiler 的设计） | **dui 的核心 = 自建** |
| AI | — | 只用于上面留下的 `已推断` 格子和命名 |

### 5.4 许可证适配（dui 是 Apache-2.0 开源项目；为让用户和企业可商业使用 dui，依赖仅限 MIT、Apache、BSD、ISC）

| 对象 | 判断 |
|---|---|
| Graphify、CodeGraph、Axon、Descry、SCIP、scip-typescript、Aider、Continue、stack-graphs、Joern、Kythe、tree-sitter、ts-morph、react-docgen-typescript、React、rust-analyzer、syn、XState | 允许 |
| GitNexus（PolyForm Noncommercial） | 禁止 |
| Serena 应用（GPL-3.0+） | 禁止。只允许 SolidLSP 部分（MIT） |
| Semgrep 引擎、FlowDroid（LGPL-2.1） | 不使用（按政策不算强 copyleft，但审查静态链接与分发条件的成本高 → 回避） |
| CodeQL CLI | 禁止（分析非公开代码属付费条款） |
| CodeGraph | 允许，但默认发送匿名使用统计（`TELEMETRY.md`）→ 如内置，默认设 `CODEGRAPH_TELEMETRY=0` |

## 6. 结论（推荐技术栈）

2026-11-19 决定（详见[设计：技术栈](design-v0.md#技术栈2026-11-19-决定)）：

- **核心**：Rust 单一二进制（CLI、通过 `rmcp` 的 MCP、分析器、组合规则）。代码图基于 tree-sitter + oxc 自研；Graphify 和 CodeGraph 仅参考。
- **TS**：tree-sitter（快速扫描）→ oxc（解析、语义）→ **通过 `TypeOracle` 从项目的官方 TypeScript 编译器获取类型**（仅 TS 7.0+；通过小型 Node 边车使用 TS 7.1 `typescript/sync`）→ dui 自己的数据流（React Compiler 方式）→ oxc 转换（注入）。
- **Rust**：tree-sitter 扫描 → **rust-analyzer `ra_ap_*`（固定版本）获取类型** → syn proc-macro（注入）。
- **渲染**：由 Rust 通过 CDP 驱动 Chromium（chromiumoxide）；网络模拟通过 CDP Fetch 域。
- AI 只用于填补 `已推断` 格子和命名。组合与覆盖率用确定性算法。
- 待确认：React Compiler 内部能否复用，TS 7.1 API 是否提供足够的类型检查能力，不经 Node 从 Rust 查询 TS 7 的方法（`未确认`），固定 `ra_ap_hir` 版本的成本。
