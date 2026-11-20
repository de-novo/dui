# Contributing to dui

Start with the [first capture](README.md#first-capture) to build the binary and see the output a user receives.

## Where a change belongs

| Change | Location |
| --- | --- |
| State combinations, representative data, verification criteria | [dui-core](crates/dui-core/src) |
| CLI, MCP, browser sessions, result files, replay | [dui](crates/dui/src) |
| Compiler queries, Vite instrumentation, code that runs in the browser | [TypeScript adapters](adapters) |
| Meaning of a framework’s imported symbols | [Rule pack](rules/web.json) and its [schema](rules/schema.json) |

Keep planning and verification deterministic. The shell owns compiler processes, files, and browser sessions. The [ports](crates/dui/src/ports.rs) define the contracts between the shell and its adapters. Write first-party browser and Node code in TypeScript 7.

## Changing analysis or capture

Describe the input that exposes the problem and what should appear in the report. Reproduce it with the smallest fixture that exercises the normal CLI path. The [typed API fixture](fixtures/typed-api) covers response synthesis, [toggle](fixtures/toggle) covers state changes, and [replay](fixtures/replay) covers a saved plan.

Keep application routes, data samples, and selectors in the capture manifest or fixture. A framework rule matches an imported package and symbol. Include a fixture that demonstrates the match when adding a rule. For verification changes, check both a passing cell and a cell whose evidence is missing.

## Local checks

Run from the repository root after installing dependencies with `npm ci`.

```sh
cargo fmt --all --check
npm exec -- tsc --noEmit
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

For a host, mock, or browser change, also run the [first capture](README.md#first-capture). Check the generated response in `facts.json`, the recorded request in `report.json`, and the screenshot in `graph.html`. Include those observations when describing the change.

Use dependencies under permissive licenses such as MIT, Apache-2.0, BSD, or ISC. Keep capture output, installed dependencies, and third-party project checkouts out of the patch.

## Package a release

From a clean checkout with Node.js 26, npm, Rust, and tar installed, run the following commands.

```sh
npm ci
npm run package:release
```

The script builds for the Rust host target. It writes `dui-<version>-<target>.tar.gz` and its `.sha256` file to the Cargo target directory under `dist/`. Build on each supported target and test the extracted executable there before distributing it. Both Cargo packages are private to the workspace. [Installation from an archive](docs/binary-install.md)
