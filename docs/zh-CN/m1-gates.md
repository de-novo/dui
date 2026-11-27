# M1 确定性规划与覆盖率验收门禁

[English](../en/m1-gates.md) | 简体中文 | [한국어](../ko/m1-gates.md)

> 本文是[英文原文](../en/m1-gates.md)的简体中文翻译。如有出入，以英文为准。

> 状态：2026-11-21 已验证 · [架构 §7](architecture.md#7-m0-和-m1-验收标准) 与 [#4](https://github.com/de-novo/dui/issues/4) 的可复现输入与预期输出。

## 1. 复现全部 M1 门禁

```sh
cargo test
npm run check:capture
npm run check:m1
```

`npm run check:m1` 针对 [fixtures/toggle](../../fixtures/toggle/) 运行 [scripts/check-m1-gates.mts](../../scripts/check-m1-gates.mts)，使用固件中安装的官方 TypeScript 7 编译器、Vite 主机与无头 Chromium，同时验证 `dui` CLI 与 MCP 服务器。

## 2. 可复现输入

| 输入 | 路径 | 目的 |
|---|---|---|
| 基准固件 | `fixtures/toggle/main.tsx` + `fixtures/toggle/hooks.ts`（`sha256: c5f99550ee0a141fdcec0da5f6a7705c075ceca2979789c46327d85e7f5f3f8e`） | 使用 `zod` 枚举（`z.infer<typeof Mode>`）与布尔 `useState` 的 React 组件，通过重新导出加别名导入（`export { useState as choose }` → `import { choose as useChoice }`） |
| 直接与别名变体 | 由 `scripts/check-m1-gates.mts` 生成 | 将导入行替换为直接 `import { useState } from "react"` 与别名 `import { useState as useChoice } from "react"` |
| 完整规划清单 | `fixtures/toggle/dui.json`（`widths: [375, 1280]`） | 在 CLI 与 MCP 中验证完整的成对规划耗尽 |
| 收敛保护清单 | 同一清单设置 `maxNoImprovementRounds: 1` | 确认分支覆盖不再提升时以 `termination: "no-improvement"` 终止，并将未执行的规划格子完整保留在 `report.json`、`graph.json` 与兼容的计划可渲染 `renderable` 计数中，同时排除在 `capturedRenderable` 覆盖率分母之外 |
| 核心与外壳单元测试 | `crates/dui-core/src/discovery.rs`、`crates/dui/src/analysis.rs` | 验证事实顺序不变性、带动态 ID 模式折叠的无环多跳 BFS、仅运行时分支（`runtimeOnlyInstrumentedBranches`）与渲染限制（`status: "unrenderable"`，`reason: "no-render-env"`） |

## 3. 已验证的预期输出

| 门禁 | 预期结果 |
|---|---|
| 别名与重新导出解析 | 直接、别名与重新导出三种导入在官方 TypeScript `7.0.2` 下产生完全一致的规范事实（`2` 个槽、`2` 个分支、`diagnostics: []`） |
| 完整规划（CLI & MCP） | `scan` 事实、规划状态 ID、格子 ID 与判定完全一致；`coverage: { planned: 18, captured: 18, verified: 18, capturedRenderable: 18, renderable: 18, attempted: 18, unrenderable: 0, branchOutcomes: 4, observedBranchOutcomes: 4 }`、`unobservedBranches: []`、`runtimeOnlyInstrumentedBranches: []`、`termination: "exhausted"` |
| 收敛保护（`maxNoImprovementRounds: 1`，CLI & MCP） | 规划、格子 ID、判定、退出码 `5` / MCP `isError: true` 完全一致；停在 `plannedStates: 4`（`attempted: 8`、`renderable: 18` 为兼容计划计数，`capturedRenderable: 8` 为覆盖率分母、`remainingPlans: 5`、`observedBranchOutcomes: 4`、`10` 个未尝试格子在 `report.json` 与 `graph.json` 中均标记为 `unverified (not-rendered)`，`termination: "no-improvement"`） |
| 仅运行时分支与渲染限制（Rust 测试） | 仅运行时命中的分支单独报告在 `runtimeOnlyInstrumentedBranches` 与 `runtimeOutcome` 图节点中，不计入 `observedBranchOutcomes`；缺少渲染器时格子标记为 `unrenderable (no-render-env)` 并从 `renderable` 中排除 |
