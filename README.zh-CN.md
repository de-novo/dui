# dui

[English](README.md) | 简体中文 | [한국어](README.ko.md)

dui 通过项目官方编译器读取类型，自动生成模拟数据并在真实渲染环境中按状态与视口宽度捕获 Web、桌面、移动端和 TUI 界面。使用内置 2D 画布（`dui open`）或原生 GPUI 桌面查看器（`dui desktop`）浏览界面地图、实时挂载组件与变更对比，并通过 CLI / MCP 向 AI 代理输出结构化 JSON。

[产品路线图](docs/zh-CN/roadmap.md) · [变更日志](docs/zh-CN/changelog.md) · [完整支持栈列表](docs/zh-CN/supported-frameworks.md)

## 可分析范围

### 支持的技术栈与框架

| 类别 | 当前已支持 | 计划支持 |
| --- | --- | --- |
| 语言与编译器 | TypeScript 7.x / 6.x (`tsc`)、Rust edition 2024 (`rustc`) | Swift 6、Dart 3、Kotlin 2、Go |
| Web (Level 1 & Level 2) | Universal Web (`static` / `url` / `command` 支持任意 HTML/JS 应用)、React 18 / 19+、Vite 7 / 8+、Nitro SSR、Next.js 16 (`host: "next"`)、Chromium (CDP) | Preact 11、Solid 2、SolidStart 2、Rspack / Rsbuild 2 |
| 桌面应用 | GPUI (`gpui-pre` 0.3.x、`gpui-kit` / `gpui-component` 0.10.x，Metal + AccessKit `renderTree`)、Electron 45 (`host: "electron"` + `ipcMocks`) | Tauri 2、Dioxus 0.7、Leptos 0.8、egui 0.37、Iced 0.14 |
| 移动应用 (Android & iOS) | Android (`adb` `screencap` + `uiautomator`)、iOS (`xcrun simctl` + XCUI)、React Native 0.88 / Expo Router 5 (`tsc` 规则包) | 官方工具链探测 (`dui toolchains`)；SwiftUI / Compose / Flutter 适配器规划中（见[支持框架](docs/zh-CN/supported-frameworks.md)） |
| 终端 UI (TUI) | Universal TUI (`pty` + VT100 单元网格 → `RenderNodeSnapshot` + 等宽 PNG)、Ink 5 (`tsc` 规则包) | Ratatui (`rustc`)、Bubble Tea (`go`) 深度状态计划 |
| 路由与 URL 状态 | TanStack Router 1、TanStack Start 1、React Router 7 / 8、Next.js App Router (`app/**/page.tsx`)、标准 Web `URLSearchParams` | Android Intent 与深度链接编译器分析 |
| 状态、表单与模式 | React `useState`、TanStack Query 5、Zustand 5、Redux Toolkit 2、Jotai 3、XState 5、React Hook Form 7、zod 4、valibot 1、Radix UI 1 / shadcn/ui 4 | — |

### 分析与验证能力

| 领域 | 能力 | 状态 |
| --- | --- | --- |
| 界面与链接发现 | 路由文件自动发现、无环多跳 BFS (`a[href]`)、动态 ID 模式折叠 (`/orders/:id`)、界面间 `linksTo` 连线 | 已支持 |
| 状态规划与验证 | 官方编译器类型推导、贪心成对覆盖数组（Covering Array）、`<select>` 真实 DOM 选项发现、视觉变化佐证 (`visualWitness`)、收敛保护 | 已支持 |
| 数据与 API 验证 | 类型化 `GET` JSON 自动模拟、CSV 模拟、显式 `mockResponses`、`hybrid` / `live` 真实 API 模式（只读保护）、认证会话（`storageState.json`、`POST authFlow`、交互式 `dui auth login`）与密钥脱敏 | 已支持 |
| Diff 与变更评审 | 2D 画布内联对比（`元素` 渲染树 Diff · 2px 容差、`变更后`、`变更前`、惰性 `像素` 叠加）、分支对比 (`dui diff --base main`)、`--changed-since` | 已支持 |
| 质量、多语言与审计 | 99% 覆盖率审计 (`dui audit`)、多语言与 RTL 校验、缺译键与文本溢出检测、SQLite 评审评论、基于模块图的内容寻址缓存 | 已支持 |
| 实时编辑与查看器 | 共享 CLI/MCP 实时会话 (`dui live`)、内置 Web 画布 (`dui open`)、原生 GPUI 桌面查看器 (`dui desktop`) | 已支持 |
| 元素检查器与盒模型 | 开发者工具级元素属性检查、盒模型图与叠加（宽高、外边距、内边距、边框、gap、圆角）以及元素间交互式间距标尺 | 已支持 |
| 通用路由、模态框与画布 | 全页真实比例捕获、WAI-ARIA 导航展开与模态框/抽屉自动捕获、避障走廊连线、原生 macOS `TitleBar`（`apps/desktop`）、多工作进程并行捕获 (`concurrency`)、`SurfaceRegistry` 流水线 | 已支持 |

