# dui 目标语言与 UI 技术栈全面调研 v0

[English](../en/language-survey.md) | 简体中文 | [한국어](../ko/language-survey.md)

> 本文是[英文原文](../en/language-survey.md)的简体中文翻译。如有出入，以英文为准。

> 编写于 2026-11-19。目的：有依据地选择 dui 要支持的语言和框架。
> 只列出有来源的数字。无法确认的标记为 `未确认`。star 数为 2026-11-19 的 GitHub API 数值。
> 这里的许可证是**目标框架**的许可证，与 dui 的依赖政策（dui 是 Apache-2.0 开源项目，依赖必须允许商用）无关。dui 只是在用户项目中插入开发用代码，并不再分发这些框架。

## 0. dui 对每个技术栈的 4 项要求

| 标准 | 问题 |
|---|---|
| 类型 | 能否从类型中得到全部状态形态？ |
| 强制注入 | 能否在构建时改写状态所在位置（包括本地状态），从外部注入值？ |
| 网络拦截 | 能否把 API 响应替换为模拟数据？ |
| 无窗口渲染 | 能否把注入状态后的界面输出为图像（可用于 CI、可并行）？ |

难度综合以上四项：**低** = 现有工具全部能做到，**中** = 需要我们自己做一两项，**高** = 某个核心环节受阻。

## 1. 使用规模（语言）

