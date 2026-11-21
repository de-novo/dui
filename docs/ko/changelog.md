# 변경 이력 (Changelog)

[English](../../CHANGELOG.md) | 한국어 | [简体中文](../zh-CN/changelog.md)

`dui`의 주요 기능 추가, 변경, 수정, 제거 사항을 마일스톤(`v0.0.1`–`v0.0.4`)별로 기록합니다.
형식은 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)를 따르며, 각 릴리스 버전은 SemVer Git 태그(`vX.Y.Z`)에 대응합니다.

## 체인지로그 관리 규칙

- `main`에 머지되는 사용자 가시적 변경(기능 추가·동작 변경·버그 수정·제거)은 `[Unreleased]` 섹션에 기록합니다.
- 항목은 **추가(Added)**, **변경(Changed)**, **수정(Fixed)**, **제거(Removed)**로 분류하고 관련 PR(`#N`)과 이슈(`#N`) 링크를 함께 남깁니다.
- 릴리스 태그 `vX.Y.Z`를 발행할 때 `[Unreleased]` 항목을 `## [vX.Y.Z] - YYYY-MM-DD`로 승격하고, 같은 PR에서 `CHANGELOG.md`·`docs/ko/changelog.md`·`docs/zh-CN/changelog.md`를 함께 갱신합니다.

---

## [Unreleased]

- 다음 예정 마일스톤: `v0.0.5` (화면·상태·폭·전환 위치에 리뷰 코멘트 부착 및 CLI/MCP/뷰어 연동, `gpui-kit` 네이티브 데스크톱 뷰어).

---

## [v0.0.4] - 2026-11-21

**핵심 목표:** 행동 기반 전환 리플레이, 단일 바이너리 내장 웹 뷰어, 태그 기반 설치·업데이트, Figma 수준의 화면 지도 가독성 및 트랙패드 조작.

### 추가 (Added)

