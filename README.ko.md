# dui

[English](README.md) | [简体中文](README.zh-CN.md) | 한국어

dui는 웹 또는 GPUI 앱에 목데이터와 상태를 넣고 실제로 그려진 화면을 검증한다. 사람은 내장 웹 뷰어(`dui open`)에서 화면 지도·라이브 편집·캡처 비교를 보고, 에이전트는 CLI와 MCP로 같은 결과를 JSON으로 읽는다.

[제품 로드맵](docs/ko/roadmap.md) · [변경 이력](docs/ko/changelog.md) · [전체 지원 스택 표](docs/ko/supported-frameworks.md)

## 분석 가능한 항목

### 지원 스택 및 프레임워크

| 분류 | 현재 지원 | 예정 |
| --- | --- | --- |
| 언어 · 컴파일러 | TypeScript 7.x / 6.x (`tsc`), Rust edition 2024 (`rustc`) | Swift 6, Dart 3, Kotlin 2, Go |
| 웹 (Level 1 & Level 2) | Universal Web (`static` / `url` / `command` — 모든 HTML/JS 앱), React 19, Vite 8, Nitro SSR, Chromium (CDP) | Next.js 16, Preact 11, Solid 2, SolidStart 2, Rspack / Rsbuild 2 |
| 데스크톱 앱 | GPUI (`gpui-pre` 0.3.x), `gpui-kit` / `gpui-component` 0.10.x (macOS Metal + AccessKit) | Electron 45, Tauri 2, Dioxus 0.7, Leptos 0.8, egui 0.37, Iced 0.14 |
| 모바일 앱 (Android & iOS) | Android (`adb` `screencap` + `uiautomator` → `RenderNodeSnapshot`) | Jetpack Compose / Kotlin 2 컴파일러 심층 분석, iOS (`xcrun simctl` / SwiftUI / Swift 6), React Native 0.88 / Expo 58, Flutter 3 / Dart 3 |
| 터미널 UI (TUI) | Universal TUI (`pty` + VT100 셀 그리드 → `RenderNodeSnapshot` + 모노스페이스 PNG) | Ratatui (Rust), Ink (TS/React), Bubble Tea (Go) |
| 라우팅 · URL 상태 | TanStack Router 1, TanStack Start 1, React Router 8, 표준 Web `URLSearchParams` | Next.js App Router, Android 인텐트 및 딥링크 |
| 상태 · 폼 · 스키마 | React `useState`, TanStack Query 5, Zustand 5, Redux Toolkit 2, Jotai 3, XState 5, React Hook Form 7, zod 4, valibot 1 | Radix UI 1, shadcn/ui 4 선언형 룰팩 |

### 분석 및 검증 기능

| 영역 | 기능 | 상태 |
| --- | --- | --- |
| 화면 · 링크 탐색 | 라우터 파일 자동 발견, 무한루프 방지 다단계 BFS (`a[href]`), 동적 ID 패턴 축약 (`/orders/:id`), 화면 간 `linksTo` 연결선 | 지원 |
| 상태 계획 · 검증 | 공식 컴파일러 타입 기반 조합, Greedy Pairwise Covering Array, `<select>` 실옵션 자동 수집, 시각 변화 검증(`visualWitness`), 수렴 가드 | 지원 |
| 데이터 · API 검증 | 타입 기반 `GET` JSON 자동 목, CSV 목, 명시적 `mockResponses`, `hybrid` / `live` 실서버 API 모드(기본 읽기 전용 가드 · 비밀값 무유출) | 지원 |
| Diff · 변경 리뷰 | 2D 캔버스 인라인 비교(`요소` 렌더 트리 Diff · 2px 노이즈 허용, `이후`, `이전`, 지연 `픽셀` 오버레이), 브랜치 비교(`dui diff --base main`), `--changed-since` | 지원 |
| 품질 · 다국어 · 감사 | 99% 커버리지 감사(`dui audit`), 다국어·RTL·미번역 키·텍스트 넘침 검사, SQLite 기반 리뷰 코멘트, 모듈 그래프 기반 콘텐츠 해시 캐싱 | 지원 |
| 라이브 편집 · 뷰어 | 공유 CLI/MCP 라이브 세션(`dui live`), 내장 웹 캔버스(`dui open`), 네이티브 GPUI 데스크톱 뷰어(`dui desktop`) | 지원 |
| 요소 인스펙터 · 라이브 컴포넌트 | 개발자도구형 요소 속성 확인, 박스 모델 오버레이(너비·높이·마진·패딩·간격 가이드), 정적 캡처에서 캔버스 위 실제 컴포넌트 직접 렌더링으로 진화 | 예정 |
| 디자인 결함 탐지 · 엔진 확장 | `renderTree` 기반 자동 디자인 결함·접근성 진단(넘침·겹침·잘림·색상 대비·터치 타깃·간격/정렬 불일치 — 사람용 캔버스 오버레이 + AI용 CLI/MCP 정형 JSON), `SurfaceRegistry` 멀티 서피스 플러그인 파이프라인, 멀티 탭 병렬 캡처, GPUI AccessKit 렌더 트리 Diff | 예정 |

