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

## 첫 캡처

소스에서 빌드해 예제를 실행하려면 Unix 환경, Rust edition 2024를 지원하는 도구 체인, Node.js 26과 npm, Chromium 또는 Google Chrome이 필요하다. 브라우저를 자동으로 찾지 못하면 `DUI_CHROMIUM`에 실행 파일 경로를 지정한다.

저장소 루트에서 실행한다.

```sh
npm ci
cargo build --release --locked
npm ci --prefix fixtures/typed-api
target/release/dui run fixtures/typed-api fixtures/typed-api/dui.json .dui/shelf
```

브라우저에서 `.dui/shelf/graph.html`을 열면 책 세 권이 담긴 768px 화면 한 칸이 **검증됨**으로 표시된다. dui는 앱의 `fetchShelf()` 반환 타입을 읽어 `GET /api/shelf` 응답을 생성한다. 세 번째 책이 화면에 나타나야 검증을 통과한다. [앱 소스](fixtures/typed-api/main.tsx) · [캡처 설정](fixtures/typed-api/dui.json)

## 내 프로젝트에 적용하기

현재 웹 캡처는 Vite 8과 Chromium을 사용한다. 먼저 대상 프로젝트의 의존성을 설치한다. dui는 프로젝트에 설치된 공식 TypeScript 6.x 또는 7.x 컴파일러로 타입을 조회한다. React 앱은 19.x를 지원하며, dui 자체 어댑터는 TypeScript 7로 빌드한다.

캡처 설정에는 시작 경로, 화면 폭, 콘텐츠가 준비됐음을 확인할 CSS 선택자를 지정한다. 화면이 여러 개면 시작 화면의 탐색 링크에서 경로를 수집할 수 있다. 보고서에서 **한 칸**은 경로 하나의 상태 하나를 특정 폭으로 캡처할 자리다.

- [캡처 레퍼런스](docs/ko/reference.md) — 경로와 목데이터를 설정하고, 검증률을 해석하고, CLI·MCP로 실행을 자동화한다.
- [기여 가이드 · English](CONTRIBUTING.md) — 변경할 크레이트와 규칙 팩의 위치, 검증 명령을 확인한다.
- [설계](docs/ko/design-v0.md) · [아키텍처](docs/ko/architecture.md) · [지원 예정 스택](docs/ko/supported-frameworks.md)

## 라이선스

[Apache License 2.0](LICENSE)
