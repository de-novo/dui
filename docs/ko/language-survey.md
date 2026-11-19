# dui 대상 언어·UI 스택 전수조사 v0

[English](../en/language-survey.md) | [简体中文](../zh-CN/language-survey.md) | 한국어

> 이 문서는 [영어 원문](../en/language-survey.md)의 한국어 번역이다. 내용이 다르면 영어가 기준이다.

> 작성 2026-11-19. 목적: dui가 지원할 언어·프레임워크를 근거 있게 고르기.
> 숫자는 출처가 있는 것만 적는다. 확인 못 한 것은 `미확인`. 스타 수는 2026-11-19 GitHub API 값이다.
> 여기서 라이선스는 **대상 프레임워크** 것이라 dui 의존성 정책(dui는 Apache-2.0 오픈소스, 의존성은 상업 사용 허용 라이선스만)과는 별개다. dui는 사용자 프로젝트에 개발용 코드를 넣을 뿐 그 프레임워크를 재배포하지 않는다.

## 0. dui가 각 스택에 요구하는 4가지

| 기준 | 질문 |
|---|---|
| 타입 | 상태 모양을 타입에서 다 꺼낼 수 있나 |
| 강제 주입 | 상태가 사는 곳(로컬 포함)을 빌드할 때 바꿔서 밖에서 값을 넣을 수 있나 |
| 네트워크 가로채기 | API 응답을 목으로 바꿀 수 있나 |
| 창 없이 렌더 | 상태 넣은 화면을 이미지로 뽑을 수 있나(CI·병렬 가능) |

난이도는 이 넷을 합쳐 매긴다: **하** = 기존 도구로 다 됨, **중** = 한두 개를 우리가 만들어야 함, **상** = 핵심 하나가 막힘.

## 1. 사용 규모 (언어)

