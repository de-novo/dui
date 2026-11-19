[English](../en/supported-frameworks.md) | 简体中文 | [한국어](../ko/supported-frameworks.md)

# 支持的框架

下面优先级 1–3 中的全部 38 个技术栈都是正式支持目标。优先级只表示实现顺序，不是支持等级。“暂不支持”中的 11 个技术栈目前为 `不支持`，每项都写明原因和加入条件。

版本于 2026-11-20（KST）按每行链接的官方来源核对。无法核对的内容标为 `未确认`。版本规则：[设计：支持的版本](design-v0.md#支持的版本只支持当前最新稳定主版本)。

## 采用的原则

1. **只用官方编译器。** 类型来自项目已在使用的编译器（`tsc`、`rustc`、`swiftc`、`kotlinc`、`dart`）。
2. **只支持当前最新稳定主版本。** 每个技术栈一个基线。更新的版本始终支持；只有 dui 发布新主版本时才上调基线。对 0.x 项目，“主版本”指 0.minor 版本线。
3. **只支持类型化语言。**
4. **明确说明渲染限制。** Apple 目标需要 Mac 才能渲染。某些技术栈可以分析但无法渲染；这些单元报告为 `无法渲染`。
5. **AI 原生。** AI 驱动 CLI 和 MCP 服务器；人只审阅画布。

## 汇总

| 优先级 | 数量 | 技术栈 |
|---|---:|---|
| 优先级 1（MVP） | 8 | TypeScript, Node.js, React, Vite, TanStack Router, TanStack Query, zod, Chromium (Chrome stable) |
| 优先级 2 | 15 | Rust, GPUI (`gpui-pre`), gpui-component / gpui-kit, Zustand, Redux Toolkit, Jotai, XState, React Hook Form, valibot, Next.js, React Router, TanStack Start, Electron, Radix UI, shadcn/ui |
| 优先级 3 | 15 | Solid, SolidStart, Preact, Rspack / Rsbuild, Tauri, Dioxus, Leptos, egui, Iced, React Native, Expo, Flutter / Dart, SwiftUI / Swift, Jetpack Compose / Kotlin, Compose Multiplatform |
| 暂不支持 | 11 | Vue, Nuxt, Svelte, SvelteKit, Astro, Angular, Qwik, Remix, Turbopack, Slint, Playwright |

## 优先级 1（MVP）

| 技术栈 | 领域 | 支持的主版本 | 最新稳定版（已核对） | 发布日期 | 来源 | dui 支持它的原因 | 备注 | 待定问题 |
|---|---|---|---|---|---|---|---|---|
| **TypeScript** | 语言/运行时 | 7.x | 7.0.2 | 2026-07-08 | [www.npmjs.com](https://www.npmjs.com/package/typescript) | 主要类型化语言；官方 `tsc` 7 提供 dui 所需的类型信息。 | TS 适配器使用的 7.1（sync API）仍是 beta。 | — |
| **Node.js** | 语言/运行时 | 26.x | 26.13.1 (LTS) | 2026-11-10 | [nodejs.org](https://nodejs.org/en/about/previous-releases) | 运行 TS sidecar 和用户的开发服务器。 | — | — |
| **React** | Web UI | 19.x | 19.3.1 | 2026-10-14 | [www.npmjs.com](https://www.npmjs.com/package/react) | 最常见的类型化 UI 技术栈；第一个渲染目标。 | — | — |
| **Vite** | 打包器/开发服务器 | 8.x | 8.3.8 | 2026-11-11 | [www.npmjs.com](https://www.npmjs.com/package/vite) | dui 需要提供注入后的开发构建；Vite 是常见的 React 开发服务器。 | — | — |
| **TanStack Router** | 路由 | 1.x | 1.170.46 | 2026-10-22 | [www.npmjs.com](https://www.npmjs.com/package/@tanstack/react-router) | 类型化路由可直接枚举页面。 | — | — |
| **TanStack Query** | 状态/数据 | 5.x | 5.108.1 | 2026-11-19 | [www.npmjs.com](https://www.npmjs.com/package/@tanstack/react-query) | 服务端状态（加载/错误/空）是状态单元的核心。 | — | — |
| **zod** | 状态/数据 | 4.x | 4.6.7 | 2026-10-12 | [www.npmjs.com](https://www.npmjs.com/package/zod) | schema 提供真实的示例数据与取值范围。 | — | — |
| **Chromium (Chrome stable)** | 渲染/测试基础设施 | 不固定 | 158.0.8127.36 | 2026-11-17 | [chromiumdash.appspot.com](https://chromiumdash.appspot.com/releases?platform=Linux) | 通过 CDP 渲染 Web 页面的无头目标。 | — | — |

## 优先级 2

| 技术栈 | 领域 | 支持的主版本 | 最新稳定版（已核对） | 发布日期 | 来源 | dui 支持它的原因 | 备注 | 待定问题 |
|---|---|---|---|---|---|---|---|---|
| **Rust** | 语言/运行时 | edition 2024 | 1.100.0 (toolchain) | 2026-11-12 | [github.com](https://github.com/rust-lang/rust/releases) | 第二种官方编译器语言（`rustc` + rust-analyzer）。 | edition 2024 内的最低 toolchain 仍未定。 | 是 |
| **GPUI (`gpui-pre`)** | Rust UI | 0.3.x | 0.3.13 | 2026-11-16 | [crates.io](https://crates.io/crates/gpui-pre) | Web 之后计划的原生渲染目标。 | 1.0 之前：每个 0.x 次版本都可能不兼容。 | — |
| **gpui-component / gpui-kit** | UI 组件库 | 0.10.x | 0.10.0 | 2026-11-16 | [crates.io](https://crates.io/crates/gpui-component) | GPUI 上的主要组件库；状态经由其组件体现。 | 以两个 crate 名发布（`gpui-component`、`gpui-kit`），均为 0.10.0，同一仓库。 | — |
| **Zustand** | 状态/数据 | 5.x | 5.1.0 | 2026-11-06 | [www.npmjs.com](https://www.npmjs.com/package/zustand) | 常见客户端 store；store 结构决定 UI 状态。 | — | — |
| **Redux Toolkit** | 状态/数据 | 2.x | 2.15.0 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/@reduxjs/toolkit) | 类型化 slice 显式列出状态。 | — | — |
| **Jotai** | 状态/数据 | 3.x | 3.0.2 | 2026-11-09 | [www.npmjs.com](https://www.npmjs.com/package/jotai) | 原子级状态；React 应用中常见。 | 新主版本 3.0.0 于 2026-09-08 发布。 | — |
| **XState** | 状态/数据 | 5.x | 5.33.4 | 2026-11-07 | [www.npmjs.com](https://www.npmjs.com/package/xstate) | 状态机按定义列出所有状态。 | — | — |
| **React Hook Form** | 状态/数据 | 7.x | 7.92.0 | 2026-11-11 | [www.npmjs.com](https://www.npmjs.com/package/react-hook-form) | 表单校验/提交中/错误状态。 | — | — |
| **valibot** | 状态/数据 | 1.x | 1.5.0 | 2026-09-09 | [www.npmjs.com](https://www.npmjs.com/package/valibot) | 在示例数据方面与 zod 作用相同。 | — | — |
| **Next.js** | 元框架 | 16.x | 16.4.6 | 2026-11-09 | [www.npmjs.com](https://www.npmjs.com/package/next) | 使用最多的 React 元框架。 | Server Components 在服务端渲染；在那里注入状态的方式未确认。 | 是 |
| **React Router** | 元框架 | 8.x | 8.5.2 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/react-router) | 常见路由；framework 模式是 Remix 的后继。 | 新主版本 8.0.0 于 2026-06-17 发布。 | — |
| **TanStack Start** | 元框架 | 1.x | 1.169.7 | 2026-11-19 | [www.npmjs.com](https://www.npmjs.com/package/@tanstack/react-start) | 基于 MVP 已支持的 TanStack Router。 | — | — |
| **Electron** | 桌面外壳 | 45.x | 45.4.1 | 2026-11-19 | [www.npmjs.com](https://www.npmjs.com/package/electron) | UI 为 Web 的桌面应用；走同一条 Chromium 渲染路径。 | 大约每 8 周一个新主版本，基线变化频繁。 | 是 |
| **Radix UI** | UI 组件库 | 1.x | 1.8.1 | 2026-10-12 | [www.npmjs.com](https://www.npmjs.com/package/radix-ui) | 许多 React 组件库背后的无样式原语；开/关状态。 | — | — |
| **shadcn/ui** | UI 组件库 | 4.x (CLI) | 4.23.0 | 2026-10-27 | [www.npmjs.com](https://www.npmjs.com/package/shadcn) | 非常常见的 React 组件来源；代码位于用户仓库。 | 被复制进仓库，并非运行时库；“支持”可能只是文档。 | 是 |

## 优先级 3

| 技术栈 | 领域 | 支持的主版本 | 最新稳定版（已核对） | 发布日期 | 来源 | dui 支持它的原因 | 备注 | 待定问题 |
|---|---|---|---|---|---|---|---|---|
| **Solid** | Web UI | 2.x | 2.0.3 | 2026-11-19 | [www.npmjs.com](https://www.npmjs.com/package/solid-js) | 类型化 JSX；细粒度 signal。 | 主版本 2.0.0 于 2026-11-17 发布；生态可能滞后。 | — |
| **SolidStart** | 元框架 | 2.x | 2.2.3 | 2026-11-16 | [www.npmjs.com](https://www.npmjs.com/package/@solidjs/start) | Solid 的元框架。 | npm license 字段为空（许可证未确认）。 | — |
| **Preact** | Web UI | 11.x | 11.2.0 | 2026-11-06 | [www.npmjs.com](https://www.npmjs.com/package/preact) | 与 React 兼容的 API；大部分 React 工作可复用。 | — | — |
| **Rspack / Rsbuild** | 打包器/开发服务器 | 2.x | 2.3.3 | 2026-11-17 | [www.npmjs.com](https://www.npmjs.com/package/@rspack/core) | webpack 风格项目的第二条开发服务器路径。 | 每多一个开发服务器就多一条需维护的注入路径。 | — |
| **Tauri** | 桌面外壳 | 2.x | 2.12.7 (crate) / 2.12.5 (@tauri-apps/api) | 2026-11-18 | [crates.io](https://crates.io/crates/tauri) | Rust + Web 桌面；契合 Rust 核心。 | 使用系统 webview（macOS/Linux 上为 WebKit），不是 Chromium；渲染一致性未确认。 | — |
| **Dioxus** | Rust UI | 0.7.x | 0.7.10 | 2026-07-30 | [crates.io](https://crates.io/crates/dioxus) | 面向 Web/桌面的 Rust UI。 | 1.0 之前。 | — |
| **Leptos** | Rust UI | 0.8.x | 0.8.23 | 2026-10-31 | [crates.io](https://crates.io/crates/leptos) | Rust Web UI；在 Chromium 中渲染。 | 1.0 之前。 | — |
| **egui** | Rust UI | 0.37.x | 0.37.0 | 2026-10-15 | [crates.io](https://crates.io/crates/egui) | 下载量最高的 Rust 即时模式 UI。 | 即时模式：状态不在组件树中，寻找状态可能需要不同方法。 | 是 |
| **Iced** | Rust UI | 0.14.x | 0.14.0 | 2025-12-07 | [crates.io](https://crates.io/crates/iced) | Elm 风格 Rust UI；消息枚举状态。 | 自 2025-12-07 以来没有新的稳定版。 | 是 |
| **React Native** | 移动/原生 | 0.88.x | 0.88.1 | 2026-10-27 | [www.npmjs.com](https://www.npmjs.com/package/react-native) | 移动端 React；复用 React 分析。 | 仍为 0.x，“主版本”只能按 0.minor 定义；需要模拟器（iOS 需要 Mac）。 | 是 |
| **Expo** | 移动/原生 | 58.x | 58.0.26 | 2026-11-17 | [www.npmjs.com](https://www.npmjs.com/package/expo) | 最常见的 React Native 应用起步方式。 | 与 React Native 相同的渲染限制。 | 是 |
| **Flutter / Dart** | 移动/原生 | Flutter 3.x / Dart 3.x | Flutter 3.50.0 (Dart 3.14.0); Dart SDK 3.14.1 | Flutter 2026-11-11; Dart 2026-11-17 | [docs.flutter.dev](https://docs.flutter.dev/release/archive) | 类型化，官方编译器（Dart）；移动端占比大。 | 需要新的 Dart 语言适配器；无头渲染路径未确认。 | 是 |
| **SwiftUI / Swift** | 移动/原生 | Swift 6.x | 6.4.2 | 2026-10-30 | [github.com](https://github.com/swiftlang/swift/releases) | 官方编译器 `swiftc` + SourceKit-LSP；苹果原生 UI。 | 渲染需要 Mac（Xcode/模拟器）；类型分析可在其他平台运行。 | 是 |
| **Jetpack Compose / Kotlin** | 移动/原生 | Kotlin 2.x; Compose UI 1.x | Kotlin 2.4.21; Compose UI 1.13.0 (BOM 2026.11.01) | Kotlin 2026-10-08; Compose: `未确认` | [developer.android.com](https://developer.android.com/jetpack/androidx/releases/compose-ui) | 官方编译器 `kotlinc`；安卓原生 UI。 | 需要 Kotlin 适配器和 Android/JVM 渲染路径。 | 是 |
| **Compose Multiplatform** | 移动/原生 | 1.x | 1.12.2 | 2026-10-15 | [github.com](https://github.com/JetBrains/compose-multiplatform/releases) | 同一个 Kotlin 适配器；桌面目标在 JVM 上渲染。 | 取决于上面的 Kotlin 决定。 | 是 |

## 暂不支持

**规则：** 本表中的技术栈在阻碍消除后立即加入，例如其官方工具接受 TypeScript 7 时。在此之前，dui 在扫描前将其拒绝为 `不支持`（原因）。

Vue、Nuxt、Svelte、SvelteKit、Astro 和 Angular：它们的官方类型检查工具还不接受 TypeScript 7（npm peer 范围，2026-11-20 核对）。

| 技术栈 | 领域 | 最新主版本 | 最新稳定版（已核对） | 发布日期 | 来源 | 阻碍 | 原因 | 何时加入 |
|---|---|---|---|---|---|---|---|---|
| **Vue** | Web UI | 3.x | 3.5.45 | 2026-11-05 | [www.npmjs.com](https://www.npmjs.com/package/vue) | TS 7 工具 | 模板类型检查经由 `vue-tsc`；TS 7 支持未确认，与 TS 7+ 冲突。 | 其官方工具接受 TypeScript 7。 |
| **Nuxt** | 元框架 | 4.x | 4.6.4 | 2026-11-02 | [www.npmjs.com](https://www.npmjs.com/package/nuxt) | TS 7 工具 | 基于 Vue；跟随 Vue 的状态。 | Vue 被加入。 |
| **Svelte** | Web UI | 5.x | 5.58.0 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/svelte) | TS 7 工具 | `svelte-check` 4.7.8 的 TS peer 为 ^5 \|\| ^6，不含 7。 | 其官方工具接受 TypeScript 7。 |
| **SvelteKit** | 元框架 | 3.x | 3.4.0 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/@sveltejs/kit) | TS 7 工具 | TS peer 为 ^6.0.0，不含 7。 | 其官方工具接受 TypeScript 7。 |
| **Astro** | 元框架 | 7.x | 7.4.13 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/astro) | TS 7 工具 | `@astrojs/check` 的 TS peer 为 ^5 \|\| ^6，不含 7。 | 其官方工具接受 TypeScript 7。 |
| **Angular** | Web UI | 22.x | 22.3.0 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/@angular/core) | TS 7 工具 | `@angular/compiler-cli` 22.3.0 要求 TS >=6.0 <6.1，与 TS 7+ 冲突。 | 其官方工具接受 TypeScript 7。 |
| **Qwik** | Web UI | 1.x (v2 in RC) | 1.20.4 | 2026-11-09 | [www.npmjs.com](https://www.npmjs.com/package/@builder.io/qwik) | 主版本仍为 RC | v2（`@qwik.dev/core`）为 2.0.0-rc.5；现在支持 1.x 意味着基线很快过时。 | v2 稳定（待定问题）。 |
| **Remix** | 元框架 | 2.x (legacy) | 2.17.5 (@remix-run/react) | 2026-06-01 | [www.npmjs.com](https://www.npmjs.com/package/@remix-run/react) | 旧版 | 旧版本线；由 React Router 覆盖。 | 不计划：由 React Router 覆盖。 |
| **Turbopack** | 打包器/开发服务器 | 随 Next.js 提供 | 无独立包（随 next 16.4.6 提供） | 2026-11-09 | [www.npmjs.com](https://www.npmjs.com/package/next) | 无独立包 | 没有可单独跟踪的版本。 | 不计划：由 Next.js 覆盖。 |
| **Slint** | Rust UI | 1.x | 1.19.0 | 2026-11-16 | [crates.io](https://crates.io/crates/slint) | 许可证 | 自有标记语言需要单独适配器；许可证为 GPL-3.0 或 Slint 自有许可证。 | 待定问题（许可证、单独适配器）。 |
| **Playwright** | 渲染/测试基础设施 | 1.x | 1.65.1 | 2026-11-13 | [www.npmjs.com](https://www.npmjs.com/package/playwright) | 不是依赖 | 只有在用户希望把 dui 状态回放为测试时才相关。 | 待定问题（把状态回放为测试）。 |

## 需关注的 dui 自身依赖

画布库 `@xyflow/react` 发布了新主版本：13.0.0 于 2026-11-16，最新 13.0.2 于 2026-11-17（[npm](https://www.npmjs.com/package/@xyflow/react)）。这是 dui 自身的 UI，不是受支持的用户技术栈。

## 待定问题

尚未决定。列在这里以免遗忘。

- **Rust**: edition 2024 内的最低 toolchain 仍未定。
- **Next.js**: Server Components 在服务端渲染；在那里注入状态的方式未确认。
- **Electron**: 大约每 8 周一个新主版本，基线变化频繁。
- **shadcn/ui**: 被复制进仓库，并非运行时库；“支持”可能只是文档。
- **egui**: 即时模式：状态不在组件树中，寻找状态可能需要不同方法。
- **Iced**: 自 2025-12-07 以来没有新的稳定版。
- **React Native**: 仍为 0.x，“主版本”只能按 0.minor 定义；需要模拟器（iOS 需要 Mac）。
- **Expo**: 与 React Native 相同的渲染限制。
- **Flutter / Dart**: 需要新的 Dart 语言适配器；无头渲染路径未确认。
- **SwiftUI / Swift**: 渲染需要 Mac（Xcode/模拟器）；类型分析可在其他平台运行。
- **Jetpack Compose / Kotlin**: 需要 Kotlin 适配器和 Android/JVM 渲染路径。
- **Compose Multiplatform**: 取决于上面的 Kotlin 决定。
- **Qwik**: v2（`@qwik.dev/core`）为 2.0.0-rc.5；现在支持 1.x 意味着基线很快过时。
- **Slint**: 自有标记语言需要单独适配器；许可证为 GPL-3.0 或 Slint 自有许可证。
- **Playwright**: 只有在用户希望把 dui 状态回放为测试时才相关。
