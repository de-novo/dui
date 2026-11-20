# dui (de-novo-ui)

English | [简体中文](README.zh-CN.md) | [한국어](README.ko.md)

> **Vision (2026-11-19)**: dui supports *well-built products* only. When state is expressed in types and APIs in DTOs, every screen, state, and width is drawn on one canvas. Fully drawn means good code; many `unverified` cells signal scattered state management. The goal is an industry standard for UI code quality: ① support criteria as public rules (+ lint), ② a single number, "UI state coverage %", ③ an open format for state graphs and snapshots.

> **Positioning**: design-generation tools show *how it will be built*: AI imagines screens that do not exist yet. dui shows *how it is actually built*: it reads existing code through the official compilers and really renders every state that exists. Nothing is invented; what cannot be shown is marked `unverified` or `unrenderable`.

> **Principle #1 — AI-native (2026-11-19)**: dui is AI-native. **AI uses everything; humans only look at the canvas.** AI agents drive dui through the `dui` CLI and MCP server (analyze, inject, render, verify, and fix `unverified` cells). Humans see only the canvas (screens × states × widths × transitions) and leave comments on cells; AI reads the comments, fixes the code, and the canvas re-renders. See [design §0](docs/en/design-v0.md#0-core-principle-ai-native).

> **Principle #2 — official-compiler-dependent (2026-11-19)**: dui relies on each language's **official compiler / toolchain** (TypeScript: `tsc` from Microsoft's `typescript` package, 7.0+ only; Rust: `rustc` + rust-analyzer; later Swift: `swiftc` + SourceKit-LSP; Kotlin: `kotlinc`). It uses the official compiler already in your project and never reimplements type checking. Unofficial or alternative compilers / toolchains are `unsupported`. Only the current latest stable major of each stack is supported. See [design: Tech stack](docs/en/design-v0.md#tech-stack-decided-2026-11-19).

> Idea-stage repository. No code yet.

Design: [docs/en/design-v0.md](docs/en/design-v0.md) (developer tool design v0)
Architecture: [docs/en/architecture.md](docs/en/architecture.md) (functional core, ports and adapters, monorepo and crate boundaries)
Language survey: [docs/en/language-survey.md](docs/en/language-survey.md) (survey of target languages and UI stacks, recommended order)
Analysis approaches: [docs/en/analysis-approaches.md](docs/en/analysis-approaches.md) (how code-analysis AI tools work, recommended analyzer stack for dui)
Translations and glossary: [docs/i18n.md](docs/i18n.md)

## In one line
A tool that reads a codebase and lays out **every screen × every state × every responsive width** of an app on an infinite, Figma-like canvas at a glance. It covers web and desktop apps (including native).

## Why (the need)
- See **screens drawn by the real code**, per state, all at once, the way you look at designs in Figma.
- During QA and review, check "what does the empty / loading / error / no-permission state of this screen look like?" without opening each one by hand.
- Missing states (states never drawn yet) must be visible.

## Core concepts
- **Node = a screen + one state of that screen** (e.g. review screen / after approving the last page).
- **Edge = a state transition** (which condition or action leads to the next state). Conditional navigation is also expressed as state → the whole app becomes one state graph.
- Each node gets **mock data** to render that state. No interaction (internal handling) is needed; a still screen is enough.
- Spreading one node horizontally shows **responsive widths** (mobile, tablet, desktop); light/dark and locale axes can be added if needed.
- Cells that could not be checked are marked `unverified`.

## Goal: 99% coverage (measured, not guessed)
1. **Measure screen code branches** (if, loading / error / empty states, per permission) **with code coverage** → "what % of screen branches have actually been drawn".
2. Mock data is **generated automatically from types / API schemas (OpenAPI, zod, etc.)** → including edge values such as empty values, long values, and error responses.
3. When an undrawn branch appears, AI creates a mock that enters that branch and renders again → repeat until 99%.
4. Our own code follows a rule to **make screen state explicit** with types and state machines → the list of states itself becomes 100% accurate.

## Graph extraction
- Static analysis: routers, links, `navigate()` / `router.push()`, menu items → the skeleton.
- AI inference: conditional navigation, dynamic URLs → marked `inferred`.
- Execution traces: overlay with recordings of real navigation to verify.

## Supported frameworks

All 38 stacks in priorities 1–3 are official support targets; the priority is only the implementation order. The 11 stacks not supported yet are added as soon as their blocker clears (for most: their official tooling accepts TypeScript 7). Versions, reasons, and open questions: [docs/en/supported-frameworks.md](docs/en/supported-frameworks.md).

| Priority | Count | Stacks |
|---|---:|---|
| Priority 1 (MVP) | 8 | TypeScript, Node.js, React, Vite, TanStack Router, TanStack Query, zod, Chromium (Chrome stable) |
| Priority 2 | 15 | Rust, GPUI (`gpui-pre`), gpui-component / gpui-kit, Zustand, Redux Toolkit, Jotai, XState, React Hook Form, valibot, Next.js, React Router, TanStack Start, Electron, Radix UI, shadcn/ui |
| Priority 3 | 15 | Solid, SolidStart, Preact, Rspack / Rsbuild, Tauri, Dioxus, Leptos, egui, Iced, React Native, Expo, Flutter / Dart, SwiftUI / Swift, Jetpack Compose / Kotlin, Compose Multiplatform |
| Not supported yet | 11 | Vue, Nuxt, Svelte, SvelteKit, Astro, Angular, Qwik, Remix, Turbopack, Slint, Playwright |