## 설치

macOS와 Linux에서는 아래 명령 하나로 최신 릴리스 태그(`vX.Y.Z`, 태그가 없으면 `main`)를 클론해 소스에서 빌드·설치한다. 특정 태그를 고정하려면 `DUI_TAG=v0.0.1`을 지정한다.

```sh
curl -fsSL https://github.com/de-novo/dui/raw/main/scripts/install | bash
```

Git, curl, C 빌드 도구가 필요하다. macOS에서는 Xcode 명령줄 도구를 사용한다. 설치기는 내 컴퓨터에서 dui를 빌드하고, 설치 경로(기본 `~/.local/bin`)를 셸 `PATH`에 자동 등록(원치 않으면 `DUI_NO_MODIFY_PATH=1`)하며 가능하면 현재 `PATH`의 쓰기 가능한 경로에 심볼릭 링크를 만들어 바로 `dui` 명령어를 쓸 수 있게 한다. Rust나 Node.js 26 및 npm이 없으면 빌드에 필요한 도구를 임시로 받아 사용한 뒤 정리한다.

설치된 태그 확인 및 새 릴리스 태그 업데이트 확인은 아래 명령을 사용한다.

```sh
dui --version
dui update --check
dui update
```

**웹 화면을 캡처할 때는** Node.js 26, Chromium 또는 Google Chrome, 대상 앱의 의존성이 필요하다.

설치 경로는 `DUI_INSTALL_DIR`로 바꿀 수 있다. [바이너리 설치 안내](docs/binary-install.md)

## 화면 자동 발견

의존성을 설치한 Vite 앱에서 실행한다.

```sh
~/.local/bin/dui map .
```

dui는 화면 컴포넌트가 있는 TanStack 파일 라우트를 찾는다. 구체적인 경로를 찾지 못하면 `/`에서 시작해 같은 출처의 링크를 한 단계 따라간다. 각 주소를 375px와 1280px로 캡처한다. `dui open` 또는 `dui open <output>`을 실행하면 내장 화면 지도 뷰어가 열린다. MCP에서는 `map(project)`를 호출한다.

