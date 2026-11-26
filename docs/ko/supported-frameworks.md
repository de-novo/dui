[English](../en/supported-frameworks.md) | [简体中文](../zh-CN/supported-frameworks.md) | 한국어

# dui가 분석하고 렌더링할 수 있는 범위

이 표는 `main` 개발판 기준이다. 기본 설치판에서는 [검증된 첫 캡처](../../README.ko.md#first-run)를 실행하고, 개발판 명령을 고르기 전에 표시된 버전을 확인한다.

타입을 알아낼 때 dui는 프로젝트의 공식 컴파일러에 질의한다. 화면 지도를 만들 때는 플랫폼 렌더러로 실행 중인 앱을 관찰할 수도 있다. 한 프로젝트에서 두 경로를 함께 쓸 수 있다. [아키텍처](architecture.md) · [화면 지도 명세](web-viewer.md)

| 작업 | 입력과 결과 | 검증 근거 |
|---|---|---|
| 타입이 있는 UI 상태 추출 | `dui scan`은 프로젝트의 TypeScript 프로그램을 읽는다. 규칙은 특정 심볼을 찾아 컴파일러가 제공하는 타입으로 상태 슬롯, 경로 매개변수 등의 정보를 추출한다. | [추출기](../../adapters/typescript/facts.mts) · [규칙 검사](../../scripts/check-p2-rules.mts) |
| 웹에서 이동 가능한 페이지 탐색 | `dui map`에 로컬 URL(`--url`)이나 정적 빌드(`--build-dir`)를 넘기면 dui가 브라우저 없이 서버가 보낸 HTML의 동일 출처 링크를 따라간다. `--capture`를 주면 Chromium이 렌더링된 페이지의 링크를 따라가며 지정한 너비로 페이지를 캡처한다. 실행 중인 Angular 앱에는 URL을 쓸 수 있다. Vue Router, SvelteKit, Astro, Preact는 빌드한 예제 앱으로 정적 빌드 경로를 검사한다. | [URL 검사](../../scripts/check-map-url.mts) · [빌드한 앱 검사](../../scripts/check-web-frameworks.mts) |
| 네이티브 화면 선언 탐색 | `dui map`은 프로젝트의 TypeScript 도구로 React Navigation과 Expo Router 선언을, Dart 분석기로 Flutter 경로를 읽는다. Xcode가 있는 macOS에서는 앱의 인덱스 스토어와 swift-syntax로 SwiftUI 화면을 읽는다([규칙](roadmap.md#swiftui-screen-discovery), 실험 단계). 카드마다 소스 위치를 표시한다. 기기 없이 실행할 수 있으며 이 단계의 캡처 시도는 0건이다. | [탐색 어댑터](../../adapters/native-map) · [오픈소스 앱 검사](../../scripts/check-native-map.mts) |
| 네이티브 컴포넌트 소스 미리보기 | 미리보기 도구는 제한된 UI 기본 요소를 제공하고 `swiftc` 또는 `kotlinc`로 SwiftUI·Compose 컴포넌트를 컴파일한다. 지도에서 해당 카드를 선택하면 관찰한 상태를 보고 스위치를 조작할 수 있다. 독립 HTML 파일로도 내보낼 수 있다. 지원하지 않는 호출이 있으면 카드에 이유가 표시된다. | [내보내기 도구](../../scripts/source-preview.mts) · [상호작용 검사](../../scripts/check-source-preview.mts) · [지도에서 열기](web-viewer.md#portable-source-map) |
| 네이티브 모바일 UI 캡처 | Android는 연결한 기기나 에뮬레이터를, iOS는 macOS의 시뮬레이터에 설치한 앱을 사용한다. 촬영한 PNG를 소스 카드에 연결하려면 `dui map attach`를 쓴다. 가져온 칸은 미검증(`unverified`)으로 남고 검증 커버리지는 늘지 않는다. 캡처에 필요한 실행 환경이 없으면 해당 칸을 렌더 불가(`unrenderable`)로 표시하고 이유를 기록한다. | [Compose 에뮬레이터 검사](../../scripts/check-compose.mts) · [iOS 시뮬레이터 검사](../../.github/workflows/ios-simulator.yml) · [캡처 연결 검사](../../scripts/check-native-map-recordings.mts) |
| 데스크톱·터미널 UI 캡처 | GPUI 예제는 선언된 상태를 렌더링하고 요소 트리를 기록한다. Electron 호스트는 Chromium으로 웹 렌더러의 콘텐츠를 캡처하며 IPC 목을 지정할 수 있다. 가상 터미널에서 명령을 실행하면 문자 격자와 PNG를 얻는다. | [GPUI](../../scripts/check-gpui.mts) · [Electron](../../scripts/check-electron.mts) · [터미널](../../scripts/check-tui.mts) |

## 라이브러리 규칙의 역할

예를 들어 TypeScript 규칙은 Zustand의 `create`를 스토어로, Zod의 `object`를 스키마로 인식한다. 각 호출의 타입은 컴파일러가 제공한다. 패키지의 설치 여부만으로는 화면이 어떤 상태를 사용하는지 알 수 없다. 인식하는 패키지와 심볼은 기본 규칙에서 확인할 수 있으며 프로젝트별 규칙은 `rulePacks`로 추가한다. [기본 규칙](../../rules/web.json) · [매니페스트 명세](../reference.md)

웹 캡처의 범위는 브라우저가 도달할 수 있는 페이지로 정해진다. 템플릿 언어에 대한 규칙이 없어도 페이지를 지도에 표시할 수 있다. 숨은 경로를 찾거나 컴포넌트 입력을 읽거나 타입을 바탕으로 상태를 계획하려면 각 작업에 맞는 소스 근거가 필요하다. 로그인이 필요하거나 들어오는 링크가 없는 페이지는 세션이나 진입점을 지정해야 할 수 있다. [화면 지도 명세](web-viewer.md)

## 컴파일러와 호스트 요구 사항

설계 기준은 공식 TypeScript 7이다. 현재 CLI에는 공식 안정판 6.x 프로젝트용 호환 경로도 있다. `dui check-ts PROJECT`로 프로젝트의 컴파일러를 확인한다. TypeScript 스캔은 설치된 React·React DOM의 안정판 18+와 Vite 7+를 검사하고, Next 호스트에서 Next.js가 설치되어 있으면 16+를 검사한다. 라이브러리 규칙은 버전에 따라 진단 메모를 남기고 스캔을 계속할 수 있다. [컴파일러 검사](../../crates/dui/src/main.rs) · [스캔 버전 검사](../../adapters/typescript/facts.mts)

`dui toolchains`는 설치된 `tsc`, `rustc`, `go`, `swiftc`, `kotlinc`, `dart`의 버전을 확인한다. 이 검사에 성공하면 해당 도구를 사용할 수 있다는 뜻이다. 타입 정보 추출, 화면 탐색, 렌더링의 검증 근거는 위 표에서 각각 확인할 수 있다. 예정된 작업은 로드맵에서 관리한다. [도구 설치 검사](../../crates/dui-app/src/compiler_oracle.rs) · [로드맵](roadmap.md)


## 언어 어댑터 (기본 파이프라인)

위 표는 오늘 `main`의 동작이다. [기본 파이프라인](roadmap.md#default-pipeline)에서는 새 언어를 붙이는 방법이 바뀐다. 어댑터 하나가 그 언어의 소스를 버전 있는 모델로 옮긴다. 모델, 근사 렌더러, 핫리로드, Figma 내보내기는 재사용한다.

어댑터가 매핑하는 것은 다음과 같다.

- 레이아웃 요소를 모델의 레이아웃 타입으로
- 선언된 상태와 데이터 슬롯
- 화면 이동(라우터, 링크, 목적지)
- 스타일 문법을 공통 스타일 속성(spacing, color, typography, radius, size, layout)으로, 토큰 이름은 토큰 층에 남김

스타일 난이도:

- 쉬움: Tailwind 클래스, SwiftUI modifier, Compose Modifier, 인라인 스타일은 바로 매핑한다.
- 보통: 테마, 디자인 토큰, CSS 변수는 정의 파일에서 해석한다.
- 어려움: CSS 파일과 CSS Modules는 캐스케이드, 우선순위, 미디어 쿼리가 필요하다. 브라우저 없는 가벼운 CSS 해석기가 이를 근사한다. 런타임 CSS-in-JS 값은 슬롯처럼 다룬다.

기본 어댑터 경로는 브라우저나 에뮬레이터를 실행하면 안 되고, 무거운 렌더러로 카드를 채우는 폴백을 쓰면 안 된다. [절대 하지 않는 것](architecture.md#never-default-path)을 본다.
