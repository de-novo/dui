# dui

English | [简体中文](README.zh-CN.md) | [한국어](README.ko.md)

dui captures a running web app across UI states and viewport widths. It reads the app’s TypeScript types, generates mock responses, and checks what Chromium actually rendered. Browse the screenshots in a local HTML gallery or consume the same results as JSON through the CLI or MCP.

## Install

On macOS or Linux, run this command to install or update dui from the current `main` branch.

```sh
curl -fsSL https://github.com/de-novo/dui/raw/main/scripts/install | bash
```

You need Git, curl, and a C toolchain (Xcode Command Line Tools on macOS). The installer builds dui on your machine. If Rust or Node.js 26 with npm is missing, the installer downloads a temporary copy for the build and removes it afterward.

Check the installed version with the full path. To run `dui` by name, add `~/.local/bin` to your `PATH`.

```sh
~/.local/bin/dui --version
```

**For web capture**, install Node.js 26, Chromium or Google Chrome, and the target app’s dependencies.

Set `DUI_INSTALL_DIR` to change the install directory. [Prebuilt binaries](docs/binary-install.md)

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

3. Run the capture. The three arguments are the project directory (`.`), the settings file, and the output directory. dui starts and stops the local host itself.

```sh
~/.local/bin/dui run . dui.json .dui/capture
```

4. Open `.dui/capture/graph.html` in your browser. On macOS, run `open .dui/capture/graph.html`. On a Linux desktop, use `xdg-open .dui/capture/graph.html`. Each cell shows one route and state at one width. If dui exits with code 5, open `report.json` in the same directory for the reason each cell was left unverified or unrenderable.

**More screens.** Add `"routeSelector": "nav a[href]"` to collect matching same-origin links one level from the start page. Use `scenarios` for deeper routes or screens that need a different readiness selector.

**Mock data.** For supported typed GET requests to a fixed same-origin path, dui generates JSON responses from the app’s types. See `facts.json` under `autoMocks.generated` for generated values and `autoMocks.unresolved` for contracts that need a mock. [Routes and mock settings](docs/reference.md)

## First capture

To try a working app with dui already installed, run these commands from a directory where you want to clone the example.

```sh
git clone --depth 1 https://github.com/de-novo/dui.git
cd dui/fixtures/typed-api
npm ci
~/.local/bin/dui run . dui.json .dui/shelf
```

Open `.dui/shelf/graph.html` from the example directory. Expect one verified 768 px capture with three books. dui generates the response to `GET /api/shelf` from the return type of `fetchShelf()`. If Chrome or Chromium is installed in a custom location, set `DUI_CHROMIUM` to its executable path. [Example source](fixtures/typed-api/main.tsx)

- [Capture reference](docs/reference.md) — configure routes and mocks, read coverage, and automate runs with CLI or MCP.
- [Contributing](CONTRIBUTING.md) — find the relevant crate, change a rule, and run the checks.
- [Design](docs/en/design-v0.md) · [Architecture](docs/en/architecture.md) · [Planned stack support](docs/en/supported-frameworks.md)

## License

[Apache License 2.0](LICENSE)