来源 A：[Stack Overflow Developer Survey 2025，技术 > 语言](https://survey.stackoverflow.co/2025/technology) —— 全部受访者中"过去一年大量使用该语言"的比例。
来源 B：[GitHub Octoverse 2025](https://github.blog/news-insights/octoverse/octoverse-a-new-developer-joins-github-every-second-as-ai-leads-typescript-to-1/) —— 以 2025 年 8 月月度贡献者数计。

| 语言 | SO 2025 使用率 (A) | Octoverse 2025 (B) | 类型强度 |
|---|---|---|---|
| TypeScript | 43.6% | **第 1**，月贡献者 2,636,006 人，同比 +66.6% | 静态，结构化。可被 `any` 绕过 |
| Java | 29.4% | 第 4 | 静态，强 |
| C# | 27.8% | 第 5 | 静态，强 |
| C++ | 23.5% | 未确认 | 静态，模板和宏使分析困难 |
| Go | 16.4% | 未确认 | 静态，简单 |
| Rust | 14.8% | 未确认 | 静态，非常强（enum、模式匹配） |
| Kotlin | 10.8% | 未确认 | 静态，强（sealed class、空安全） |
| Dart | 5.9% | 未确认 | 静态，sound null safety |
| Swift | 5.4% | 未确认 | 静态，强（enum、可选类型） |

参考：JavaScript 66%、Python 57.9% 是无类型语言，不在范围内（设计 v0 的非目标）。
注意：SO 调查中 Web 和后端开发者占比高，移动端语言（Swift、Kotlin、Dart）可能低于实际。移动端专门的使用规模数据 `未确认`（需进一步核实，例如 JetBrains State of Developer Ecosystem）。

UI 框架使用率（SO 2025 Web 框架，全部受访者）：React 44.7%、Next.js 20.8%、Angular 18.2%、Vue.js 17.6%、Svelte 7.2%、Blazor 7.0%。移动和桌面框架（Flutter、React Native、SwiftUI、Compose、Electron 等）的 2025 使用率 `未确认`（在该页面上未找到相应条目）。

## 2. 各技术栈调研

### 2.1 TypeScript

| 技术栈 | ★（GitHub） | 状态所在位置 | 强制注入方式 | 网络 | 无窗口渲染 | 现有预览 | 难度 |
|---|---|---|---|---|---|---|---|
| React（+Vite/Next） | react 252,767 | `useState`、`useReducer`、context、store（Zustand/Redux）、查询缓存、URL | Babel/SWC 转换（Vite 插件） | MSW（Service Worker / Node） | Playwright 无头 | Storybook | **低** |
| Vue 3 | vuejs/core 55,045 | `ref`/`reactive`、Pinia、URL | Vite 插件（SFC 编译阶段） | MSW | Playwright | Storybook | 低 |
| Svelte 5 | 89,756 | runes（`$state`）、store | 编译器预处理 | MSW | Playwright | Storybook | 低 |
| Solid | 36,760 | signal、store | Babel 转换 | MSW | Playwright | Storybook | 低 |
| Angular | 101,461 | 组件字段、signal、service（DI） | 通过 DI 替换 service + 构建转换 | `HttpClient` 拦截器 / MSW | Playwright | Storybook | 中（有自己的构建链） |
| React Native | react-native 126,964（Expo 53,662） | 与 React 相同 | Babel（Metro）转换 | JS fetch 模拟 / MSW native | **需要模拟器**。可用 react-native-web 绕过，但可能与真实情况不同 | Storybook RN | 中 |
| Lynx | 15,264 | React 系（ReactLynx） | 未确认 | 未确认 | 未确认 | 未确认 | 未确认 |
| Electron（WebView） | 123,999 | 渲染进程与 Web 相同 + IPC | 与 Web 相同，模拟 preload/IPC | MSW | Playwright（支持 Electron） | Storybook | 低 |
| Tauri（WebView） | 112,993 | Web + `invoke` | 与 Web 相同，模拟 `invoke` | MSW | 在浏览器中渲染 | Storybook | 低 |
| Ionic/Capacitor | ionic 52,763 | 与 Web 相同 | 与 Web 相同 | MSW | Playwright | Storybook | 低 |

### 2.2 Swift（Apple）

| 技术栈 | 状态所在位置 | 强制注入方式 | 网络 | 无窗口渲染 | 现有预览 | 难度 |
|---|---|---|---|---|---|---|
| SwiftUI | `@State`、`@Observable` 模型、`@Environment` | Swift 宏只能添加新代码，不能修改已有 `@State` 的初始值 → **在开发构建前用 SwiftSyntax 改写源码**（SwiftPM build plugin）。`@Observable` 模型通过替换创建处注入 | 注册 `URLProtocol` 拦截 | `ImageRenderer`（iOS 16 / macOS 13+）、[swift-snapshot-testing](https://github.com/pointfreeco/swift-snapshot-testing)（MIT，★4,397）。**需要 macOS + Xcode 机器** | `#Preview` | 中 |
| UIKit / AppKit | 分散在视图控制器字段中，命令式 | 可以改写源码，但状态分散导致准确度低 | `URLProtocol` | 快照测试（上述库），可能需要模拟器 | Xcode Preview（经由 UIViewRepresentable） | 高 |

### 2.3 Kotlin / Java（Android、JVM）

| 技术栈 | ★ | 状态所在位置 | 强制注入方式 | 网络 | 无窗口渲染 | 现有预览 | 难度 |
|---|---|---|---|---|---|---|---|
| Jetpack Compose（Android） | 未确认 | `remember { mutableStateOf }`、ViewModel `StateFlow` | **Kotlin 编译器插件**（Compose 本身就是编译器插件）。插件 API 的官方稳定性未确认 | OkHttp 拦截器 / MockWebServer | 在 JVM 上无需设备：[Paparazzi](https://github.com/cashapp/paparazzi)（Apache-2.0，★2,641）、[Roborazzi](https://github.com/takahirom/roborazzi)（Apache-2.0，★1,084）、[Compose Preview Screenshot Testing](https://developer.android.com/studio/preview/compose-screenshot-testing)（实验阶段；独立插件已停止维护，正迁移到 AGP 9.5+ test suite） | `@Preview` | 中 |
| Compose Multiplatform | 19,488 | 同上 | 同上 | Ktor 客户端模拟引擎 | 据了解有桌面离屏渲染（`ImageComposeScene`），文档未确认 | `@Preview` | 中 |
| Android Views（Kotlin/Java） | — | View、Fragment 字段，XML | 字节码转换（AGP Instrumentation / ASM）—— 比源码层面更难 | OkHttp 拦截器 | Paparazzi、Roborazzi 也支持 View | Layout Editor | 高 |
| Swing / JavaFX | jfx 3,310（GPL-2.0） | 组件字段 | 注解处理器只能生成新文件 → 需要字节码转换 | 替换 `HttpClient` | Swing 可绘制到 `BufferedImage`，JavaFX 无头（Monocle）未确认 | 无 | 高 |

### 2.4 Dart（Flutter）

| 技术栈 | ★ | 状态所在位置 | 强制注入方式 | 网络 | 无窗口渲染 | 现有预览 | 难度 |
|---|---|---|---|---|---|---|---|
| Flutter | 181,293（BSD-3） | `State<T>` 字段、Provider/Riverpod/Bloc | Dart 宏已于 [2025-01 停止开发](https://dart.dev/blog/an-update-on-dart-macros-data-serialization)。`build_runner` 只能生成新文件 → 需要**在开发构建前加一步源码改写**。Riverpod、Bloc 等可通过 override 轻松注入 | `HttpOverrides`、http `MockClient`、Dio 拦截器 | **golden test**（`flutter_test`，在主机上无需设备） | [Widget Previewer](https://docs.flutter.dev/tools/widget-previewer) `@Preview`（3.35 实验 → 据文档 3.47 稳定） | 中 |

### 2.5 C#（.NET）

| 技术栈 | ★ | 状态所在位置 | 强制注入方式 | 网络 | 无窗口渲染 | 现有预览 | 难度 |
|---|---|---|---|---|---|---|---|
| Avalonia | 31,847 | MVVM ViewModel（`INotifyPropertyChanged`） | Source Generator（只能添加）+ [interceptors](https://github.com/dotnet/roslyn/blob/main/docs/features/interceptors.md)（替换调用点；二手资料称自 .NET 9.0.2xx SDK 起稳定，原始来源未确认） | 替换 `HttpMessageHandler` | Avalonia.Headless（文档未确认） | XAML 预览 | 中 |
| WPF | 7,758 | ViewModel | 同上 | 同上 | `RenderTargetBitmap`，**仅限 Windows** | XAML 设计器 | 中 |
| WinUI 3 | microsoft-ui-xaml 8,618 | ViewModel | 同上 | 同上 | 未确认 | XAML Hot Reload | 高 |
| .NET MAUI | 23,322 | ViewModel | 同上 | 同上 | 需要设备 / 模拟器，无头未确认 | XAML Hot Reload | 高 |
| Uno Platform | 10,125 | ViewModel | 同上 | 同上 | 未确认 | 未确认 | 未确认 |
| Blazor | aspnetcore 38,533 / SO 使用率 7.0% | 组件字段、cascading 值 | Source Generator / 渲染树是 DOM | 替换 `HttpClient` | Playwright（结果是 Web） | bUnit（测试） | 中 |

### 2.6 Rust

| 技术栈 | ★ | 状态所在位置 | 强制注入方式 | 网络 | 无窗口渲染 | 现有预览 | 难度 |
|---|---|---|---|---|---|---|---|
| **GPUI**（pippip） | zed 95,318（gpui crate Apache-2.0） | `Entity<T>` 模型、View 字段 | proc-macro + 开发 feature | 替换 HTTP 客户端（reqwest） | **代码中存在**：已确认 pippip 使用的 `gpui-pre 0.3.12` 源码中有 `HeadlessAppContext::capture_screenshot`、`Window::render_to_image`（`test-support` feature）。实际运行未确认 | 无 | 中 |
| egui | 32,385 | 应用结构体字段（immediate mode） | proc-macro | 替换客户端 | [egui_kittest](https://docs.rs/egui_kittest) `image_snapshot`，wgpu 无头 | 无 | 低～中 |
| Iced | 32,080 | Elm 式单一 State + Message | 状态集中在一处，注入容易 | 替换客户端 | `iced_test` 模拟器快照，用 tiny-skia 无需 GPU | 无 | 低～中 |
| Slint | 24,644（自有许可证，GitHub 显示 NOASSERTION） | `.slint` DSL 属性 + Rust 模型 | DSL 属性可从 Rust 设置 | 替换客户端 | 测试后端 `take_snapshot`、`SLINT_BACKEND=headless-software` | Slint 实时预览 | 中 |
| Dioxus | 39,850 | signal、hooks | proc-macro / RSX 转换 | 替换客户端 | Web 和桌面（WebView）用 Playwright，原生（Blitz）未确认 | 无 | 中 |
| Leptos | 21,560 | signal | proc-macro | 替换客户端 | 结果是 Web → Playwright | 无 | 中 |
| Tauri（Rust 侧） | 112,993 | 界面是 Web | 在 Web（TS）侧处理 | 模拟 `invoke` | Playwright | Storybook | 低 |

### 2.7 C++ / Go

| 技术栈 | ★ | 状态所在位置 | 强制注入方式 | 网络 | 无窗口渲染 | 现有预览 | 难度 |
|---|---|---|---|---|---|---|---|
| Qt Widgets / QML | 官方仓库不在 GitHub 上（★ 未确认） | QObject 属性，QML 具有 JS 特性 | 可以用 Clang 工具改写源码，但很重。QML 是动态的 | 替换 `QNetworkAccessManager` | `-platform offscreen` + `grabToImage` | Qt Design Studio | 高 |
| Fyne（Go） | 28,850 | 组件字段、binding | Go 没有宏 → `go generate` / 源码改写 | 替换 `http.RoundTripper` | 测试包中的软件渲染（未确认） | 无 | 高 |
| Wails（Go） | 37,160 | 界面是 Web | 在 Web（TS）侧处理 | 模拟绑定 | Playwright | Storybook | 低 |

## 3. 现有预览工具与 dui 的区别

| 工具 | 做什么 | 与 dui 相比缺少什么 |
|---|---|---|
| Storybook（★91,647） | 按状态把组件渲染成 story，Web 系 | 状态需要**人工逐个编写**。没有转换图。没有覆盖率概念 |
| SwiftUI `#Preview` | 在 Xcode 中预览视图 | 同上 + 只在 Xcode 内，没有画布和宽度矩阵 |
| Compose `@Preview`（+ 截图测试） | 把预览变成图像 / 回归测试 | 状态需手动，包含 ViewModel 的整个界面很难 |
| Flutter Widget Previewer | 按尺寸、主题预览 `@Preview` 组件 | 手动编写，没有转换图 |
| XAML 设计器 / Qt Design Studio | 布局预览 | 不是真实数据状态 |

共同的空白：**自动发现状态 + 强制注入本地状态 + 转换图 + 覆盖率数字**。这四点是 dui 的核心；各生态已有的渲染、快照工具则作为 dui 的渲染工作进程复用。

## 4. 推荐顺序

评分 = 市场规模 × 可行性 × 我们自己项目的需要。

| 排名 | 对象 | 阶段 | 理由 |
|---|---|---|---|
| 1 | **TS Web：React（+Vite/Next）** | MVP | 有类型语言使用量第 1（Octoverse 2025），React 44.7%（SO 2025）。注入、拦截、渲染工具齐全（难度低）。obora 在这里 |
| 2 | **Electron / Tauri WebView** | MVP | 用与第 1 项相同的引擎，几乎零成本。obora-desktop 是 Electron |
| 3 | **Rust GPUI** | MVP（spike） | pippip 需要。已确认代码中有无头渲染 API，最大风险降低。市场小，但用于我们自己的验证 |
| 4 | Vue、Svelte、Solid、Angular，Blazor、Leptos、Dioxus Web | 下一步 | 渲染是 Web，可复用 Playwright、MSW。只需新增注入转换 |
| 5 | **Kotlin Compose**（Android + Multiplatform） | 下一步 | Kotlin 10.8%（SO 2025）。在 JVM 上无需设备即可渲染（Paparazzi、Roborazzi），状态集中在 `remember` / ViewModel。编译器插件是主要成本 |
| 6 | **Flutter** | 下一步 | ★181,293，跨平台规模最大。用 golden test 在主机渲染很容易。宏已停止 → 源码改写步骤是成本 |
| 7 | **SwiftUI** | 下一步 | iOS 市场大，但必须有 macOS + Xcode 运行器，`@State` 注入需要源码改写 |
| 8 | React Native | 之后 | 注入容易，但真实原生渲染需要模拟器 |
| 9 | C# Avalonia / WPF | 之后 | MVVM 使状态集中，有利；可用 interceptors 注入。确认企业桌面需求之后 |
| 10 | egui / Iced / Slint | 之后 | 无头渲染工具已经很好。市场小，有需求时很容易做 |
| 11 | UIKit/AppKit、Android Views、Swing/JavaFX、MAUI、WinUI、Qt、Fyne | 暂缓 | 状态分散的命令式 UI，或无头渲染受阻（难度高）。与 99% 目标不符 |

结论：**语言先固定为 "TS + Rust"，但设计上为 Kotlin、Dart、Swift 预留为下一批目标。** 这三种语言都有强类型和无需设备的渲染路径，dui 的方法可行。卡住的是注入步骤（编译器插件 / 源码改写），因此把核心的"注入点 → 转换"契约设计成与语言无关即可。

## 5. 待确认

- 移动端语言、框架的使用规模数据（JetBrains State of Developer Ecosystem 等）：未确认。
- Kotlin 编译器插件 API 的稳定性、C# interceptors 稳定时间的原始来源：未确认。
- Compose Desktop `ImageComposeScene`、Avalonia.Headless、Fyne 测试渲染：文档未确认。
- 在 pippip 上实际运行 GPUI 无头截图：未确认（M2 spike）。
