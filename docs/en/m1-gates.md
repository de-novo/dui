# M1 deterministic planning and coverage gates

English | [简体中文](../zh-CN/m1-gates.md) | [한국어](../ko/m1-gates.md)

> Status: verified 2026-11-21 · reproducible inputs and expected outputs for [Architecture §7](architecture.md#7-m0-and-m1-gates) and [#4](https://github.com/de-novo/dui/issues/4).

## 1. Reproducing all M1 gates

```sh
cargo test
npm run check:capture
npm run check:m1
```

`npm run check:m1` runs [scripts/check-m1-gates.mts](../../scripts/check-m1-gates.mts) against [fixtures/toggle](../../fixtures/toggle/) using the fixture's installed official TypeScript 7 compiler, Vite host, and headless Chromium through both the `dui` CLI and MCP server.

## 2. Reproducible inputs

| Input | Path | Purpose |
|---|---|---|
| Reference fixture | `fixtures/toggle/main.tsx` + `fixtures/toggle/hooks.ts` (`sha256: c5f99550ee0a141fdcec0da5f6a7705c075ceca2979789c46327d85e7f5f3f8e`) | React component with `zod` enum (`z.infer<typeof Mode>`) and boolean `useState`, imported via re-export + alias (`export { useState as choose }` → `import { choose as useChoice }`) |
| Direct & aliased variants | Generated in `scripts/check-m1-gates.mts` | Replaces the import line with direct `import { useState } from "react"` and aliased `import { useState as useChoice } from "react"` |
| Full-budget manifest | `fixtures/toggle/dui.json` (`widths: [375, 1280]`, `stateBudget: 16`) | Exercises full pairwise plan exhaustion across CLI and MCP |
| Insufficient-budget manifest | Same manifest with `stateBudget: 2` | Confirms unattempted planned cells remain in `report.json`, `graph.json`, and the `renderable` denominator |
| Stagnation-guard manifest | Same manifest with `stateBudget: 16`, `maxNoImprovementRounds: 1` | Confirms the fill-in loop terminates with `termination: "no-improvement"` once branch coverage stops improving |
| Pure core & shell unit tests | `crates/dui-core/src/discovery.rs`, `crates/dui/src/analysis.rs` | Verifies fact-order invariance, runtime-only branches (`runtimeOnlyInstrumentedBranches`), and render limits (`status: "unrenderable"`, `reason: "no-render-env"`) |

## 3. Verified expected outputs

| Gate | Expected result |
|---|---|
| Alias & re-export resolution | Direct, aliased, and re-exported imports produce identical normalized facts (`2` slots, `2` branches, `diagnostics: []`) from official TypeScript `7.0.2` |
| Full budget (CLI & MCP) | Identical `scan` facts, planned state IDs, cell IDs, and verdicts; `coverage: { verified: 18, renderable: 18, attempted: 18, unrenderable: 0, branchOutcomes: 4, observedBranchOutcomes: 4 }`, `unobservedBranches: []`, `runtimeOnlyInstrumentedBranches: []`, `termination: "exhausted"` |
| Insufficient budget (`stateBudget: 2`, CLI & MCP) | Identical plan, cell IDs, verdicts, exit code `5` / MCP `isError: true`; `attempted: 4`, `renderable: 18` (denominator preserved), `remainingPlans: 7`, `14` unattempted cells marked `unverified (not-rendered)` in both `report.json` and `graph.json`, `termination: "budget-limit"` |
| Stagnation guard (`maxNoImprovementRounds: 1`) | Stops at `plannedStates: 4` with `observedBranchOutcomes: 4`, `remainingPlans: 5`, `termination: "no-improvement"` |
| Runtime-only branches & render limits (Rust tests) | Runtime-only hits appear in `runtimeOnlyInstrumentedBranches` and as `runtimeOutcome` graph nodes without inflating `observedBranchOutcomes`; missing renderer marks cells `unrenderable (no-render-env)` excluded from `renderable` |
