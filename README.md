# dui

English | [简体中文](README.zh-CN.md) | [한국어](README.ko.md)

dui captures a running web or GPUI app across UI states and viewport widths. It reads the app’s official compiler types, generates mock responses, and checks what was actually rendered. Browse the screen map, live sessions, and diffs in the embedded web viewer (`dui open`) or consume the same results as JSON through the CLI or MCP.

[Product roadmap](docs/en/roadmap.md) · [Changelog](CHANGELOG.md) · [Full stack matrix](docs/en/supported-frameworks.md)

## What dui analyzes

### Supported stacks & frameworks

| Category | Supported now | Planned |
| --- | --- | --- |
| Languages & compilers | TypeScript 7.x / 6.x (`tsc`), Rust edition 2024 (`rustc`) | Swift 6, Dart 3, Kotlin 2, Go |
| Web (Level 1 & Level 2) | Universal Web (`static` / `url` / `command` for any HTML/JS app), React 19, Vite 8, Nitro SSR, Chromium (CDP) | Next.js 16, Preact 11, Solid 2, SolidStart 2, Rspack / Rsbuild 2 |
| Desktop App | GPUI (`gpui-pre` 0.3.x), `gpui-kit` / `gpui-component` 0.10.x (macOS Metal + AccessKit) | Electron 45, Tauri 2, Dioxus 0.7, Leptos 0.8, egui 0.37, Iced 0.14 |
| Mobile App (Android & iOS) | Android (`adb` `screencap` + `uiautomator` -> `RenderNodeSnapshot`) | Jetpack Compose / Kotlin 2 compiler analysis, iOS (`xcrun simctl` / SwiftUI / Swift 6), React Native 0.88 / Expo 58, Flutter 3 / Dart 3 |
| Terminal UI (TUI) | Universal TUI (`pty` + VT100 cell grid -> `RenderNodeSnapshot` + monospace PNG) | Ratatui (Rust), Ink (TS/React), Bubble Tea (Go) |
| Routing & URL state | TanStack Router 1, TanStack Start 1, React Router 8, standard Web `URLSearchParams` | Next.js App Router, Android intents & deep links |
| State, forms & schemas | React `useState`, TanStack Query 5, Zustand 5, Redux Toolkit 2, Jotai 3, XState 5, React Hook Form 7, zod 4, valibot 1 | Radix UI 1, shadcn/ui 4 declarative packs |

### Analysis & verification capabilities

| Area | Capabilities | Status |
| --- | --- | --- |
| Screen & link discovery | Router file discovery, loop-free multi-hop BFS (`a[href]`), dynamic ID pattern collapsing (`/orders/:id`), `linksTo` screen-map edges | Supported |
| State planning & verification | Compiler-derived types, greedy pairwise covering array, `<select>` real DOM `<option>` discovery, visual-change witness (`visualWitness`), convergence guard | Supported |
| Data & API verification | Typed `GET` JSON auto-mocks, CSV mocks, explicit `mockResponses`, `hybrid` / `live` real-API modes with read-only mutation guard & secret redaction | Supported |
| Diff & change review | 2D canvas inline diff (`Elements` render-tree diff with 2px tolerance, `After`, `Before`, lazy `Pixel` overlay), branch diff (`dui diff --base main`), `--changed-since` | Supported |
| Quality, i18n & audit | 99% coverage audit (`dui audit`), multi-locale & RTL checks, missing translation & overflow detection, SQLite-backed review comments, content-addressed module-graph cache | Supported |
| Live editing & viewers | Shared CLI/MCP live sessions (`dui live`), embedded web canvas (`dui open`), native GPUI desktop viewer (`dui desktop`) | Supported |
| Element inspector & live components | DevTools-style element attributes, box-model overlay (width, height, margin, padding, gap, spacing redlines), and evolving from capture-based cards to rendering live components directly on the canvas | Planned |
| Design-defect detection & scale | Automatic visual/design-defect detection from `renderTree` (overflow, overlap, clipping, contrast, touch target size, spacing/alignment drift — visual overlays for humans + structured CLI/MCP JSON for AI), `SurfaceRegistry` multi-surface plugin pipeline, multi-tab parallel capture, GPUI AccessKit render-tree diff | Planned |

## Install

On macOS or Linux, run this command to clone the latest release tag (`vX.Y.Z`, falling back to `main` if no tags exist) and build dui from source. Pass a tag with `DUI_TAG=v0.0.1` to pin a specific release.

```sh
curl -fsSL https://github.com/de-novo/dui/raw/main/scripts/install | bash
```

