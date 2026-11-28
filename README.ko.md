# dui

[English](README.md) | [简体中文](README.zh-CN.md) | 한국어

dui는 웹 또는 GPUI 앱에 목데이터와 상태를 넣고 실제로 그려진 화면을 검증한다. 사람은 내장 웹 뷰어(`dui open`)에서 화면 지도·라이브 편집·캡처 비교를 보고, 에이전트는 CLI와 MCP로 같은 결과를 JSON으로 읽는다.

**목표.** dui의 목표는 기획자·디자이너·QA가 실제 사용자에게 보일 화면을 확인하고 코드가 바뀌면 그 결과를 바로 보는 것이다. 출시할 코드를 기준으로 검수하면 피그마 설계에서 개발을 거쳐 디자인·QA 검수로 돌아오는 왕복을 줄이고 AI가 작성한 변경도 빠르게 확인할 수 있다. 엔진의 목표는 기본 단위를 정확하게 그리는 것이다. 고정 웹 목록은 공통 엔진의 변경 효과를 진단하는 데 쓴다.

[제품 로드맵](docs/ko/roadmap.md) · [변경 이력](docs/ko/changelog.md) · [분석·렌더링 지원 범위](docs/ko/supported-frameworks.md)

아래 기능 표는 `main` 개발판을 기준으로 한다. 기본 설치판은 [첫 캡처](#first-run)부터 시작한다.

## 분석 가능한 항목

### 분석과 캡처 방식

dui는 프로젝트의 공식 컴파일러에서 타입을 읽는다. 라이브러리 규칙은 코드에서 특정 API 호출을 식별한다. 실행 화면은 플랫폼별 렌더러로 캡처한다.

| 확인할 내용 | 방법 |
| --- | --- |
| 타입이 있는 상태 슬롯과 경로 매개변수 | `dui scan`이 프로젝트의 TypeScript 컴파일러와 해당 심볼 규칙을 사용한다. |
| 너비별로 이동 가능한 웹 화면 | `dui map`에 로컬 URL이나 정적 빌드를 넘긴다. dui가 서버가 보낸 HTML의 동일 출처 링크를 따라가고, `--capture`를 주면 Chromium으로 렌더링한다. |
| 코드에 선언된 네이티브 화면 | React Navigation, Expo Router, Flutter 프로젝트에서 `dui map`을 실행한다. 각 카드에서 화면의 선언 위치를 확인할 수 있다. |
| 렌더링된 UI | 설정한 웹·모바일·데스크톱·터미널 호스트에서 캡처한다. 일부 SwiftUI·Compose 컴포넌트는 조작 가능한 HTML 미리보기로도 내보낼 수 있다. |

### 분석 및 검증 기능

| 영역 | 기능 | 상태 |
| --- | --- | --- |
| 화면 · 링크 탐색 | 라우터 파일 자동 발견, 무한루프 방지 다단계 BFS (`a[href]`), 동적 ID 패턴 축약 (`/orders/:id`), 화면 간 `linksTo` 연결선 | 지원 |
| 상태 계획 · 검증 | 공식 컴파일러 타입 기반 조합, Greedy Pairwise Covering Array, `<select>` 실옵션 자동 수집, 시각 변화 검증(`visualWitness`), 수렴 가드 | 지원 |
| 데이터 · API 검증 | 타입 기반 `GET` JSON 자동 목, CSV 목, 명시적 `mockResponses`, `hybrid` / `live` 실서버 API 모드(읽기 전용 가드), 인증 세션(`storageState.json`, `POST authFlow`, 대화형 `dui auth login`) 및 비밀값 무유출 | 지원 |
| Diff · 변경 리뷰 | 2D 캔버스 인라인 비교(`요소` 렌더 트리 Diff · 2px 노이즈 허용, `이후`, `이전`, 지연 `픽셀` 오버레이), 브랜치 비교(`dui diff --base main`), `--changed-since` | 지원 |
| 품질 · 다국어 · 감사 | 99% 커버리지 감사(`dui audit`), 다국어·RTL·미번역 키·텍스트 넘침 검사, SQLite 기반 리뷰 코멘트, 모듈 그래프 기반 콘텐츠 해시 캐싱 | 지원 |
| 라이브 편집 · 뷰어 | 공유 CLI/MCP 라이브 세션(`dui live`), 내장 웹 캔버스(`dui open`), 네이티브 GPUI 데스크톱 뷰어(`dui desktop`) | 지원 |
| 요소 인스펙터 · 박스 모델 | 개발자도구형 요소 속성 확인, 박스 모델 다이어그램·오버레이(너비·높이·마진·패딩·테두리·gap·radius), 요소 간 간격 레드라인 측정 가이드 | 지원 |
| 범용 라우팅 · 모달 · 캔버스 | 풀페이지 실제 비율 캡처, WAI-ARIA 네비게이션 펼침·모달/드로어 자동 캡처, 장애물 회피 거터 연결선, macOS 통합 `TitleBar`(`apps/desktop`), 병렬 멀티 워커 캡처(`concurrency`), `SurfaceRegistry` 파이프라인 | 지원 |

## 설치

macOS와 Linux에서는 아래 명령 하나로 최신 릴리스 태그(`vX.Y.Z`, 태그가 없으면 `main`)를 클론해 소스에서 빌드·설치한다. 특정 버전을 설치하려면 설치기의 환경 변수 `DUI_TAG`에 [공개된 태그](https://github.com/de-novo/dui/tags)를 지정한다.

```sh
curl -fsSL https://dui.sh/install | sh
```

Git, curl, C 빌드 도구가 필요하다. macOS에서는 Xcode 명령줄 도구를 사용한다. 설치기는 내 컴퓨터에서 dui를 빌드하고, 설치 경로(기본 `~/.local/bin`)를 셸 `PATH`에 자동 등록(원치 않으면 `DUI_NO_MODIFY_PATH=1`)하며 가능하면 현재 `PATH`의 쓰기 가능한 경로에 심볼릭 링크를 만들어 바로 `dui` 명령어를 쓸 수 있게 한다. Rust나 Node.js 26 및 npm이 없으면 빌드에 필요한 도구를 임시로 받아 사용한 뒤 정리한다.

설치된 태그 확인 및 새 릴리스 태그 업데이트 확인은 아래 명령을 사용한다.

```sh
dui --version
dui update --check
```

설치 경로는 `DUI_INSTALL_DIR`로 바꿀 수 있다. [바이너리 설치 안내](docs/binary-install.md)

<a id="first-run"></a>

## 기본 설치판으로 첫 화면 캡처하기

Node.js 26, Chrome 또는 Chromium과 Vite 앱의 의존성을 설치한다. [검증에 사용하는 앱](fixtures/toggle)은 Vite 8과 React 19를 쓴다. 앱 루트의 `dui.json`에 원하는 화면이 준비됐을 때 보이는 요소를 `readySelector`로 지정한다. 예를 들어 본문 안에 페이지 제목이 있는 앱에서는 아래 설정을 쓸 수 있다.

<!-- quickstart:manifest -->
```json
{ "readySelector": "main h1" }
```

<!-- quickstart:first-run -->
```sh
dui --version
dui map .
dui open --no-open
```

첫 명령은 설치된 버전을 표시하고, 두 번째 명령은 화면을 캡처하고 결과 디렉터리를 `output`으로 출력한다. 마지막 명령이 출력한 로컬 URL을 열면 캡처를 볼 수 있다. 뷰어를 종료하려면 Ctrl+C를 누른다. 캡처 종료 코드가 5이면 결과 디렉터리의 `report.json`에서 실패 이유를 확인한다.

## `main` 개발판 CLI

개발판은 `curl -fsSL https://dui.sh/install | DUI_TAG=main sh`로 설치한다. 아래 예제와 명령 표는 개발판 기준이다. 홈페이지에서 [기본 설치판의 릴리스 문서](https://dui.sh/?lang=ko#release-guide)도 열 수 있다.

개발판의 `dui map`은 브라우저 없이 화면·상태·폭을 계획하고, `dui map --capture`는 계획한 모든 셀을 캡처한다. [공통 화면 지도](docs/ko/web-viewer.md#공통-화면-지도)를 참고한다.

정적 사이트를 이미 `dist/`에 빌드했다면 프로젝트 루트에서 아래 명령을 실행한다. 결과는 위의 뷰어 명령으로 연다.

<!-- quickstart:static -->
```sh
dui map . --build-dir dist
```

### `dui map` vs `dui run`

| 명령어 | 언제 쓰나 |
| --- | --- |
| `dui map [PROJECT]` | **라우터가 있는 앱의 기본 명령** (TanStack Router · React Router 7/8 · Next.js · 링크). 화면을 자동 발견하고, `dui.json`의 `routeParams`(:id 샘플 값)·`storageState`·`readySelector`·라우터 밖 추가 `scenarios`를 자동 병합해 브라우저 없이 모든 셀을 계획한다. `--capture`는 모달까지 한 번에 캡처한다. |
| `dui run [PROJECT] [MANIFEST]` | **명시적 매니페스트 직접 캡처.** `dui.json` 설정 그대로 실행한다(인자를 생략하면 `.` / `dui.json`을 사용하며, 명시적 `path`/`host`가 없으면 `dui map`으로 자동 발견). |

| 분류 | 명령어 | 역할 |
| --- | --- | --- |
| 캡처 | `dui map [PROJECT]` · `dui run [PROJECT] [MANIFEST]` · `dui scan [PROJECT]` | 화면·모달 자동 발견·캡처 또는 브라우저 없이 컴파일러 사실/목 출력 |
| 실패 항목 재검증 | `dui recheck RUN [PROJECT [MANIFEST]]` | 이전 캡처에서 검증에 실패한 항목만 다시 렌더링하고 해결 여부를 확인 |
| 인증 | `dui auth login URL [--out FILE] [--wait-url PATTERN]` | 인증된 화면 캡처용 `storageState.json` 저장(`0600`) |
| 뷰어 | `dui open [RUN_DIR]` · `dui desktop` | 내장 2D 웹 캔버스 또는 네이티브 GPUI 데스크톱 뷰어 |
| 비교 · 품질 | `dui diff --base BRANCH [--regressions-only] [--markdown]` · `dui audit RUN_DIR` · `dui comment` | 요소/픽셀 Diff, 99% 커버리지 감사, 리뷰 코멘트 |
| 도구 | `dui toolchains` · `dui history` · `dui prune` · `dui update` · `dui mcp` | 공식 컴파일러 진단, 아카이브 목록, 캐시 정리, 업데이트, MCP 서버 |

## 화면 자동 발견

의존성을 설치한 Vite 앱에서 실행한다.

```sh
~/.local/bin/dui map .
```

dui는 화면 컴포넌트가 있는 TanStack 파일 라우트를 찾는다. 구체적인 경로를 찾지 못하면 `/`에서 시작해 탐색 한도 안에서 도달 가능한 동일 출처 링크를 따라간다. 각 주소를 375px와 1280px로 캡처한다. `dui open` 또는 `dui open <output>`을 실행하면 내장 화면 지도 뷰어가 열린다. MCP에서는 `map(project)`를 호출한다.

캡처한 칸을 열어 같은 Chromium 세션에서 불리언·열거형 상태를 변경하고 복원할 수 있다. [CLI·MCP 사용 예](docs/ko/web-viewer.md#climcp에서-라이브-세션-사용)

프로젝트의 `dui.json`에는 필요할 때만 `appRoot`, 목 응답, 링크 탐색용 시작 `path`, 동적 경로의 예시 값(예: `"routeParams": {"id": "demo"}`)을 넣는다. 화면 내용을 검증하려면 전역 또는 `routeOptions["/path"]`에 해당 콘텐츠의 `readySelector`를 지정한다. 기준이 없으면 이미지를 미검증으로 저장하고 종료 코드 5를 반환한다. 보고서에는 주소를 찾은 근거와 아직 풀지 못한 경로 패턴이 남는다.

## 내 앱 캡처하기

1. 이 Vite 예제에서는 `package.json`과 `tsconfig.json`이 있는 앱 폴더에서 의존성을 설치한다. `dui check-ts .`로 프로젝트의 공식 TypeScript 컴파일러를 확인한다. [컴파일러와 호스트 요구 사항](docs/ko/supported-frameworks.md#컴파일러와-호스트-요구-사항)
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

**여러 화면을 보려면** 설정에 `"routeSelector": "nav a[href]"`를 추가한다. 이 선택자에 맞는 동일 출처 링크를 재귀적으로 따라간다. 링크로 연결되지 않은 경로나 준비 조건이 다른 화면은 `scenarios`에 추가한다.

**목데이터를 확인하려면** 결과 폴더의 `facts.json`을 연다. dui는 같은 출처의 고정 경로로 보내는 GET 요청에서 지원하는 응답 타입을 읽어 JSON을 만든다. 생성된 값은 `autoMocks.generated`, 발견했지만 생성하지 못한 항목은 `autoMocks.unresolved`에 있다. [경로와 목데이터 설정](docs/ko/reference.md)

## 첫 캡처 예제

내 앱에 적용하기 전에 동작을 확인하려면, dui를 설치한 뒤 예제를 내려받을 폴더에서 실행한다.

```sh
git clone --depth 1 https://github.com/de-novo/dui.git
cd dui/fixtures/typed-api
npm ci
~/.local/bin/dui run . dui.json
```

`~/.local/bin/dui open`으로 예제 앱을 선택하면 책 세 권이 보이는 768px 화면 한 칸이 **검증됨**으로 표시된다. 책 목록은 dui가 `fetchShelf()`의 반환 타입을 읽어 `GET /api/shelf`에 돌려준 목데이터다. Playwright가 설치한 브라우저(`PLAYWRIGHT_BROWSERS_PATH` 또는 기본 캐시)도 찾는다. Chrome이나 Chromium을 다른 경로에 설치했다면 `DUI_CHROMIUM`에 실행 파일 경로를 지정한다. 브라우저가 없으면 캡처는 확인한 경로를 나열하고 코드 5로 끝난다. [예제 소스](fixtures/typed-api/main.tsx)

### 실제 오픈소스 앱 캡처하기

MIT 라이선스의 npm 의존성 뷰어 **npmgraph**를 시작 주소 하나로 캡처한다. 설정 파일에 패키지 두 개의 목 응답이 들어 있다. dui는 의존성 그래프를 그린 뒤 홈 링크를 따라 검색 화면을 찾는다. 두 화면을 375px와 1280px에서 확인하면 총 4칸이다. [예제 실행 명령](examples/npmgraph/README.md#run) · [캡처 설정](examples/npmgraph/dui.json)

AI에 이렇게 요청하면 된다. “dui 저장소에서 npmgraph 예제를 실행해. 갤러리 경로를 알려주고, 보고서에서 자동으로 찾은 경로와 검증된 칸 수를 확인해 줘.”

- [캡처 레퍼런스](docs/ko/reference.md) — 경로와 목데이터를 설정하고, 검증률을 해석하고, CLI·MCP로 실행을 자동화한다.
- [기여 가이드 · English](CONTRIBUTING.md) — 변경할 크레이트와 규칙 팩의 위치, 검증 명령을 확인한다.
- [설계](docs/ko/design-v0.md) · [아키텍처](docs/ko/architecture.md) · [분석·렌더링 지원 범위](docs/ko/supported-frameworks.md)

## 라이선스

오픈코어(Open-Core) 모델을 따른다:

- 코어(`ee/` 제외 전체): [Apache License 2.0](LICENSE) — CLI, MCP 서버, 로컬 SQLite 저장소, 웹 뷰어, 로컬 GPUI 데스크톱 뷰어.
- 엔터프라이즈(`ee/`): [dui Enterprise License](ee/LICENSE) — 유료 팀·기업 기능(감사 로그, SSO/RBAC, 원격 팀 동기화 등). 자세한 경계는 [ee/README.md](ee/README.md) 참고.
