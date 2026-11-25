[English](../en/supported-frameworks.md) | 简体中文 | [한국어](../ko/supported-frameworks.md)

# dui 可以分析和渲染什么

此表描述 `main` 开发版。默认安装请先运行[经过验证的首次捕获](../../README.zh-CN.md#first-run)，并在选择开发版命令前确认显示的版本。

提取类型时，dui 查询项目自己的官方编译器。绘制界面地图时，也可以通过平台渲染器观察运行中的应用。同一项目可以单独或同时使用这两条路径。[架构](architecture.md) · [界面地图约定](web-viewer.md)

| 任务 | 输入与结果 | 验证依据 |
|---|---|---|
| 提取有类型的 UI 状态 | `dui scan` 读取项目的 TypeScript 程序。规则匹配特定符号，根据编译器提供的类型提取状态槽、路由参数等信息。 | [提取器](../../adapters/typescript/facts.mts) · [规则检查](../../scripts/check-p2-rules.mts) |
| 绘制可访问的网页 | 向 `dui map` 提供本地 URL（`--url`）或静态构建目录（`--build-dir`）。Chromium 沿同源链接访问页面，按指定宽度捕获。运行中的 Angular 应用可以使用 URL。静态构建路径使用 Vue Router、SvelteKit、Astro 和 Preact 的构建产物进行检查。 | [URL 检查](../../scripts/check-map-url.mts) · [构建产物检查](../../scripts/check-web-frameworks.mts) |
| 查找原生界面声明 | `dui map` 用项目的 TypeScript 工具读取 React Navigation 和 Expo Router 声明，用 Dart 分析器读取 Flutter 路由。每张卡片指向源码位置。此步骤无需设备，捕获尝试次数为零。 | [发现适配器](../../adapters/native-map) · [开源应用检查](../../scripts/check-native-map.mts) |
| 从源码预览原生组件 | 导出工具提供有限的 UI 基元，使用 `swiftc` 或 `kotlinc` 编译 SwiftUI 或 Compose 组件。在地图上选择对应的卡片即可查看观察到的状态并操作开关。也可导出独立的 HTML 文件。卡片会显示无法预览的原因。 | [导出工具](../../scripts/source-preview.mts) · [交互检查](../../scripts/check-source-preview.mts) · [地图工作流](web-viewer.md#portable-source-map) |
| 捕获原生移动界面 | Android 使用已连接的设备或模拟器。iOS 使用安装在 macOS 模拟器上的应用构建。用 `dui map attach` 将录制的 PNG 关联到源码卡片。导入的格子保持未验证（`unverified`），已验证覆盖率不增加。 缺少捕获所需的运行环境时，相应格子标为无法渲染（`unrenderable`）并记录原因。 | [Compose 模拟器检查](../../scripts/check-compose.mts) · [iOS 模拟器检查](../../.github/workflows/ios-simulator.yml) · [录制关联检查](../../scripts/check-native-map-recordings.mts) |
| 捕获桌面或终端界面 | GPUI 示例渲染已声明的状态并记录元素树。Electron 宿主通过 Chromium 捕获 Web 渲染进程的内容，可配置 IPC 模拟。命令在伪终端运行后得到字符网格和 PNG。 | [GPUI](../../scripts/check-gpui.mts) · [Electron](../../scripts/check-electron.mts) · [终端](../../scripts/check-tui.mts) |

## 库规则的作用

例如，TypeScript 规则将 Zustand 的 `create` 识别为状态存储，将 Zod 的 `object` 识别为模式。各次调用的类型由编译器提供。仅凭安装了某个包，无法判断界面使用了哪个状态。内置规则列出了具体的包和符号，项目可以通过 `rulePacks` 添加自己的规则。[内置规则](../../rules/web.json) · [清单参考](../reference.md)

网页捕获取决于浏览器可以访问哪些页面。即使缺少模板语言的规则，Chromium 可以访问的页面也能出现在地图上。枚举隐藏路由、读取组件输入或根据类型规划状态，都需要相应的源码依据。需要登录或没有入站链接的页面可能需要提供会话或显式入口。[界面地图约定](web-viewer.md)

## 编译器与宿主要求

设计以官方 TypeScript 7 为目标。当前 CLI 也为官方稳定版 6.x 项目提供兼容路径。运行 `dui check-ts PROJECT` 可检查项目的编译器。TypeScript 扫描会检查已安装的 React 和 React DOM 稳定版 18+、Vite 7+，在使用 Next 宿主且已安装 Next.js 时检查 16+。规则所用库的版本可能产生诊断提示，扫描会继续。[编译器检查](../../crates/dui/src/main.rs) · [扫描版本检查](../../adapters/typescript/facts.mts)

`dui toolchains` 报告已安装的 `tsc`、`rustc`、`go`、`swiftc`、`kotlinc`、`dart` 版本。探测成功说明工具可用。上表分别列出了事实提取、界面发现和渲染的验证依据。后续工作见路线图。[工具链探测](../../crates/dui-app/src/compiler_oracle.rs) · [路线图](roadmap.md)
