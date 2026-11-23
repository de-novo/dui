[English](../en/supported-frameworks.md) | [简体中文](../zh-CN/supported-frameworks.md) | 한국어

# 지원 프레임워크

아래 우선순위 1–3의 38개 스택은 모두 공식 지원 대상이다. 우선순위는 구현 순서일 뿐 지원 수준이 아니다. "아직 미지원"의 11개 스택은 지금은 `미지원`이고, 각각 이유와 추가 조건을 적었다.

버전은 2026-11-20(KST)에 각 행에 링크한 공식 출처로 확인했다. 확인하지 못한 것은 `미확인`으로 표시했다. 버전 규칙: [설계: 지원 버전](design-v0.md#지원-버전-현재-최신-안정-메이저만).

## 적용한 원칙

1. **공식 컴파일러만.** 타입 정보는 프로젝트가 이미 쓰는 컴파일러(`tsc`, `rustc`, `swiftc`, `kotlinc`, `dart`)에서 얻는다.
2. **현재 최신 안정 메이저만.** 스택마다 기준선 하나. 더 새로운 버전은 항상 지원하고, 기준선은 dui 새 메이저에서만 올린다. 0.x 프로젝트는 0.minor 라인을 ‘메이저’로 본다.
3. **타입 언어만.**
4. **렌더 한계를 명시한다.** 애플 대상은 렌더에 Mac이 필요하다. 분석은 되지만 렌더가 안 되는 스택은 해당 셀을 `렌더 불가`로 보고한다.
5. **AI 네이티브.** AI가 CLI와 MCP 서버를 다루고, 사람은 캔버스만 본다.

## 요약

| 우선순위 | 개수 | 스택 |
|---|---:|---|
| 우선순위 1 (MVP) | 8 | TypeScript, Node.js, React, Vite, TanStack Router, TanStack Query, zod, Chromium (Chrome stable) |
| 우선순위 2 | 15 | Rust, GPUI (`gpui-pre`), gpui-component / gpui-kit, Zustand, Redux Toolkit, Jotai, XState, React Hook Form, valibot, Next.js, React Router, TanStack Start, Electron, Radix UI, shadcn/ui |
| 우선순위 3 | 15 | Solid, SolidStart, Preact, Rspack / Rsbuild, Tauri, Dioxus, Leptos, egui, Iced, React Native, Expo, Flutter / Dart, SwiftUI / Swift, Jetpack Compose / Kotlin, Compose Multiplatform |
| 아직 미지원 | 11 | Vue, Nuxt, Svelte, SvelteKit, Astro, Angular, Qwik, Remix, Turbopack, Slint, Playwright |

## 우선순위 1 (MVP)

| 스택 | 분야 | 지원 메이저 | 최신 안정판(확인됨) | 출시일 | 출처 | dui가 지원하는 이유 | 참고 | 열린 질문 |
|---|---|---|---|---|---|---|---|---|
| **TypeScript** | 언어/런타임 | 7.x | 7.0.2 | 2026-07-08 | [www.npmjs.com](https://www.npmjs.com/package/typescript) | 주력 타입 언어. 공식 `tsc` 7이 dui에 필요한 타입 정보를 준다. | TS 어댑터가 쓰는 7.1(sync API)은 아직 beta. | — |
| **Node.js** | 언어/런타임 | 26.x | 26.13.1 (LTS) | 2026-11-10 | [nodejs.org](https://nodejs.org/en/about/previous-releases) | TS 사이드카와 사용자 dev 서버를 실행한다. | — | — |
| **React** | 웹 UI | 19.x | 19.3.1 | 2026-10-14 | [www.npmjs.com](https://www.npmjs.com/package/react) | 가장 흔한 타입 UI 스택. 첫 렌더 대상. | — | — |
| **Vite** | 번들러/dev 서버 | 7.x / 8.x | 8.3.8 | 2026-11-11 | [www.npmjs.com](https://www.npmjs.com/package/vite) | dui가 주입된 dev 빌드를 띄워야 한다. Vite는 흔한 React dev 서버. | Vite 7에서는 Oxc `parseSync`가 없을 때 프로젝트의 공식 TypeScript AST 파서로 폴백한다. | — |
| **TanStack Router** | 라우터 | 1.x | 1.170.46 | 2026-10-22 | [www.npmjs.com](https://www.npmjs.com/package/@tanstack/react-router) | 타입 라우트에서 화면 목록을 바로 얻는다. | — | — |
| **TanStack Query** | 상태/데이터 | 5.x | 5.108.1 | 2026-11-19 | [www.npmjs.com](https://www.npmjs.com/package/@tanstack/react-query) | 서버 상태(로딩/에러/빈 값)가 상태 셀의 핵심. | — | — |
| **zod** | 상태/데이터 | 4.x | 4.6.7 | 2026-10-12 | [www.npmjs.com](https://www.npmjs.com/package/zod) | 스키마에서 실제적인 샘플 데이터와 값 범위를 얻는다. | — | — |
| **Chromium (Chrome stable)** | 렌더/테스트 인프라 | 고정 안 함 | 158.0.8127.36 | 2026-11-17 | [chromiumdash.appspot.com](https://chromiumdash.appspot.com/releases?platform=Linux) | CDP로 웹 화면을 렌더하는 headless 대상. | — | — |

## 우선순위 2

| 스택 | 분야 | 지원 메이저 | 최신 안정판(확인됨) | 출시일 | 출처 | dui가 지원하는 이유 | 참고 | 열린 질문 |
|---|---|---|---|---|---|---|---|---|
| **Rust** | 언어/런타임 | edition 2024 | 1.100.0 (toolchain) | 2026-11-12 | [github.com](https://github.com/rust-lang/rust/releases) | 두 번째 공식 컴파일러 언어(`rustc` + rust-analyzer). | edition 2024 안에서 최소 toolchain이 아직 미정. | 예 |
| **GPUI (`gpui-pre`)** | Rust UI | 0.3.x | 0.3.13 | 2026-11-16 | [crates.io](https://crates.io/crates/gpui-pre) | 웹 다음으로 계획된 네이티브 렌더 대상. | 1.0 이전: 0.x 마이너마다 깨질 수 있다. | — |
| **gpui-component / gpui-kit** | UI 킷 | 0.10.x | 0.10.0 | 2026-11-16 | [crates.io](https://crates.io/crates/gpui-component) | GPUI의 주요 컴포넌트 킷. 상태가 이 위젯을 통해 드러난다. | crate 이름이 두 개(`gpui-component`, `gpui-kit`), 둘 다 0.10.0, 같은 저장소. | — |
| **Zustand** | 상태/데이터 | 5.x | 5.1.0 | 2026-11-06 | [www.npmjs.com](https://www.npmjs.com/package/zustand) | 흔한 클라이언트 스토어. 스토어 모양이 UI 상태를 정한다. | — | — |
| **Redux Toolkit** | 상태/데이터 | 2.x | 2.15.0 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/@reduxjs/toolkit) | 타입 slice가 상태를 명시적으로 나열한다. | — | — |
| **Jotai** | 상태/데이터 | 3.x | 3.0.2 | 2026-11-09 | [www.npmjs.com](https://www.npmjs.com/package/jotai) | atom 단위 상태. React 앱에서 흔하다. | 새 메이저 3.0.0이 2026-09-08 출시. | — |
| **XState** | 상태/데이터 | 5.x | 5.33.4 | 2026-11-07 | [www.npmjs.com](https://www.npmjs.com/package/xstate) | 상태 머신은 정의상 모든 상태를 나열한다. | — | — |
| **React Hook Form** | 상태/데이터 | 7.x | 7.92.0 | 2026-11-11 | [www.npmjs.com](https://www.npmjs.com/package/react-hook-form) | 폼 검증/제출 중/에러 상태. | — | — |
| **valibot** | 상태/데이터 | 1.x | 1.5.0 | 2026-09-09 | [www.npmjs.com](https://www.npmjs.com/package/valibot) | 샘플 데이터에서 zod와 같은 역할. | — | — |
| **Next.js** | 메타 프레임워크 | 16.x | 16.4.6 | 2026-11-09 | [www.npmjs.com](https://www.npmjs.com/package/next) | 가장 많이 쓰는 React 메타 프레임워크. | Server Components는 서버에서 렌더된다. 그곳에 상태를 주입하는 방법은 미확인. | 예 |
| **React Router** | 메타 프레임워크 | 7.x / 8.x | 8.5.2 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/react-router) | 흔한 라우터. framework 모드가 Remix의 후속. | 새 메이저 8.0.0이 2026-06-17 출시. 7.x도 동일한 선언형 라우터·검색 파라미터 API를 공유한다. | — |
| **TanStack Start** | 메타 프레임워크 | 1.x | 1.169.7 | 2026-11-19 | [www.npmjs.com](https://www.npmjs.com/package/@tanstack/react-start) | MVP에 있는 TanStack Router 위에 있다. | — | — |
| **Electron** | 데스크톱 셸 | 45.x | 45.4.1 | 2026-11-19 | [www.npmjs.com](https://www.npmjs.com/package/electron) | UI가 웹인 데스크톱 앱. 같은 Chromium 경로로 렌더. | 대략 8주마다 새 메이저라 기준이 자주 바뀐다. | 예 |
| **Radix UI** | UI 킷 | 1.x | 1.8.1 | 2026-10-12 | [www.npmjs.com](https://www.npmjs.com/package/radix-ui) | 많은 React 킷의 기반 headless 원시 컴포넌트. 열림/닫힘 상태. | — | — |
| **shadcn/ui** | UI 킷 | 4.x (CLI) | 4.23.0 | 2026-10-27 | [www.npmjs.com](https://www.npmjs.com/package/shadcn) | 매우 흔한 React 컴포넌트 출처. 코드가 사용자 저장소에 있다. | 저장소에 복사되는 코드라 런타임 라이브러리가 아니다. ‘지원’이 문서뿐일 수 있다. | 예 |

## 우선순위 3

| 스택 | 분야 | 지원 메이저 | 최신 안정판(확인됨) | 출시일 | 출처 | dui가 지원하는 이유 | 참고 | 열린 질문 |
|---|---|---|---|---|---|---|---|---|
| **Solid** | 웹 UI | 2.x | 2.0.3 | 2026-11-19 | [www.npmjs.com](https://www.npmjs.com/package/solid-js) | 타입 JSX. 세밀한 signal. | 메이저 2.0.0이 2026-11-17 출시. 생태계가 뒤처질 수 있다. | — |
| **SolidStart** | 메타 프레임워크 | 2.x | 2.2.3 | 2026-11-16 | [www.npmjs.com](https://www.npmjs.com/package/@solidjs/start) | Solid의 메타 프레임워크. | npm license 필드가 비어 있다(라이선스 미확인). | — |
| **Preact** | 웹 UI | 11.x | 11.2.0 | 2026-11-06 | [www.npmjs.com](https://www.npmjs.com/package/preact) | React 호환 API. React 작업 대부분이 이어진다. | — | — |
| **Rspack / Rsbuild** | 번들러/dev 서버 | 2.x | 2.3.3 | 2026-11-17 | [www.npmjs.com](https://www.npmjs.com/package/@rspack/core) | webpack 계열 프로젝트를 위한 두 번째 dev 서버 경로. | dev 서버가 늘 때마다 유지할 주입 경로가 늘어난다. | — |
| **Tauri** | 데스크톱 셸 | 2.x | 2.12.7 (crate) / 2.12.5 (@tauri-apps/api) | 2026-11-18 | [crates.io](https://crates.io/crates/tauri) | Rust + 웹 데스크톱. Rust 코어와 잘 맞는다. | OS webview(macOS/Linux는 WebKit)를 쓰고 Chromium이 아니다. 렌더 일치 여부 미확인. | — |
| **Dioxus** | Rust UI | 0.7.x | 0.7.10 | 2026-07-30 | [crates.io](https://crates.io/crates/dioxus) | 웹/데스크톱 대상 Rust UI. | 1.0 이전. | — |
| **Leptos** | Rust UI | 0.8.x | 0.8.23 | 2026-10-31 | [crates.io](https://crates.io/crates/leptos) | Rust 웹 UI. Chromium에서 렌더. | 1.0 이전. | — |
| **egui** | Rust UI | 0.37.x | 0.37.0 | 2026-10-15 | [crates.io](https://crates.io/crates/egui) | 다운로드가 가장 많은 Rust 즉시 모드 UI. | 즉시 모드라 상태가 컴포넌트 트리에 없다. 상태 찾기 방식이 달라야 할 수 있다. | 예 |
| **Iced** | Rust UI | 0.14.x | 0.14.0 | 2025-12-07 | [crates.io](https://crates.io/crates/iced) | Elm 스타일 Rust UI. 메시지가 상태를 나열한다. | 2025-12-07 이후 새 안정판이 없다. | 예 |
| **React Native** | 모바일/네이티브 | 0.88.x | 0.88.1 | 2026-10-27 | [www.npmjs.com](https://www.npmjs.com/package/react-native) | 모바일 React. React 분석을 재사용. | 아직 0.x라 ‘메이저’를 0.minor로 봐야 한다. 시뮬레이터 필요(iOS는 Mac 필요). | 예 |
| **Expo** | 모바일/네이티브 | 58.x | 58.0.26 | 2026-11-17 | [www.npmjs.com](https://www.npmjs.com/package/expo) | React Native 앱을 시작하는 가장 흔한 방법. | React Native와 같은 렌더 제약. | 예 |
| **Flutter / Dart** | 모바일/네이티브 | Flutter 3.x / Dart 3.x | Flutter 3.50.0 (Dart 3.14.0); Dart SDK 3.14.1 | Flutter 2026-11-11; Dart 2026-11-17 | [docs.flutter.dev](https://docs.flutter.dev/release/archive) | 타입 언어, 공식 컴파일러(Dart). 모바일 비중이 크다. | 새 Dart 언어 어댑터가 필요. headless 렌더 경로 미확인. | 예 |
| **SwiftUI / Swift** | 모바일/네이티브 | Swift 6.x | 6.4.2 | 2026-10-30 | [github.com](https://github.com/swiftlang/swift/releases) | 공식 컴파일러 `swiftc` + SourceKit-LSP. 애플 네이티브 UI. | 렌더에는 Mac(Xcode/시뮬레이터)이 필요. 타입 분석은 다른 곳에서도 가능할 수 있다. | 예 |
| **Jetpack Compose / Kotlin** | 모바일/네이티브 | Kotlin 2.x; Compose UI 1.x | Kotlin 2.4.21; Compose UI 1.13.0 (BOM 2026.11.01) | Kotlin 2026-10-08; Compose: `미확인` | [developer.android.com](https://developer.android.com/jetpack/androidx/releases/compose-ui) | 공식 컴파일러 `kotlinc`. 안드로이드 네이티브 UI. | Kotlin 어댑터와 Android/JVM 렌더 경로가 필요. | 예 |
| **Compose Multiplatform** | 모바일/네이티브 | 1.x | 1.12.2 | 2026-10-15 | [github.com](https://github.com/JetBrains/compose-multiplatform/releases) | 같은 Kotlin 어댑터. 데스크톱 대상은 JVM에서 렌더. | 위 Kotlin 결정에 달려 있다. | 예 |

## 아직 미지원

**규칙:** 이 표의 스택은 막힌 점이 풀리는 즉시 추가한다. 예를 들어 공식 도구가 TypeScript 7을 받아들일 때다. 그 전까지 dui는 스캔 전에 `미지원`(이유)으로 거절한다.

Vue, Nuxt, Svelte, SvelteKit, Astro, Angular: 공식 타입 검사 도구가 아직 TypeScript 7을 받지 않는다(npm peer 범위, 2026-11-20 확인).

| 스택 | 분야 | 최신 메이저 | 최신 안정판(확인됨) | 출시일 | 출처 | 막힌 점 | 이유 | 추가 시점 |
|---|---|---|---|---|---|---|---|---|
| **Vue** | 웹 UI | 3.x | 3.5.45 | 2026-11-05 | [www.npmjs.com](https://www.npmjs.com/package/vue) | TS 7 도구 | 템플릿 타입 검사가 `vue-tsc`를 거친다. TS 7 지원 미확인이라 TS 7+ 원칙과 충돌. | 공식 도구가 TypeScript 7을 받아들일 때. |
| **Nuxt** | 메타 프레임워크 | 4.x | 4.6.4 | 2026-11-02 | [www.npmjs.com](https://www.npmjs.com/package/nuxt) | TS 7 도구 | Vue 기반이라 Vue 상태를 따른다. | Vue가 추가될 때. |
| **Svelte** | 웹 UI | 5.x | 5.58.0 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/svelte) | TS 7 도구 | `svelte-check` 4.7.8의 TS peer가 ^5 \|\| ^6이라 7이 아니다. | 공식 도구가 TypeScript 7을 받아들일 때. |
| **SvelteKit** | 메타 프레임워크 | 3.x | 3.4.0 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/@sveltejs/kit) | TS 7 도구 | TS peer가 ^6.0.0이라 7이 아니다. | 공식 도구가 TypeScript 7을 받아들일 때. |
| **Astro** | 메타 프레임워크 | 7.x | 7.4.13 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/astro) | TS 7 도구 | `@astrojs/check`의 TS peer가 ^5 \|\| ^6이라 7이 아니다. | 공식 도구가 TypeScript 7을 받아들일 때. |
| **Angular** | 웹 UI | 22.x | 22.3.0 | 2026-11-18 | [www.npmjs.com](https://www.npmjs.com/package/@angular/core) | TS 7 도구 | `@angular/compiler-cli` 22.3.0이 TS >=6.0 <6.1을 요구해 TS 7+와 충돌. | 공식 도구가 TypeScript 7을 받아들일 때. |
| **Qwik** | 웹 UI | 1.x (v2 in RC) | 1.20.4 | 2026-11-09 | [www.npmjs.com](https://www.npmjs.com/package/@builder.io/qwik) | 메이저가 아직 RC | v2(`@qwik.dev/core`)가 2.0.0-rc.5. 지금 1.x를 지원하면 기준이 곧 낡는다. | v2가 안정판이 될 때(열린 질문). |
| **Remix** | 메타 프레임워크 | 2.x (legacy) | 2.17.5 (@remix-run/react) | 2026-06-01 | [www.npmjs.com](https://www.npmjs.com/package/@remix-run/react) | 레거시 | 레거시 라인. React Router로 커버. | 계획 없음: React Router로 커버. |
| **Turbopack** | 번들러/dev 서버 | Next.js에 포함 | 단독 패키지 없음(next 16.4.6에 포함) | 2026-11-09 | [www.npmjs.com](https://www.npmjs.com/package/next) | 단독 패키지 없음 | 따로 추적할 버전이 없다. | 계획 없음: Next.js로 커버. |
| **Slint** | Rust UI | 1.x | 1.19.0 | 2026-11-16 | [crates.io](https://crates.io/crates/slint) | 라이선스 | 자체 마크업 언어라 별도 어댑터 필요. 라이선스가 GPL-3.0 또는 Slint 자체 라이선스. | 열린 질문(라이선스, 별도 어댑터). |
| **Playwright** | 렌더/테스트 인프라 | 1.x | 1.65.1 | 2026-11-13 | [www.npmjs.com](https://www.npmjs.com/package/playwright) | 의존성 아님 | 사용자가 dui 상태를 테스트로 재생하길 원할 때만 의미가 있다. | 열린 질문(상태를 테스트로 재생). |

## 주시할 dui 자체 의존성

캔버스 라이브러리 `@xyflow/react`에 새 메이저가 나왔다: 13.0.0이 2026-11-16, 최신 13.0.2가 2026-11-17([npm](https://www.npmjs.com/package/@xyflow/react)). 이것은 dui 자신의 UI이고, 지원 대상 사용자 스택이 아니다.

## 열린 질문

아직 정하지 않았다. 잊지 않도록 여기 적어 둔다.

- **Rust**: edition 2024 안에서 최소 toolchain이 아직 미정.
- **Next.js**: Server Components는 서버에서 렌더된다. 그곳에 상태를 주입하는 방법은 미확인.
- **Electron**: 대략 8주마다 새 메이저라 기준이 자주 바뀐다.
- **shadcn/ui**: 저장소에 복사되는 코드라 런타임 라이브러리가 아니다. ‘지원’이 문서뿐일 수 있다.
- **egui**: 즉시 모드라 상태가 컴포넌트 트리에 없다. 상태 찾기 방식이 달라야 할 수 있다.
- **Iced**: 2025-12-07 이후 새 안정판이 없다.
- **React Native**: 아직 0.x라 ‘메이저’를 0.minor로 봐야 한다. 시뮬레이터 필요(iOS는 Mac 필요).
- **Expo**: React Native와 같은 렌더 제약.
- **Flutter / Dart**: 새 Dart 언어 어댑터가 필요. headless 렌더 경로 미확인.
- **SwiftUI / Swift**: 렌더에는 Mac(Xcode/시뮬레이터)이 필요. 타입 분석은 다른 곳에서도 가능할 수 있다.
- **Jetpack Compose / Kotlin**: Kotlin 어댑터와 Android/JVM 렌더 경로가 필요.
- **Compose Multiplatform**: 위 Kotlin 결정에 달려 있다.
- **Qwik**: v2(`@qwik.dev/core`)가 2.0.0-rc.5. 지금 1.x를 지원하면 기준이 곧 낡는다.
- **Slint**: 자체 마크업 언어라 별도 어댑터 필요. 라이선스가 GPL-3.0 또는 Slint 자체 라이선스.
- **Playwright**: 사용자가 dui 상태를 테스트로 재생하길 원할 때만 의미가 있다.
