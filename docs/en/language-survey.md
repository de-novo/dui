# dui target languages and UI stacks: exhaustive survey v0

English | [简体中文](../zh-CN/language-survey.md) | [한국어](../ko/language-survey.md)

> Written 2026-11-19. Purpose: choose the languages and frameworks dui supports, based on evidence.
> Only numbers with a source are given. Anything not confirmed is `unverified`. Star counts are GitHub API values as of 2026-11-19.
> Licenses here are those of the **target frameworks**, separate from dui's dependency policy (dui is Apache-2.0 open source; its dependencies must allow commercial use). dui only inserts dev-time code into the user's project; it does not redistribute those frameworks.

## 0. Four things dui needs from each stack

| Criterion | Question |
|---|---|
| Types | Can every state shape be derived from types? |
| Forced injection | Can the places where state lives (including local state) be rewritten at build time so values can be set from outside? |
| Network interception | Can API responses be replaced with mocks? |
| Rendering without a window | Can a screen with injected state be output as an image (CI, parallel)? |

Difficulty combines these four: **low** = existing tools cover everything, **medium** = we need to build one or two, **high** = one core piece is blocked.

## 1. Usage scale (languages)

Source A: [Stack Overflow Developer Survey 2025, Technology > Languages](https://survey.stackoverflow.co/2025/technology) — share of all respondents who "did extensive development work in the past year".
Source B: [GitHub Octoverse 2025](https://github.blog/news-insights/octoverse/octoverse-a-new-developer-joins-github-every-second-as-ai-leads-typescript-to-1/) — by monthly contributors, August 2025.

| Language | SO 2025 usage (A) | Octoverse 2025 (B) | Type strength |
|---|---|---|---|
| TypeScript | 43.6% | **#1**, 2,636,006 monthly contributors, +66.6% YoY | Static, structural. Can be bypassed with `any` |
| Java | 29.4% | #4 | Static, strong |
| C# | 27.8% | #5 | Static, strong |
| C++ | 23.5% | unverified | Static; templates and macros make analysis hard |
| Go | 16.4% | unverified | Static, simple |
| Rust | 14.8% | unverified | Static, very strong (enums, pattern matching) |
| Kotlin | 10.8% | unverified | Static, strong (sealed classes, null safety) |
| Dart | 5.9% | unverified | Static, sound null safety |
| Swift | 5.4% | unverified | Static, strong (enums, optionals) |

Note: JavaScript 66% and Python 57.9% are untyped and out of scope (a non-goal of design v0).
Caution: the SO survey skews toward web and backend developers, so mobile languages (Swift, Kotlin, Dart) may come out lower than in reality. Mobile-specific usage numbers are `unverified` (needs further checking, e.g. JetBrains State of Developer Ecosystem).

UI framework usage (SO 2025 web frameworks, all respondents): React 44.7%, Next.js 20.8%, Angular 18.2%, Vue.js 17.6%, Svelte 7.2%, Blazor 7.0%. 2025 usage for mobile and desktop frameworks (Flutter, React Native, SwiftUI, Compose, Electron, etc.) is `unverified` (those items were not found on this page).

## 2. Per-stack survey

### 2.1 TypeScript

| Stack | ★ (GitHub) | Where state lives | Forced injection | Network | Render without window | Existing preview | Difficulty |
|---|---|---|---|---|---|---|---|
| React (+Vite/Next) | react 252,767 | `useState`, `useReducer`, context, stores (Zustand/Redux), query cache, URL | Babel/SWC transform (Vite plugin) | MSW (Service Worker / Node) | Playwright headless | Storybook | **low** |
| Vue 3 | vuejs/core 55,045 | `ref`/`reactive`, Pinia, URL | Vite plugin (SFC compile step) | MSW | Playwright | Storybook | low |
| Svelte 5 | 89,756 | runes (`$state`), stores | Compiler preprocessing | MSW | Playwright | Storybook | low |
| Solid | 36,760 | signals, stores | Babel transform | MSW | Playwright | Storybook | low |
| Angular | 101,461 | Component fields, signals, services (DI) | Swap services via DI + build transform | `HttpClient` interceptors / MSW | Playwright | Storybook | medium (own build chain) |
| React Native | react-native 126,964 (Expo 53,662) | Same as React | Babel (Metro) transform | JS fetch mocks / MSW native | **Needs a simulator**. react-native-web is a workaround but may differ from the real thing | Storybook RN | medium |
| Lynx | 15,264 | React family (ReactLynx) | unverified | unverified | unverified | unverified | unverified |
| Electron (webview) | 123,999 | Renderer same as web + IPC | Same as web; mock preload/IPC | MSW | Playwright (Electron support) | Storybook | low |
| Tauri (webview) | 112,993 | Web + `invoke` | Same as web; mock `invoke` | MSW | Render in a browser | Storybook | low |
| Ionic/Capacitor | ionic 52,763 | Same as web | Same as web | MSW | Playwright | Storybook | low |

### 2.2 Swift (Apple)

| Stack | Where state lives | Forced injection | Network | Render without window | Existing preview | Difficulty |
|---|---|---|---|---|---|---|
| SwiftUI | `@State`, `@Observable` models, `@Environment` | Swift macros can only add new code and cannot change the initial value of an existing `@State` → **rewrite source with SwiftSyntax before the dev build** (SwiftPM build plugin). Inject `@Observable` models by replacing creation sites | Intercept by registering a `URLProtocol` | `ImageRenderer` (iOS 16 / macOS 13+), [swift-snapshot-testing](https://github.com/pointfreeco/swift-snapshot-testing) (MIT, ★4,397). **Requires a macOS + Xcode machine** | `#Preview` | medium |
| UIKit / AppKit | Scattered across view controller fields, imperative | Source rewriting possible, but scattered state lowers accuracy | `URLProtocol` | Snapshot tests (library above); may need a simulator | Xcode Preview (via UIViewRepresentable) | high |

### 2.3 Kotlin / Java (Android, JVM)

| Stack | ★ | Where state lives | Forced injection | Network | Render without window | Existing preview | Difficulty |
|---|---|---|---|---|---|---|---|
| Jetpack Compose (Android) | unverified | `remember { mutableStateOf }`, ViewModel `StateFlow` | **Kotlin compiler plugin** (Compose itself is a compiler plugin). Official stability of the plugin API is unverified | OkHttp interceptor / MockWebServer | On the JVM without a device: [Paparazzi](https://github.com/cashapp/paparazzi) (Apache-2.0, ★2,641), [Roborazzi](https://github.com/takahirom/roborazzi) (Apache-2.0, ★1,084), [Compose Preview Screenshot Testing](https://developer.android.com/studio/preview/compose-screenshot-testing) (experimental; the standalone plugin is no longer maintained and is moving to AGP 9.5+ test suites) | `@Preview` | medium |
| Compose Multiplatform | 19,488 | Same as above | Same as above | Ktor client mock engine | Desktop offscreen rendering (`ImageComposeScene`) believed to exist, docs unverified | `@Preview` | medium |
| Android Views (Kotlin/Java) | — | View / Fragment fields, XML | Bytecode transform (AGP Instrumentation / ASM) — harder than source level | OkHttp interceptor | Paparazzi and Roborazzi also support Views | Layout Editor | high |
| Swing / JavaFX | jfx 3,310 (GPL-2.0) | Component fields | Annotation processors only generate new files → bytecode transform needed | Replace `HttpClient` | Swing can paint into a `BufferedImage`; JavaFX headless (Monocle) unverified | none | high |

### 2.4 Dart (Flutter)

| Stack | ★ | Where state lives | Forced injection | Network | Render without window | Existing preview | Difficulty |
|---|---|---|---|---|---|---|---|
| Flutter | 181,293 (BSD-3) | `State<T>` fields, Provider/Riverpod/Bloc | Dart macros were [discontinued in 2025-01](https://dart.dev/blog/an-update-on-dart-macros-data-serialization). `build_runner` only generates new files → needs a **source-rewrite step before the dev build**. Riverpod, Bloc, etc. are easy to inject via overrides | `HttpOverrides`, http `MockClient`, Dio interceptors | **golden tests** (`flutter_test`, on the host without a device) | [Widget Previewer](https://docs.flutter.dev/tools/widget-previewer) `@Preview` (experimental in 3.35 → stable in 3.47 per docs) | medium |

### 2.5 C# (.NET)

| Stack | ★ | Where state lives | Forced injection | Network | Render without window | Existing preview | Difficulty |
|---|---|---|---|---|---|---|---|
| Avalonia | 31,847 | MVVM ViewModels (`INotifyPropertyChanged`) | Source generators (add-only) + [interceptors](https://github.com/dotnet/roslyn/blob/main/docs/features/interceptors.md) (replace call sites; secondary sources say stable from .NET 9.0.2xx SDK, primary source unverified) | Swap `HttpMessageHandler` | Avalonia.Headless (docs unverified) | XAML previewer | medium |
| WPF | 7,758 | ViewModels | Same as above | Same as above | `RenderTargetBitmap`, **Windows only** | XAML designer | medium |
| WinUI 3 | microsoft-ui-xaml 8,618 | ViewModels | Same as above | Same as above | unverified | XAML Hot Reload | high |
| .NET MAUI | 23,322 | ViewModels | Same as above | Same as above | Needs a device / emulator; headless unverified | XAML Hot Reload | high |
| Uno Platform | 10,125 | ViewModels | Same as above | Same as above | unverified | unverified | unverified |
| Blazor | aspnetcore 38,533 / SO usage 7.0% | Component fields, cascading values | Source generators / render tree is DOM | Swap `HttpClient` | Playwright (output is web) | bUnit (testing) | medium |

### 2.6 Rust

| Stack | ★ | Where state lives | Forced injection | Network | Render without window | Existing preview | Difficulty |
|---|---|---|---|---|---|---|---|
| **GPUI** (pippip) | zed 95,318 (gpui crate Apache-2.0) | `Entity<T>` models, View fields | proc-macro + dev feature | Swap HTTP client (reqwest) | **Present in code**: confirmed that the `gpui-pre 0.3.12` source used by pippip has `HeadlessAppContext::capture_screenshot` and `Window::render_to_image` (`test-support` feature). Actually running it is unverified | none | medium |
| egui | 32,385 | App struct fields (immediate mode) | proc-macro | Swap client | [egui_kittest](https://docs.rs/egui_kittest) `image_snapshot`, wgpu headless | none | low–medium |
| Iced | 32,080 | Elm-style single State + Message | State is in one place, so injection is easy | Swap client | `iced_test` simulator snapshots, GPU-free via tiny-skia | none | low–medium |
| Slint | 24,644 (custom license, GitHub shows NOASSERTION) | `.slint` DSL properties + Rust models | DSL properties can be set from Rust | Swap client | Testing backend `take_snapshot`, `SLINT_BACKEND=headless-software` | Slint live preview | medium |
| Dioxus | 39,850 | signals, hooks | proc-macro / RSX transform | Swap client | Web and desktop (webview) via Playwright; native (Blitz) unverified | none | medium |
| Leptos | 21,560 | signals | proc-macro | Swap client | Output is web → Playwright | none | medium |
| Tauri (Rust side) | 112,993 | Screens are web | Handled on the web (TS) side | Mock `invoke` | Playwright | Storybook | low |

### 2.7 C++ / Go

| Stack | ★ | Where state lives | Forced injection | Network | Render without window | Existing preview | Difficulty |
|---|---|---|---|---|---|---|---|
| Qt Widgets / QML | Official repos are outside GitHub (★ unverified) | QObject properties; QML is JS-like | Source rewriting with Clang tooling is possible but heavy. QML is dynamic | Swap `QNetworkAccessManager` | `-platform offscreen` + `grabToImage` | Qt Design Studio | high |
| Fyne (Go) | 28,850 | Widget fields, bindings | Go has no macros → `go generate` / source rewriting | Swap `http.RoundTripper` | Software rendering in the test package (unverified) | none | high |
| Wails (Go) | 37,160 | Screens are web | Handled on the web (TS) side | Mock bindings | Playwright | Storybook | low |

## 3. Existing preview tools vs dui

| Tool | What it does | What it lacks compared with dui |
|---|---|---|
| Storybook (★91,647) | Renders components as per-state stories, web family | States are **written by hand one by one**. No transition graph. No notion of coverage |
| SwiftUI `#Preview` | Previews views inside Xcode | Same + Xcode only, no canvas or width matrix |
| Compose `@Preview` (+ screenshot tests) | Previews as images / regression tests | States are manual; whole screens with ViewModels are hard |
| Flutter Widget Previewer | `@Preview` widgets per size / theme | Hand-written, no transition graph |
| XAML designers / Qt Design Studio | Layout previews | Not real data states |

The common gap: **automatic state discovery + forced injection of local state + a transition graph + a coverage number**. These four are dui's core, and each ecosystem's render / snapshot tools are reused as dui's render workers.

## 4. Recommended order

Score = market size × feasibility × need in our own projects.

| Rank | Target | Phase | Reason |
|---|---|---|---|
| 1 | **TS web: React (+Vite/Next)** | MVP | #1 typed language (Octoverse 2025), React 44.7% (SO 2025). Tools for injection, interception, and rendering all exist (difficulty low). obora is here |
| 2 | **Electron / Tauri webviews** | MVP | Almost free with the same engine as #1. obora-desktop is Electron |
| 3 | **Rust GPUI** | MVP (spike) | Needed by pippip. Confirming a headless render API in the code reduces the biggest risk. Small market, but our own validation target |
| 4 | Vue, Svelte, Solid, Angular, Blazor, Leptos, Dioxus web | Next | Rendering is web, so Playwright and MSW are reused. Only the injection transform is added |
| 5 | **Kotlin Compose** (Android + Multiplatform) | Next | Kotlin 10.8% (SO 2025). Device-free rendering on the JVM (Paparazzi, Roborazzi); state gathered in `remember` / ViewModels. The compiler plugin is the main cost |
| 6 | **Flutter** | Next | ★181,293, the largest cross-platform framework. Easy host rendering with golden tests. Macros discontinued → the source-rewrite step is the cost |
| 7 | **SwiftUI** | Next | Large iOS market, but a macOS + Xcode runner is required and `@State` injection needs source rewriting |
| 8 | React Native | Later | Injection is easy, but real native rendering needs a simulator |
| 9 | C# Avalonia / WPF | Later | MVVM keeps state together, which helps; injection possible with interceptors. After confirming enterprise desktop demand |
| 10 | egui / Iced / Slint | Later | Headless render tools are already good. Small market; easy if requested |
| 11 | UIKit/AppKit, Android Views, Swing/JavaFX, MAUI, WinUI, Qt, Fyne | On hold | Imperative UI with scattered state, or headless rendering blocked (difficulty high). Does not fit the 99% goal |

Conclusion: **start by fixing the languages to "TS + Rust", but keep the design open to Kotlin, Dart, and Swift as next targets.** All three have strong types and device-free render paths, so dui's approach works. What blocks them is the injection step (compiler plugin / source rewriting), so keep the core "injection point → transform" contract language-neutral.

## 5. Still to confirm

- Usage numbers for mobile languages and frameworks (JetBrains State of Developer Ecosystem, etc.): unverified.
- Stability of the Kotlin compiler plugin API; primary source for when C# interceptors became stable: unverified.
- Compose Desktop `ImageComposeScene`, Avalonia.Headless, Fyne test rendering: docs unverified.
- Actually running GPUI headless capture on pippip: unverified (M2 spike).