## How each family is rendered
| Family | Examples | How to render | Difficulty |
|---|---|---|---|
| Web | React, Vite, Next.js, Solid, Preact | Render in Chromium over CDP with injected state and intercepted requests | Easy |
| Web-based desktop | Electron, Tauri | Same as web (Tauri: mock `invoke`) | Easy |
| Native | GPUI (pippip) | Render per state in an in-app gallery mode | Labor-intensive |

For the web family: one shared engine plus a thin per-framework screen-list reader. For native: attach a gallery mode inside the framework.

## Where to view it (canvas)
- Option 1: auto-layout in **Figma** (one row per screen, states horizontal, widths beside them). Can be prototyped right away with the Figma connector.
- Option 2: our own infinite canvas with React Flow (MIT). Auto-updates when code changes. tldraw is on hold because production use requires a separate commercial license ([design doc](docs/en/design-v0.md) §11).

## Similar things (surveyed 2026-11-19)
- Storybook (+ AI workflow, MCP): stories per state, real code. But list-shaped, web only.
- Figma: a canvas, but design files, not real code.
- Selfloop: reads code, screen graph and flow tests. Web and mobile only; no mention of desktop.
- UI Verify, Chromatic, Percy: screen comparison (visual regression). Not a canvas or a state graph.
- XState: state machine visualization. Written by hand, no real screens.
- AgentController, Peekaboo: MCP servers where AI drives and captures macOS apps.

**The gap**: we have not yet found a tool that lays out screens drawn by real code — web or desktop alike — with a state graph and responsive widths on an infinite canvas.

## First prototype candidate
Render a few obora web screens (review, export, settings) per state and per width and auto-place them in one Figma file. Then obora desktop (its existing state gallery), then pippip (GPUI).

## Base engine candidates: open-source code knowledge graphs (2026-11-19)
Good open-source tools already build function-call and dependency graphs. dui uses them as the base and adds only a **state-transition layer** on top.
- [Graphify](https://github.com/Graphify-Labs/graphify/) (★ ~150k, Apache-2.0): ~40 languages via tree-sitter, `graph.html` visualization, each edge marked `EXTRACTED` / `INFERRED`. **First candidate**.
- [CodeGraph](https://github.com/colbymchenry/codegraph/) (★ ~78k, MIT): auto-update, links web routes ↔ handlers, traces Tauri web ↔ Rust calls.
- ~~[GitNexus](https://github.com/abhigyanpatwari/GitNexus/)~~ (★ ~49k): **excluded** due to the PolyForm Noncommercial license.

### State-transition layer (what dui builds)
Call graph = "function A calls B" (code structure). What dui wants = "doing this action in this state leads to this state / screen" (screen behavior).
1. Take screen events (onClick, shortcuts, etc.) as starting points.
2. Follow the call graph to state-change points (setState, store actions, reducers, GPUI `cx.notify()`).
3. Turn them into `state + event → next state` rules.
4. Starting from the initial state, expand the rules automatically to produce every reachable state and sequence (model-checking style). Infinite expansion is cut with representative values (0, 1, last, one past last).
5. Replay the paths to verify that the state actually renders.

Like Graphify, every edge is marked `extracted` (explicit in code) / `inferred` (AI inference) / `verified` (confirmed by replay).

## What state can be extracted
| Kind | Examples | Method | Accuracy |
|---|---|---|---|
| Data shape | empty list, one item, very many, long text, missing value | Generate mocks from types / API schemas | High |
| Code branch | loading, error, permission, `length === 0` | Static analysis + coverage measurement | High (99% target feasible) |
| Sequence-created state | uploading, after approving the last page, half-filled modal | Trace event → state change + expand + replay | Medium → raised by a rule to keep state in one type / reducer |

## License and reference policy (2026-11-19, updated for open source)
dui will be released as **open source** (release timing after 준혁's approval; private for now). To avoid problems caused by referenced projects, we follow these rules.

1. **No code copying by default.** Other projects are referenced for design and ideas only; we write our own code. Items marked as "references" in the docs (React Compiler, GATOR, CodeGraph, react-docgen-typescript, eslint-plugin-react-hooks, XState, etc.) are idea references, not code copies.
2. **Reuse only as dependencies.** Only MIT, Apache-2.0, BSD, and ISC are allowed as dependencies. If a source snippet must be copied, it must be under the same permissive licenses, and the original path, commit, and license go into `THIRD_PARTY_NOTICES`.
3. **Restricted licenses are not opened or copied.** Code under restrictive terms such as GPL, AGPL, LGPL, PolyForm Noncommercial, SSPL, BSL, or the tldraw license (GitNexus, Serena, the Semgrep engine, FlowDroid, CodeQL, etc.) is not read while implementing (clean room). Only concepts at the level of public docs and papers are used.
4. **AI-written code follows the same rules.** AI agents get the working rule "do not bring in code from specific projects; dependencies must be permissively licensed". Dependency licenses are checked automatically in CI (e.g. `license-checker`, `cargo-deny`).
5. **Names and trademarks.** Other projects' names such as Storybook, Figma, or Graphify are not used in product or package names; only in descriptions like "compatible with ~".
6. **dui's own license: Apache-2.0 (final, 2026-11-19).** Its patent grant makes it easy for companies to adopt and helps in aiming for an industry standard. Contributors sign off with the DCO (`git commit -s`). Full text: [LICENSE](LICENSE); notice: [NOTICE](NOTICE); contribution rules: [CONTRIBUTING.md](CONTRIBUTING.md).
7. **Multilingual (decided 2026-11-19).** Project docs are kept in three languages: English, Simplified Chinese, and Korean. **English is the source of truth**; change English first, then update Chinese and Korean in the same PR or add an "outdated" banner. Rules and glossary: [docs/i18n.md](docs/i18n.md).
8. **Pre-release check:** scan the whole repository for external code snippets, secrets, and internal paths (internal information from obora / pippip).