## 安装

在 macOS 或 Linux 上运行以下命令，按最新 SemVer 发布标签（`vX.Y.Z`，无标签时回退到 `main`）拉取仓库并在本机编译安装。可通过 `DUI_TAG=v0.0.1` 指定特定标签。

```sh
curl -fsSL https://github.com/de-novo/dui/raw/main/scripts/install | bash
```

需要 Git、curl 和 C 编译工具链（macOS 上使用 Xcode 命令行工具）。安装程序在本机编译 dui，自动将安装目录（默认 `~/.local/bin`）加入 shell `PATH`（设置 `DUI_NO_MODIFY_PATH=1` 可跳过），并在可用时创建链接使 `dui` 命令立即可用。如果缺少 Rust 或带 npm 的 Node.js 26，安装程序会临时下载所需工具，并在构建结束后清理。

查看已安装标签、检查是否有新标签或执行标签更新：

```sh
dui --version
dui update --check
dui update
```

**捕获网页时**需要 Node.js 22+ LTS（22 / 24 / 26）、Chromium 或 Google Chrome，以及目标应用的依赖。

## 快速开始与常用命令

运行 `dui --help` 可随时查看按功能分组的完整命令指南。大多数项目只需 **4 条命令**：

```sh
# 1. 从路由或链接自动发现界面并捕获（自动合并项目根目录的 dui.json）
dui map

# 2. （可选）捕获需登录应用的浏览器会话（支持表单 / MFA / OAuth）
dui auth login http://localhost:5173/login --out auth-state.json

# 3. 在 2D 画布或原生桌面查看器中审阅
dui open
dui desktop

# 4. 分支对比生成 PR Markdown 摘要，并检查覆盖率
dui diff --base main --markdown
dui diff --base main --regressions-only --markdown
dui audit <run-dir>
```

### `dui map` 与 `dui run` 的区别

| 命令 | 适用场景 |
| --- | --- |
| `dui map [PROJECT]` | **带路由应用的默认命令**（TanStack Router、React Router 7/8、Next.js 或链接）。自动发现路由与模态框，并合并 `dui.json` 中的 `routeParams`、`storageState`、`readySelector` 及额外 `scenarios`。 |
| `dui run [PROJECT] [MANIFEST]` | **按清单直接捕获。** 直接运行 `dui.json`（省略参数默认使用 `.` / `dui.json`；若未指定 `path` 或 `host` 则自动回退到 `dui map`）。 |

| 分组 | 命令 | 用途 |
| --- | --- | --- |
| 捕获 | `dui map [PROJECT]` · `dui run [PROJECT] [MANIFEST]` · `dui scan [PROJECT]` | 自动发现/捕获界面与模态框，或在不启动浏览器的情况下输出编译器事实与自动模拟 |
| 复验失败项 | `dui recheck RUN [PROJECT [MANIFEST]]` | 仅重新渲染上次捕获未通过的单元格，并报告已恢复的项目 |
| 认证 | `dui auth login URL [--out FILE] [--wait-url PATTERN]` | 保存 Playwright 兼容的 `storageState.json`（`0600` 权限） |
| 查看 | `dui open [RUN_DIR]` · `dui desktop` | 内置 2D Web 画布或原生 GPUI 桌面查看器 |
| 对比与质量 | `dui diff --base BRANCH [--regressions-only] [--markdown]` · `dui audit RUN_DIR` · `dui comment` | 元素级与像素级 Diff、99% 覆盖率审计、评审评论 |
| 工具 | `dui toolchains` · `dui history` · `dui prune` · `dui update` · `dui mcp` | 官方编译器探测、历史归档、缓存清理、版本更新、MCP stdio 服务器 |

## 自动发现页面

在已安装依赖的 Vite 应用目录中运行：

```sh
~/.local/bin/dui map .
```

dui 查找带页面组件的 TanStack 文件路由。若未找到具体路径，则从 `/` 开始，跟随入口页面的同源链接一层。每个地址按 375 和 1280 像素两种宽度捕获。运行 `dui open`（或 `dui open <output>`）打开内置界面地图查看器。MCP 提供相同的 `map(project)` 操作。

