# 변경 이력 (Changelog)

[English](../../CHANGELOG.md) | 한국어 | [简体中文](../zh-CN/changelog.md)

이 프로젝트의 주요 변경 사항을 기록합니다.
형식은 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)를 따르며, 버전은 [Semantic Versioning](https://semver.org/spec/v2.0.0.html)을 준수합니다.

## [Unreleased]

### Added

- 모바일·TUI 컴파일러 심층 상태 추출 룰팩(Ink 5 · React Native · Expo Router 5), 공용 컴파일러 오라클 IR 및 공식 6개 툴체인 진단(`dui toolchains`) 추가 ([#124](https://github.com/de-novo/dui/pull/124)).
- 2D 캔버스 직접 라이브 컴포넌트 마운트(`Static | Live mount`, 루프백 검증, `postMessage` 실시간 `RenderNodeSnapshot` → `ElementInspector` 스트리밍) 및 원커맨드 `dui open --live` / `--live-url`, GPUI 데스크톱 라이브 표시 패리티 추가 ([#130](https://github.com/de-novo/dui/pull/130), [#138](https://github.com/de-novo/dui/pull/138)).
- SemVer 및 3개 언어 `CHANGELOG` 버전 동기화 CI 가드(`npm run check:version`)를 `check:style`에 연동 ([#131](https://github.com/de-novo/dui/pull/131)).
- 공식 TypeScript 6/7 AST 폴백(`adapters/vite/parser.mts`, `npm run check:vite7`)으로 Vite 7 정식(Level 2) 지원 추가 ([#133](https://github.com/de-novo/dui/pull/133)).
- Vite 7·8 양쪽에서 동작하는 React Router 7/8 정적 라우트 자동 발견(`createBrowserRouter`, `createHashRouter`, `createMemoryRouter`, 중첩 JSX `<Route>`, `index`, `:param`, `*` splat) 추가 ([#134](https://github.com/de-novo/dui/pull/134), [#135](https://github.com/de-novo/dui/pull/135)).
- 대화형 브라우저 로그인 세션 캡처(`dui auth login <URL> --out FILE`) 추가 — `HttpOnly` 쿠키·`localStorage`·`sessionStorage`를 `0600` 권한의 Playwright 호환 `storageState.json`으로 저장 ([#137](https://github.com/de-novo/dui/pull/137)).
- 뷰어 UI 설계·검수용 에이전트 스킬(`anti-ui-slop`, `web-design-guidelines`) 등록 ([#132](https://github.com/de-novo/dui/pull/132)).

### Changed

- 생태계·룰팩 라이브러리를 하드 버전 게이트에서 분리: 하드 `>=` 기준선 검사는 핵심 컴파일러·호스트·UI 런타임(`tsc`, `vite` 7+, `react` 18+, `host: "next"` 시 `next` 16+)에만 적용하고, 더 새로운 안정 메이저는 항상 허용하며 룰팩 라이브러리 버전 차이는 스캔 중단 없이 비치명적 `versionNotes`로 기록 ([#136](https://github.com/de-novo/dui/pull/136)).

## [v0.1.0] - 2026-11-23

### Added

- 범용 인증 세션 지원(`storageState.json` · `localStorage` · `sessionStorage` · `extraHeaders` · chunked 응답 지원 `POST authFlow` · `env:` 비밀값 간접 참조/리댁션 · 인증별 캐시 분리) 및 GitHub PR 코멘트용 `dui diff ... --markdown` 출력 ([#128](https://github.com/de-novo/dui/pull/128)).
- iOS Simulator 서피스 어댑터(`host: "ios"`, `xcrun simctl` 스크린샷 + XCUI 접근성 트리 → `RenderNodeSnapshot`) 및 `macos-required` / `no-ios-simulator` 정직 보고 ([#123](https://github.com/de-novo/dui/pull/123)).
- GPUI 데스크톱 뷰어(`apps/desktop`)에 개발자도구형 요소 인스펙터 · 중첩 박스 모델 다이어그램 · 디자인 결함 오버레이 패리티 추가 ([#122](https://github.com/de-novo/dui/pull/122)).

### Changed

- TypeScript 오라클 런타임 요구 사항을 Node 26 고정에서 Node 22+ LTS(`22` / `24` / `26+`)로 완화 ([#126](https://github.com/de-novo/dui/pull/126)).
- `designAudit` 결함을 `(kind, path, width, nodeId)` 기준으로 중복 제거(`occurrences` · `rawTotal` 포함)하고 겹침·터치 타깃·간격 편차 오탐 기준을 강화 ([#126](https://github.com/de-novo/dui/pull/126)).
- 워크스페이스 버전을 `apps/desktop`과 맞춰 `0.1.0`으로 동기화.

### Fixed

- 탐색한 페이지 문서가 HTTP `4xx`/`5xx`(404 · 500 등)를 반환하면 `unverified` (`http-error`)로 실패 처리(`/favicon.ico` 404 오탐 방지) ([#126](https://github.com/de-novo/dui/pull/126)).
- TUI 캡처(`htop`, `fzf`)용 VT100 `CSI b` (`REP`) / `CSI X` (`ECH`), 블록 문자(`▌` `▏` `█`), 소문자 `p`/`q` 글리프 정렬 및 텍스트 셀 대비 샘플링 수정 ([#117](https://github.com/de-novo/dui/pull/117)).

## [v0.0.9] - 2026-11-23

### Added

- Android 모바일 서피스 어댑터(`host: "android"`, `adb screencap` + `uiautomator` XML → `RenderNodeSnapshot`) ([#111](https://github.com/de-novo/dui/pull/111)).
- Electron 45 데스크톱 어댑터(`host: "electron"` + `ipcMocks`) 및 GPUI AccessKit → `RenderNodeSnapshot` 렌더 트리 Diff ([#112](https://github.com/de-novo/dui/pull/112)).
- Radix UI 1 / shadcn/ui 4 선언형 `control` 룰팩 및 Next.js 16 App Router 호스트(Server Component 경계 보고 포함) ([#113](https://github.com/de-novo/dui/pull/113)).
- 6대 디자인·접근성 결함 자동 진단 엔진(AI 정형 JSON + 2D 캔버스 오버레이) 및 병렬 멀티 워커 캡처(`concurrency`) ([#114](https://github.com/de-novo/dui/pull/114)).
- 개발자도구형 요소 인스펙터, 중첩 박스 모델 오버레이(`margin` · `padding` · `border` · `gap` · `borderRadius` · 속성), 요소 간 거리 레드라인 가이드 ([#115](https://github.com/de-novo/dui/pull/115)).

## [v0.0.8] - 2026-11-23

### Added

- 멀티 서피스 추상화 기반(`SurfaceRegistry` · `NoopOracle` · 박스 모델/속성 확장 `RenderNodeSnapshot` IR) ([#107](https://github.com/de-novo/dui/pull/107), [#108](https://github.com/de-novo/dui/pull/108)).
- 번들러 비종속 Universal Web 호스트(`static` / `url` / `command`) ([#109](https://github.com/de-novo/dui/pull/109)).
- Universal TUI 어댑터(`host: "pty"`, 순수 VT100/ANSI 에뮬레이터 → `RenderNodeSnapshot` + 모노스페이스 PNG) ([#110](https://github.com/de-novo/dui/pull/110)).

## [v0.0.7] - 2026-11-22

### Added

- 실 API 프록시 모드(`apiMode: "live"` / `"hybrid"`, 기본 변경 요청 차단), 브랜치 비교(`dui diff --base`), 요소 단위 `renderDiff` + 지연 픽셀 Diff 및 2D 캔버스 인라인 브랜치 변경 하이라이트 ([#81](https://github.com/de-novo/dui/pull/81), [#85](https://github.com/de-novo/dui/pull/85), [#88](https://github.com/de-novo/dui/pull/88), [#92](https://github.com/de-novo/dui/pull/92)).
- 선언형 P2 룰팩 확장(Redux Toolkit 2 · Jotai 3 · XState 5 · React Hook Form 7 · valibot 1) 및 임의 `stateBudget` 제한 제거 ([#90](https://github.com/de-novo/dui/pull/90), [#94](https://github.com/de-novo/dui/pull/94)).

## [v0.0.6] - 2026-11-22

### Added

- 99% UI 상태 커버리지 감사(`dui audit` / MCP `audit` / 뷰어 `/api/audit`) 및 미해결 코멘트 배지 ([#45](https://github.com/de-novo/dui/pull/45)).
- 표준 `URLSearchParams`와 선언형 `kind: "search"` 룰 기반의 라이브러리 비종속 URL 쿼리 파라미터 추적 및 `normalizedQuery` 기록 ([#58](https://github.com/de-novo/dui/pull/58)).
- GPUI 데스크톱 뷰어 `dui live` 실시간 상태 조작, 미검증 HTTP 오류 진단 카드, Figma 키보드 단축키 ([#60](https://github.com/de-novo/dui/pull/60)).

### Changed

- GPUI 데스크톱 뷰어의 디자인 토큰·레이아웃·조작을 웹 캔버스 UI와 1:1 일치시킴 ([#47](https://github.com/de-novo/dui/pull/47), [#49](https://github.com/de-novo/dui/pull/49)).
- 웹·GPUI 뷰어 모두 노드 선택 시 전체 지도를 유지하고 비연관 노드·엣지만 흐리게 처리하도록 개선 ([#51](https://github.com/de-novo/dui/pull/51)).

### Fixed

- GPUI 데스크톱 뷰어의 SQLite `cells.status_json` 파싱(`status` 태그) 및 인스펙터 폭 동기화 오류 수정 ([#53](https://github.com/de-novo/dui/pull/53), [#55](https://github.com/de-novo/dui/pull/55)).

## [v0.0.5] - 2026-11-21

### Added

- SQLite · CLI(`dui comment`) · MCP(`comment_*`) · 뷰어(`/api/comments`) 전반의 화면·상태·폭·전환 리뷰 코멘트 ([#39](https://github.com/de-novo/dui/pull/39)).
- `~/.dui/dui.db`를 직접 읽는 GPUI + `gpui-kit 0.10.0` 네이티브 데스크톱 뷰어(`apps/desktop`, `dui desktop`) ([#41](https://github.com/de-novo/dui/pull/41)).
- 오픈코어 라이선스 경계 구축(`Apache-2.0` 코어 + `ee/LICENSE` 상용 엔터프라이즈) ([#43](https://github.com/de-novo/dui/pull/43)).

## [v0.0.4] - 2026-11-21

### Added

- 행동 기반 전환 리플레이(`dui transitions` / MCP `transitions`) 및 뷰어 내 `extracted` · `inferred` · `verified` 엣지 출처 표시 ([#32](https://github.com/de-novo/dui/pull/32)).
- `dui open` · `dui serve`로 바로 여는 단일 바이너리 내장 웹 뷰어 ([#29](https://github.com/de-novo/dui/pull/29)).
- 태그 기반 버전 표기(`dui --version`), 업데이트 확인·적용(`dui update --check` / `dui update`), `PATH` 자동 설정이 포함된 태그 기반 소스 설치기 ([#29](https://github.com/de-novo/dui/pull/29), [#30](https://github.com/de-novo/dui/pull/30)).
- 화면 지도 Figma 방식 두 손가락 트랙패드 이동, 핀치 줌, 줌 단축키 지원 ([#36](https://github.com/de-novo/dui/pull/36)).

### Changed

- 화면 지도 노드를 실제 캡처 폭과 1:1 크기로 키우고 최대 3배 확대 및 `1:1` 이동 버튼 추가 ([#34](https://github.com/de-novo/dui/pull/34)).

### Fixed

- 모노레포 상위 디렉터리에 호이스트된 `node_modules/typescript` 탐색 지원 ([#30](https://github.com/de-novo/dui/pull/30)).

### Removed

- 내장 뷰어로 대체된 레거시 정적 `graph.html` 및 `history.html` 갤러리 제거 ([#29](https://github.com/de-novo/dui/pull/29)).

## [v0.0.3] - 2026-11-21

### Added

- 모듈 그래프 기반 콘텐츠 해시 캡처 캐시, `--force`, `dui prune` ([#25](https://github.com/de-novo/dui/pull/25)).
- 두 캡처 간 변경 칸 diff(`dui diff` / MCP `diff` / 뷰어 diff 모드), 변경 소스 파일 표기 및 픽셀 차이 오버레이 ([#26](https://github.com/de-novo/dui/pull/26)).
- AccessKit 요소 트리 관찰과 렌더 환경 부재 보고를 포함한 오프스크린 네이티브 GPUI(`gpui-kit`) 렌더링 ([#27](https://github.com/de-novo/dui/pull/27)).
- 공유 `dui live` · MCP `live` 계약 기반 영속 네이티브 `gpui-kit` 라이브 세션 ([#28](https://github.com/de-novo/dui/pull/28)).

## [v0.0.2] - 2026-11-21

### Added

- React, Tailwind CSS, shadcn/ui, React Flow 기반 아카이브 화면 지도 웹 뷰어 ([#15](https://github.com/de-novo/dui/pull/15)).
- `dui inspect` 및 MCP `inspect`를 통한 타입드 컴포넌트 상태 조회 ([#17](https://github.com/de-novo/dui/pull/17)).
- 영속 웹 라이브 세션(`dui live` / MCP `live`) 상태 편집·초기화·스냅샷 저장 및 뷰어 연동 ([#18](https://github.com/de-novo/dui/pull/18), [#19](https://github.com/de-novo/dui/pull/19), [#20](https://github.com/de-novo/dui/pull/20)).
- `dui-core` 순수 도메인 모델 및 저장소 포트 ([#21](https://github.com/de-novo/dui/pull/21)).
- Drizzle 마이그레이션 공유 및 무손실 히스토리 가져오기를 지원하는 로컬 SQLite 메타데이터 저장소 ([#22](https://github.com/de-novo/dui/pull/22)).
- 결정적 계획 및 커버리지 검증 게이트(`npm run check:m1`) ([#23](https://github.com/de-novo/dui/pull/23)).
- 다국어 화면 캡처, 누락 번역·텍스트 넘침·RTL 감사 및 뷰어 `en` / `zh-CN` / `ko` UI 언어 전환 ([#24](https://github.com/de-novo/dui/pull/24)).

## [v0.0.1] - 2026-11-20

### Added

- 초기 `dui` CLI 및 MCP stdio 서버 (`scan`, `run`, `map`, `replay`, `history`, `mcp`).
- 공식 TypeScript 6/7 컴파일러 분석, 타입드 GET 자동 목, Vite 호스트 및 다폭 Chromium CDP 캡처.
- 프로젝트·브랜치·워크트리별 `~/.dui/` 로컬 아카이빙 및 소스 설치기.

[Unreleased]: https://github.com/de-novo/dui/compare/v0.1.0...HEAD
[v0.1.0]: https://github.com/de-novo/dui/releases/tag/v0.1.0
[v0.0.9]: https://github.com/de-novo/dui/compare/v0.0.8...v0.0.9
[v0.0.8]: https://github.com/de-novo/dui/compare/v0.0.7...v0.0.8
[v0.0.7]: https://github.com/de-novo/dui/compare/v0.0.6...v0.0.7
[v0.0.6]: https://github.com/de-novo/dui/compare/v0.0.5...v0.0.6
[v0.0.5]: https://github.com/de-novo/dui/compare/v0.0.4...v0.0.5
[v0.0.4]: https://github.com/de-novo/dui/compare/v0.0.3...v0.0.4
[v0.0.3]: https://github.com/de-novo/dui/compare/v0.0.2...v0.0.3
[v0.0.2]: https://github.com/de-novo/dui/compare/v0.0.1...v0.0.2
[v0.0.1]: https://github.com/de-novo/dui/releases/tag/v0.0.1