출처 A: [Stack Overflow Developer Survey 2025, 기술 > 언어](https://survey.stackoverflow.co/2025/technology) — 전체 응답자 중 "지난 1년 많이 쓴 언어" 비율.
출처 B: [GitHub Octoverse 2025](https://github.blog/news-insights/octoverse/octoverse-a-new-developer-joins-github-every-second-as-ai-leads-typescript-to-1/) — 2025년 8월 월간 기여자 수 기준.

| 언어 | SO 2025 사용률 (A) | Octoverse 2025 (B) | 타입 강도 |
|---|---|---|---|
| TypeScript | 43.6% | **1위**, 월 기여자 2,636,006명, 전년 대비 +66.6% | 정적, 구조적. `any`로 뚫릴 수 있음 |
| Java | 29.4% | 4위 | 정적, 강함 |
| C# | 27.8% | 5위 | 정적, 강함 |
| C++ | 23.5% | 미확인 | 정적, 템플릿·매크로로 분석 어려움 |
| Go | 16.4% | 미확인 | 정적, 단순 |
| Rust | 14.8% | 미확인 | 정적, 매우 강함(enum·패턴 매칭) |
| Kotlin | 10.8% | 미확인 | 정적, 강함(sealed class, null 안전) |
| Dart | 5.9% | 미확인 | 정적, sound null safety |
| Swift | 5.4% | 미확인 | 정적, 강함(enum·옵셔널) |

참고: JavaScript 66%, Python 57.9%는 타입 없는 언어라 범위 밖(설계 v0 비목표).
주의: SO 설문은 웹·백엔드 개발자 비중이 높아 모바일 언어(Swift, Kotlin, Dart)가 실제보다 낮게 나올 수 있다. 모바일 전용 사용 규모 수치는 `미확인`(JetBrains State of Developer Ecosystem 등 추가 확인 필요).

UI 프레임워크 사용률(SO 2025 웹 프레임워크, 전체 응답자): React 44.7%, Next.js 20.8%, Angular 18.2%, Vue.js 17.6%, Svelte 7.2%, Blazor 7.0%. 모바일·데스크톱 프레임워크(Flutter, React Native, SwiftUI, Compose, Electron 등)의 2025 사용률은 `미확인`(이번 페이지에서 해당 항목을 찾지 못함).

## 2. 스택별 조사

### 2.1 TypeScript

| 스택 | ★ (GitHub) | 상태 위치 | 강제 주입 방법 | 네트워크 | 창 없이 렌더 | 기존 미리보기 | 난이도 |
|---|---|---|---|---|---|---|---|
| React (+Vite/Next) | react 252,767 | `useState`·`useReducer`·context·store(Zustand/Redux)·쿼리 캐시·URL | Babel/SWC 변환(Vite 플러그인) | MSW(Service Worker / Node) | Playwright 헤드리스 | Storybook | **하** |
| Vue 3 | vuejs/core 55,045 | `ref`/`reactive`·Pinia·URL | Vite 플러그인(SFC 컴파일 단계) | MSW | Playwright | Storybook | 하 |
| Svelte 5 | 89,756 | runes(`$state`)·store | 컴파일러 전처리 | MSW | Playwright | Storybook | 하 |
| Solid | 36,760 | signal·store | Babel 변환 | MSW | Playwright | Storybook | 하 |
| Angular | 101,461 | 컴포넌트 필드·signal·service(DI) | DI로 service 교체 + 빌드 변환 | `HttpClient` 인터셉터 / MSW | Playwright | Storybook | 중(빌드 체인이 자체) |
| React Native | react-native 126,964 (Expo 53,662) | React와 같음 | Babel(Metro) 변환 | JS fetch 목 / MSW native | **시뮬레이터 필요**. react-native-web으로 우회 가능하나 실제와 다를 수 있음 | Storybook RN | 중 |
| Lynx | 15,264 | React 계열(ReactLynx) | 미확인 | 미확인 | 미확인 | 미확인 | 미확인 |
| Electron (웹뷰) | 123,999 | 렌더러는 웹과 같음 + IPC | 웹과 같음, preload/IPC 목 | MSW | Playwright(Electron 지원) | Storybook | 하 |
| Tauri (웹뷰) | 112,993 | 웹 + `invoke` | 웹과 같음, `invoke` 목 | MSW | 브라우저에서 렌더 | Storybook | 하 |
| Ionic/Capacitor | ionic 52,763 | 웹과 같음 | 웹과 같음 | MSW | Playwright | Storybook | 하 |

### 2.2 Swift (Apple)

| 스택 | 상태 위치 | 강제 주입 방법 | 네트워크 | 창 없이 렌더 | 기존 미리보기 | 난이도 |
|---|---|---|---|---|---|---|
| SwiftUI | `@State`·`@Observable` 모델·`@Environment` | Swift macro는 새 코드 추가만 가능하고 기존 `@State` 초기값을 바꾸지 못함 → **SwiftSyntax로 개발 빌드 전 소스 재작성**(SwiftPM build plugin). `@Observable` 모델은 생성 지점 교체로 주입 | `URLProtocol` 등록으로 가로채기 | `ImageRenderer`(iOS 16·macOS 13+), [swift-snapshot-testing](https://github.com/pointfreeco/swift-snapshot-testing)(MIT, ★4,397). **macOS·Xcode 머신 필요** | `#Preview` | 중 |
| UIKit / AppKit | 뷰 컨트롤러 필드에 흩어짐, 명령형 | 소스 재작성은 가능하나 상태가 흩어져 정확도 낮음 | `URLProtocol` | 스냅샷 테스트(위 라이브러리), 시뮬레이터 필요할 수 있음 | Xcode Preview(UIViewRepresentable 경유) | 상 |

### 2.3 Kotlin / Java (Android, JVM)

| 스택 | ★ | 상태 위치 | 강제 주입 방법 | 네트워크 | 창 없이 렌더 | 기존 미리보기 | 난이도 |
|---|---|---|---|---|---|---|---|
| Jetpack Compose (Android) | 미확인 | `remember { mutableStateOf }`·ViewModel `StateFlow` | **Kotlin 컴파일러 플러그인**(Compose 자체가 컴파일러 플러그인). 플러그인 API의 공식 안정성은 미확인 | OkHttp 인터셉터 / MockWebServer | JVM에서 기기 없이: [Paparazzi](https://github.com/cashapp/paparazzi)(Apache-2.0, ★2,641), [Roborazzi](https://github.com/takahirom/roborazzi)(Apache-2.0, ★1,084), [Compose Preview Screenshot Testing](https://developer.android.com/studio/preview/compose-screenshot-testing)(실험 단계, 단독 플러그인은 유지 중단·AGP 9.5+ test suite로 이전) | `@Preview` | 중 |
| Compose Multiplatform | 19,488 | 위와 같음 | 위와 같음 | Ktor 클라이언트 목 엔진 | 데스크톱 오프스크린 렌더(`ImageComposeScene`) 존재로 알고 있으나 문서 미확인 | `@Preview` | 중 |
| Android Views (Kotlin/Java) | — | View·Fragment 필드, XML | 바이트코드 변환(AGP Instrumentation/ASM) — 소스 수준보다 어렵다 | OkHttp 인터셉터 | Paparazzi·Roborazzi가 View도 지원 | Layout Editor | 상 |
| Swing / JavaFX | jfx 3,310 (GPL-2.0) | 컴포넌트 필드 | annotation processor는 새 파일만 생성 → 바이트코드 변환 필요 | `HttpClient` 교체 | Swing은 `BufferedImage`에 그리기 가능, JavaFX 헤드리스(Monocle)는 미확인 | 없음 | 상 |

### 2.4 Dart (Flutter)

| 스택 | ★ | 상태 위치 | 강제 주입 방법 | 네트워크 | 창 없이 렌더 | 기존 미리보기 | 난이도 |
|---|---|---|---|---|---|---|---|
| Flutter | 181,293 (BSD-3) | `State<T>` 필드·Provider/Riverpod/Bloc | Dart 매크로는 [2025-01 개발 중단](https://dart.dev/blog/an-update-on-dart-macros-data-serialization). `build_runner`는 새 파일만 생성 → **개발 빌드 전 소스 재작성 단계** 필요. Riverpod·Bloc 등은 override로 주입 쉬움 | `HttpOverrides`·http `MockClient`·Dio 인터셉터 | **golden test**(`flutter_test`, 호스트에서 기기 없이) | [Widget Previewer](https://docs.flutter.dev/tools/widget-previewer) `@Preview`(3.35 실험 → 문서상 3.47 안정) | 중 |

### 2.5 C# (.NET)

| 스택 | ★ | 상태 위치 | 강제 주입 방법 | 네트워크 | 창 없이 렌더 | 기존 미리보기 | 난이도 |
|---|---|---|---|---|---|---|---|
| Avalonia | 31,847 | MVVM ViewModel(`INotifyPropertyChanged`) | Source Generator(추가만) + [interceptors](https://github.com/dotnet/roslyn/blob/main/docs/features/interceptors.md)(호출 지점 교체, .NET 9.0.2xx SDK부터 안정이라는 2차 자료, 원문 미확인) | `HttpMessageHandler` 교체 | Avalonia.Headless(문서 미확인) | XAML 미리보기 | 중 |
| WPF | 7,758 | ViewModel | 위와 같음 | 위와 같음 | `RenderTargetBitmap`, **Windows 전용** | XAML 디자이너 | 중 |
| WinUI 3 | microsoft-ui-xaml 8,618 | ViewModel | 위와 같음 | 위와 같음 | 미확인 | XAML Hot Reload | 상 |
| .NET MAUI | 23,322 | ViewModel | 위와 같음 | 위와 같음 | 기기·에뮬레이터 필요, 헤드리스 미확인 | XAML Hot Reload | 상 |
| Uno Platform | 10,125 | ViewModel | 위와 같음 | 위와 같음 | 미확인 | 미확인 | 미확인 |
| Blazor | aspnetcore 38,533 / SO 사용률 7.0% | 컴포넌트 필드·cascading 값 | Source Generator / 렌더 트리가 DOM | `HttpClient` 교체 | Playwright(결과가 웹) | bUnit(테스트) | 중 |

### 2.6 Rust

| 스택 | ★ | 상태 위치 | 강제 주입 방법 | 네트워크 | 창 없이 렌더 | 기존 미리보기 | 난이도 |
|---|---|---|---|---|---|---|---|
| **GPUI** (pippip) | zed 95,318 (gpui crate Apache-2.0) | `Entity<T>` 모델·View 필드 | proc-macro + 개발 feature | HTTP 클라이언트 교체(reqwest) | **코드에 있음**: pippip이 쓰는 `gpui-pre 0.3.12` 소스에 `HeadlessAppContext::capture_screenshot`, `Window::render_to_image`(`test-support` feature) 존재 확인. 실제 실행은 미확인 | 없음 | 중 |
| egui | 32,385 | 앱 구조체 필드(immediate mode) | proc-macro | 클라이언트 교체 | [egui_kittest](https://docs.rs/egui_kittest) `image_snapshot`, wgpu 헤드리스 | 없음 | 하~중 |
| Iced | 32,080 | Elm 방식 단일 State + Message | State가 한 곳이라 주입 쉬움 | 클라이언트 교체 | `iced_test` 시뮬레이터 snapshot, tiny-skia로 GPU 없이 | 없음 | 하~중 |
| Slint | 24,644 (자체 라이선스, GitHub 표기 NOASSERTION) | `.slint` DSL 속성 + Rust 모델 | DSL 속성은 Rust에서 set 가능 | 클라이언트 교체 | 테스트 백엔드 `take_snapshot`, `SLINT_BACKEND=headless-software` | Slint 라이브 미리보기 | 중 |
| Dioxus | 39,850 | signal·hooks | proc-macro / RSX 변환 | 클라이언트 교체 | 웹·데스크톱(웹뷰)은 Playwright, 네이티브(Blitz)는 미확인 | 없음 | 중 |
| Leptos | 21,560 | signal | proc-macro | 클라이언트 교체 | 결과가 웹 → Playwright | 없음 | 중 |
| Tauri (Rust 쪽) | 112,993 | 화면은 웹 | 웹(TS) 쪽에서 처리 | `invoke` 목 | Playwright | Storybook | 하 |

### 2.7 C++ / Go

| 스택 | ★ | 상태 위치 | 강제 주입 방법 | 네트워크 | 창 없이 렌더 | 기존 미리보기 | 난이도 |
|---|---|---|---|---|---|---|---|
| Qt Widgets / QML | 공식 저장소는 GitHub 밖(★ 미확인) | QObject 속성, QML은 JS 성격 | Clang 도구로 소스 재작성 가능하나 무겁다. QML은 동적 | `QNetworkAccessManager` 교체 | `-platform offscreen` + `grabToImage` | Qt Design Studio | 상 |
| Fyne (Go) | 28,850 | 위젯 필드·binding | Go엔 매크로 없음 → `go generate`·소스 재작성 | `http.RoundTripper` 교체 | 테스트 패키지 소프트웨어 렌더(미확인) | 없음 | 상 |
| Wails (Go) | 37,160 | 화면은 웹 | 웹(TS) 쪽에서 처리 | 바인딩 목 | Playwright | Storybook | 하 |

## 3. 기존 미리보기 도구와 dui의 차이

| 도구 | 하는 것 | dui 대비 빠진 것 |
|---|---|---|
| Storybook (★91,647) | 컴포넌트를 상태별 스토리로 렌더, 웹 계열 | 상태를 **사람이 하나하나 작성**. 전환 그래프 없음. 커버리지 개념 없음 |
| SwiftUI `#Preview` | 뷰를 Xcode 안에서 미리보기 | 같음 + Xcode 안에서만, 캔버스·폭 매트릭스 없음 |
| Compose `@Preview` (+ 스크린샷 테스트) | 프리뷰를 이미지·회귀 테스트로 | 상태는 수동, ViewModel 이 들어간 화면 전체는 어려움 |
| Flutter Widget Previewer | `@Preview` 위젯을 크기·테마별로 | 수동 작성, 전환 그래프 없음 |
| XAML 디자이너 / Qt Design Studio | 레이아웃 미리보기 | 실제 데이터 상태 아님 |

공통 빈자리: **상태 자동 발견 + 로컬 상태 강제 주입 + 전환 그래프 + 커버리지 숫자**. dui는 이 넷이 핵심이고, 각 생태계의 렌더·스냅샷 도구는 dui의 렌더 워커로 재사용한다.

## 4. 추천 순서

점수 = 시장 규모 × 구현 가능성 × 우리 프로젝트 필요.

| 순위 | 대상 | 단계 | 이유 |
|---|---|---|---|
| 1 | **TS 웹: React(+Vite/Next)** | MVP | 타입 언어 사용 1위(Octoverse 2025), React 44.7%(SO 2025). 주입·가로채기·렌더 도구가 다 있음(난이도 하). obora가 여기 |
| 2 | **Electron·Tauri 웹뷰** | MVP | 1번과 같은 엔진으로 거의 공짜. obora-desktop이 Electron |
| 3 | **Rust GPUI** | MVP(스파이크) | pippip 필요. 헤드리스 렌더 API가 코드에 있음을 확인해 가장 큰 위험이 줄어듦. 시장은 작지만 우리 쪽 검증용 |
| 4 | Vue·Svelte·Solid·Angular, Blazor·Leptos·Dioxus 웹 | 다음 | 렌더가 웹이라 Playwright·MSW 재사용. 주입 변환만 추가 |
| 5 | **Kotlin Compose** (Android + Multiplatform) | 다음 | Kotlin 10.8%(SO 2025). JVM에서 기기 없이 렌더(Paparazzi·Roborazzi) 가능, 상태가 `remember`/ViewModel로 모여 있음. 컴파일러 플러그인 작업이 핵심 비용 |
| 6 | **Flutter** | 다음 | ★181,293로 크로스플랫폼 최대. golden test로 호스트 렌더 쉬움. 매크로 중단 → 소스 재작성 단계가 비용 |
| 7 | **SwiftUI** | 다음 | iOS 시장이 크지만 macOS·Xcode 러너가 필수이고 `@State` 주입에 소스 재작성 필요 |
| 8 | React Native | 나중 | 주입은 쉬우나 실제 네이티브 렌더에 시뮬레이터 필요 |
| 9 | C# Avalonia·WPF | 나중 | MVVM이라 상태가 모여 있어 유리, interceptors로 주입 가능. 데스크톱 기업 수요 확인 후 |
| 10 | egui·Iced·Slint | 나중 | 헤드리스 렌더 도구가 이미 좋음. 시장 작음, 요청 오면 쉬움 |
| 11 | UIKit/AppKit, Android Views, Swing/JavaFX, MAUI, WinUI, Qt, Fyne | 보류 | 상태가 흩어진 명령형 UI이거나 헤드리스 렌더가 막힘(난이도 상). 99% 목표와 안 맞음 |

결론: **언어 고정은 "TS + Rust"로 시작하되, 설계는 Kotlin·Dart·Swift를 다음 대상으로 열어둔다.** 세 언어 모두 강한 타입과 기기 없는 렌더 경로가 있어 dui 방식이 통한다. 막히는 건 주입 단계(컴파일러 플러그인 / 소스 재작성)라서, 코어의 "주입 지점 → 변환" 계약을 언어 중립으로 두면 된다.

## 5. 남은 확인

- 모바일 언어·프레임워크 사용 규모 수치(JetBrains State of Developer Ecosystem 등): 미확인.
- Kotlin 컴파일러 플러그인 API 안정성, C# interceptors 안정 시점 원문: 미확인.
- Compose Desktop `ImageComposeScene`, Avalonia.Headless, Fyne 테스트 렌더: 문서 미확인.
- GPUI 헤드리스 캡처를 pippip에서 실제 실행: 미확인(M2 스파이크).
