# dui 설계 v0 (개발자 도구)

[English](../en/design-v0.md) | [简体中文](../zh-CN/design-v0.md) | 한국어

> 이 문서는 [영어 원문](../en/design-v0.md)의 한국어 번역이다. 내용이 다르면 영어가 기준이다.

> 상태: 초안 · 작성 2026-11-19 · 대상 독자: dui를 만들 개발자
> 이 문서의 외부 사실(라이선스·스타 수·버전)은 2026-11-19에 GitHub API로 확인했다. 확인 못 한 것은 `미확인`으로 적는다.

> **위치**: 디자인 생성 도구는 *이렇게 만들 것이다*를 보여준다: 아직 없는 화면을 AI가 상상해서 그린다. dui는 *실제로 이렇게 만들어져 있다*를 보여준다: 공식 컴파일러로 지금 있는 코드를 읽고, 실제로 존재하는 모든 상태를 진짜로 렌더한다. 아무것도 지어내지 않는다. 보여줄 수 없는 것은 `미검증` 또는 `렌더 불가`로 표시한다.

## 0. 핵심 원칙: AI 네이티브

**dui는 AI 네이티브다. 모든 건 AI가 쓰고, 사람은 캔버스만 본다.** (2026-11-19 결정)

| 면 | 누가 | 무엇 |
|---|---|---|
| AI 면 | AI 에이전트 | `dui` CLI + MCP 서버. JSON 출력, 고정된 종료 코드, 대화형 질문 없음, 멱등 명령. 분석·주입·렌더·검증을 AI가 돌리고 `미검증` 칸도 AI가 고친다 |
| 사람 면 | 사람 | **캔버스만.** 설정 화면도, UI 안의 명령어 안내도 없다. 화면 × 상태 × 폭 × 전환만 보여준다 |
| 고리 | 둘 다 | 사람이 캔버스 칸에 코멘트 → AI가 MCP로 코멘트를 읽음 → 코드 수정 → 캔버스 다시 렌더 |

아래 모든 구성요소는 이 분담을 따른다: 사람이 "실행"할 것은 AI 에이전트가 실행하고, 사람이 "봐야" 할 것은 캔버스에 있다.

### 원칙 #2: 공식 컴파일러 의존 (2026-11-19 결정)

**dui는 공식 컴파일러에 의존하는 프로젝트다.**

