# AGENTS.md

Entry point for AI coding agents (and humans) working on `de-novo/dui`. Read this first, then follow the links.
Updated: 2026-11-27 · Stage: v0.2.0 workspace crates (`dui-core`, `dui-ports`, `dui-storage`, `dui-app`, `dui`) & fact-only core ready · Next goal: the [default pipeline](docs/en/roadmap.md#default-pipeline) (AST model, approximate renderer)

## Mission

dui reads existing UI code, reassembles screens, relations, slots, declared state, item schemas, and styles into a versioned model, and draws them with its own approximate renderer. Official compilers may refine types. Official-engine captures are optional pixel verification, never the default path. AI agents drive it through the `dui` CLI and MCP server; humans only look at the canvas. Nothing is invented: what cannot be shown is a placeholder, `unverified`, or `unrenderable`.

## Source-of-truth docs (English is canonical)

| Topic | Doc |
|---|---|
| Product design, principles, milestones | [docs/en/design-v0.md](docs/en/design-v0.md) |
| Web viewer structure, session protocol and acceptance gates | [Web viewer architecture](docs/en/web-viewer.md) |
| Code structure, patterns, crate boundaries, M0/M1 gates | [docs/en/architecture.md](docs/en/architecture.md) |
| Supported stacks and versions | [docs/en/supported-frameworks.md](docs/en/supported-frameworks.md) |
| Discussions, issues, labels, milestones, board, PRs, approvals | [docs/en/workflow.md](docs/en/workflow.md) |
| Translation rules and glossary | [docs/i18n.md](docs/i18n.md) |
| DCO, clean-room, licenses | [CONTRIBUTING.md](CONTRIBUTING.md) |

Do not guess specs, terms, versions, or API names. Check the docs. If the docs do not answer, add `needs-decision` to the issue with the question and a recommendation.

## Repo settings

| Item | Value |
|---|---|
| Languages | Docs in en (source of truth), zh-CN, ko. Code identifiers, comments, issues, and PRs in English |
| area labels | `area:core`, `area:ports`, `area:adapters`, `area:app`, `area:rules`, `area:canvas`, `area:docs`, `area:tooling` |
| Checks | [Architecture §6.2](docs/en/architecture.md#62-automated-checks-ci-fails-on-any-violation) plus the touched crates' tests and `insta` snapshots. |
| PR evidence | Rendered images or CLI / MCP output for behavior changes |
| Milestone due dates | Only when the maintainer sets one |
| Merge | Checks pass + no conflicts + no review blockers → the lead may merge. Merge commit, not squash |
| Board | One Projects board, `Status`: Todo / In progress / In review / Done. No extra fields |
| Branches | `feat/` `fix/` `refactor/` `chore/` `docs/` + short name, target `main` |
| Commits | Conventional Commits, every commit signed off (`git commit -s`, DCO) |

## Product constraints

1. **Official-compiler-dependent.** When type information is used, it comes only from each language's official compiler / toolchain already in the user's project (TypeScript: `tsc` 7.0+). Never reimplement type checking. Unofficial compilers are `unsupported`. Syntax-only extraction may proceed without types.
2. **Latest stable major only.** Older majors are `unsupported` (a project-level status, not a cell status; excluded from coverage).
3. **Deterministic combinations.** States and combinations come from rules, not from AI. AI only fills gaps, marked as inferred.
4. **Rendering limits are stated, not hidden.** A cell that cannot be drawn is `unrenderable (reason)`, separate from `unverified`.
5. **No audience declaration.** Do not add "who it's for" or similar audience lines to any doc.
6. **No other products' names or trademarks** in dui product or package names, and none in the positioning line.
7. **No memory-heavy rendering on the default path.** See [Never](#never).

## Never

The default path for `dui map`, `dui open`, hot reload, and drawing cards **must not** launch Chromium or any other memory-heavy renderer: no headless browser, no live browser session, and no emulator or simulator. Empty or incomplete cards stay empty or show a placeholder. A heavy renderer is allowed only as an explicit opt-in for pixel verification (`--capture`, `dui run`, or a render machine). It is never a fallback that fills cards on the default path.

This rule is non-negotiable. Do not add a default-path feature that starts a browser or emulator "just this once," or "until the approximate renderer is ready." The same rule is recorded in [Architecture §3](docs/en/architecture.md#never-default-path) and the [roadmap](docs/en/roadmap.md#never).

## Code rules

For viewer UI changes, choose the skill for the stage of work:

- **Design** with [Uizze anti-ui-slop](.agents/skills/anti-ui-slop/SKILL.md). Use real dui captures to decide layout, hierarchy and empty states.
- **Implement** with [shadcn/ui](.agents/skills/shadcn/SKILL.md). Reuse viewer tokens and shared controls.
- **Review** with [Vercel Web Interface Guidelines](.agents/skills/web-design-guidelines/SKILL.md). Check keyboard access, focus, navigation, responsive layout and themes.
- **Self-dogfood before shipping**: verify every `apps/canvas` or `site/` change across both mobile (`390x844`) and desktop (`1440x920`) viewports (zero horizontal overflow, zero clipped primary controls, zero broken images, and non-blank `renderTree` captures via `npm run verify:pages-demo` for Pages deploys).

The [viewer contract](docs/en/web-viewer.md) defines archive evidence, capture status and map behavior.

1. Rust, edition 2024. One binary, `dui`.
2. **Functional core, imperative shell.** `dui-core` has no I/O, no async, no clock, no randomness; same input, same output.
3. **Exactly three ports**: `TypeOracle`, `Host`, `Renderer` in `dui-ports`. No `if framework == …` in the core.
4. **Dependency direction** ([Architecture §6](docs/en/architecture.md#6-dependency-direction-and-enforcement)):
   - `dui-core` depends on nothing in the workspace.
   - `dui-ports` depends only on `dui-core`.
   - Each adapter depends only on `dui-ports` / `dui-core`, never on another adapter.
   - `dui-app` depends on no concrete adapter.
   - Only `dui-cli` wires concrete adapters and produces a binary.
5. Denied lints: `unwrap_used`, `wildcard_enum_match_arm`; `iter_over_hash_type` in the core. Do not loosen a lint or a check to make it pass; fix the code.
6. New stacks: a language adds `dui-lang-<x>`, a dev server `dui-host-<x>`, a renderer `dui-render-<x>`; most frameworks add only a rule pack + fixture + `stacks.toml` entry.
7. **Dependencies: permissive licenses only** (MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib). See [Architecture §6.2](docs/en/architecture.md#62-automated-checks-ci-fails-on-any-violation) for the build-only `lightningcss` exception. dui is Apache-2.0.
8. **Clean-room.** Ideas only from other projects; never copy or paraphrase their code. See [CONTRIBUTING §2](CONTRIBUTING.md#2-clean-room-rule).

## Workflow rules

Full rules: [docs/en/workflow.md](docs/en/workflow.md).

1. **Flow**: Discussion (`Ideas` or `RFC`) → conclusion written into the English doc → Issue → milestone + board → branch → PR (`Closes #N`, evidence) → merge. Small changes: Issue → PR.
2. **Issues**: one task + done criteria (checklist) + doc links. Labels: exactly 1 type (`bug` `feature` `refactor` `chore` `docs`), 1 priority (`P0` `P1` `P2`), 1 area. Helpers: `needs-decision`, `blocked` only. Every open issue has a milestone and is on the board.
3. **State lives on the board**, never in labels or issue bodies. Decisions live in docs, never only in Discussions or chat.
4. **Milestones** are `vX.Y.Z` (M0 = `v0.0.1`, M1 = `v0.0.2`, …), at most 2 open.
5. **Three-language docs**: change English first; in the same PR update zh-CN and ko or add the "outdated" banner. Keep numbers, links, code, tables, and diagrams identical; keep headings and tables aligned.
6. **PR body**: summary, `Closes #N` + doc links, checks run and results, evidence, docs changed (or why not), approval quote for irreversible steps.

## Approvals

Do reversible work directly and report it. These need the maintainer's explicit approval first; an agent's agreement is not approval:

- making the repository or any artifact public;
- release candidate or release tags, publishing packages;
- force-push, rewriting `main` history, skipping checks (`--no-verify`);
- deleting labels, milestones, Discussions, or other GitHub objects;
- spending money or adding paid services.

Never write secrets, tokens, one-time codes, or private data into the repo, issues, PRs, or Discussions.
