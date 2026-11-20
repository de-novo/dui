# dui

English | [简体中文](README.zh-CN.md) | [한국어](README.ko.md)

dui captures a running web app across UI states and viewport widths. It reads the app’s TypeScript types, generates mock responses, and checks what Chromium actually rendered. Browse the screenshots in a local HTML gallery or consume the same results as JSON through the CLI or MCP.

## Install

Install from source on macOS or Linux with Git, curl, and a C toolchain (Xcode Command Line Tools on macOS).

```sh
curl -fsSL https://github.com/de-novo/dui/raw/main/scripts/install | bash
```

The installer reuses Rust and Node.js 26 with npm when available. It downloads missing build tools into a temporary directory and removes them after the build. It installs `dui` in `~/.local/bin`. Set `DUI_INSTALL_DIR` to choose another directory. Web capture needs Node.js 26 and Chromium at runtime. [Prebuilt binary installation](docs/binary-install.md)

## First capture

To build the example from source, you need a Unix environment, Rust with edition 2024 support, Node.js 26 with npm, and a locally installed Chromium or Google Chrome. If dui cannot find the browser, set `DUI_CHROMIUM` to its executable path.

Run the following commands from the repository root.

```sh
npm ci
cargo build --release --locked
npm ci --prefix fixtures/typed-api
target/release/dui run fixtures/typed-api fixtures/typed-api/dui.json .dui/shelf
```

Open `.dui/shelf/graph.html` in a browser. You should see one verified capture at 768 px with three books. The app declares the return type of `fetchShelf()`. dui uses that type to generate the response to `GET /api/shelf`. The capture waits for the third book to appear. [Fixture source](fixtures/typed-api/main.tsx) · [Capture settings](fixtures/typed-api/dui.json)

## Use your own app

The current web capture path uses Vite 8 and Chromium. Install the target project’s dependencies first. dui queries the project’s official TypeScript 6.x or 7.x compiler. React projects use React 19. dui’s own adapters are built with TypeScript 7.

A capture needs an entry path, viewport widths, and a CSS selector that identifies the rendered content. For apps with several screens, dui can collect navigation links from the entry page. Each route, state, and width produces one **cell** in the report.

- [Capture reference](docs/reference.md) — configure routes and mocks, read coverage, and automate runs with CLI or MCP.
- [Contributing](CONTRIBUTING.md) — find the relevant crate, change a rule, and run the checks.
- [Design](docs/en/design-v0.md) · [Architecture](docs/en/architecture.md) · [Planned stack support](docs/en/supported-frameworks.md)

## License

[Apache License 2.0](LICENSE)
