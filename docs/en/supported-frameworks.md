English | [简体中文](../zh-CN/supported-frameworks.md) | [한국어](../ko/supported-frameworks.md)

# Supported frameworks

All 38 stacks in priorities 1–3 below are official support targets. The priority is only the order in which they are implemented; it is not a level of support. The 11 stacks under "Not supported yet" are `unsupported` for now, each with its reason and the condition for adding it.

Versions were checked on 2026-11-20 (KST) against the official source linked in each row. Anything that could not be checked is marked `unconfirmed`. Version rules: [design: Supported versions](design-v0.md#supported-versions-current-latest-stable-major-only).

## Rules applied

1. **Official compiler only.** Types come from the compiler the project already uses (`tsc`, `rustc`, `swiftc`, `kotlinc`, `dart`).
2. **Latest stable major only.** One baseline per stack. Newer versions are always supported; the baseline moves up only in a new dui major. For 0.x projects the "major" is the 0.minor line.
3. **Typed languages only.**
4. **Rendering limits are stated.** Apple targets need a Mac to render. A stack can be analyzed but not rendered; those cells are reported as `unrenderable`.
5. **AI-native.** An AI drives the CLI and MCP server; people review the canvas.

## Summary

| Priority | Count | Stacks |
|---|---:|---|
| Priority 1 (MVP) | 8 | TypeScript, Node.js, React, Vite, TanStack Router, TanStack Query, zod, Chromium (Chrome stable) |
| Priority 2 | 15 | Rust, GPUI (`gpui-pre`), gpui-component / gpui-kit, Zustand, Redux Toolkit, Jotai, XState, React Hook Form, valibot, Next.js, React Router, TanStack Start, Electron, Radix UI, shadcn/ui |
| Priority 3 | 15 | Solid, SolidStart, Preact, Rspack / Rsbuild, Tauri, Dioxus, Leptos, egui, Iced, React Native, Expo, Flutter / Dart, SwiftUI / Swift, Jetpack Compose / Kotlin, Compose Multiplatform |
| Not supported yet | 11 | Vue, Nuxt, Svelte, SvelteKit, Astro, Angular, Qwik, Remix, Turbopack, Slint, Playwright |

## Priority 1 (MVP)

| Stack | Area | Supported major | Latest stable (verified) | Released | Source | Why dui supports it | Notes | Open question |
|---|---|---|---|---|---|---|---|---|
| **TypeScript** | language/runtime | 7.x | 7.0.2 | 2026-07-08 | [www.npmjs.com](https://www.npmjs.com/package/typescript) | Primary typed language; official `tsc` 7 gives the types dui needs. | 7.1 (sync API used by the TS adapter) is still beta. | — |
| **Node.js** | language/runtime | 26.x | 26.13.1 (LTS) | 2026-11-10 | [nodejs.org](https://nodejs.org/en/about/previous-releases) | Runs the TS sidecar and the user's dev server. | — | — |
| **React** | web UI | 19.x | 19.3.1 | 2026-10-14 | [www.npmjs.com](https://www.npmjs.com/package/react) | Most common typed UI stack; first renderer target. | — | — |
| **Vite** | bundler/dev server | 7.x / 8.x | 8.3.8 | 2026-11-11 | [www.npmjs.com](https://www.npmjs.com/package/vite) | dui must serve the injected dev build; Vite is the common React dev server. | Vite 7 uses the project's official TypeScript AST parser fallback when Oxc `parseSync` is absent. | — |
| **TanStack Router** | router | 1.x | 1.170.46 | 2026-10-22 | [www.npmjs.com](https://www.npmjs.com/package/@tanstack/react-router) | Typed routes give screen enumeration directly. | — | — |
| **TanStack Query** | state/data | 5.x | 5.108.1 | 2026-11-19 | [www.npmjs.com](https://www.npmjs.com/package/@tanstack/react-query) | Server state (loading / error / empty) is the core of state cells. | — | — |
| **zod** | state/data | 4.x | 4.6.7 | 2026-10-12 | [www.npmjs.com](https://www.npmjs.com/package/zod) | Schemas give realistic sample data and value ranges. | — | — |
| **Chromium (Chrome stable)** | render/test infra | not pinned | 158.0.8127.36 | 2026-11-17 | [chromiumdash.appspot.com](https://chromiumdash.appspot.com/releases?platform=Linux) | Headless render target over CDP for web screens. | — | — |

## Priority 2

| Stack | Area | Supported major | Latest stable (verified) | Released | Source | Why dui supports it | Notes | Open question |
|---|---|---|---|---|---|---|---|---|
| **Rust** | language/runtime | edition 2024 | 1.100.0 (toolchain) | 2026-11-12 | [github.com](https://github.com/rust-lang/rust/releases) | Second official-compiler language (`rustc` + rust-analyzer). | Minimum toolchain inside edition 2024 is still open. | yes |
| **GPUI (`gpui-pre`)** | Rust UI | 0.3.x | 0.3.13 | 2026-11-16 | [crates.io](https://crates.io/crates/gpui-pre) | Native renderer target planned after web. | Pre-1.0: each 0.x minor can break. | — |
| **gpui-component / gpui-kit** | UI kit | 0.10.x | 0.10.0 | 2026-11-16 | [crates.io](https://crates.io/crates/gpui-component) | Main component kit on GPUI; state comes through its widgets. | Published under two crate names (`gpui-component`, `gpui-kit`), both 0.10.0, same repo. | — |
| **Zustand** | state/data | 5.x | 5.1.0 | 2026-11-06 | [www.npmjs.com](https://www.npmjs.com/package/zustand) | Common client store; store shape defines UI states. | — | — |
| **Redux Toolkit** | state/data | 2.x | 2.15.0 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/@reduxjs/toolkit) | Typed slices enumerate states explicitly. | — | — |
| **Jotai** | state/data | 3.x | 3.0.2 | 2026-11-09 | [www.npmjs.com](https://www.npmjs.com/package/jotai) | Atom-level state; common in React apps. | New major 3.0.0 on 2026-09-08. | — |
| **XState** | state/data | 5.x | 5.33.4 | 2026-11-07 | [www.npmjs.com](https://www.npmjs.com/package/xstate) | State machines list every state by definition. | — | — |
| **React Hook Form** | state/data | 7.x | 7.92.0 | 2026-11-11 | [www.npmjs.com](https://www.npmjs.com/package/react-hook-form) | Form validation / submitting / error states. | — | — |
| **valibot** | state/data | 1.x | 1.5.0 | 2026-09-09 | [www.npmjs.com](https://www.npmjs.com/package/valibot) | Same role as zod for sample data. | — | — |
| **Next.js** | meta-framework | 16.x | 16.4.6 | 2026-11-09 | [www.npmjs.com](https://www.npmjs.com/package/next) | Most used React meta-framework. | Server Components render on the server; injecting state there is unconfirmed. | yes |
| **React Router** | meta-framework | 7.x / 8.x | 8.5.2 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/react-router) | Common router; framework mode is the successor of Remix. | New major 8.0.0 on 2026-06-17; 7.x shares the same declarative router/search APIs. | — |
| **TanStack Start** | meta-framework | 1.x | 1.169.7 | 2026-11-19 | [www.npmjs.com](https://www.npmjs.com/package/@tanstack/react-start) | Builds on TanStack Router, already in MVP. | — | — |
| **Electron** | desktop shell | 45.x | 45.4.1 | 2026-11-19 | [www.npmjs.com](https://www.npmjs.com/package/electron) | Desktop apps whose UI is web; renders with the same Chromium path. | A new major ships about every 8 weeks, so the baseline moves often. | yes |
| **Radix UI** | UI kit | 1.x | 1.8.1 | 2026-10-12 | [www.npmjs.com](https://www.npmjs.com/package/radix-ui) | Headless primitives behind many React kits; open/closed states. | — | — |
| **shadcn/ui** | UI kit | 4.x (CLI) | 4.23.0 | 2026-10-27 | [www.npmjs.com](https://www.npmjs.com/package/shadcn) | Very common React component source; code lives in the user's repo. | Copied into the repo, not a runtime library; "support" may only mean docs. | yes |

## Priority 3

| Stack | Area | Supported major | Latest stable (verified) | Released | Source | Why dui supports it | Notes | Open question |
|---|---|---|---|---|---|---|---|---|
| **Solid** | web UI | 2.x | 2.0.3 | 2026-11-19 | [www.npmjs.com](https://www.npmjs.com/package/solid-js) | Typed JSX; fine-grained signals. | Major 2.0.0 released 2026-11-17; ecosystem may lag. | — |
| **SolidStart** | meta-framework | 2.x | 2.2.3 | 2026-11-16 | [www.npmjs.com](https://www.npmjs.com/package/@solidjs/start) | Solid's meta-framework. | npm license field is empty (license unconfirmed). | — |
| **Preact** | web UI | 11.x | 11.2.0 | 2026-11-06 | [www.npmjs.com](https://www.npmjs.com/package/preact) | React-compatible API; most React work carries over. | — | — |
| **Rspack / Rsbuild** | bundler/dev server | 2.x | 2.3.3 | 2026-11-17 | [www.npmjs.com](https://www.npmjs.com/package/@rspack/core) | Second dev-server path for webpack-style projects. | Each extra dev server is another injection path to maintain. | — |
| **Tauri** | desktop shell | 2.x | 2.12.7 (crate) / 2.12.5 (@tauri-apps/api) | 2026-11-18 | [crates.io](https://crates.io/crates/tauri) | Rust + web desktop; fits a Rust core. | Uses the OS webview (WebKit on macOS/Linux), not Chromium; render parity unconfirmed. | — |
| **Dioxus** | Rust UI | 0.7.x | 0.7.10 | 2026-07-30 | [crates.io](https://crates.io/crates/dioxus) | Rust UI with web/desktop targets. | Pre-1.0. | — |
| **Leptos** | Rust UI | 0.8.x | 0.8.23 | 2026-10-31 | [crates.io](https://crates.io/crates/leptos) | Rust web UI; renders in Chromium. | Pre-1.0. | — |
| **egui** | Rust UI | 0.37.x | 0.37.0 | 2026-10-15 | [crates.io](https://crates.io/crates/egui) | Most downloaded Rust immediate-mode UI. | Immediate mode: state is not in a component tree, so finding states may need a different method. | yes |
| **Iced** | Rust UI | 0.14.x | 0.14.0 | 2025-12-07 | [crates.io](https://crates.io/crates/iced) | Elm-style Rust UI; messages enumerate states. | No new stable release since 2025-12-07. | yes |
| **React Native** | mobile/native | 0.88.x | 0.88.1 | 2026-10-27 | [www.npmjs.com](https://www.npmjs.com/package/react-native) | React on mobile; reuses React analysis. | Still 0.x, so "major" must mean the 0.minor line; needs simulators (iOS needs a Mac). | yes |
| **Expo** | mobile/native | 58.x | 58.0.26 | 2026-11-17 | [www.npmjs.com](https://www.npmjs.com/package/expo) | Most common way to start React Native apps. | Same rendering limits as React Native. | yes |
| **Flutter / Dart** | mobile/native | Flutter 3.x / Dart 3.x | Flutter 3.50.0 (Dart 3.14.0); Dart SDK 3.14.1 | Flutter 2026-11-11; Dart 2026-11-17 | [docs.flutter.dev](https://docs.flutter.dev/release/archive) | Typed, official compiler (Dart); large mobile share. | Needs a new Dart language adapter; headless rendering path unconfirmed. | yes |
| **SwiftUI / Swift** | mobile/native | Swift 6.x | 6.4.2 | 2026-10-30 | [github.com](https://github.com/swiftlang/swift/releases) | Official compiler `swiftc` + SourceKit-LSP; native Apple UI. | Rendering needs a Mac (Xcode / simulator); type analysis may run elsewhere. | yes |
| **Jetpack Compose / Kotlin** | mobile/native | Kotlin 2.x; Compose UI 1.x | Kotlin 2.4.21; Compose UI 1.13.0 (BOM 2026.11.01) | Kotlin 2026-10-08; Compose: `unconfirmed` | [developer.android.com](https://developer.android.com/jetpack/androidx/releases/compose-ui) | Official compiler `kotlinc`; native Android UI. | Needs a Kotlin adapter and an Android/JVM render path. | yes |
| **Compose Multiplatform** | mobile/native | 1.x | 1.12.2 | 2026-10-15 | [github.com](https://github.com/JetBrains/compose-multiplatform/releases) | Same Kotlin adapter; desktop target renders on the JVM. | Depends on the Kotlin decision above. | yes |

## Not supported yet

**Rule:** a stack in this table is added as soon as its blocker clears, for example when its official tooling accepts TypeScript 7. Until then dui rejects it before scanning as `unsupported` (reason).

Vue, Nuxt, Svelte, SvelteKit, Astro and Angular: their official type-check tools do not accept TypeScript 7 yet (npm peer ranges, checked 2026-11-20).

| Stack | Area | Latest major | Latest stable (verified) | Released | Source | Blocker | Reason | Added when |
|---|---|---|---|---|---|---|---|---|
| **Vue** | web UI | 3.x | 3.5.45 | 2026-11-05 | [www.npmjs.com](https://www.npmjs.com/package/vue) | TS 7 tooling | Template type checking runs through `vue-tsc`; TS 7 support unconfirmed, conflicts with TS 7+. | Its official tooling accepts TypeScript 7. |
| **Nuxt** | meta-framework | 4.x | 4.6.4 | 2026-11-02 | [www.npmjs.com](https://www.npmjs.com/package/nuxt) | TS 7 tooling | Built on Vue; follows Vue's status. | Vue is added. |
| **Svelte** | web UI | 5.x | 5.58.0 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/svelte) | TS 7 tooling | `svelte-check` 4.7.8 peer TS is ^5 \|\| ^6, not 7. | Its official tooling accepts TypeScript 7. |
| **SvelteKit** | meta-framework | 3.x | 3.4.0 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/@sveltejs/kit) | TS 7 tooling | Peer TS is ^6.0.0, not 7. | Its official tooling accepts TypeScript 7. |
| **Astro** | meta-framework | 7.x | 7.4.13 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/astro) | TS 7 tooling | `@astrojs/check` peer TS is ^5 \|\| ^6, not 7. | Its official tooling accepts TypeScript 7. |
| **Angular** | web UI | 22.x | 22.3.0 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/@angular/core) | TS 7 tooling | `@angular/compiler-cli` 22.3.0 requires TS >=6.0 <6.1, conflicts with TS 7+. | Its official tooling accepts TypeScript 7. |
| **Qwik** | web UI | 1.x (v2 in RC) | 1.20.4 | 2026-11-09 | [www.npmjs.com](https://www.npmjs.com/package/@builder.io/qwik) | major still RC | v2 (`@qwik.dev/core`) is 2.0.0-rc.5; supporting 1.x now means a short-lived baseline. | v2 is stable (open question). |
| **Remix** | meta-framework | 2.x (legacy) | 2.17.5 (@remix-run/react) | 2026-06-01 | [www.npmjs.com](https://www.npmjs.com/package/@remix-run/react) | legacy | Legacy line; covered by React Router. | Not planned: covered by React Router. |
| **Turbopack** | bundler/dev server | bundled in Next.js | no standalone package (ships in next 16.4.6) | 2026-11-09 | [www.npmjs.com](https://www.npmjs.com/package/next) | no standalone package | No separate version to track. | Not planned: covered by Next.js. |
| **Slint** | Rust UI | 1.x | 1.19.0 | 2026-11-16 | [crates.io](https://crates.io/crates/slint) | license | Own markup language needs its own adapter; license is GPL-3.0 or Slint's own licenses. | Open question (license, own adapter). |
| **Playwright** | render/test infra | 1.x | 1.65.1 | 2026-11-13 | [www.npmjs.com](https://www.npmjs.com/package/playwright) | not a dependency | Could matter only if users want dui states replayed as tests. | Open question (replay states as tests). |

## dui's own dependency to watch

The canvas library `@xyflow/react` had a new major: 13.0.0 on 2026-11-16, latest 13.0.2 on 2026-11-17 ([npm](https://www.npmjs.com/package/@xyflow/react)). This is dui's own UI, not a supported user stack.

## Open questions

Not decided yet. Listed here so they are not forgotten.

- **Rust**: Minimum toolchain inside edition 2024 is still open.
- **Next.js**: Server Components render on the server; injecting state there is unconfirmed.
- **Electron**: A new major ships about every 8 weeks, so the baseline moves often.
- **shadcn/ui**: Copied into the repo, not a runtime library; "support" may only mean docs.
- **egui**: Immediate mode: state is not in a component tree, so finding states may need a different method.
- **Iced**: No new stable release since 2025-12-07.
- **React Native**: Still 0.x, so "major" must mean the 0.minor line; needs simulators (iOS needs a Mac).
- **Expo**: Same rendering limits as React Native.
- **Flutter / Dart**: Needs a new Dart language adapter; headless rendering path unconfirmed.
- **SwiftUI / Swift**: Rendering needs a Mac (Xcode / simulator); type analysis may run elsewhere.
- **Jetpack Compose / Kotlin**: Needs a Kotlin adapter and an Android/JVM render path.
- **Compose Multiplatform**: Depends on the Kotlin decision above.
- **Qwik**: v2 (`@qwik.dev/core`) is 2.0.0-rc.5; supporting 1.x now means a short-lived baseline.
- **Slint**: Own markup language needs its own adapter; license is GPL-3.0 or Slint's own licenses.
- **Playwright**: Could matter only if users want dui states replayed as tests.