You need Git, curl, and a C toolchain (Xcode Command Line Tools on macOS). The installer builds dui on your machine, adds the install directory (`~/.local/bin` by default) to your shell `PATH` automatically (set `DUI_NO_MODIFY_PATH=1` to opt out), and symlinks into a writable `PATH` directory when available so `dui` works immediately. If Rust or Node.js 26 with npm is missing, the installer downloads a temporary copy for the build and removes it afterward.

Check the installed tag or check for newer release tags with:

```sh
dui --version
dui update --check
dui update
```

**For web capture**, install Node.js 26, Chromium or Google Chrome, and the target app’s dependencies.

Set `DUI_INSTALL_DIR` to change the install directory. [Prebuilt binaries](docs/binary-install.md)

## Discover screens

From an installed Vite app, run:

```sh
~/.local/bin/dui map .
```

dui finds TanStack file routes with a screen component. If it finds no concrete routes, it starts at `/` and follows same-origin links from the entry page once. It captures each URL at 375 and 1280 px. Run `dui open` (or `dui open <output>`) to open the embedded screen map viewer. MCP exposes the same operation as `map(project)`.

Inspect a captured cell and edit its boolean or enum state in a running Chromium session. [CLI and MCP examples](docs/en/web-viewer.md#run-a-live-cli-or-mcp-session)

An optional project-local `dui.json` can supply `appRoot`, mock responses, a starting `path` for link discovery, or sample dynamic values such as `"routeParams": {"id": "demo"}`. Supply a content-specific `readySelector` globally or in `routeOptions["/path"]` to verify a screen. Without one, dui saves the image as unverified and returns exit code 5. The report lists each URL’s source and unresolved route patterns.

## Use your own app

1. Open a terminal in the app directory containing `package.json` and `tsconfig.json`. Install the app’s dependencies with its usual package manager. Web capture supports Vite 8 and the project’s official TypeScript 6.x or 7.x compiler. React apps use React 19.
2. Create `dui.json` in that directory. This example captures `/` at two widths. Replace `main article` with a CSS selector that appears when **your screen’s content is ready**. For a settings page, that might be `main form`.

```json
{
  "path": "/",
  "widths": [375, 1280],
  "readySelector": "main article"
}
```

3. Capture the app. dui starts the local host, captures the screens and saves each run under `~/.dui/` in your home directory.

```sh
~/.local/bin/dui run . dui.json
```

4. Run `~/.local/bin/dui open` to open the local history viewer. Select a project and two captures to compare the same state and width side by side. Captures are grouped by branch and worktree. Each records the commit and whether the working tree had uncommitted changes. If a run exits with code 5, inspect `report.json` in the directory returned as `output`.

**More screens.** Add `"routeSelector": "nav a[href]"` to collect matching same-origin links one level from the start page. Use `scenarios` for deeper routes or screens that need a different readiness selector.

**Mock data.** For supported typed GET requests to a fixed same-origin path, dui generates JSON responses from the app’s types. See `facts.json` under `autoMocks.generated` for generated values and `autoMocks.unresolved` for contracts that need a mock. [Routes and mock settings](docs/reference.md)

## First capture

To try a working app with dui already installed, run these commands from a directory where you want to clone the example.

```sh
git clone --depth 1 https://github.com/de-novo/dui.git
cd dui/fixtures/typed-api
npm ci
~/.local/bin/dui run . dui.json
```

Run `~/.local/bin/dui open` and select the example app. Expect one verified 768 px capture with three books. dui generates the response to `GET /api/shelf` from the return type of `fetchShelf()`. If Chrome or Chromium is installed in a custom location, set `DUI_CHROMIUM` to its executable path. [Example source](fixtures/typed-api/main.tsx)

### Capture an open-source app

Capture npmgraph, an MIT-licensed npm dependency viewer, from a single entry URL. The manifest supplies two mock packages. dui renders their dependency graph and follows the home link to the search page. At 375 and 1280 px, expect four verified cells. [Run the npmgraph example](examples/npmgraph/README.md)

- [Capture reference](docs/reference.md) — configure routes and mocks, read coverage, and automate runs with CLI or MCP.
- [Contributing](CONTRIBUTING.md) — find the relevant crate, change a rule, and run the checks.
- [Design](docs/en/design-v0.md) · [Architecture](docs/en/architecture.md) · [Planned stack support](docs/en/supported-frameworks.md)

## License

Open-Core:

- Core (`ee/` excluded): [Apache License 2.0](LICENSE) — CLI, MCP server, local SQLite storage, web viewer, and local GPUI desktop viewer.
- Enterprise (`ee/`): [dui Enterprise License](ee/LICENSE) — commercial team and enterprise features (audit logs, SSO/RBAC, remote team sync). See [ee/README.md](ee/README.md).