캡처한 칸을 열어 같은 Chromium 세션에서 불리언·열거형 상태를 변경하고 복원할 수 있다. [CLI·MCP 사용 예](docs/ko/web-viewer.md#climcp에서-라이브-세션-사용)

프로젝트의 `dui.json`에는 필요할 때만 `appRoot`, 목 응답, 링크 탐색용 시작 `path`, 동적 경로의 예시 값(예: `"routeParams": {"id": "demo"}`)을 넣는다. 화면 내용을 검증하려면 전역 또는 `routeOptions["/path"]`에 해당 콘텐츠의 `readySelector`를 지정한다. 기준이 없으면 이미지를 미검증으로 저장하고 종료 코드 5를 반환한다. 보고서에는 주소를 찾은 근거와 아직 풀지 못한 경로 패턴이 남는다.

## 내 앱 캡처하기

1. 앱의 `package.json`과 `tsconfig.json`이 있는 폴더에서 터미널을 연다. 평소 쓰는 패키지 매니저로 앱 의존성을 설치한다. 현재 웹 캡처는 Vite 8과 프로젝트에 설치된 공식 TypeScript 6.x 또는 7.x를 지원한다. React 앱은 19.x를 사용한다.
2. 같은 폴더에 `dui.json`을 만든다. 아래 설정은 첫 화면(`/`)을 375px와 1280px로 캡처한다. `main article`은 **내 화면의 콘텐츠가 준비되면 나타나는 요소**의 CSS 선택자로 바꾼다. 예를 들어 설정 폼이 뜨면 준비된 화면은 `main form`을 쓴다.

```json
{
  "path": "/",
  "widths": [375, 1280],
  "readySelector": "main article"
}
```

3. 아래 명령으로 캡처한다. dui가 로컬 서버를 띄우고 실행할 때마다 사용자 홈의 `~/.dui/`에 결과를 저장한다.

```sh
~/.local/bin/dui run . dui.json
```

4. `~/.local/bin/dui open`으로 로컬 기록 뷰어를 연다. 프로젝트를 고른 뒤 브랜치·워크트리별 목록에서 이전 캡처와 현재 캡처를 선택한다. 같은 상태와 폭의 스크린샷이 나란히 보인다. 각 캡처에는 커밋과 미커밋 변경 여부가 기록된다. 종료 코드가 5이면 출력의 `output` 경로에서 `report.json`을 열어 이유를 확인한다.

**여러 화면을 보려면** 설정에 `"routeSelector": "nav a[href]"`를 추가한다. 시작 화면에서 이 선택자에 맞는 같은 출처 링크를 한 단계 따라간다. 더 깊은 경로나 준비 조건이 다른 화면은 `scenarios`에 추가한다.

**목데이터를 확인하려면** 결과 폴더의 `facts.json`을 연다. dui는 같은 출처의 고정 경로로 보내는 GET 요청에서 지원하는 응답 타입을 읽어 JSON을 만든다. 생성된 값은 `autoMocks.generated`, 발견했지만 생성하지 못한 항목은 `autoMocks.unresolved`에 있다. [경로와 목데이터 설정](docs/ko/reference.md)

## 첫 캡처 예제

내 앱에 적용하기 전에 동작을 확인하려면, dui를 설치한 뒤 예제를 내려받을 폴더에서 실행한다.

```sh
git clone --depth 1 https://github.com/de-novo/dui.git
cd dui/fixtures/typed-api
npm ci
~/.local/bin/dui run . dui.json
```

`~/.local/bin/dui open`으로 예제 앱을 선택하면 책 세 권이 보이는 768px 화면 한 칸이 **검증됨**으로 표시된다. 책 목록은 dui가 `fetchShelf()`의 반환 타입을 읽어 `GET /api/shelf`에 돌려준 목데이터다. Chrome이나 Chromium을 별도 경로에 설치했다면 `DUI_CHROMIUM`에 실행 파일 경로를 지정한다. [예제 소스](fixtures/typed-api/main.tsx)

### 실제 오픈소스 앱 캡처하기

MIT 라이선스의 npm 의존성 뷰어 **npmgraph**를 시작 주소 하나로 캡처한다. 설정 파일에 패키지 두 개의 목 응답이 들어 있다. dui는 의존성 그래프를 그린 뒤 홈 링크를 따라 검색 화면을 찾는다. 두 화면을 375px와 1280px에서 확인하면 총 4칸이다. [예제 실행 명령](examples/npmgraph/README.md#run) · [캡처 설정](examples/npmgraph/dui.json)

AI에 이렇게 요청하면 된다. “dui 저장소에서 npmgraph 예제를 실행해. 갤러리 경로를 알려주고, 보고서에서 자동으로 찾은 경로와 검증된 칸 수를 확인해 줘.”

- [캡처 레퍼런스](docs/ko/reference.md) — 경로와 목데이터를 설정하고, 검증률을 해석하고, CLI·MCP로 실행을 자동화한다.
- [기여 가이드 · English](CONTRIBUTING.md) — 변경할 크레이트와 규칙 팩의 위치, 검증 명령을 확인한다.
- [설계](docs/ko/design-v0.md) · [아키텍처](docs/ko/architecture.md) · [지원 예정 스택](docs/ko/supported-frameworks.md)

## 라이선스

오픈코어(Open-Core) 모델을 따른다:

- 코어(`ee/` 제외 전체): [Apache License 2.0](LICENSE) — CLI, MCP 서버, 로컬 SQLite 저장소, 웹 뷰어, 로컬 GPUI 데스크톱 뷰어.
- 엔터프라이즈(`ee/`): [dui Enterprise License](ee/LICENSE) — 유료 팀·기업 기능(감사 로그, SSO/RBAC, 원격 팀 동기화 등). 자세한 경계는 [ee/README.md](ee/README.md) 참고.