- **행동 기반 전환 리플레이 및 엣지 출처 검증** ([#31](https://github.com/de-novo/dui/issues/31), [PR #32](https://github.com/de-novo/dui/pull/32)):
  - `dui-core::transition`에 순수 전환 판정 및 출처 규칙(`extracted` / `inferred` / `verified`) 추가.
  - `Browser::replay_transition`, `dui transitions PROJECT MANIFEST [RUN_DIR]`, MCP `transitions` 추가(동일한 멱등 JSON 반환, `graph.json` 및 `transitions.json`에 출처·검증 폭·폭별 실패 사유 기록).
  - `apps/canvas`에서 재생 검증됨(실선 오렌지), 코드 추출(파선), 추정(점선) 엣지를 시각적으로 구분하고 출처 배지와 실패 사유를 `en`·`zh-CN`·`ko`로 표시.
  - CI에 `npm run check:transitions` 연동.
- **단일 바이너리 내장 웹 뷰어 (`dui open` / `dui serve`)** ([PR #29](https://github.com/de-novo/dui/pull/29)):
  - `crates/dui/build.rs`에서 `apps/canvas` Vite 번들을 빌드해 `dui` 바이너리에 내장.
  - 인프로세스 루프백 HTTP 서버(`crates/dui/src/viewer.rs`)로 `apps/canvas`, `/archive/*`, `/api/diff` + `/api/diff/overlay`, `/api/live` + `/api/live/frame` 서빙.
  - CI에 `npm run check:viewer` 연동.
- **태그 기반 버전·자가 업데이트·소스 설치기** ([PR #29](https://github.com/de-novo/dui/pull/29), [PR #30](https://github.com/de-novo/dui/pull/30)):
  - `dui-core::version::SemVerTag`로 `vX.Y.Z` 릴리스 태그 파싱 및 숫자 비교 구현.
  - 빌드 시점 `DUI_VERSION`을 `dui --version` (`{ name, version, tag }`)에 내장하고, `dui update [--check] [--tag TAG] [--repo URL] [--install-dir DIR]` 추가.
  - `scripts/install`이 최신 `vX.Y.Z` Git 태그를 클론·빌드(`apps/canvas` 포함)하고, 셸 `PATH`(`zsh` / `bash` / `fish` / `sh`, `DUI_NO_MODIFY_PATH=1` 옵트아웃)를 자동 등록하며 쓰기 가능한 `PATH` 디렉터리에 심볼릭 링크를 생성해 설치 직후 `dui` 실행 가능.
- **Figma 방식 트랙패드 조작** ([#35](https://github.com/de-novo/dui/issues/35), [PR #36](https://github.com/de-novo/dui/pull/36)):
  - 트랙패드 두 손가락 자유 2D 이동(`panOnScroll`, `PanOnScrollMode.Free`, `zoomOnScroll={false}`), 핀치 줌(`zoomOnPinch`), `⌘`/`Ctrl`+스크롤 확대·축소, 단축키(`+`, `-`, `0` 1:1 이동, `Shift+1` 전체 보기) 적용.

### 변경 (Changed)

- **화면 지도 노드 1:1 실크기 확대 및 최대 3배 줌** ([#33](https://github.com/de-novo/dui/issues/33), [PR #34](https://github.com/de-novo/dui/pull/34)):
  - 고정 `280px` 카드와 `16:9` 크롭을 실제 캡처 폭에 맞춘 동적 `nodeMetrics(width)`(`1280×800` 데스크톱, `375×600` 모바일)로 교체하고 `maxZoom=3`, `minZoom=0.02`, 선택 화면 1:1 중앙 정렬 `1:1` 컨트롤 버튼 추가.

### 수정 (Fixed)

- **모노레포 호이스트 TypeScript 탐색** ([PR #30](https://github.com/de-novo/dui/pull/30)):
  - `crates/dui/src/main.rs`의 `scan()`이 상위 디렉터리의 `node_modules/typescript/package.json`까지 탐색하도록 수정해 호이스트 pnpm/npm 모노레포 하위 앱에서도 `dui map` / `dui run` 동작.

### 제거 (Removed)

- **레거시 정적 HTML 갤러리** ([PR #29](https://github.com/de-novo/dui/pull/29)):
  - `crates/dui/src/graph.html`, `crates/dui/src/history.html`, `adapters/browser/graph.ts`, `adapters/browser/history.ts` 제거. `dui open`은 내장 `apps/canvas` 서버로 대체하고 `dui history`는 순수 JSON 스냅샷만 반환.

---

## [v0.0.3] - 2026-11-21

**핵심 목표:** 콘텐츠 해시 증분 캐시, 캡처 간 시각적 변경 diff, 실제 네이티브 GPUI(`gpui-kit`) 렌더링 및 라이브 세션.

### 추가 (Added)

- **콘텐츠 해시 캡처 캐시 및 `dui prune`** ([#8](https://github.com/de-novo/dui/issues/8), [PR #25](https://github.com/de-novo/dui/pull/25)):
  - `adapters/typescript/facts.mts`에서 공식 TypeScript 컴파일러 `moduleGraph`를 추출해 칸별 `CacheInputs`를 의존 소스 파일·팩트·목·상태·폭·로케일·규칙 팩·`dui` 버전·렌더러 환경으로 범위화.
  - PNG를 `~/.dui/blobs/<sha256>.png`에 1회 저장 후 실행·브랜치·워크트리 간 하드링크 공유, 전체 히트 시 Vite/Chromium 미기동(`fixtures/toggle` 기준 `14,385 ms -> 520 ms`, `27.64×` 단축).
  - CLI/MCP 동일 JSON으로 칸별 히트/미스·변경 입력 원인 표시, `--force` / MCP `force: true`, `dui prune [--max-bytes BYTES]` / MCP `prune`, `npm run check:cache` 추가.
- **캡처 diff 및 픽셀 차이 오버레이** ([#11](https://github.com/de-novo/dui/issues/11), [PR #26](https://github.com/de-novo/dui/pull/26)):
  - `dui-core::diff`에 순수 비교 및 RGBA 픽셀 차이 계산 추가(미변경 칸 숨김, 칸별 `changedFiles`, `diffPixels`·`diffRatio`·`changedBounds` + `#EF803F` 오버레이, `unverified`/`unrenderable` 칸은 픽셀 차이 없이 상태 변경만 보고).
  - `dui diff BEFORE AFTER [--overlay-dir DIR]`, MCP `diff`, 뷰어 `/api/diff` + `/api/diff/overlay`, `CaptureDiffView`(`en` / `zh-CN` / `ko`), `npm run check:diff` 추가.
- **실제 GPUI(`gpui-kit`) 오프스크린 렌더링** ([#5](https://github.com/de-novo/dui/issues/5), [PR #27](https://github.com/de-novo/dui/pull/27)):
  - `gpui-kit 0.10.0` (`gpui-pre 0.3.12`, Apache-2.0) 기반 독립 픽스처 `fixtures/gpui-kit` 추가(`HeadlessAppContext` Metal/WGPU 오프스크린 렌더링 + AccessKit / `TestQueryExt` 요소 트리 관찰).
  - 공유 `Renderer` 포트를 구현하는 `GpuiRenderer`(`crates/dui/src/gpui.rs`) 추가, OS·툴체인·버전·렌더 경로·`elementTreeAvailable` 기록 및 렌더 환경 부재 시 `unrenderable` (`no-render-env`) 보고(`npm run check:gpui`).
- **네이티브 `gpui-kit` 라이브 세션** ([#6](https://github.com/de-novo/dui/issues/6), [PR #28](https://github.com/de-novo/dui/pull/28)):
  - `fixtures/gpui-kit` 영속 `--session` JSON-lines 모드와 `crates/dui/src/live/session.rs` `Backend::Gpui` 추가(웹과 동일한 `dui live` / MCP `live` 계약 및 `dui-core::edit::decide` 재사용).
  - 두 번째 뷰 인스턴스 독립 상태 유지, AccessKit 의미 근거 검증(`missing-semantic-evidence` / `semantic-mismatch`), `~/.dui/` 저장, 30회 웜 편집 벤치마크(`p50 4.22 ms / p95 4.44 ms`, 재빌드·재시작 0회, `npm run check:gpui-live`) 검증.

---

## [v0.0.2] - 2026-11-21

**핵심 목표:** 아카이브 화면 지도 뷰어, 라이브 타입드 상태 조회·편집, DDD 도메인 모델, SQLite + Drizzle 저장소, M1 커버리지 게이트, 다국어 캡처 및 뷰어 i18n.

### 추가 (Added)

- **아카이브 화면 지도 및 `apps/canvas` 뷰어** ([#14](https://github.com/de-novo/dui/issues/14), [PR #15](https://github.com/de-novo/dui/pull/15)):
  - React 19 + Tailwind CSS 4 + shadcn/ui + React Flow (`@xyflow/react`) 기반 화면 지도·링크 근거·프로젝트/브랜치/버전 히스토리 뷰어 구현.
- **타입드 요소 상태 조회** ([#1](https://github.com/de-novo/dui/issues/1), [PR #17](https://github.com/de-novo/dui/pull/17)):
  - `dui-core::inspect`, `dui inspect` / MCP `inspect`로 불리언·열거형 컨트롤의 소유자·인스턴스·소스 위치·타입·현재 값·옵션·편집 가능 여부 반환.
- **영속 라이브 웹 세션·웜 편집 벤치마크·저장** ([#2](https://github.com/de-novo/dui/issues/2), [#3](https://github.com/de-novo/dui/issues/3), [#7](https://github.com/de-novo/dui/issues/7), [PR #18](https://github.com/de-novo/dui/pull/18), [PR #19](https://github.com/de-novo/dui/pull/19), [PR #20](https://github.com/de-novo/dui/pull/20)):
  - 루프백 라이브 세션 소유자(`dui live` / MCP `live`), `dui-core::edit` Decider(`open -> inspect -> set/reset -> save -> close`), 가시 DOM 단언 검증, 리비전·요청 ID 멱등성, 30회 웜 편집 벤치마크(`npm run check:inspect`), 화면 지도 내 라이브 편집 UI 통합.
- **DDD 도메인 모델 및 저장소 포트** ([#9](https://github.com/de-novo/dui/issues/9), [PR #21](https://github.com/de-novo/dui/pull/21)):
  - `dui-core::model` 및 `dui-core::storage`(`MemoryStore`)에 순수 애그리게이트·값 객체·불변식·분리된 저장소 포트 정의 및 3개국어 `domain-model.md` 문서화.
- **SQLite + Drizzle 메타데이터 저장소** ([#10](https://github.com/de-novo/dui/issues/10), [PR #22](https://github.com/de-novo/dui/pull/22)):
  - `crates/dui/src/sqlite_store.rs`(`rusqlite` 번들 SQLite WAL 모드)와 `packages/db/`(`drizzle-orm` `1.0.0-rc.5` SQLite 방언)가 `packages/db/migrations/` SQL 마이그레이션을 공유하고 `~/.dui/projects/`에서 무손실 가져오기·복구 지원.
- **M1 결정적 계획 및 커버리지 게이트** ([#4](https://github.com/de-novo/dui/issues/4), [PR #23](https://github.com/de-novo/dui/pull/23)):
  - 팩트 순서 불변성 강화, 보강 루프 종료 사유(`exhausted`, `budget-limit`, `no-improvement`) 기록, 런타임 전용 분기(`runtimeOutcome`) 구분 및 `npm run check:m1` 추가.
- **다국어 캡처·i18n 감사 및 뷰어 언어 전환** ([#12](https://github.com/de-novo/dui/issues/12), [PR #24](https://github.com/de-novo/dui/pull/24)):
  - CDP 로케일 에뮬레이션 기반 `screen × state × width × locale` 캡처, 번역 키 누락 대조, DOM 텍스트 넘침 선택자 탐지, RTL 불일치 검사(`dui-core::locale`), 뷰어 UI 언어 전환(`en` / `zh-CN` / `ko`)·로케일 필터 및 `npm run check:i18n` 추가.

---

## [v0.0.1] - 2026-11-20

**핵심 목표:** M0 캡처 기준선 — 공식 컴파일러 분석, Vite + Chromium 캡처, 타입드 자동 목, 로컬 아카이빙, 소스 설치기.

### 추가 (Added)

- **함수형 코어 & 포트 아키텍처 (`dui-core`, `crates/dui`)**:
  - I/O·비동기 없는 순수 Rust `dui-core`와 3개 포트 계약(`TypeOracle`, `Host`, `Renderer`), 3개국어 아키텍처·설계·워크플로·지원 프레임워크 문서 정립.
- **공식 TypeScript 6/7 컴파일러 분석 및 타입드 자동 목**:
  - 프로젝트에 설치된 공식 `typescript` 6.x 또는 7.x 컴파일러에서 React 상태 슬롯·분기·컴포넌트 렌더·타입드 `fetch().json()` 응답 형태를 추출(`adapters/typescript/facts.mts`)하고 결정적 목 응답 생성(`dui-core::mock`).
- **Vite 호스트·Chromium CDP 캡처·화면 자동 발견**:
  - `dui scan`, `dui run`, `dui map`, `dui replay`, `dui replay-report`, `dui history`, MCP stdio 서버(`dui mcp`) 구현 및 `375`·`768`·`1280` px 다폭 캡처·TanStack 라우트/동일 출처 앵커 탐색 지원.
  - `~/.dui/projects/` 아래 Git 프로젝트·브랜치·워크트리 출처와 함께 아카이브 저장(`npmgraph` 예제 및 M0 픽스처 검증).
- **소스 설치기 및 릴리스 패키징**:
  - `scripts/install`, `scripts/package-release.mts`, `dependency-catalog.json` 버전·라이선스 검증(`npm run deps:check`) 추가.
