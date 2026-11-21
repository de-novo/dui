# M1 결정적 계획 및 커버리지 게이트

[English](../en/m1-gates.md) | [简体中文](../zh-CN/m1-gates.md) | 한국어

> 이 문서는 [영어 원문](../en/m1-gates.md)의 한국어 번역이다. 내용이 다르면 영어가 기준이다.

> 상태: 2026-11-21 검증 · [아키텍처 §7](architecture.md#7-m0와-m1-완료-조건) 및 [#4](https://github.com/de-novo/dui/issues/4)의 재현 가능한 입력과 기대 출력.

## 1. 모든 M1 게이트 재현

```sh
cargo test
npm run check:capture
npm run check:m1
```

`npm run check:m1`은 [scripts/check-m1-gates.mts](../../scripts/check-m1-gates.mts)를 [fixtures/toggle](../../fixtures/toggle/)에 실행해 픽스처에 설치된 공식 TypeScript 7 컴파일러, Vite 호스트, 헤드리스 Chromium으로 `dui` CLI와 MCP 서버 양쪽을 검증한다.

## 2. 재현 가능한 입력

| 입력 | 경로 | 목적 |
|---|---|---|
| 기준 픽스처 | `fixtures/toggle/main.tsx` + `fixtures/toggle/hooks.ts` (`sha256: c5f99550ee0a141fdcec0da5f6a7705c075ceca2979789c46327d85e7f5f3f8e`) | `zod` enum(`z.infer<typeof Mode>`)과 boolean `useState`를 re-export + 별칭(`export { useState as choose }` → `import { choose as useChoice }`)으로 가져오는 React 컴포넌트 |
| 직접·별칭 변형 | `scripts/check-m1-gates.mts`에서 생성 | import 줄을 직접 `import { useState } from "react"`와 별칭 `import { useState as useChoice } from "react"`로 치환 |
| 충분한 예산 매니페스트 | `fixtures/toggle/dui.json` (`widths: [375, 1280]`, `stateBudget: 16`) | CLI와 MCP 전반에서 전체 쌍별 계획 소진 검증 |
| 부족한 예산 매니페스트 | 동일 매니페스트에 `stateBudget: 2` | 미실행 계획 칸이 `report.json`, `graph.json`, `renderable` 분모에 보존되는지 확인 |
| 정체 종료 매니페스트 | 동일 매니페스트에 `stateBudget: 16`, `maxNoImprovementRounds: 1` | 분기 커버리지가 더 개선되지 않을 때 채우기 루프가 `termination: "no-improvement"`로 종료되는지 확인 |
| 코어·셸 단위 테스트 | `crates/dui-core/src/discovery.rs`, `crates/dui/src/analysis.rs` | 사실 순서 불변성, 런타임 전용 분기(`runtimeOnlyInstrumentedBranches`), 렌더 한계(`status: "unrenderable"`, `reason: "no-render-env"`) 검증 |

## 3. 검증된 기대 출력

| 게이트 | 기대 결과 |
|---|---|
| 별칭·re-export 해석 | 직접·별칭·re-export import 모두 공식 TypeScript `7.0.2`에서 동일한 정규화 사실(`2`개 슬롯, `2`개 분기, `diagnostics: []`) 생성 |
| 충분한 예산 (CLI & MCP) | 동일한 `scan` 사실, 계획 상태 ID, 칸 ID, 판정; `coverage: { verified: 18, renderable: 18, attempted: 18, unrenderable: 0, branchOutcomes: 4, observedBranchOutcomes: 4 }`, `unobservedBranches: []`, `runtimeOnlyInstrumentedBranches: []`, `termination: "exhausted"` |
| 부족한 예산 (`stateBudget: 2`, CLI & MCP) | 동일한 계획, 칸 ID, 판정, 종료 코드 `5` / MCP `isError: true`; `attempted: 4`, `renderable: 18`(분모 보존), `remainingPlans: 7`, 미실행 칸 `14`개가 `report.json`과 `graph.json` 모두에 `unverified (not-rendered)`로 유지, `termination: "budget-limit"` |
| 정체 가드 (`maxNoImprovementRounds: 1`) | `observedBranchOutcomes: 4` 달성 후 `plannedStates: 4`, `remainingPlans: 5`, `termination: "no-improvement"`로 종료 |
| 런타임 전용 분기 및 렌더 한계 (Rust 테스트) | 런타임 전용 히트는 `observedBranchOutcomes`를 부풀리지 않고 `runtimeOnlyInstrumentedBranches`와 `runtimeOutcome` 그래프 노드로 분리 보고; 렌더러 부재 시 칸은 `unrenderable (no-render-env)`로 표시되고 `renderable`에서 제외 |