- 각 언어의 **공식 컴파일러·툴체인**만 지원한다: TypeScript → Microsoft `typescript` 패키지의 `tsc`(**7.0 이상만**), Rust → `rustc`(Cargo)와 타입 조회용 rust-analyzer, 나중에 Swift → `swiftc` + SourceKit-LSP, Kotlin → `kotlinc`.
- 비공식·대체 컴파일러와 툴체인은 **`미지원`**이다.
- dui는 사용자 프로젝트에 이미 있는 공식 컴파일러를 쓴다(버전까지 똑같이). **타입 검사를 다시 만들지 않는다**.
- 자세한 내용: [기술 스택](#기술-스택-2026-11-19-결정).

## 1. 문제와 사용자

**사용자**: TypeScript 또는 Rust로 UI를 만드는 개발자·디자이너·QA.

**문제**
- 한 화면에도 상태가 많다(빈 목록, 로딩, 오류, 권한 없음, 긴 텍스트, 마지막 장 승인 후 …). 지금은 직접 클릭해서 그 상태까지 가야 볼 수 있다.
- 어떤 상태를 아직 한 번도 그려본 적 없는지 알 방법이 없다.
- 반응형 폭마다 따로 창 크기를 바꿔 봐야 한다.
- Storybook은 상태를 사람이 하나하나 써야 하고, Figma는 실제 코드가 아니다.

**dui가 하는 일**: 코드베이스를 읽고, 앱의 **모든 화면 × 모든 상태 × 모든 폭**을 실제 코드로 렌더해서 Figma처럼 무한 캔버스에 펼친다. 못 그린 칸은 `미검증`으로 보인다. 목표 커버리지는 99%이며 측정 방법은 §8에 정의한다.

## 2. 목표와 비목표

**목표**
- 타입 있는 언어만: TypeScript(웹·웹뷰), Rust(GPUI 네이티브).
- 사람이 상태를 쓰지 않아도 AI + 타입으로 상태를 찾아내고 목 값을 만든다.
- 프레임워크별 어댑터를 손으로 쓰지 않는다. AI가 대상 코드에 맞는 주입 코드를 그때그때 만든다.
- 결과는 측정 가능해야 한다(분모·분자가 있는 커버리지).

**비목표**
- 캔버스 안에서 클릭·입력 같은 상호작용. 정지 화면만 보여준다.
- 시각 회귀 테스트(픽셀 비교 승인 흐름). Chromatic·Percy 영역이다. 나중에 붙일 수는 있다.
- 타입 없는 JavaScript, Python, Dart, Swift, Kotlin 등. (v0 범위 밖)
- 프로덕션 빌드에 SDK를 남기는 것. 모든 주입 코드는 개발 빌드 전용이다.
- 디자인 편집 도구. Figma로 내보내기는 하지만 Figma를 대체하지 않는다.

## 3. 핵심 아이디어

1. **AI가 상태가 사는 곳을 전부 찾는다**: 전역 store, 컴포넌트 로컬 상태(`useState` 등), context, URL·라우트, 서버 응답 캐시, GPUI Entity/모델 필드.
2. **빌드할 때 강제 주입 코드를 끼운다**: 소스는 그대로 두고, 개발 빌드 변환(TS는 oxc 변환, Rust는 proc-macro·build 스크립트, [기술 스택](#기술-스택-2026-11-19-결정) 참고)으로 "이 값으로 시작하라" 입구를 만든다. 그래서 로컬 상태도 밖에서 넣을 수 있다.
3. **API는 네트워크에서 가로챈다**: 타입에서 생성한 목으로 응답한다.
4. **넣고, 그리고, 검사한다**: 상태를 주입해 폭별로 렌더하고, 렌더 결과가 의도한 상태와 맞는지 검사한다. 안 맞으면 `미검증`.

타입이 핵심인 이유: 타입이 있어야 (a) 상태의 모양을 빠짐없이 나열하고 (b) 유효한 목 값을 자동으로 만들고 (c) 주입 코드가 컴파일러 검사를 통과했는지로 1차 검증이 된다.

## 4. 아키텍처

```mermaid
flowchart LR
  subgraph CLI["dui CLI"]
    A[분석기 Analyzer]
    G[(코드 그래프<br/>자체 구현: tree-sitter + oxc)]
    I[주입기 Injector<br/>빌드 변환]
    M[목 생성기<br/>Mock Generator]
    P[계획기 Planner<br/>상태 펼치기]
  end
  subgraph Run["실행"]
    N[네트워크 가로채기<br/>Interceptor]
    R[렌더 워커들<br/>CDP로 Chromium 구동 / GPUI 갤러리]
    V[검증기 Verifier]
  end
  S[(스냅샷 저장소<br/>.dui/)]
  C[캔버스 앱<br/>React Flow]
  F[Figma 내보내기]

  Code[대상 코드베이스] --> G --> A
  Code --> A
  A -->|상태 위치·타입·전환 규칙| P
  A -->|주입 지점| I
  A -->|타입·스키마| M
  P -->|렌더할 상태 목록| R
  M --> N
  M --> R
  I -->|개발 빌드| R
  N --> R
  R -->|이미지·DOM·커버리지| V
  V --> S
  S --> C
  S --> F
  V -->|안 덮인 분기| P
  AI[AI 에이전트] -->|CLI / MCP| A
  AI -->|CLI / MCP| R
  H[사람] -->|보기, 칸에 코멘트| C
  C -->|MCP로 칸 코멘트 전달| AI
```

| 구성요소 | 하는 일 | 입력 → 출력 |
|---|---|---|
| `dui` CLI | **AI 에이전트용** 입구(사람용 아님): 전체 실행·캐시·설정. `dui scan`, `dui render`, `dui open`. JSON 출력, 고정 종료 코드, 질문 없음, 멱등 | 설정 파일 → 실행 |
| MCP 서버 | CLI와 같은 작업을 AI 에이전트에게 제공 + 캔버스 칸 코멘트 읽기·처리 | MCP 호출 → JSON |
| 분석기 | 화면·상태 위치·이벤트·상태 변경 지점·전환 규칙 찾기. 자체 코드 그래프 + 공식 컴파일러의 타입 정보 + AI | 코드 → `Screen`, `StateSlot`, `Rule` |
| 주입기 | 상태 위치마다 개발용 주입 입구를 빌드 시 삽입 | `StateSlot` → 변환된 개발 빌드 |
| 목 생성기 | 타입·스키마(zod, OpenAPI, Rust 타입)에서 대표값 생성 | 타입 → 값 묶음 |
| 계획기 | 데이터 모양 × 분기 × 순서를 조합해 렌더할 상태 목록 생성, 안 덮인 분기는 다시 계획 | 규칙·커버리지 → `State[]` |
| 네트워크 가로채기 | API 요청을 가로채 상태에 맞는 목 응답 | 요청 → 목 응답 |
| 렌더 워커 | 상태 주입 후 폭별 렌더, 이미지·DOM·커버리지 수집. 병렬 실행 | `State` × 폭 → `RenderCell` |
| 검증기 | 렌더가 의도한 상태인지 검사 | `RenderCell` → 검증 상태 |
| 스냅샷 저장소 | 모든 결과를 파일로 저장(`.dui/`), 커밋 해시 단위 | — |
| 캔버스 앱 | 화면×상태 노드, 폭 열, 전환 선을 무한 캔버스에 배치 | 저장소 → 화면 |
| Figma 내보내기 | 같은 배치를 Figma 파일로 | 저장소 → Figma |

## 5. 데이터 모델

```ts
type Lang = "ts" | "rust";
type Provenance = "추출됨" | "추정됨" | "검증됨"; // 코드에 명시 / AI 추론 / 재생으로 확인

interface Screen {
  id: string;                 // "review", "export", "settings.team"
  route?: string;             // 웹: "/projects/$id/review"
  entry: SourceRef;           // 화면 루트 컴포넌트 또는 GPUI View
  lang: Lang;
}

interface StateSlot {         // 상태가 사는 한 곳
  id: string;
  kind: "store" | "local" | "context" | "route" | "server" | "entity";
  owner: SourceRef;           // 이 상태를 가진 컴포넌트·store·Entity
  type: TypeRef;              // 타입 정보(목 생성의 근거)
  injectable: boolean;        // 주입 입구 생성 성공 여부
}

interface State {
  id: string;                 // 내용 해시
  screenId: string;
  label: string;              // "목록 비어 있음", "마지막 장 승인 후"
  kind: "data-shape" | "branch" | "sequence";
  values: Record<StateSlotId, unknown>;   // 슬롯별 주입 값
  network: MockResponse[];    // 이 상태에서 API가 돌려줄 응답
  path?: EdgeId[];            // sequence 상태: 처음부터 여기까지 경로
  expect: Assertion[];        // 검증기가 확인할 것
  targets: BranchId[];        // 이 상태로 덮으려는 코드 분기
}

interface Edge {              // 상태 전환
  id: string;
  from: StateId;
  to: StateId;
  event: string;              // "onClick: 승인", "단축키 1"
  rule: SourceRef;            // 근거가 된 핸들러·reducer
  provenance: Provenance;
}

interface Snapshot {          // 한 번의 실행 결과 묶음
  id: string;
  commit: string;             // 대상 레포 커밋
  createdAt: string;
  widths: number[];           // 예: [375, 768, 1280]
  axes?: { theme?: ("light" | "dark")[]; locale?: string[] };
}

interface RenderCell {        // 캔버스의 칸 하나 = 상태 × 폭(× 축)
  snapshotId: string;
  stateId: string;
  width: number;
  image?: string;             // 파일 경로
  dom?: string;               // 웹: 직렬화 DOM / 접근성 트리
  coveredBranches: BranchId[];
  status: VerificationStatus;
}

type VerificationStatus =
  | { kind: "verified" }                          // 검증 통과
  | { kind: "unverified"; reason: "render-failed" | "assert-failed" | "not-injectable" | "not-rendered" }
  | { kind: "suspect"; reason: string };          // 시각 검사만 실패, 사람 확인 필요
  | { kind: "unrenderable"; reason: "no-render-env" | "system-ui" | string }; // 이 환경에서 렌더 불가; "렌더링 한계" 참고
```

`미검증` = `unverified` 전부. 캔버스에는 회색 빗금 칸 + 이유로 보인다.

`렌더 불가`(`unrenderable`)는 `미검증`과 다른 별도 상태다: 현재 렌더 환경에서는 그 칸을 아예 그릴 수 없다. 캔버스에는 "렌더 불가(이유)"로 보인다.

## 6. 파이프라인

1. **스캔** (`dui scan`)
   - dui 자체 코드 그래프(tree-sitter + oxc)로 호출·import 그래프 생성.
   - 화면 찾기: 라우터 정의(TanStack Router, Next app 디렉터리 등), GPUI 루트 View.
   - 상태 위치 찾기: 타입 정보 + AI. 결과는 `StateSlot[]`.
   - 코드 분기 목록: 화면에서 도달 가능한 렌더 분기(조건부 JSX, early return, `match`).
2. **상태 나열** (계획기) — 세 종류를 합친다.
   - **데이터 모양**: 타입에서 대표값 생성. 목록은 0·1·많음, 문자열은 빈 값·보통·아주 긺, optional은 있음·없음, union은 모든 갈래, API는 성공·오류(4xx/5xx)·지연.
   - **코드 분기**: 각 분기 조건을 거꾸로 풀어 그 분기로 들어가는 값을 AI가 만든다.
   - **순서**: 이벤트 핸들러 → 상태 변경 지점까지 그래프를 따라가 `상태 + 이벤트 → 다음 상태` 규칙을 만든다. 처음 상태부터 규칙을 펼친다(모델 검사 방식). 무한 펼침은 대표값(0, 1, 마지막, 마지막 다음)과 깊이 한도로 자른다. 같은 값 묶음은 하나로 합친다.
3. **주입 빌드**: 주입기가 개발 빌드를 만든다. 컴파일 실패한 슬롯은 `injectable=false`.
4. **렌더** (`dui render`): 워커가 상태마다 주입 값 + 목 응답을 넣고, 폭별로 렌더. 이미지·DOM·분기 커버리지 저장.
5. **검증**: §7.
6. **보충 반복**: 안 덮인 분기가 남으면 2단계로 돌아가 그 분기 전용 상태를 만든다. 개선이 없거나 반복 한도에 닿으면 멈춘다.
7. **순서 재생 검증**: sequence 상태는 주입만이 아니라 처음 상태에서 경로대로 이벤트를 실제로 재생해 같은 화면이 나오는지 확인한다. 맞으면 해당 선이 `검증됨`.
8. **보기**: AI 에이전트가 캔버스를 띄우거나(`dui open`) Figma로 내보낸다. 사람은 캔버스만 보고 칸에 코멘트를 남긴다. 코멘트는 MCP로 1단계에 돌아간다.

## 7. 검증

렌더가 "그렸다"는 것과 "의도한 상태를 그렸다"는 다르다. 세 단계로 검사한다.

1. **값 확인**(필수): 렌더 직후 주입한 슬롯의 실제 값을 다시 읽어 의도한 값과 같은지. 다르면 주입 실패.
2. **의미 확인**(필수): 상태에서 자동 생성한 단언을 DOM·접근성 트리(웹) 또는 GPUI 요소 트리(네이티브, 가능 여부 `미확인`)에 대해 검사. 예: "빈 목록 상태 → 목록 항목 0개 + 빈 안내 문구 존재", "오류 상태 → role=alert 존재".
3. **시각 확인**(보조): AI가 이미지를 보고 상태 설명과 맞는지 판정. 이것만 실패하면 `suspect`.

추가로 **커버리지 확인**: 그 상태가 덮으려던 분기(`targets`)가 실제로 실행됐는지 커버리지로 확인. 안 됐으면 `unverified: assert-failed`.

## 8. 99%를 어떻게 재나

감이 아니라 숫자로 잰다. 분모를 명확히 정의한다.

- **분모** = (화면별 렌더 분기 수 + 화면별 데이터 모양 대표값 수 + 깊이 한도 안에서 도달 가능한 순서 상태 수) × 설정한 폭 수.
  - 렌더 분기: 화면 루트에서 도달 가능한 컴포넌트 안의 조건부 렌더 분기(조건부 JSX, 삼항, early return, `match` 갈래). 화면에 그려지는 것에 영향 없는 분기(로깅 등)는 제외 규칙으로 뺀다.
  - 분모 자체도 분석기가 만든 것이므로 "분모에서 빠진 분기"를 따로 잰다: 런타임 커버리지 도구(V8 coverage / istanbul, Rust는 `cargo llvm-cov` 계열)의 전체 분기 목록과 대조.
- **렌더 불가 칸**("렌더링 한계" 참고)은 **분모에서 뺀다**. 대신 항상 별도 개수와 이유를 함께 보고한다. 그래서 커버리지 = `verified` 칸 수 ÷ 렌더 가능한 칸 수.
- **미지원** 프로젝트·모듈(현재 최신 안정 메이저보다 오래된 버전 또는 비공식 컴파일러, 기술 스택 참고)은 스캔 전에 거절하고 아예 재지 않는다.
- **분자** = 분모 중 `verified` 칸 수.
- **보고**: 화면별·종류별 렌더 가능 커버리지 % + 미검증 칸 목록(이유 포함) + 렌더 불가 개수(이유 포함). 예: `review 97.8% (분기 44/45, 모양 18/18, 순서 23/24) × 3폭; 렌더 불가 2 (system-ui)`.
- **99% 조건**: 우리 코드(obora, pippip)는 화면 상태를 타입·reducer·상태 기계로 한곳에 모으는 규칙을 둔다. 그래야 순서 상태의 분모가 정확해진다.

## 9. 언어별 세부

### 9.1 TypeScript (웹, Electron·Tauri 웹뷰)

| 항목 | 방법 |
|---|---|
| 화면 찾기 | 라우터 정의 정적 분석(TanStack Router, React Router, Next app 디렉터리). obora-web은 TanStack Router(확인됨) |
| 타입 | `TypeOracle`을 통해 프로젝트의 공식 TypeScript 컴파일러로 슬롯 타입 해석(기술 스택 참고). zod 스키마가 있으면 우선 사용(obora-web은 zod 사용, 확인됨) |
| 로컬 상태 주입 | 개발 빌드의 oxc 변환이 `useState(init)`을 `useState(__dui.get("slotId") ?? init)` 꼴로 바꾼다. `useReducer`, context Provider value도 같은 방식 |
| store 주입 | Zustand·Redux·Jotai 등은 생성 지점에서 초기값을 같은 방식으로 감싼다 |
| 서버 캐시 | TanStack Query는 렌더 전 `QueryClient`에 데이터 미리 채우기 또는 네트워크 목 |
| 네트워크 | CDP Fetch 도메인으로 요청을 가로채 목 응답을 준다(MSW 없음). 브라우저 밖(서버 쪽)에서 나가는 요청을 가로채는 방법은 `미확인` |
| 렌더 | Rust에서 CDP로 Chromium을 구동(chromiumoxide)해 폭별 뷰포트 렌더. 이미지 + DOM + 접근성 트리 + V8 커버리지 |
| Electron | 렌더러는 웹과 같다. preload/IPC 호출은 목으로 막고 렌더러 번들을 CDP로 Chromium에서 띄운다 |
| Tauri | `invoke` 호출을 목으로 막고 웹과 동일하게 렌더 |
| 순서 재생 | CDP 입력 이벤트로 실제 클릭·키 입력 재생 |

주입 런타임 `__dui`는 개발 빌드에만 들어가고, 프로덕션 빌드에서는 변환이 꺼진다. 빌드 산출물에 `__dui`가 없음을 CI에서 검사한다.

### 9.2 Rust (GPUI 네이티브)

pippip은 `gpui-pre 0.3.12` + `gpui-kit 0.10.0`, HTTP는 `reqwest` 사용(Cargo.toml에서 확인). GPUI crate 라이선스는 Apache-2.0(zed 레포 `crates/gpui/Cargo.toml`에서 확인).

| 항목 | 방법 |
|---|---|
| 화면 찾기 | 루트 View와 화면 전환 지점 정적 분석 + AI |
| 타입 | rust-analyzer 또는 `syn` 기반 파싱으로 Entity/모델 필드 타입 해석 |
| 상태 주입 | `#[dui::state]` proc-macro를 개발 feature에서만 붙여 모델 타입에 `serde` 직렬화 + "이 값으로 시작" 생성자를 만든다. 붙이는 작업은 AI가 build 단계에서 패치로 생성 |
| 상태 꺼내기 | 같은 매크로로 현재 상태 덤프(실사용 중 상태 수집에도 사용) |
| 네트워크 | HTTP 클라이언트 생성 지점을 개발 feature에서 목 서버(로컬) 주소로 돌리거나 클라이언트를 감싼 trait로 교체 |
| 렌더 | 앱 안 `dui` 갤러리 모드: 상태 파일을 읽어 View를 만들고 폭별 창 크기로 그린 뒤 이미지 저장. `gpui-pre 0.3.12` 소스에 `HeadlessAppContext::capture_screenshot`·`Window::render_to_image`(`test-support` feature)가 있음을 확인(2026-11-19, [언어 조사](language-survey.md)). pippip에서 실제 실행은 `미확인` → M2 스파이크 1순위. 안 되면 숨김 창 + OS 캡처로 대체 |
| 순서 재생 | 갤러리 모드에서 액션 디스패치로 재생 |
| 검증 | 값 확인은 덤프 비교로 가능. 요소 트리 기반 의미 확인은 `미확인` |

전제 규칙: 화면 상태는 Entity/모델에 둔다. View 안에 흩어진 상태는 덤프·주입에서 빠지므로 `not-injectable`로 표시된다.

## 10. 그래프 엔진

- **2026-11-19 변경**: 본체가 Rust라서 코드 그래프는 dui 바이너리 안에서 **tree-sitter + oxc로 직접 만든다**. Graphify는 Python이라 내장 엔진이 될 수 없다.
- **Graphify** (Apache-2.0, ★150,430, 2026-11-19 확인)는 이제 **참고만(아이디어)** 한다: tree-sitter로 약 40개 언어, 선마다 `EXTRACTED`/`INFERRED` 태그(README에서 확인). dui는 이 아이디어를 가져와 태그를 `추출됨`/`추정됨`/`검증됨`으로 확장한다.
- **CodeGraph** (MIT, ★78,082, 2026-11-19 확인): 프레임워크별 라우트 인식 방식만 참고(README에서 확인).
- 그래프 엔진은 바꿀 수 있게 dui 내부에는 `CodeGraphPort`(노드·선 읽기) 한 가지 인터페이스만 둔다.

## 11. 캔버스

배치: **행 = 화면×상태, 열 = 폭(및 테마·언어 축), 선 = 전환**. 미검증 칸은 회색 빗금 + 이유.

| 후보 | 라이선스(2026-11-19 확인) | 판단 |
|---|---|---|
| React Flow (`xyflow/xyflow`) | MIT, ★39,087 | **1순위**. 노드 안에 이미지 또는 iframe(웹은 살아있는 DOM) |
| tldraw (`tldraw/tldraw`) | 자체 라이선스(GitHub 표기 NOASSERTION). LICENSE.md: 개발 환경에서만 무료, **프로덕션 사용은 별도 상업 라이선스 또는 체험 라이선스 키 필요**, 라이선스 키·워터마크 강제, 사용 데이터 전송 가능 | **v0에서 제외**. 프로덕션 조건이 dui를 상업적으로 쓰는 사용자·기업에게 넘어가므로 비용·조건 확인 전에는 쓰지 않음 |
| Figma 내보내기 | Figma 플러그인/REST API (요금·한도 `미확인`) | 보조 출력. 이미지 노드로 같은 배치 생성 |

### 참고 프로젝트: Whiteboard (아이디어만)

[devdotfast/whiteboard](https://github.com/devdotfast/whiteboard) (MIT, ★3,553, 2026-11-19 확인)는 사람과 AI 에이전트가 같은 캔버스에서 소프트웨어를 설계하는 데스크톱 앱이다. dui는 **아이디어만 참고하고 코드는 복사하지 않는다**:

| Whiteboard의 아이디어 | dui에서 쓰는 방식 |
|---|---|
| 에이전트가 캔버스에 그리게 하는 SDK (AI가 API로 그림) | dui MCP 서버의 캔버스 쪽 본보기 |
| 캔버스 항목을 누르면 해당 코드로 이동 | dui 칸을 누르면 그 상태가 정의된 코드로 이동 |
| 사람이 표시하면 에이전트가 다시 그림 | dui의 고리와 같음: 캔버스 칸 코멘트 → AI가 코드 수정 → 캔버스 다시 렌더 |

dui는 Whiteboard를 **내장하지 않는다**: Code-OSS를 통째로 포함(vendor)하고 있어 묶기 무겁고, 사용자가 끄지 않으면 익명 사용 통계를 보낸다. 나중에 가능한 연동은 dui 결과를 Whiteboard 플러그인으로 보여주는 것이다. dui 이름에는 Whiteboard 상표를 쓰지 않는다.

## 12. 라이선스 정책

- dui는 **Apache-2.0 오픈소스**다. 사용자와 기업이 dui를 상업적으로 쓸 수 있도록, 의존성은 상업 사용을 허용하는 라이선스(MIT·Apache-2.0·BSD·ISC)만 쓴다.
- 금지: 비상업(PolyForm Noncommercial 등), AGPL·GPL 같은 강한 카피레프트, tldraw처럼 프로덕션 사용에 별도 계약이 필요한 것(계약 전까지).
- 확인 방법: 새 의존성은 `gh api repos/<owner>/<repo> --jq .license.spdx_id`와 LICENSE 원문으로 확인하고, JS는 라이선스 검사 도구, Rust는 `cargo deny`로 CI에서 막는다.
- 배포물에 라이선스·저작권 고지를 남긴다(Apache-2.0은 NOTICE 포함).

| 의존성 | 라이선스 | 상태 |
|---|---|---|
| Graphify | Apache-2.0 | 허용(참고만, 의존성 아님) |
| CodeGraph | MIT | 허용(참고만, 의존성 아님) |
| React Flow | MIT | 허용 |
| rmcp (공식 Rust MCP SDK) | Apache-2.0 | 허용 |
| oxc | MIT | 허용 |
| tree-sitter | MIT | 허용 |
| chromiumoxide | MIT OR Apache-2.0 | 허용 |
| syn, `ra_ap_*` | MIT OR Apache-2.0 | 허용 |
| GPUI (crate) | Apache-2.0 | 허용 |
| GitNexus | PolyForm Noncommercial | 금지 |
| tldraw | 자체(프로덕션은 유료 계약) | 보류 |

## 13. MVP 범위와 단계

**MVP**: obora-web 화면 3개(검수, 내보내기, 설정) × 자동 발견 상태 × 3폭을 React Flow 캔버스에 펼치고, 화면별 커버리지 숫자를 보여준다. 네이티브는 MVP 다음.

| 단계 | 내용 | 완료 기준 |
|---|---|---|
| **M0 스파이크** | obora-web 검수 화면 1개(obora는 먼저 TS 7로 옮긴다. TS 7 미만은 `미지원`). 손으로 고른 상태 5개를 oxc 변환 주입 + CDP Fetch 가로채기 + CDP로 구동한 Chromium으로 3폭 렌더. 모두 Rust 바이너리가 구동 | 로컬 상태 1개 이상을 밖에서 주입해 렌더 성공, 이미지 15장 |
| **M1 자동 상태** | 분석기 + 목 생성기 + 계획기. 데이터 모양·코드 분기 자동 나열, 커버리지 측정, 보충 반복 | 검수 화면 분기 커버리지 숫자 출력, 미검증 목록 |
| **M2 GPUI 스파이크** | pippip 화면 1개. 오프스크린 렌더 가능 여부 확인, `#[dui::state]` 매크로 시제품 | 상태 파일 → 이미지 1장 |
| **M3 순서 상태** | 이벤트→상태 변경 규칙 추출, 펼치기, CDP 재생 검증, 선 `검증됨` 표시 | 검수 화면 "마지막 장 승인 후" 같은 순서 상태 자동 생성·검증 |
| **M4 캔버스** | React Flow 캔버스(행=상태, 열=폭, 선=전환, 미검증·렌더 불가 표시). AI 에이전트가 CLI / MCP로 연다(`dui open`), 사람은 보기만 | obora 화면 3개 전체 배치 |
| **M5 obora 99% 시도** | 3개 화면 99% 목표, 상태 명시 규칙 적용, Figma 내보내기 | 화면별 커버리지 보고 |
| M6 이후 | pippip 전체, Electron(obora-desktop), Vue·Svelte | — |

## 14. 위험과 열린 질문

**위험**
- **주입 지점 오판**: AI가 슬롯을 틀리게 찾거나 변환이 동작을 바꿀 수 있다. → 값 확인 + 컴파일 검사 + 의미 단언으로 걸러 `미검증` 처리.
- **상태 폭발**: 순서 상태 조합이 커진다. → 대표값·깊이 한도·같은 렌더 합치기. 한도는 측정 후 조정.
- **분모 신뢰도**: 분석기가 놓친 분기는 분모에도 없다. → 런타임 커버리지 전체 분기와 대조해 "분모 밖" 수치를 따로 보고.
- **비결정 렌더**: 시간·난수·애니메이션·폰트 로딩. → 시계·난수 고정, 애니메이션 끄기, 폰트 로딩 대기.
- **GPUI 렌더 경로 불확실**: 오프스크린 렌더가 안 되면 OS 캡처로 가야 하고 macOS 권한·속도 문제가 생긴다.
- **AI 비용·시간**: 큰 코드베이스에서 스캔 비용. → 그래프·결과를 커밋 단위 캐시, 바뀐 파일만 다시.
- **공식 컴파일러 API 의존**: TS 7의 프로그래밍 API는 새것(7.1 beta)이고 `ra_ap_*`는 호환성 보장이 없다. → `TypeOracle` 뒤에 두고 버전 고정.

**열린 질문** (중요한 순)
1. GPUI(`gpui-pre 0.3.12`)의 헤드리스 캡처 API(코드에 존재 확인)가 pippip 화면에서 실제로 동작하나? 안 되면 네이티브 렌더 경로를 무엇으로 하나.
2. 커버리지 분모에서 "화면에 영향 없는 분기"를 어떻게 자동으로 빼나. 이 규칙이 99%의 의미를 결정한다.
3. 순서 상태의 펼침 한도(깊이, 대표값)를 얼마로 잡아야 실용적인 렌더 수 안에서 의미 있는 상태를 다 덮나.
4. TS 어댑터가 Rust에서 TS 7.1 API를 어떻게 부를지(`typescript/sync`를 쓰는 작은 Node 사이드카, 또는 문서화된 통신 프로토콜. 후자는 `미확인`).
5. 캔버스를 로컬 웹 앱으로 낼지(AI 에이전트가 `dui open` 실행, 사람은 보기만) 호스팅 서비스로 낼지. 호스팅이면 스냅샷 저장 비용·보안 검토 필요.
6. Figma 내보내기의 요금·API 한도 (`미확인`).

## 원칙: 조합은 AI가 아니라 시스템이 만든다 (2026-11-19 결정)
AI는 상태가 **어디 있고 무슨 타입인지 찾는 데만** 쓴다. 어떤 조합을 그릴지는 결정적 알고리즘으로 정해서 매번 같은 결과가 나오게 한다.
1. 대표값은 타입에서 기계적으로: 목록 0/1/많음, bool 둘 다, enum 전부, 문자열 빈 값/보통/아주 긺, optional은 null 포함, 숫자는 0/경계값.
2. 독립 분해: 코드 그래프에서 같은 렌더 영역에 함께 쓰이는 상태끼리만 묶고, 나머지는 따로 펼친다(곱 → 합).
3. 묶음 안은 pairwise(모든 두 값 쌍을 최소 1회) 결정적 생성.
4. 순서 상태는 깊이 제한 BFS, 방문한 상태 재방문 안 함.
5. 불가능 조합은 타입·코드 조건(판별 유니온, 가드)으로 제거.

**보장 범위**: 타입·enum·판별 유니온·상태 머신으로 명확히 표현된 상태만 보장한다. 흩어진 암묵 상태는 `미검증`으로 정직하게 표시한다. 상태를 명확히 쓸수록 커버리지가 올라가는 것이 dui가 주는 "좋은 코드로 가는 지름길"이다.

## 렌더링 한계 (2026-11-19 결정)

어디서나 렌더할 수 없는 UI가 있다. 대표적인 예가 애플 생태계다:

| 한계 | 내용 |
|---|---|
| 툴체인 종속 | SwiftUI / UIKit / AppKit은 Xcode 툴체인이 있는 macOS에서만 렌더된다. Mac 러너가 필요하고, 리눅스 CI에서는 렌더할 수 없다 |
| iOS 렌더 | 시뮬레이터, `ImageRenderer`, 스냅샷 테스트 중 하나가 필요하다. 모두 느리고 Mac에서만 돈다 |
| 시스템 UI | 앱 밖의 UI(권한 팝업, 알림, 키보드, 일부 시트)는 주입도 렌더도 안 된다 |
| OS 의존 렌더 | 폰트, 다크 모드, 다이내믹 타입, OS 버전에 따라 결과가 바뀌어 OS 버전마다 렌더가 다를 수 있다 |

원칙:

1. 렌더할 수 없는 칸은 절대 숨기지 않는다. 캔버스에 `미검증`과 구분되는 별도 상태 **렌더 불가(이유)** 로 보여준다.
2. 플랫폼마다 필요한 렌더 환경을 문서에 명시한다(예: 애플 플랫폼은 Xcode가 있는 Mac 러너).

## 기술 스택 (2026-11-19 결정)

**처음부터 본체는 Rust**: 바이너리 하나이고 자체적으로 Node나 Python 런타임이 필요 없다. CLI, MCP 서버, 분석기, 결정적 조합 규칙이 모두 Rust 안에 있다. 앞에 나온 Vite / Babel / SWC, MSW, Playwright, ts-morph, 엔진으로서의 Graphify는 이것으로 대체한다. 버전과 라이선스는 2026-11-19에 확인했다(crates.io, npm, GitHub).

| 층 | 선택 | 라이선스 | 비고 |
|---|---|---|---|
| 본체 | Rust, edition 2024 | — | 바이너리 하나. dui를 빌드하려면 Rust ≥ 1.98 필요: oxc 0.159.0과 `ra_ap_hir` 0.0.363이 `rust-version` 1.98을 선언(crates.io) |
| MCP 서버 | `rmcp` 3.11.0 (공식 SDK, [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk)) | Apache-2.0 (workspace `Cargo.toml`, GitHub 표기는 NOASSERTION) | CLI와 같은 작업 + 캔버스 코멘트 읽기 |
| 빠른 훑기 | `tree-sitter` 0.27.2 | MIT | 화면·파일 목록, 모든 언어 |
| TS/JS 파싱 + 주입 변환 | `oxc` 0.159.0 (`oxc_parser`, `oxc_semantic`, `oxc_transformer`) | MIT | Babel / SWC / Vite 플러그인 대체 |
| 코드 그래프 | tree-sitter + oxc로 자체 구현, `CodeGraphPort` 뒤 | — | Graphify(Python)와 CodeGraph는 참고만 |
| TS 타입 | `TypeOracle` → TypeScript 어댑터 → 프로젝트의 공식 컴파일러 | Apache-2.0 ([microsoft/TypeScript](https://github.com/microsoft/TypeScript)) | 아래 "TypeScript 컴파일러 현황" 참고. 타입을 못 구하면 결과는 `추정됨` |
| Rust 타입 | `ra_ap_*`(버전 고정, `ra_ap_hir` 0.0.363) + `syn` 3.0.10 | MIT OR Apache-2.0 | `ra_ap_*`는 호환성 보장이 없다 → 버전 고정 |
| 웹 렌더 | `chromiumoxide` 0.9.1 (2026-02-25 출시), CDP로 Chromium 구동, 폭마다 뷰포트 하나 | MIT OR Apache-2.0 | 마지막 출시 2026-02-25 → 유지보수 지켜보기 |
| 네트워크 가로채기 | CDP Fetch 도메인 (chromiumoxide의 `Fetch.pdl`, `examples/interception.rs`) | — | MSW 없음 |
| GPUI 렌더 | 나중에 같은 Rust 프로세스 안에서 (`gpui-pre`) | Apache-2.0 | §9.2 참고 |
| 캔버스 UI | React + React Flow, 정적 파일을 Rust 바이너리가 직접 띄움 | MIT | 사람은 이것만 본다(§0) |

**TypeScript 컴파일러 현황** (공식 자료):
- TS 7.0은 Go 네이티브 컴파일러다. npm 정식판 `typescript` 7.0.2가 2026-07-08에 나왔다. 7.0 RC부터 명령 이름은 `tsc`다(미리보기 이름 `tsgo`는 더 이상 안 씀) ([typescript-go README](https://github.com/microsoft/typescript-go), 지금은 보관 처리됐고 개발은 microsoft/TypeScript로 옮겨감). TS 7은 LSP 언어 서버를 제공한다([TS 7.0 발표](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/)).
- 프로그래밍 API는 **TS 7.1**(beta 2026-10-20)에서 JS 진입점 `typescript/sync`, `typescript/async`, `typescript/fs`, `typescript/ast`로 돌아온다([TS 7.1 beta 발표](https://devblogs.microsoft.com/typescript/announcing-typescript-7-1-beta/)).
- 그래서 TS 어댑터는 프로젝트 자체의 Node 위에 작은 사이드카를 띄워 프로젝트의 `typescript`를 불러 타입 질문에 답하게 한다. Node 없이 Rust에서 네이티브 컴파일러를 바로 부르려면 문서화된 프로토콜이 필요한데, 이건 `미확인`이다.

### 언어 어댑터: 본체는 언어를 모른다

- Rust 본체는 어떤 언어도 모른다. 언어마다 **언어 어댑터** 하나로 붙는다 = `TypeOracle`(공식 컴파일러에서 받은 타입 정보) + 주입기 + **렌더러 플러그인**.
- **언어 추가 = 어댑터 하나 추가.** 본체는 그대로다.
- **컴파일러 선택 규칙**:
  1. 사용자 프로젝트에 이미 있는 공식 컴파일러를 프로젝트와 **똑같은 버전**으로 쓴다.
  2. 없을 때만 프로젝트가 선언한 버전의 **공식 배포본**을 받아 캐시한다.
  3. 그것도 안 되면 그 프로젝트는 `미지원`(이유 표시).

| 언어 | 공식 컴파일러·툴체인 | 라이선스(출처) | 상태 |
|---|---|---|---|
| TypeScript | npm `typescript`(Microsoft)의 `tsc` | Apache-2.0 ([microsoft/TypeScript](https://github.com/microsoft/TypeScript)) | v0 |
| Rust | `rustc` / Cargo, 타입 조회는 rust-analyzer | MIT와 Apache-2.0 ([rust-lang/rust README](https://github.com/rust-lang/rust)), rust-analyzer MIT OR Apache-2.0 | v0 |
| Swift | `swiftc` + SourceKit-LSP | Apache-2.0 + Runtime Library Exception ([swiftlang/swift LICENSE.txt](https://github.com/swiftlang/swift/blob/main/LICENSE.txt)), SourceKit-LSP Apache-2.0 | 나중. Apple 플랫폼 렌더에는 Xcode(Apple 라이선스)가 필요 → dui는 설치된 Xcode를 쓰고, 절대 받거나 재배포하지 않는다 |
| Kotlin | `kotlinc` (JetBrains) | Apache-2.0 ([JetBrains/kotlin license/README.md](https://github.com/JetBrains/kotlin/blob/master/license/README.md), GitHub에는 SPDX 표기 없음) | 나중 |

### 지원 버전: 현재 최신 안정 메이저만

**규칙 (2026-11-20 결정)**: 스택마다 dui는 **현재 최신 안정 메이저 버전**(2026년 11월 기준)과 그보다 새 버전만 지원한다. 새 버전은 항상 지원한다. 그보다 오래된 메이저는 `dui scan`이 처음에 **`미지원`(이유)**으로 거절한다: 이건 `렌더 불가`가 아니고, 칸 상태도 아니며, 커버리지에서 빠진다(§8). 비공식 컴파일러도 똑같이 `미지원`이다. 기준선은 **나중의 dui 메이저 릴리스**에서만 올린다.

2026-11-20에 공식 자료로 확인(npm registry, crates.io, GitHub releases, nodejs.org):

| 스택 | 지원 기준선 | 최신 안정판(날짜, 출처) | 미지원 |
|---|---|---|---|
| TypeScript | **7.x** (7.0+) | 7.0.2 (2026-07-08, npm `latest`, 7.1.0은 아직 beta) | 6.x와 그 이전 |
| React | **19.x** (19.0+) | 19.3.1 (2026-10-14, npm `latest`) | 18.x와 그 이전 |
| Node.js (TS 사이드카 실행, 프로젝트의 것 사용) | **26.x** | 26.13.1 (2026-11-10, LTS "Lithium", nodejs.org, 26.0.0: 2026-05-05, 27은 alpha뿐) | 25.x와 그 이전 |
| Rust | **edition 2024** | [Edition Guide](https://doc.rust-lang.org/edition-guide/) 기준 최신 edition, 최신 툴체인 1.100.0 (2026-11-12, GitHub release) | edition 2021과 그 이전 |
| GPUI (`gpui-pre`) | **0.3.x** | 0.3.13 (2026-11-16, crates.io) | 0.2.x와 그 이전 |

참고:
- **Rust**는 1.0 이후 메이저 릴리스가 없어서 edition을 "메이저"로 본다. 최소 툴체인 버전까지 요구할지는 아직 정하지 않았다(`미확인`).
- **`gpui-pre`**는 0.x crate다. Cargo semver 규칙에서 0.x의 minor 올림은 호환이 깨지므로 0.3을 메이저 줄로 본다.
- **TypeScript**: dui가 쓰는 타입 API(`typescript/sync`)는 7.1에 들어 있고 7.1은 아직 beta다. 7.1이 정식이 되기 전까지 TS 어댑터의 API 경로는 `미확인`이다.
- **Chromium**: v0에서는 버전을 고정하지 않는다. dui가 CDP로 어떤 브라우저 바이너리를 구동할지는 `미확인`이다.
- obora(M0 대상)는 TS 7로 옮길 예정이다.