打开已捕获的单元格，在同一个 Chromium 会话中修改和恢复布尔值或枚举值。[CLI 和 MCP 示例](docs/zh-CN/web-viewer.md#通过-cli-或-mcp-使用实时会话)

可在项目的 `dui.json` 中提供 `appRoot`、模拟响应、链接发现的起始 `path` 或动态路径示例值（如 `"routeParams": {"id": "demo"}`）。在全局或 `routeOptions["/path"]` 中设置与页面内容对应的 `readySelector` 即可检查页面。没有此条件时，dui 将图像标为未验证并返回退出码 5。报告列出每个地址的来源和未解析的路由模式。

## 捕获自己的应用

1. 在应用的 `package.json` 和 `tsconfig.json` 所在目录打开终端，用项目的包管理器安装依赖。网页捕获支持 Vite 7 / 8 和项目中安装的官方 TypeScript 6.x 或 7.x 编译器。React 项目使用 React 19。
2. 在此目录创建 `dui.json`。下面的配置以 375px 和 1280px 两种宽度捕获首页。将 `main article` 换成页面内容加载完成后才出现的元素的 CSS 选择器，例如设置页的 `main form`。

```json
{
  "path": "/",
  "widths": [375, 1280],
  "readySelector": "main article"
}
```

3. 运行以下命令。dui 会启动本地服务器，将每次捕获保存在用户主目录的 `~/.dui/` 中。

```sh
~/.local/bin/dui run . dui.json
```

4. 运行 `~/.local/bin/dui open` 打开本地历史视图。选择项目，然后从按分支和工作树分组的列表中选择两次捕获，并排查看相同状态和宽度的截图。每次捕获都记录提交和工作树是否有未提交的更改。退出码为 5 时，可从结果中的 `output` 路径打开 `report.json` 查看原因。

**更多页面。** 加入 `"routeSelector": "nav a[href]"` 可收集起始页上匹配的同源链接，深度为一层。用 `scenarios` 指定更深的路由或需要不同就绪条件的页面。

**模拟数据。** 对于访问同源固定路径且响应类型受支持的 GET 请求，dui 会根据类型生成 JSON。打开 `facts.json`，在 `autoMocks.generated` 中查看生成的数据，在 `autoMocks.unresolved` 中查看无法生成的项。 [路由与模拟数据配置（英文）](docs/reference.md)

## 首次捕获示例

安装 dui 后，在要下载示例的目录运行以下命令。

```sh
git clone --depth 1 https://github.com/de-novo/dui.git
cd dui/fixtures/typed-api
npm ci
~/.local/bin/dui run . dui.json
```

运行 `~/.local/bin/dui open` 并选择示例项目。应看到一个已验证的 768px 截图，内含三本书。dui 根据 `fetchShelf()` 的返回类型为 `GET /api/shelf` 生成响应。若 Chrome 或 Chromium 安装在自定义路径，可将 `DUI_CHROMIUM` 设为其可执行文件路径。 [示例源码](fixtures/typed-api/main.tsx)

### 捕获真实的开源应用

npmgraph 是一个使用 MIT 许可证的 npm 依赖查看器。示例从一个入口地址开始，用两份模拟软件包数据渲染依赖图。dui 从页头的主页链接发现搜索页。两个页面各以 375px 和 1280px 捕获，共四个格子。 [运行示例](examples/npmgraph/README.md#run) · [捕获配置](examples/npmgraph/dui.json)

- [捕获参考（英文）](docs/reference.md) — 配置路由与模拟数据、解读覆盖率以及通过 CLI / MCP 自动化运行。
- [贡献指南](CONTRIBUTING.md) — 了解 crate 边界、规则包扩展与自动化检查。
- [设计文档](docs/zh-CN/design-v0.md) · [架构](docs/zh-CN/architecture.md) · [支持栈列表](docs/zh-CN/supported-frameworks.md)

## 许可证

采用 Open-Core 模式：

- 核心部分（`ee/` 目录之外）：[Apache License 2.0](LICENSE) — CLI、MCP 服务器、本地 SQLite 存储、内置 Web 查看器与本地 GPUI 桌面查看器。
- 企业版（`ee/`）：[dui Enterprise License](ee/LICENSE) — 面向团队与企业的商业功能（审计日志、SSO/RBAC、远程团队同步等）。详见 [ee/README.md](ee/README.md)。
