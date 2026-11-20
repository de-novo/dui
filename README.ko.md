# dui

[English](README.md) | [简体中文](README.zh-CN.md) | 한국어

dui는 웹 앱에 목데이터를 넣고, 상태와 화면 폭을 바꿔 가며 Chromium에 실제로 그려진 화면을 검증한다. 사람은 로컬 HTML 갤러리에서 캡처를 보고, 에이전트는 CLI와 MCP로 같은 결과를 JSON으로 읽는다.

## 설치

macOS와 Linux에서는 아래 명령 하나로 설치한다. 다시 실행하면 현재 `main` 버전으로 업데이트한다.

```sh
curl -fsSL https://github.com/de-novo/dui/raw/main/scripts/install | bash
```

Git, curl, C 빌드 도구가 필요하다. macOS에서는 Xcode 명령줄 도구를 사용한다. 설치기는 내 컴퓨터에서 dui를 빌드한다. Rust나 Node.js 26 및 npm이 없으면 빌드에 필요한 도구를 임시로 받아 사용한 뒤 정리한다.

설치 후 아래 명령으로 버전을 확인한다. `dui`만 입력해 실행하려면 `~/.local/bin`을 `PATH`에 추가한다.

```sh
~/.local/bin/dui --version
```

**웹 화면을 캡처할 때는** Node.js 26, Chromium 또는 Google Chrome, 대상 앱의 의존성이 필요하다.

설치 경로는 `DUI_INSTALL_DIR`로 바꿀 수 있다. [바이너리 설치 안내](docs/binary-install.md)

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

[Apache License 2.0](LICENSE)
