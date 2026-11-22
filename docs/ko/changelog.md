# 변경 이력 (Changelog)

[English](../../CHANGELOG.md) | 한국어 | [简体中文](../zh-CN/changelog.md)

이 프로젝트의 주요 변경 사항을 기록합니다.
형식은 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)를 따르며, 버전은 [Semantic Versioning](https://semver.org/spec/v2.0.0.html)을 준수합니다.

## [Unreleased]

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

[Unreleased]: https://github.com/de-novo/dui/compare/v0.0.6...HEAD
[v0.0.6]: https://github.com/de-novo/dui/compare/v0.0.5...v0.0.6
[v0.0.5]: https://github.com/de-novo/dui/compare/v0.0.4...v0.0.5
[v0.0.4]: https://github.com/de-novo/dui/compare/v0.0.3...v0.0.4
[v0.0.3]: https://github.com/de-novo/dui/compare/v0.0.2...v0.0.3
[v0.0.2]: https://github.com/de-novo/dui/compare/v0.0.1...v0.0.2
[v0.0.1]: https://github.com/de-novo/dui/releases/tag/v0.0.1
