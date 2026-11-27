# Web 查看器架构

[English](../en/web-viewer.md) | 简体中文 | [한국어](../ko/web-viewer.md)

## 1. 范围与入口

本文规定 v0.0.2 的实现方式。当前由 `crates/dui` 写入独立 HTML 图库，`dui open` 打开历史文件。新查看器位于 `apps/canvas`，使用 TypeScript 7、React 19、TanStack Start 的 SPA 模式、shadcn/ui、Tailwind CSS 和 React Flow。这些依赖归查看器所有，被捕获项目使用自己的技术栈。

构建 dui 时生成客户端，将入口页与带哈希的资源嵌入 Rust 二进制。源码安装与发行构建共用该构建过程。只需 dui 和浏览器即可打开归档。保留旧独立 HTML 图库。实时会话还需要目标工具链和渲染器。[核心架构](architecture.md) · [里程碑](roadmap.md#v002)

**商业分发。** 地图使用 MIT 许可的 `@xyflow/react`，保留默认署名。dui 的付费版本可以使用该库，无需订阅 Pro。上游请求隐藏界面署名的项目订阅 Pro。分发内嵌查看器的二进制时，还须附上实际使用版本的版权声明和完整 MIT 许可证。更新锁文件时检查打包依赖。使用公开 API 实现地图功能，仓库不收录 Pro 示例或模板。 [MIT](https://github.com/xyflow/xyflow/blob/main/LICENSE) · [ProOptions](https://reactflow.dev/api-reference/types/pro-options) · [React Flow Pro](https://reactflow.dev/pro/content)

### 共享屏幕地图

**地图默认不启动浏览器（2026-11-27 决定，取代此前默认截图的决定；参见[禁止事项](../../AGENTS.md#never)）。** `dui map` 规划界面、状态和宽度但不截图：每个格子保持原因为 `not-rendered` 的 `unverified`，覆盖率不计为尝试。链接发现对每个可达页面读取一次应用服务器发送的 HTML，不运行脚本，因此只由脚本添加的链接会缺失，`screenDiscovery.limitations` 会说明这一点。Vite 应用的界面来自其路由器。`--no-capture` 表示默认行为。`dui map . --capture` 为像素验证截取所有计划的格子，因此查看器用浏览者的浏览器绘制已保存的 PNG 和 HTML 快照，无需运行中的应用。只有在请求时才在运行中的应用里从源码渲染：没有截图的卡片提供 **从源码渲染**，宽度预览和响应式预览在打开期间渲染。渲染所有者进程在最后一个会话后保留 20 秒。`dui run` 和显式设置 `capture: true` 的 MCP map 调用截取所有计划的格子；截图缓存使未变化的格子没有开销。

运行 `dui map . --entry src/App.tsx` 从 TSX 源模型提取界面和导航关系。归档保存 `source-model.json`、查看器图、源码位置和未解析的导航。该路径无需服务器、构建产物或浏览器。没有 `dui.json` 时，自动选择 `src/App.tsx` 或 `App.tsx` 中唯一存在的入口。若均不存在，则选择项目根目录及 `src/` 直属目录中唯一的 TSX 模块。在可运行的应用上使用 `dui map . --capture` 获取像素证据。

<<<<<<< HEAD
使用 `dui map . --url http://localhost:4200/` 为运行中的本地 Web 应用生成地图。dui 从指定地址开始，沿服务器发送的 HTML 中的同源链接导航。使用 `--capture` 时，它在 Chromium 中渲染每个页面，沿渲染后页面的链接导航并执行布局检查。Angular 等 Web 框架共享此流程。在 Vite 项目中，dui 还会先读取路由器，因此没有链接可达的路由也会进入地图，`screenDiscovery.mode` 为 `router+links`。需要开发服务器的应用（例如 `configureServer` 中间件或 `server.proxy`）应使用此方式。内置主机把这类应用渲染为空白时，报告会在 `screenDiscovery.devServerHint` 中说明。从源码分析框架的路由、守卫、组件输入和状态需要适配项目编译器和路由器的规则。
=======
每个截图还有一张缩略图 `thumb-IMAGE`：宽 200 px 的副本，在截图结束后写入，缺失时重新写入。地图卡片在屏幕上不超过 200 设备像素宽时绘制缩略图，更宽时绘制 PNG，因此缩小查看的数百个屏幕的地图不会加载每张原尺寸截图。卡片只在接近视口时加载图片。验证依据仍是 PNG。

使用 `dui map . --url http://localhost:4200/` 为运行中的本地 Web 应用生成地图。dui 从指定地址开始，沿同源链接导航，并使用 Chromium 执行截图和布局检查。Angular 等 Web 框架共享此流程。在 Vite 项目中，dui 还会先读取路由器，因此没有链接可达的路由也会从运行中的服务器截取，`screenDiscovery.mode` 为 `router+links`。需要开发服务器的应用（例如 `configureServer` 中间件或 `server.proxy`）应使用此方式。内置主机把这类应用渲染为空白时，报告会在 `screenDiscovery.devServerHint` 中说明。从源码分析框架的路由、守卫、组件输入和状态需要适配项目编译器和路由器的规则。
>>>>>>> origin/main

构建静态部署产物后，运行 `dui map . --build-dir dist`。dui 在临时回环端口提供该目录，默认以 375 和 1280 px 为沿链接可到达的页面生成地图，使用 `--capture` 时截图。在 `dui.json` 中设置 `path` 可指定其他 HTML 入口。若使用通过 `index.html` 处理导航的 history 路由，设置 `spaFallback: true`。Vue Router、使用 adapter-static 的 SvelteKit、Astro 静态输出和使用 preact-iso 的 Preact 共用此流程。需要应用服务器的部署使用 `--url`。需要登录或没有可达链接的页面需要项目提供入口或会话数据。

对 React Navigation、Expo Router 或 Flutter 应用包运行 `dui map PATH`，可以在没有设备的情况下从路由声明生成地图。React Navigation 和 Expo Router 使用项目所安装的 TypeScript。Flutter 使用 Dart SDK，dui 在首次运行时将锁定版本的分析器安装到自己的缓存中。在 monorepo 中可指定应用包路径，或在 `dui.json` 中设置 `appRoot`。

使用 `dui open` 打开归档。源码卡片显示声明名称、文件和行号。按名称注册的屏幕可以使用导航键。条件路由保留条件表达式，参数路由保留路径模式。发现结果列出无法解析的声明。纯源码地图的截图尝试次数为 0。原生界面外观、跳转和运行时覆盖率需要实际截图证据。

录制工具打开原生路由后，将 PNG 和下列格式的 JSON 保存在同一目录。从与录制版本对应的源码地图复制快照 ID 和声明 ID。录制工具提供实际到达的路径和逻辑视口宽度。`runtime` 接受 `android`、`ios`、`web` 和 `desktop`。

```json
{
  "sourceSnapshot": "<snapshot.id>",
  "captures": [{
    "screenId": "<graph.nodes[].id>",
    "runtime": "android",
    "observedPath": "/settings",
    "width": 393,
    "image": "settings.png"
  }]
}
```

对本地原始源码归档运行 `dui map attach SOURCE_RUN recordings.json`，再用 `dui open` 打开新结果。已关联的卡片显示录制图像并保留源码位置。归档记录 PNG 尺寸、运行环境和实际到达的路由。由于对应关系由录制工具提供，导入的图像标为 `unverified`。自动验证覆盖计数保持为零。每组声明、运行环境和宽度在一个记录文件中对应一张截图。

收集 iOS 证据时，将应用的模拟器构建安装到 iPhone 模拟器，确认路由的预期内容可见后截图。记录逻辑宽度、PNG 尺寸、实际路由和应用修订版本。模拟器测试使用公开样本数据。默认在运行器上验证图像。只有明确请求时才导出 PNG，保留一天。

### HTML 卡片

Web 截图会在 PNG 旁保存 `HASH-WIDTH.html`，并记录在 `observation.htmlSnapshot` 中：`file`、`bytes`、`fidelity`（以截图宽度重新渲染快照时不同像素的比例；在截图结束后于截图浏览器中测量，快照和 PNG 不变时复用）`omitted`（无法内嵌的资源，例如跨源图片或浏览器拒绝读取的画布）和 `assets`。至少 8 KiB 的字体或图片在每次运行中只以 `asset-SHA256.EXT` 在快照旁保存一次，快照按该名称引用它，因此共用同一字体的屏幕不会各带一份副本。`assets` 列出快照使用的名称。快照不含脚本。输入保留其值，打开的对话框保持打开，动画显示拍摄 PNG 时的状态。

当卡片在屏幕上宽度至少为 320 CSS px 且 `fidelity` 不超过 0.5 % 时，地图用快照绘制卡片，否则绘制 PNG。同时最多挂载 12 个快照框架，移出视口的卡片回到 PNG。框架是禁止脚本和同源访问的沙箱，查看器以只允许内联样式以及来自 `data:` 或查看器上该运行共享资源的图片、字体的内容安全策略提供快照。快照附带指向 `/archive/r/RUN/` 下的 `<base>`，因此资源名称在其所属运行内解析。验证依据仍是 PNG，检查器会说明卡片显示的是哪一种。


### 宽度预览

在检查器中，**宽度预览** 以 320 CSS px 以上的任意宽度显示所选界面。

项目可以运行时（与 **响应式预览** 条件相同），通过实时会话在运行中的应用里按该宽度从源码渲染界面，包含脚本。宽度改变时帧随之更新。它不记录为捕获，也不计入覆盖率。

否则（例如在项目之外打开的归档），改为按该宽度排版捕获的 HTML 快照，从最接近所选宽度的已捕获宽度开始。快照保留带媒体查询的样式表，但不包含其他宽度下由脚本选择的布局（例如在某个断点以下渲染另一种菜单的组件）。已捕获宽度标为已捕获，其他宽度标为未验证的 CSS 重排预览。若无法打开实时会话，对话框会说明原因并显示快照。

为说明快照在其他宽度下的可信程度，清单中设置了 `"htmlReflow": true` 的运行会把每个界面的快照按该界面的其他已捕获宽度（相同状态、场景和区域设置）渲染，并与该宽度拍摄的 PNG 比较。`report.json` 在 `htmlReflow.pairs` 中以 `path`、`from`、`to` 和 `mismatch` 记录每一对，并记录 `measured`、`reused` 和 `ms`。快照与 PNG 的名称是其内容的哈希，因此已用相同 dui 和 Chromium 构建测量过的对会从 `~/.dui/reflow` 读取而不再渲染；所有捕获都来自缓存的运行不做任何测量。快照预览列出这些结果；若该界面任一对差异超过 0.5 %，则提示脚本也会改变布局。

### React Flow 验证

开发预览从已完成的运行中读取入口截图和发现的链接。在仓库根目录使用 Node 26 运行下列命令，打开输出的回环地址。将最后的路径替换为捕获结果的输出目录。

```sh
npm ci --prefix apps/canvas
npm --prefix apps/canvas run preview:archive -- /path/to/run
```

查看器使用 [Vite](../../dependency-catalog.json) 和官方 Tailwind Vite 插件构建。`npm --prefix apps/canvas run build` 将浏览器资源写入 `apps/canvas/dist`。CSS 构建工具适用范围有限的[许可证例外](../en/architecture.md#62-automated-checks-ci-fails-on-any-violation)。

使用 npmgraph 归档确认了两个界面节点、一条附带来源与选择器依据的发现链接（标注未回放验证），以及 375 和 1280 像素的四张截图。Chrome 检查覆盖展开状态与宽度、从所选界面打开实时会话、在双控件夹具中编辑 `boolean` 与 `enum` 值并保存已验证修订、返回总览时保留所选界面、键盘焦点恢复、刷新、后退和默认署名。已安装的 CLI 仍打开独立图库。

新地图中，若已捕获页面的 DOM 含有指向另一已捕获页面地址的同源链接，则绘制有向边。重复链接合并为一条边，仅在观测到该链接的宽度显示。它说明页面提供此导航选项；验证实际跳转还需回放。

链接边从卡片截图中通往目标的链接元素开始，而不是从卡片边框开始。查看器从截图记录的元素树中读取该元素：`href` 解析为目标路径且矩形非空的 `a` 元素。若多个元素通往同一界面，边从文档顺序中的第一个开始；选择卡片时也会为其余元素描边。边从卡片上下边框中较近的一侧直线穿出，同一卡片发出的边错开高度，互不交叉。起点按截图自身的坐标放置，因此在 PNG 和 HTML 快照上相同。在所显示宽度下没有这种元素的边，例如对话框边或折叠菜单内的链接，从卡片边框开始。跨行换行的链接记录为包住各片段的一个矩形，因此其边从该矩形的上边或下边中点开始，而不是从文字上开始。如需对比，可在 **Map options** › **连接起点** 中选择 **卡片边框**（URL 键 `edges=card`），所有边将像以前一样从卡片边框开始；选择 **链接元素** 则恢复从链接元素开始。

预览可接收一个或多个运行目录。省略参数时从 `~/.dui/projects` 查找已保存的界面地图。在 **Select workspace** 中选择项目、已记录的分支和版本。侧边栏依次提供工作区选择、主题和折叠。**Map options** 提供界面查找、连接模式、连接起点和捕获宽度。选择卡片可查看该界面的状态、宽度、验证结果、缺失截图原因、已观测链接依据及 **Open live session** 操作。返回总览时会保留所选界面。卡片统计已有状态和宽度中的未验证与无法渲染格子数。

原生桌面应用的概览为每组相连的屏幕绘制一棵生成树。选中屏幕后，高亮显示所有已记录的传入和传出连接，包括树之外的连接。箭头保留记录的方向与证据类型。未选择屏幕时，可将指针悬停在屏幕上预览这些连接。

同一项目 ID 在所有分支、工作树和版本中共用一个显示名称。首次保存截图时记录主项目的 `package.json` 名称，缺失时使用根目录名。`dui history` 为已有本地归档登记名称。

使用**查找屏幕**或 `Cmd+K` / `Ctrl+K` 筛选已捕获的路由。使用方向键在结果间移动，按 Enter 打开所选屏幕。原生桌面地图也支持按路由和显示名称搜索。按 Esc 可将键盘焦点返回查找屏幕按钮。 桌面地图控件还可切换缩略地图。如果 `/` 成功返回的文档在挂载期间转到同源路由，详情面板会显示请求路径和观测到的目标路径。

新的 Chromium 捕获在 CLI 和 MCP 的 `inspect` 以及网页和桌面检查器中提供 `screenEvidence`。捕获状态记录捕获检查的结果。页面验证还要求文档响应成功、到达请求的路由（或允许的入口重定向）、页面稳定，并存在匹配项目 `readySelector` 的可见元素。使用默认就绪条件时，页面身份保持未验证。若页面有唯一的同源 canonical 链接，检查器自动比较声明的地址与实际到达的路由。检查器显示观测到的标题。旧归档保留原有证据。

在网页查看器的**元素检查器**中开启**元素边框**可查看捕获中记录的元素边界。开启**对齐参考线**可查看所选元素的边线或两个控件之间的对齐偏移量。两个开关分别控制，默认关闭。查看器将同一屏幕、场景、状态和语言中可识别的同级控件与相邻的较窄和较宽捕获进行比较。如果两侧参考宽度下的边缘或中心线均在 1 个 CSS 像素内对齐，而所选宽度下这些对齐关系均已消失，检查器会显示控件对、偏移量和参考宽度。有意的响应式布局变化也可能出现。比较证据不足时，仍可手动检查记录的元素边界。 Chromium 捕获整页时，dui 会保留垂直滚动条占用的空间。较早的归档可能保存了浏览器为截图调整布局之前的坐标。如果 PNG 与元素边框不一致，请重新捕获页面。

在**地图选项 → 全部地图卡片**中，可以分别开启或关闭**元素边框**和**对齐参考线**。每张卡片使用当前显示图像的记录坐标。参考线标记该卡片上测得的最大响应式对齐变化。如果没有这样的变化，则标记捕获中最多互不重叠的同级控件共用的行和列。捕获缺失或无法唯一匹配时不显示叠加层。

在项目目录运行 `dui recheck RUN`，可复验捕获结果为未验证、不可渲染或已有页面验证未通过的单元格。复验使用当前项目，按归档中的路径、状态输入、宽度和语言区域重新渲染。使用自定义清单时，运行 `dui recheck RUN PROJECT MANIFEST`。MCP 工具 `recheck` 接受 `run`、`project` 和可选的 `manifest`。新归档仅含选中的单元格，`recheck.source` 指向原归档，`selected`、`skipped`、`resolved` 和 `remaining` 分别记录选中、跳过、已解决和待解决的数量。仍有项目缺少证据时，CLI 退出码为 `5`。原归档中的单元格全部通过时，退出码为 `0`，无需启动渲染器或创建归档。

评审 PR 时先捕获两个分支，再运行 `dui diff --base main --regressions-only --markdown`。比较使用各分支上该应用的最新捕获，跳过选择性复验和实时编辑记录。要指定确切的运行目录，使用 `dui diff BEFORE AFTER --regressions-only`。工具按路由、场景、状态、宽度和语言区域匹配单元格，分别比较捕获验证与页面验证。只有基准分支已通过、变更分支有明确未通过记录的检查才计为回归。JSON 在 `verification.regressions` 中列出受影响的单元格，同时报告已比较的检查数和未匹配的单元格数。缺少证据的检查保持未知。比较完成时，确认有回归的 CLI 退出码为 `5`，无回归为 `0`。没有任何可比较的检查时，判定为 `inconclusive`，仅回归模式的 CLI 退出码为 `2`。向 MCP `diff` 传入 `regressionsOnly: true` 可获得相同的判定数据。

GitHub 的 **Compare base and head captures** 检查在 PR 打开或更新时运行。它检出事件中的两个提交 SHA，使用基准提交构建的同一 dui 二进制文件，为双方共有的每个网页项目分别生成新归档。工具通过 Git 跟踪的 `index.html` 或与 `package.json` 同目录的 `dui.json` 发现项目。嵌套静态页面归入上层项目，npm 应用按锁文件安装依赖。路由、状态输入、宽度、语言区域和就绪条件来自各提交的项目配置和 dui 自动发现。作业摘要显示两个 SHA、未匹配的项目或单元格以及每项新增失败的观测证据。确认回归或无法判定时检查失败，推送修复提交后会重新捕获。

地图先根据捕获数据估算画框尺寸。渲染后，使用包含标题的卡片实测高度计算行位置和连线间距。图片加载或卡片尺寸变化时，地图会重新布局。

### 查看器设计系统

预览使用 Tailwind CSS 4.3.5 和 shadcn/ui 的 Base UI 预设（`base-nova`）。通用按钮与对话框由 shadcn CLI 4.23.0 生成，运行时使用 `@base-ui/react` 1.9.0。Tailwind 与屏幕地图共用主题变量。

查看器在首次绘制前应用已保存的主题。选择系统主题时，即使归档仍在加载，也会跟随设备的外观变化。

[tokens.css](../../apps/canvas/src/styles/tokens.css) · [Button](../../apps/canvas/src/components/ui/button.tsx) · [Dialog](../../apps/canvas/src/components/ui/dialog.tsx) · [CaptureCard](../../apps/canvas/src/features/map/CaptureCard.tsx)

- **色彩与材质** — 两种主题的画布底色均使用不透明黑白表面。橙色 `--primary` (#EF803F) 用于 dui 标记和主要操作。侧栏与状态/详情面板均悬浮在地图上方，使用更通透的磨砂玻璃表面（浅色主题为半透明白，深色主题为半透明炭灰），与截图卡片共用 `--blur` (16 px)。
- **字体与间距** — 控件使用系统无衬线字体，路径使用系统等宽字体。默认字号下，元信息、正文和面板标题分别为 12 / 14 / 20 px，间距使用 4 / 8 / 12 / 16 / 24 px。控件圆角为 6 px，面板为 12 px。
- **控件** — 面板操作复用 `Button` 的默认、主要或图标样式。纯图标控件需提供无障碍名称。轮廓线用于键盘焦点，位置和字重表示选中状态。
- **截图与判定** — 两种地图视图均使用 `CaptureCard`。保持原始宽高比，路径名称位于截图下方。卡片显示待检查的格子数，详情面板显示记录的判定和原因。
- **导航** — 侧栏顺序固定为工作区、主题、折叠。界面查找位于地图中。选择界面可放大截图，每页显示两个观测到的相邻界面。关闭详情可返回原地图视口。
- **地图控件** — 导航按钮和地图选项的底边对齐，来源标注位于下方。在手机上，缩放控件和地图选项排成一行，每个按钮的触摸区域至少为 44 × 44 CSS 像素，按钮间距至少为 8 CSS 像素。查找屏幕位于卡片/二维地图切换按钮旁。卡片视图的控件可按需换行。自动适配的全图会随视口和侧栏尺寸调整。手动平移或缩放后，视图会保持原位，直到再次选择适配全图。

在实际查看器中检查 375 和 1280 px 下两种主题的总览和选中界面。确认可以访问所有相邻界面，箭头方向与归档相符，截图保留原色，关闭详情后键盘焦点复原。

<a id="portable-source-map"></a>

### 在地图中查看组件源码预览

在可信本地项目的 `dui.json` 中声明可独立编译的 SwiftUI 或 Compose 源文件及入口。路径相对于该文件。

```json
{
  "sourcePreviews": [
    { "source": "ios/Settings.swift", "entry": "Settings" },
    { "source": "android/Settings.kt", "entry": "example.Settings" }
  ]
}
```

依次运行 `dui map .` 和 `dui open`。卡片显示源码声明并保持未验证。选择卡片即可在检查器中操作观测到的开关。生成地图需要 Node 26 以及 Swift 6 或 Kotlin 2/JDK。打开已保存的地图只需要查看器。归档保存源文件 SHA-256、布局元素及观测到的状态转换。查看器将这些数据以 HTML 文本和控件重新绘制并加以说明。这并非真实的 iOS 或 Android 界面。要在卡片上显示真实界面，请用 `dui map attach` 关联模拟器或设备录制。

卡片标识由项目、源文件和限定入口名决定。缺少源文件或遇到不支持的调用时，对应卡片会显示原因。从相应的本地项目打开归档时还会检查当前声明和源文件摘要。更改后再次运行 `dui map .`。源码预览的原生捕获尝试数和验证数均为零。通过 `dui map attach` 关联原生录制时须指定匹配的快照 ID 和屏幕 ID。所选宽度下没有截图的原生源码卡片会按录制本身的宽度显示该录制。仓库中的示例即是如此。**Source preview native captures** 工作流不加修改地用 SwiftUI 构建 `fixtures/source-preview/Settings.swift`，用 Jetpack Compose 构建 `Settings.kt`，并在 iPhone 模拟器和 Android 模拟器上截图。每张图片连同源文件、宿主与图片 SHA-256、设备、操作系统和工作流运行记录保存在 `fixtures/source-preview-native/recordings/`。Pages 演示会关联这些截图；若截图后源文件发生变化则构建失败。

## 2. 模块与依赖

```text
apps/canvas/src/
  routes/                        # TanStack Start
  features/{history,map,inspector}/
  model/                         # selection, records, projections
  data/                          # HTTP, archive decoding, session client
  components/ui/                 # shadcn/ui primitives
  styles/                        # shared Tailwind tokens
packages/schemas/                # archive and session JSON Schemas
```

路由组合功能并校验 URL 参数。功能模块依赖共享模型和基础控件，通过选择项或带类型的回调通信。模型只含纯函数与不可变值，不依赖 React 或 I/O。只有地图把记录转换为 React Flow 节点和边。首次打开时加载地图与详情模块。生成的 shadcn/ui 代码和主题变量仅用于查看器。

Rust 负责验证、覆盖率和归档写入。CLI 与 MCP 调用相同的应用操作。本地 HTTP 适配器向查看器提供数据。在核心架构规划的 crate 拆分之前，这些外层职责放在 `crates/dui`。保留已有的三个端口。功能有了第一个调用方时才创建模块。

### 2.1 整洁架构边界

按规则的归属组织变更。源码依赖指向内层。运行时需要外部工具时，通过内层定义的端口调用。

| 边界 | 职责 | 允许的依赖 |
|---|---|---|
| 领域 | 带类型的观测值、格子判定与会话决策。相同输入产生相同结果。 | `dui-core` 内的纯值和函数。 |
| 应用 | 查询、编辑和保存用例。协调命令顺序，调用端口，把观测值交给领域，返回带类型的结果。 | 领域及 `TypeOracle`、`Host`、`Renderer` 契约。 |
| 适配与组装 | CLI、MCP、HTTP 转换输入及错误。编译器和 Chromium 适配器收集证据。二进制入口连接具体实现。 | 应用或端口契约、领域值及各适配器所用的工具。 |
| 浏览器呈现 | URL 选择、视图模型和控件。路由加载器通过 `data/` 读取，组件接收模型值和回调。 | `data/` 将传输类型转为 `model/` 的值，`features/` 呈现模型。模型不依赖这两层。 |

**一次编辑的路径。** CLI 和 MCP 将请求解码为同一个编辑命令。用例请求领域作出决策。过期修订号返回冲突，对渲染器的调用为零。获准的编辑产生待执行的效果。应用通过渲染端口执行，将观测结果转换为领域值后返回判定。仅由传输适配器将冲突映射为相应协议错误。用内存渲染器测试拒绝路径，用普通观测值测试判定。

**迁移位置。** 当前 `ports.rs` 传递 JSON 值，`analysis.rs` 在核心验证结束后补充 `missing-screen-assertion`。实现实时验证时，将该判定移入共享验证器，在适配边界转换有关 JSON 字段。保持 CLI 和 MCP 结果兼容。领域输入不得包含 React Flow 节点、CDP 响应或 HTTP 状态码。纯操作使用普通函数，trait 仅用于已有端口边界。

## 3. 数据与导航

以 `packages/schemas` 中的公开 JSON Schema 作为传输契约。按契约检查 Rust 输出，并生成 TypeScript 传输类型。外部 JSON 在统一入口解码为查看器模型。允许新增字段。用旧格式解码器读取当前无版本的归档，新归档记录模式版本。版本不受支持或必填字段无效时，只对该次运行显示数据错误，其他记录仍可打开。

已保存的运行记录不可变。标识和 Git 信息来自 `snapshot.json`，格子的证据来自 `report.json`，连接结构来自 `graph.json`。在所选运行内查询不透明 ID。分支与工作树用于筛选历史。路由为 `/projects/$project/runs/$run`，校验查询键 `screen`、`state`、`width` 和 `view`。省略可选键可打开总览。列表、地图和详情共用同一选择项。后退、刷新或通过键盘返回后恢复选择。缺失的格子显示为不可用。

Rust 监听回环地址。在 `/api/runs/$project/$run` 提供已完成的运行，通过该运行内的引用查找图像。查看器导航返回静态入口页。缺失的 API 记录或资源分别返回对应错误。实时修改权限限于所属本地会话。导航 URL 不包含控制凭据或文件系统路径。

### 查看捕获的状态

从该次运行的 `report.json` 中选择一个单元格的 `state` 和 `width`。

```sh
dui inspect RUN_DIRECTORY '{"state":"STATE_ID","width":768}'
```

MCP 工具 `inspect` 接受 `directory` 和相同的 `selection` 对象。每项记录包含源码位置、所属函数、实例、编译器类型及已观测的值。实例 ID 仅在该单元格内有效。添加 `slot` 和 `instance` 可选择一项。添加 `value` 可验证候选布尔值或枚举值，不修改状态。`editable` 表示捕获时具有 setter 且类型受支持。缺少实例记录的旧归档返回 `instance-evidence-unavailable`。

## 4. 实时编辑与失败处理

一个本地 dui 进程持有实时会话的宿主与渲染器。CLI 和 MCP 通过会话 ID 访问该进程。查看器通过回环地址读取同一会话的修订号和最近观测的画面。已保存的图像标为归档。实时详情展示运行中目标的最近观测帧和编辑状态。修订号变化时刷新，丢弃迟到的旧响应。

使用核心的 Decider（命令 → 事件与效果），外层负责释放资源。操作为 `open → inspect → set/reset → save → close`，其中查询、编辑、保存可重复。查询返回所有者、实例、源码位置、类型、当前值和可编辑性。每次编辑带上会话、格子、实例、预期修订号和请求 ID。同一会话的编辑串行处理。类型无效、实例不明确或修订号过期时，在写入前拒绝。同一请求重试时返回已记录的结果，同一 ID 携带不同内容时拒绝。

写入后回读值，并核对 DOM 或无障碍树的可见结果。写入后验证失败时返回观测值和原因，递增修订号并标记未验证。渲染器断开时使会话失效，可从归档格子重开。保存时原子地发布新的本地运行记录。关闭时释放持有的渲染器与宿主。切换格子不会改变正在执行的编辑的目标。

首个客户端启动同一二进制的后台实例。在 `~/.dui` 中使用用户私有锁协调启动，绑定成功后写入回环地址。后续客户端复用该实例。会话 ID 失效后需重新打开格子。查看器通过同源请求读取 JSON，CLI 和 MCP 发送经过认证的本地命令。详情打开时轮询会话修订号，关闭后停止。

```text
open(project, run, cell, requestId) -> session, revision
inspect(session, cell) -> revision, controls[], observation
set(session, cell, instance, slot, value, expectedRevision, requestId) -> editResult
reset(session, cell, instance, slot, expectedRevision, requestId) -> editResult
save(session, cell, expectedRevision, requestId) -> project, run, cell
close(session) -> closed
editResult = {session, cell, instance, slot, revision, observedValue, status, reason?, evidence}
```

重置将所选控件恢复到会话打开时的值。保存时记录最近观测的修订号及验证结果。先检查请求 ID，再检查预期修订号，因此响应丢失后重试仍返回首次结果。请求结果保留至会话关闭，所属进程重启后旧会话失效。若目标应用的版本与归档不同，则单独显示实时运行的来源。

### 通过 CLI 或 MCP 使用实时会话

将 JSON 对象传给 `dui live '{…}'`，或作为 MCP 工具 `live` 的参数。选择归档目录和报告中的单元格，将下列占位符替换为返回的会话、实时实例和槽位 ID。 对于 `dui map` 生成的归档，可省略 `manifest` 以重新发现项目。

```jsonl
{"op":"open","project":"PROJECT","manifest":"MANIFEST","run":"RUN_DIRECTORY","cell":{"state":"STATE_ID","width":768},"requestId":"open-1"}
{"op":"inspect","session":"SESSION","cell":{"state":"STATE_ID","width":768}}
{"op":"set","session":"SESSION","cell":{"state":"STATE_ID","width":768},"instance":"INSTANCE","slot":"SLOT","value":true,"expectedRevision":0,"requestId":"edit-1","expect":{"selector":"#menu-toggle","text":"Close"}}
{"op":"reset","session":"SESSION","cell":{"state":"STATE_ID","width":768},"instance":"INSTANCE","slot":"SLOT","expectedRevision":1,"requestId":"reset-1","expect":{"selector":"#menu-toggle","text":"Open"}}
{"op":"save","session":"SESSION","cell":{"state":"STATE_ID","width":768},"expectedRevision":2,"requestId":"save-1"}
{"op":"close","session":"SESSION"}
```

每次编辑使用唯一请求 ID，丢失响应后可重发相同请求。对于 set 和 reset，`expect.selector` 必须匹配一个可见的 HTML 元素，去除首尾空白后的文本必须等于 `expect.text`。写入后断言失败仍会推进修订号，并返回 `unverified`、观测值和文本。reset 恢复打开时的值，close 释放浏览器和主机。空闲会话在 30 分钟后过期。save 返回 `~/.dui` 下新归档的路径。运行 `dui open`（或 `dui open RUN_DIRECTORY`）打开内置查看器，或向 `dui inspect` 传入保存的目录和单元格。归档包含 PNG、SHA-256 摘要、观测值和文本断言、编辑序列及 Git 来源。再次实时编辑时打开原始单元格。

### 热编辑延迟基线

在固定版本的 React inspect 夹具上交替编辑 30 次，从所有者进程接受请求到验证状态值及可见文本，**p50 为 111.4 ms**，**p95 为 112.9 ms**。在 Apple M1 Pro、Darwin 25.5.0、Node 26.13.0、TypeScript 7.0.2 和 Chrome 157.0.8103.79 上，打开新会话耗时 **1.72 s**。冷启动构建一次主机并启动一次浏览器，热编辑期间两项均为零。依次运行 `npm ci`、`npm ci --prefix fixtures/replay` 和 `npm run check:inspect` 可复现。每次运行均输出环境和夹具哈希。

### 调整已挂载网页的尺寸

在已安装依赖的本地 Vite 项目中运行 `dui map .`，然后在同一目录运行 `dui open`。选择地图卡片并打开 **响应式预览**。用 CSS 像素设置宽度或高度后点击 **应用**。选择 CSS 查询容器可单独调整其尺寸约束。显示的文本输入框和单选下拉菜单可编辑已挂载页面的值。每一帧都来自 Chromium 完成布局后的画面。关闭对话框即结束会话。相同项目、运行和源码状态的会话共享一个已构建的主机和浏览器，各自使用独立的浏览器上下文，因此只有第一个会话需要构建并启动浏览器；该共享进程在最后一个会话关闭后保留两分钟，源码变化时会启动新的进程。仅有归档的卡片保留原始尺寸和来源。原生适配器不提供此网页功能。

CLI 与 MCP 实时会话使用相同的测量方式。向已打开的会话发送带原始单元格的 `layout` 可读取视口尺寸、滚动范围、容器 ID 和观测到的元素树。`resize` 接受一个或两个轴，以及可选的 `container` ID。`sample` 接受 `axis`、整数 `min` 和 `max`（1–4096 CSS 像素），以及 2–32 次观测的 `budget`。修改请求使用当前 `expectedRevision` 和新的 `requestId`。采样会恢复原有尺寸约束，返回实测坐标、元素可见性、文本行数或兄弟元素对齐的变化以及未采样区间。证据范围限于采样帧和渲染器返回的元素。源归档及已验证覆盖率保持不变。

## 5. 模式与验收

| 模式 | 验收结果 |
|---|---|
| 边界解码与纯函数投影 | 读取新旧归档，地图与列表解析到相同格子。未知状态显示数据错误。三种判定及原因均由 Rust 提供。 |
| 功能内临时状态与共享选择 | 仅用键盘进入详情并返回。后退恢复焦点和选择，刷新保留已选宽度。 |
| 组合无障碍控件 | 检查已验证、未验证和不可渲染格子。在 375 和 1280 像素下均可读到状态与原因，加载占位图保留记录的判定。 |
| 单会话串行命令 | 用 CLI 和 MCP 编辑布尔值和枚举。检查独立控件、重复请求、过期编辑、断线和重置，同时核对回读值与可见证据。 |
| 内嵌客户端与不可变归档 | 从干净的检出安装。在目标应用停止时直接打开详情链接。缺失图像显示错误。保存实时状态后，新旧记录均可重新打开。 |
| 多语言捕获与查看器 i18n（[#12](https://github.com/de-novo/dui/issues/12)） | 根据清单 `locales` 或自动检测的消息文件（`locales/*.json` / `messages/*.json`）按 `screen × state × width × locale` 捕获。按语言区域汇总（`report.locales`）并报告缺失翻译键、文本溢出选择器与 RTL 不匹配。可在工作区侧栏将查看器界面语言切换为 `en`、`zh-CN`、`ko`，并按语言区域筛选归档格子（`npm run check:i18n`）。 |
| 捕获差异与像素差异叠加层（[#11](https://github.com/de-novo/dui/issues/11)） | 通过 `dui diff BEFORE AFTER [--overlay-dir DIR]`、MCP `diff` 与查看器的 `?view=diff` 视图（`/api/diff`）对比两次捕获。隐藏内容哈希一致的未变更格子，按界面、状态和宽度分组列出新增/移除/变更格子与变更源文件。已验证格子展示变更前后图像与像素差异叠加层，未验证或不可渲染格子仅展示状态变化而非像素差异（`npm run check:diff`）。 |
| 地图重新渲染 | `npm run check:viewer-renders` 映射一个带对话框的 24 页网站，在开发版查看器中打开，并统计每次交互实际运行的 React 组件。静止的地图不重新渲染任何内容。指针经过卡片时只重新渲染进入和离开的卡片的连线，选择卡片时只重新渲染选中或相关状态改变的卡片。无关连线和卡片的淡化由地图上的 CSS（`data-edge-focus`、`data-node-focus`）完成，因此不会重新渲染它们。 |
| 查看器与网站的自我验证 | `npm run verify:pages-demo` 用 `dui map --url` 映射所有已发布的演示页面和网站，并运行 `dui audit --design apps/canvas/audit/dui.json`。向用户显示机器或构建路径、手机上小于 24 × 24 CSS px 的独立触控目标（WCAG 2.2 SC 2.5.8）、没有焦点指示的 Tab 停留处（SC 2.4.7）、Tab 无法到达的控件（SC 2.1.1）、Escape 无法关闭或丢失焦点的浮层（SC 2.4.3），或任何未验证单元都会导致失败。 |

实施顺序为归档和模式夹具 → 路由与可键盘操作的列表 → 地图 → 实时查询与编辑 → 打包和测量。使用 npmgraph 的两个真实界面，以及包含两个独立控件的可再分发夹具。测量 30 次热编辑，期间宿主重建和浏览器重启均为零。分别报告冷启动时间与热编辑的 p50、p95。[shadcn/ui](../../.agents/skills/shadcn/SKILL.md)
