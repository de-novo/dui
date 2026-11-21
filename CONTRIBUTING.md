# Contributing to dui

Thanks for your interest in dui (de-novo-ui). dui follows an **Open-Core**
licensing model:

- All files outside `ee/` directories are licensed under the
  [Apache License 2.0](LICENSE).
- All files inside `ee/` directories are licensed separately under the
  [dui Enterprise License](ee/LICENSE).

By contributing to files outside `ee/`, you agree that your contributions are
licensed under the Apache License 2.0. By contributing to files inside `ee/`,
you grant de-novo a perpetual, worldwide, irrevocable, royalty-free license to
use, distribute, and sublicense your contribution under both the dui Enterprise
License and commercial terms.

## 1. Sign off every commit (DCO)

We use the [Developer Certificate of Origin 1.1](https://developercertificate.org/)
instead of a CLA. Every commit must carry a `Signed-off-by` line that matches
the commit author:

```
git commit -s -m "docs: fix typo"
```

By signing off, you certify that you wrote the change or otherwise have the
right to submit it under the project's license. Commits without a sign-off
cannot be merged.

## 2. Clean-room rule

dui learns from many projects, but it copies code from none of them.

- **Ideas only.** Projects mentioned in our docs as references (for example
  React Compiler, GATOR, CodeGraph, react-docgen-typescript,
  eslint-plugin-react-hooks, XState) inform design. Write your own code.
- **Do not open restricted code while implementing.** Do not read, copy, or
  paraphrase source code under GPL, AGPL, LGPL, PolyForm Noncommercial, SSPL,
  BSL, the tldraw license, or other restrictive or proprietary terms (for
  example GitNexus, the Serena application, the Semgrep engine, FlowDroid,
  the CodeQL CLI). Public documentation and papers at the concept level are
  fine.
- **Permissive snippets need attribution.** If a snippet from a permissively
  licensed project (MIT, Apache-2.0, BSD, ISC) must be copied, record the
  original path, commit, and license in `THIRD_PARTY_NOTICES` in the same PR.
- **AI-written code follows the same rule.** If you use an AI assistant, tell it
  not to reproduce code from other projects, and review its output with that
  in mind. You are responsible for what you submit.

## 3. Permissive dependencies only

New dependencies must be licensed under MIT, Apache-2.0, BSD (2- or 3-clause),
or ISC. Copyleft, noncommercial, source-available, and "production requires a
commercial key" licenses are not accepted. CI checks dependency licenses
(for example `license-checker` for npm and `cargo-deny` for Rust). If you are
unsure about a license, ask in the PR before adding the dependency.

Build tooling may use unmodified `lightningcss` and its official platform binaries transitively. It may also install `tslib` (0BSD) as an optional transitive development dependency. Both exceptions require exclusion from distributed artifacts. See [Architecture §6.2](docs/en/architecture.md#62-automated-checks-ci-fails-on-any-violation).

Do not use other projects' names or trademarks (Storybook, Figma, Graphify,
...) in dui product or package names. Using them to describe compatibility is
fine.

## 4. Translation rule

dui is trilingual: English, Simplified Chinese (zh-CN), and Korean (ko).
**English is the source of truth.** See [docs/i18n.md](docs/i18n.md).

- Change the English file first.
- In the same PR, update `zh-CN` and `ko`, or add the "outdated" banner from
  `docs/i18n.md` to the files you could not update.
- Keep numbers, links, code, tables, and diagrams identical across languages.
  Use the glossary in `docs/i18n.md` for key terms.

## 5. Discussions, issues, and pull requests

How work moves through GitHub (Discussions, issues, labels, milestones, the
project board, PRs, merge conditions, and approvals) is defined in
[docs/en/workflow.md](docs/en/workflow.md). In short:

- Proposals and open questions start in Discussions (`Ideas` or `RFC`).
  Decisions are written into the English docs.
- Work is tracked as an issue with done criteria, one type, priority, and
  area label, a milestone, and a place on the project board.
- One PR, one purpose. The PR body says `Closes #N` and includes the checks
  run and evidence.
- A PR is merged when checks pass, there are no conflicts, and no review
  blockers remain.
- AI coding agents start from [AGENTS.md](AGENTS.md).
- Do not commit secrets, credentials, or internal information from other
  projects.

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

Edit [dependency-catalog.json](dependency-catalog.json) to update an npm library or tool.
Run `npm run deps:sync` from the root with Node 26, then commit the changed manifests
and lockfiles together. New direct dependencies need a catalog entry and a declaration
in the consuming package. The root, canvas, and fixtures each support a standalone
`npm ci`. To install updated versions locally, run `npm ci` in each affected package.

Declare Rust dependencies in the root `Cargo.toml` under `[workspace.dependencies]`.
Member crates inherit them with `workspace = true`. Commit resolved version changes in
`Cargo.lock`.

Run from the repository root after installing dependencies with `npm ci`.

```sh
npm run deps:check
npm run check:style
cargo fmt --all --check
npm exec -- tsc --noEmit
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

Oxlint checks TypeScript in `adapters`, `apps`, `fixtures` and `scripts`. Warnings fail the check. Oxfmt formats those TypeScript and CSS sources with two spaces, double quotes, semicolons and a 100-column print width. Use `npm run format` to apply the format. The Source style workflow runs the same `npm run check:style` command on pull requests and pushes to main. Type checking uses the official TypeScript compiler. Rust uses Clippy and rustfmt.

For a host, mock, or browser change, also run the [first capture](README.md#first-capture). Check the generated response in `facts.json`, the recorded request in `report.json`, and the screenshot in `graph.html`. Include those observations when describing the change.

Use dependencies under permissive licenses such as MIT, Apache-2.0, BSD, or ISC. Keep capture output, installed dependencies, and third-party project checkouts out of the patch.

For changes to capture instrumentation, install the public fixture dependencies with `npm ci --prefix fixtures/toggle` and run `npm run check:capture` with Node 26. This checks the transformed TSX with the fixture’s TypeScript compiler, builds it with Vite, and scans the clean production artifacts for probe markers.

## Package a release

From a clean checkout with Node.js 26, npm, Rust, and tar installed, run the following commands.

```sh
npm ci
npm run package:release
```

The script builds for the Rust host target. It writes `dui-<version>-<target>.tar.gz` with its `.sha256` file and the versioned release’s `install.sh` to the Cargo target directory under `dist/`. Build on each supported target and test the extracted executable there before distributing it. For an approved GitHub release, use tag `v<version>` in `de-novo/dui` and attach `install.sh` once alongside the archives and checksums for all released targets. The stable installer URL resolves through GitHub’s latest release. Packaging produces local files only. Both Cargo packages are private to the workspace. [Installation from an archive](docs/binary-install.md)
