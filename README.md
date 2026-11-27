# dui

English | [简体中文](README.zh-CN.md) | [한국어](README.ko.md)

dui captures a running web or GPUI app across UI states and viewport widths. It reads the app’s official compiler types, generates mock responses, and checks what was actually rendered. Browse the screen map, live sessions, and diffs in the embedded web viewer (`dui open`) or consume the same results as JSON through the CLI or MCP.

**Goal.** Today planning documents, Figma designs, and code are separate sources of truth. dui makes the code itself the single source of truth for development and UI work: the screen map is generated from the code, and UI edits go back into the same code.

[Product roadmap](docs/en/roadmap.md) · [Changelog](CHANGELOG.md) · [Analysis and rendering support](docs/en/supported-frameworks.md)

The capability tables describe development on `main`. For the default installation, start with the [first capture](#first-run).

## What dui analyzes

### Analysis and capture paths

dui reads types from the project's official compiler. Library rules identify specific API calls in that program. Runtime capture uses the renderer for each platform.

| To see | Use |
| --- | --- |
| Typed state slots and route parameters | `dui scan` with the project's TypeScript compiler and matching symbol rules. |
| Reachable web screens at different widths | `dui map` with a local URL or static build. dui follows same-origin links in the served HTML; `--capture` renders them in Chromium. |
| Declared native screens | `dui map` on a React Navigation, Expo Router or Flutter project. Each card points to its declaration. |
| Rendered UI | Capture on the configured web, mobile, desktop or terminal host. Selected SwiftUI and Compose components can also be exported as interactive HTML previews. |

### Analysis & verification capabilities

| Area | Capabilities | Status |
| --- | --- | --- |
| Screen & link discovery | Router file discovery, loop-free multi-hop BFS (`a[href]`), dynamic ID pattern collapsing (`/orders/:id`), `linksTo` screen-map edges | Supported |
| State planning & verification | Compiler-derived types, greedy pairwise covering array, `<select>` real DOM `<option>` discovery, visual-change witness (`visualWitness`), convergence guard | Supported |
| Data & API verification | Typed `GET` JSON auto-mocks, CSV mocks, explicit `mockResponses`, `hybrid` / `live` real-API modes with read-only guard, authenticated sessions (`storageState.json`, `POST authFlow`, interactive `dui auth login`) & secret redaction | Supported |
| Diff & change review | 2D canvas inline diff (`Elements` render-tree diff with 2px tolerance, `After`, `Before`, lazy `Pixel` overlay), branch diff (`dui diff --base main`), `--changed-since` | Supported |
| Quality, i18n & audit | 99% coverage audit (`dui audit`), multi-locale & RTL checks, missing translation & overflow detection, SQLite-backed review comments, content-addressed module-graph cache | Supported |
| Live editing & viewers | Shared CLI/MCP live sessions (`dui live`), embedded web canvas (`dui open`), native GPUI desktop viewer (`dui desktop`) | Supported |
| Element inspector & box model | DevTools-style element attributes, box-model diagram & overlay (width, height, margin, padding, border, gap, radius), and interactive spacing redline guides between elements | Supported |
| Universal routing, modals & canvas | Full-page true-aspect captures, WAI-ARIA navigation disclosure & modal/drawer auto-capture, obstacle-free gutter edge routing, native macOS TitleBar (`apps/desktop`), parallel multi-worker capture (`concurrency`), `SurfaceRegistry` pipeline | Supported |

## Install

On macOS or Linux, run this command to clone the latest release tag (`vX.Y.Z`, falling back to `main` if no tags exist) and build dui from source. To pin a release, set `DUI_TAG` to a [published tag](https://github.com/de-novo/dui/tags) in the installer’s environment.

```sh
curl -fsSL https://dui.sh/install | sh
```

You need Git, curl, and a C toolchain (Xcode Command Line Tools on macOS). The installer builds dui on your machine, adds the install directory (`~/.local/bin` by default) to your shell `PATH` automatically (set `DUI_NO_MODIFY_PATH=1` to opt out), and symlinks into a writable `PATH` directory when available so `dui` works immediately. If Rust or Node.js 26 with npm is missing, the installer downloads a temporary copy for the build and removes it afterward.

Check the installed tag or check for newer release tags with:

```sh
dui --version
dui update --check
```

Set `DUI_INSTALL_DIR` to change the install directory. [Prebuilt binaries](docs/binary-install.md)

<a id="first-run"></a>

## First capture with the default installation

Install Node.js 26, Chrome or Chromium, and the dependencies of a Vite app. The [tested app](fixtures/toggle) uses Vite 8 and React 19. In its root, set `readySelector` in `dui.json` to an element that appears when the intended screen is ready. For example, use this for an app whose main content contains its page heading.

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

The first command reports the installed version. The second writes a capture and prints its `output` directory. Open the local URL printed by the last command to see the capture, then press Ctrl+C to stop the viewer. If capture exits with code 5, read `report.json` in the output directory for the failed checks.

## Development CLI on `main`

To install the development build, run `curl -fsSL https://dui.sh/install | DUI_TAG=main sh`. The following example and command reference describe that build. The website also links to the [documentation for the default release](https://dui.sh/?lang=en#release-guide).

On this build `dui map` plans the screens, states and widths without starting a browser; `dui map --capture` captures every planned cell. See [Shared screen maps](docs/en/web-viewer.md#shared-screen-maps).

For a static site already built in `dist/`, run this command from its project root. Open the result with the viewer command above.

<!-- quickstart:static -->
```sh
dui map . --build-dir dist
```

### `dui map` vs `dui run`

| Command | When to use |
| --- | --- |
| `dui map [PROJECT]` | **Default for apps with a router** (TanStack Router, React Router 7/8, Next.js, or links). Automatically discovers screens, merges optional `dui.json` (`routeParams`, `storageState`, `readySelector`, and extra out-of-router `scenarios`), and plans every cell without a browser. `--capture` captures everything, modals included. |
| `dui run [PROJECT] [MANIFEST]` | **Explicit manifest capture.** Runs `dui.json` directly (omitting arguments defaults to `.` / `dui.json`, and falls back to `dui map` if no explicit `path` or `host` is set). |

| Group | Command | Purpose |
| --- | --- | --- |
| Capture | `dui map [PROJECT]` · `dui run [PROJECT] [MANIFEST]` · `dui scan [PROJECT]` | Discover/capture screens & modals or print compiler facts without launching a browser |
| Retry failed cells | `dui recheck RUN [PROJECT [MANIFEST]]` | Re-render only unsuccessful cells from a capture and report which ones recovered |
| Auth | `dui auth login URL [--out FILE] [--wait-url PATTERN]` | Save Playwright-compatible `storageState.json` (`0600`) for authenticated routes |
| View | `dui open [RUN_DIR]` · `dui desktop` | Embedded 2D web canvas or native GPUI desktop viewer |
| Diff & Quality | `dui diff --base BRANCH [--regressions-only] [--markdown]` · `dui audit RUN_DIR` · `dui comment` | Element-level & pixel diff, 99% coverage audit, review comments |
| Tools | `dui toolchains` · `dui history` · `dui prune` · `dui update` · `dui mcp` | Official compiler probes, archive history, cache GC, self-update, MCP stdio server |

## Discover screens

From an installed Vite app, run:

```sh
~/.local/bin/dui map .
```

dui finds TanStack file routes with a screen component. If it finds no concrete routes, it starts at `/` and follows reachable same-origin links within the discovery budget. It captures each URL at 375 and 1280 px. Run `dui open` (or `dui open <output>`) to open the embedded screen map viewer. MCP exposes the same operation as `map(project)`.

Inspect a captured cell and edit its boolean or enum state in a running Chromium session. [CLI and MCP examples](docs/en/web-viewer.md#run-a-live-cli-or-mcp-session)

An optional project-local `dui.json` can supply `appRoot`, mock responses, a starting `path` for link discovery, or sample dynamic values such as `"routeParams": {"id": "demo"}`. Supply a content-specific `readySelector` globally or in `routeOptions["/path"]` to verify a screen. Without one, dui saves the image as unverified and returns exit code 5. The report lists each URL’s source and unresolved route patterns.

## Use your own app

1. For this Vite example, open the app directory containing `package.json` and `tsconfig.json` and install its dependencies. Use `dui check-ts .` to check the project’s official TypeScript compiler. [Compiler and host requirements](docs/en/supported-frameworks.md#compiler-and-host-requirements)
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

**More screens.** Add `"routeSelector": "nav a[href]"` to follow matching same-origin links recursively. Use `scenarios` for unlinked routes or screens that need a different readiness selector.

**Mock data.** For supported typed GET requests to a fixed same-origin path, dui generates JSON responses from the app’s types. See `facts.json` under `autoMocks.generated` for generated values and `autoMocks.unresolved` for contracts that need a mock. [Routes and mock settings](docs/reference.md)

## First capture

To try a working app with dui already installed, run these commands from a directory where you want to clone the example.

```sh
git clone --depth 1 https://github.com/de-novo/dui.git
cd dui/fixtures/typed-api
npm ci
~/.local/bin/dui run . dui.json
```

Run `~/.local/bin/dui open` and select the example app. Expect one verified 768 px capture with three books. dui generates the response to `GET /api/shelf` from the return type of `fetchShelf()`. dui also finds a browser installed by Playwright (`PLAYWRIGHT_BROWSERS_PATH` or its default cache). If Chrome or Chromium is installed in another location, set `DUI_CHROMIUM` to its executable path. Without a browser, capture exits with code 5 and lists the paths it checked. [Example source](fixtures/typed-api/main.tsx)

### Capture an open-source app

Capture npmgraph, an MIT-licensed npm dependency viewer, from a single entry URL. The manifest supplies two mock packages. dui renders their dependency graph and follows the home link to the search page. At 375 and 1280 px, expect four verified cells. [Run the npmgraph example](examples/npmgraph/README.md)

- [Capture reference](docs/reference.md) — configure routes and mocks, read coverage, and automate runs with CLI or MCP.
- [Contributing](CONTRIBUTING.md) — find the relevant crate, change a rule, and run the checks.
- [Design](docs/en/design-v0.md) · [Architecture](docs/en/architecture.md) · [Analysis and rendering support](docs/en/supported-frameworks.md)

## License

Open-Core:

- Core (`ee/` excluded): [Apache License 2.0](LICENSE) — CLI, MCP server, local SQLite storage, web viewer, and local GPUI desktop viewer.
- Enterprise (`ee/`): [dui Enterprise License](ee/LICENSE) — commercial team and enterprise features (audit logs, SSO/RBAC, remote team sync). See [ee/README.md](ee/README.md).
