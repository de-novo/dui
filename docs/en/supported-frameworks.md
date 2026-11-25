English | [简体中文](../zh-CN/supported-frameworks.md) | [한국어](../ko/supported-frameworks.md)

# What dui can analyze and render

This table describes `main`. For a default installation, use the [tested first capture](../../README.md#first-run) and check its reported version before choosing a development command.

Start with the result you need. For types, dui queries the project's official compiler. For a screen map, it can also observe a running app through its platform renderer. A project can use either path or both. [Architecture](architecture.md) · [Screen map contract](web-viewer.md)

| Task | Input and result | Evidence |
|---|---|---|
| Extract typed UI state | `dui scan` reads the project's TypeScript program. Rules match specific symbols and use compiler types to describe state slots, route parameters and other facts. | [Fact extractor](../../adapters/typescript/facts.mts) · [Rule checks](../../scripts/check-p2-rules.mts) |
| Map reachable web pages | Give `dui map` a local URL (`--url`) or a static build (`--build-dir`). Chromium follows same-origin links and records pages at the requested widths. An Angular app can use the URL path. Built Vue Router, SvelteKit, Astro and Preact fixtures exercise the static path. | [URL check](../../scripts/check-map-url.mts) · [Built app checks](../../scripts/check-web-frameworks.mts) |
| Find native screen declarations | `dui map` reads React Navigation and Expo Router declarations with the project's TypeScript tooling, and Flutter routes with the Dart analyzer. Each card points to its source. This works without a device and starts with zero attempted captures. | [Discovery adapters](../../adapters/native-map) · [Upstream app checks](../../scripts/check-native-map.mts) |
| Preview a native component from source | The preview exporter compiles a SwiftUI or Compose component with `swiftc` or `kotlinc` against a limited set of UI primitives. The viewer opens the observed states and switches on the corresponding map card. Standalone HTML export is also available. The card shows the reason when a call is unsupported. | [Exporter](../../scripts/source-preview.mts) · [Interaction checks](../../scripts/check-source-preview.mts) · [Map workflow](web-viewer.md#portable-source-map) |
| Capture native mobile UI | Android uses an attached device or emulator. iOS uses an installed simulator build on macOS. To show a recorded PNG on a source card, use `dui map attach`. The imported cell stays `unverified` and adds no verified coverage. If capture requires an unavailable runtime, the affected cell is reported as `unrenderable` with a reason. | [Compose emulator check](../../scripts/check-compose.mts) · [iOS simulator check](../../.github/workflows/ios-simulator.yml) · [Recording checks](../../scripts/check-native-map-recordings.mts) |
| Capture desktop or terminal UI | The GPUI fixture renders declared states and reports its element tree. The Electron host captures web renderer content in Chromium with optional IPC mocks. A terminal command running in a pseudoterminal produces a cell grid and PNG. | [GPUI](../../scripts/check-gpui.mts) · [Electron](../../scripts/check-electron.mts) · [Terminal](../../scripts/check-tui.mts) |

## What library rules contribute

For example, the TypeScript rules recognize Zustand's `create` as a store and Zod's `object` as a schema. The compiler supplies types for those calls. Installing a package alone supplies no evidence that a screen uses a particular state. Inspect the rule list for exact package and symbol names. A project can add its own rules with `rulePacks`. [Built-in rules](../../rules/web.json) · [Manifest reference](../reference.md)

Web capture has a different boundary. A page can appear on the map as soon as Chromium reaches it, even when dui has no rules for its template language. Enumerating hidden routes, reading component inputs or planning typed states needs source evidence for each operation. A page behind authentication or without an incoming link may need a session or an explicit entry point. [Screen map contract](web-viewer.md)

## Compiler and host requirements

The design targets official TypeScript 7. The current CLI also accepts official stable 6.x projects through a compatibility path. Run `dui check-ts PROJECT` for the project's compiler. The TypeScript scan checks stable React and React DOM 18+ and Vite 7+ when installed, and Next.js 16+ for the Next host when installed. Versions of libraries used by rules can produce a diagnostic note while the scan continues. [Compiler check](../../crates/dui/src/main.rs) · [Scan version checks](../../adapters/typescript/facts.mts)

`dui toolchains` reports installed versions of `tsc`, `rustc`, `go`, `swiftc`, `kotlinc` and `dart`. A successful probe establishes tool availability. The table above links the separate checks for extracting facts, discovering screens and rendering. Future work is tracked in the roadmap. [Toolchain probe](../../crates/dui-app/src/compiler_oracle.rs) · [Roadmap](roadmap.md)
