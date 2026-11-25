# dui

[English](README.md) | 简体中文 | [한국어](README.ko.md)

dui 通过项目官方编译器读取类型，自动生成模拟数据并在真实渲染环境中按状态与视口宽度捕获 Web、桌面、移动端和 TUI 界面。使用内置 2D 画布（`dui open`）或原生 GPUI 桌面查看器（`dui desktop`）浏览界面地图、实时挂载组件与变更对比，并通过 CLI / MCP 向 AI 代理输出结构化 JSON。

**目标。** 如今策划文档、Figma 设计和代码是分散的事实来源。dui 让代码本身成为开发与 UI 工作的单一事实来源：界面地图由代码生成，UI 修改再写回同一份代码。

[产品路线图](docs/zh-CN/roadmap.md) · [变更日志](docs/zh-CN/changelog.md) · [分析与渲染范围](docs/zh-CN/supported-frameworks.md)

以下能力表描述 `main` 开发版。使用默认安装版本时，请从[首次捕获](#first-run)开始。

## 可分析范围

### 分析与捕获方式

dui 从项目的官方编译器读取类型。库规则识别程序中的特定 API 调用。运行画面由各平台的渲染器捕获。

| 要查看的内容 | 方法 |
| --- | --- |
| 有类型的状态槽和路由参数 | `dui scan` 使用项目的 TypeScript 编译器及匹配符号的规则。 |
| 不同宽度下可访问的网页 | 向 `dui map` 提供本地 URL 或静态构建目录。Chromium 可跨框架沿同源链接访问页面。 |
| 源码中声明的原生界面 | 在 React Navigation、Expo Router 或 Flutter 项目中运行 `dui map`。每张卡片指向相应的声明。 |
| 渲染后的 UI | 在配置的 Web、移动、桌面或终端宿主上捕获。部分 SwiftUI 和 Compose 组件也可导出为可交互的 HTML 预览。 |

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

在 macOS 或 Linux 上运行以下命令，按最新 SemVer 发布标签（`vX.Y.Z`，无标签时回退到 `main`）拉取仓库并在本机编译安装。要固定版本，可在安装程序的环境变量 `DUI_TAG` 中指定[已发布的标签](https://github.com/de-novo/dui/tags)。

```sh
curl -fsSL https://dui.sh/install | sh
```

需要 Git、curl 和 C 编译工具链（macOS 上使用 Xcode 命令行工具）。安装程序在本机编译 dui，自动将安装目录（默认 `~/.local/bin`）加入 shell `PATH`（设置 `DUI_NO_MODIFY_PATH=1` 可跳过），并在可用时创建链接使 `dui` 命令立即可用。如果缺少 Rust 或带 npm 的 Node.js 26，安装程序会临时下载所需工具，并在构建结束后清理。

查看已安装标签、检查是否有新标签或执行标签更新：

```sh
dui --version
dui update --check
```

<a id="first-run"></a>

## 使用默认安装版本捕获第一个界面

安装 Node.js 26、Chrome 或 Chromium 以及 Vite 应用的依赖。[验证用的应用](fixtures/toggle)使用 Vite 8 和 React 19。在应用根目录的 `dui.json` 中，将 `readySelector` 设为目标界面就绪后可见的元素。例如，正文中包含页面标题的应用可以使用以下配置。

<!-- quickstart:manifest -->
```json
{ "readySelector": "main h1" }
```

<!-- quickstart:first-run -->
```sh
dui --version
dui map .
dui open --no-open
```

第一条命令显示安装版本，第二条命令捕获界面并在 `output` 中给出结果目录。打开最后一条命令输出的本地 URL 即可查看捕获，按 Ctrl+C 关闭查看器。如果捕获的退出码为 5，请查看结果目录中 `report.json` 记录的失败原因。

## `main` 开发版 CLI

运行 `curl -fsSL https://dui.sh/install | DUI_TAG=main sh` 安装开发版。以下示例和命令表适用于开发版。网站也提供[默认安装版本的发布文档](https://dui.sh/?lang=zh-CN#release-guide)。

如果静态站点已构建到 `dist/`，在项目根目录运行以下命令，再用上面的查看器命令打开结果。

<!-- quickstart:static -->
```sh
dui map . --build-dir dist
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

dui 查找带页面组件的 TanStack 文件路由。若未找到具体路径，则从 `/` 开始，在探索限额内沿可访问的同源链接继续探索。每个地址按 375 和 1280 像素两种宽度捕获。运行 `dui open`（或 `dui open <output>`）打开内置界面地图查看器。MCP 提供相同的 `map(project)` 操作。

打开已捕获的单元格，在同一个 Chromium 会话中修改和恢复布尔值或枚举值。[CLI 和 MCP 示例](docs/zh-CN/web-viewer.md#通过-cli-或-mcp-使用实时会话)

可在项目的 `dui.json` 中提供 `appRoot`、模拟响应、链接发现的起始 `path` 或动态路径示例值（如 `"routeParams": {"id": "demo"}`）。在全局或 `routeOptions["/path"]` 中设置与页面内容对应的 `readySelector` 即可检查页面。没有此条件时，dui 将图像标为未验证并返回退出码 5。报告列出每个地址的来源和未解析的路由模式。

## 捕获自己的应用

1. 对于此 Vite 示例，在包含 `package.json` 和 `tsconfig.json` 的应用目录中安装依赖。用 `dui check-ts .` 检查项目的官方 TypeScript 编译器。[编译器与宿主要求](docs/zh-CN/supported-frameworks.md#编译器与宿主要求)
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

**更多页面。** 加入 `"routeSelector": "nav a[href]"` 可递归访问匹配的同源链接。用 `scenarios` 指定无链接指向的路由或需要不同就绪条件的页面。

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
- [设计文档](docs/zh-CN/design-v0.md) · [架构](docs/zh-CN/architecture.md) · [分析与渲染范围](docs/zh-CN/supported-frameworks.md)

## 许可证

采用 Open-Core 模式：

- 核心部分（`ee/` 目录之外）：[Apache License 2.0](LICENSE) — CLI、MCP 服务器、本地 SQLite 存储、内置 Web 查看器与本地 GPUI 桌面查看器。
- 企业版（`ee/`）：[dui Enterprise License](ee/LICENSE) — 面向团队与企业的商业功能（审计日志、SSO/RBAC、远程团队同步等）。详见 [ee/README.md](ee/README.md)。
