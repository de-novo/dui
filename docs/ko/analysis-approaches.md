# 코드 분석 AI 도구들은 코드베이스를 어떻게 이해하나 (dui 분석기 설계 전 조사)

[English](../en/analysis-approaches.md) | [简体中文](../zh-CN/analysis-approaches.md) | 한국어

> 이 문서는 [영어 원문](../en/analysis-approaches.md)의 한국어 번역이다. 내용이 다르면 영어가 기준이다.

> 작성 2026-11-19. 각 도구의 README·소스·공식 문서를 직접 읽고 정리했다. 확인 못 한 것은 `미확인`.
> ★·라이선스는 2026-11-19 GitHub API 값이다(`spdx_id`가 비어 있으면 LICENSE 원문이나 패키지 메타데이터로 확인).
> 전제(준혁 결론): 상태 위치·타입·분기·`이벤트 → setState` 규칙은 대부분 **AST + 타입 정보**로 뽑을 수 있다. AI는 빈칸만 채우고, 조합은 결정적 알고리즘으로 만든다.

## 0. 용어

| 용어 | 뜻 |
|---|---|
| AST | 코드를 문법 구조 트리로 읽은 것. 타입·이름 연결은 없음 |
| tree-sitter | 언어 수십 개를 같은 방식으로 빠르게 AST로 바꾸는 파서. 깨진 코드도 부분 트리를 냄. **타입 정보 없음** |
| 컴파일러 API | TS compiler API, rust-analyzer처럼 언어 공식 도구가 주는 **타입·이름 해석** |
| LSP | 에디터용 언어 서버 프로토콜(정의로 가기, 참조 찾기). 질의 단위가 작음 |
| SCIP | Sourcegraph가 만든 언어 중립 인덱스 형식. 정의·참조·구현을 **타입 해석된 상태**로 파일 하나에 저장 |
| 데이터 흐름 | 값이 어디서 와서 어디로 가는지 추적(예: state → JSX) |
| 임베딩/RAG | 코드 조각을 벡터로 바꿔 비슷한 것을 찾는 방식. 결과가 확률적 |

## 1. 코드 그래프·AI 코딩 도구

| 도구 | ★ / 라이선스 | 파서 | 그래프 | LLM 쓰는 곳 | 임베딩 | 증분 갱신 | 언어 |
|---|---|---|---|---|---|---|---|
| [Graphify](https://github.com/Graphify-Labs/graphify) | 150,438 / Apache-2.0 | tree-sitter ("deterministic, no LLM") | calls·imports·inherits, 선마다 EXTRACTED/INFERRED, Leiden 커뮤니티 | 코드에는 안 씀. 문서·PDF·미디어의 의미 추출과 커뮤니티 이름 붙이기에만(선택) | **없음**("Not a vector index") | `graphify watch` | 약 40개 |
| [CodeGraph](https://github.com/colbymchenry/codegraph) | 78,088 / MIT | tree-sitter(Rust 커널, `codegraph-kernel/Cargo.toml`) | 심볼·호출·참조 + **프레임워크 라우트·`navigates` 선**(React Router, TanStack Router, Next, Vue Router 등), Tauri `invoke` 추적 | README상 그래프 구축에 LLM 없음 | 없음, SQLite FTS5 전문 검색 | 파일 변경 감시로 자동 동기화 | 20개+ |
| [GitNexus](https://github.com/abhigyanpatwari/GitNexus) | 48,658 / **PolyForm Noncommercial** | tree-sitter 쿼리(`ARCHITECTURE.md`) | 19단계 파이프라인: 심볼·MRO·DI·Leiden 커뮤니티·실행 흐름(process), LadybugDB 저장 | "CLI는 LLM 없이 인덱싱", 위키 생성 등은 선택 | 선택(원격 임베딩) | `--watch` | 다수 |
| [Axon](https://github.com/harshkedia177/axon) | 820 / MIT(pyproject) | tree-sitter(Python·TS·JS) | 호출·타입 관계·커뮤니티·실행 흐름, KuzuDB | 없음("Zero cloud") | 로컬 384차원(bge-small) + BM25 혼합 | `--watch` | 3개 |
| [Descry](https://github.com/06chaynes/descry) | 1 / MIT | 정규식(+ast-grep) + **SCIP로 타입 해석 덧씌움** | 호출·import·정의, 해석 결과를 resolved/unresolved/external로 구분 | 없음 | 선택(sentence-transformers) | 캐시 | SCIP 지원 언어 전부 |
| [SCIP](https://github.com/scip-code/scip) (+scip-typescript 등) | 892 / Apache-2.0 | 언어별 컴파일러: scip-typescript는 TS compiler API(`ts.createProgram`, `getTypeChecker`), Rust는 `rust-analyzer scip` | 정의·참조·구현(정확) | 없음 | 없음 | 인덱서마다 다름 | TS/JS, Rust, Java/Kotlin, C#, C/C++, Dart, Go 등 |
| Sourcegraph Cody | 미확인(저장소 공개 상태 미확인) | 키워드 검색 + SCIP 코드 그래프 | SCIP 정의·참조 | 답변 생성, 후보 재순위 | 있음(임베딩 검색) | 미확인 | 다수 |
| [Aider repo map](https://aider.chat/docs/repomap.html) | 50,182 / Apache-2.0 | tree-sitter + 언어별 `*-tags.scm` 쿼리(def/ref 태그), 참조 없는 언어는 Pygments 보충 | **파일 그래프**(참조 파일 → 정의 파일) + **개인화 PageRank**로 순위 | 그래프는 LLM에게 줄 문맥 요약용 | 없음 | 파일 mtime 캐시 | tree-sitter 지원 언어 |
| [Continue](https://github.com/continuedev/continue) | 36,519 / Apache-2.0 | tree-sitter로 함수·클래스 단위 청크(`core/indexing/chunk/code.ts`) | 없음(검색 위주) | 답변 | **있음**, LanceDB 벡터 저장 | 캐시 키 기반 | 다수 |
| Cursor | 비공개 | 공개 문서 기준: Instant Grep(로컬 정규식 인덱스) + 탐색용 하위 에이전트 | 미확인 | 검색 결과 요약·탐색 | 현재 문서는 "검색용 코드 임베딩을 저장하지 않는다"고 명시([문서](https://cursor.com/docs/context/semantic-search)) | 로컬 인덱스 | 언어 무관 |
| Windsurf(Codeium) | 비공개 | "구조를 이용해 코드를 블록으로 파싱"(파서 종류 미확인) | 미확인 | 재순위·답변 | **있음**(로컬 인덱싱: 조각을 서버로 보내 임베딩, 저장은 로컬) | 원격 인덱스는 주기 재색인 | 언어 무관 |
| GitHub 코드 탐색 | [stack-graphs](https://github.com/github/stack-graphs) 877 / Apache·MIT, **2025-09-09 보관(archived)** | tree-sitter + 이름 결속 규칙 DSL(stack graphs) | 정의·참조(파일별로 따로 만들고 질의 때 합침 → 증분) | 없음 | 없음 | 파일 단위 증분 | Python·TS 등 |
| [Serena](https://github.com/oraios/serena) | 31,530 / **SolidLSP는 MIT, Serena 앱은 GPL-3.0+** | **LSP**(언어 서버 40개+) 또는 JetBrains 플러그인 | 그래프 저장 없음, 그때그때 LSP 질의(정의·참조·심볼) | 에이전트가 도구로 사용 | 없음 | LSP가 처리 | 40개+ |
| [Joern](https://github.com/joernio/joern) | 3,655 / Apache-2.0 | 언어별 프런트엔드(퍼지 파서 포함) | **Code Property Graph**: AST + CFG + PDG(데이터 흐름)를 한 그래프로 | 없음 | 없음 | 미확인 | C/C++, Java, JS, Python, Go, Kotlin, PHP, Ruby, Swift |
| [CodeQL](https://github.com/github/codeql) | 10,385 / 쿼리는 MIT, **CLI는 별도 약관**: 비공개 코드 분석은 유료 GitHub Code Security 필요 | 컴파일러 수준 추출기(빌드 관찰) | 관계형 DB + 데이터 흐름·taint 라이브러리 | 없음 | 없음 | 아니오(재추출) | 10여 개 |
| [Semgrep](https://github.com/semgrep/semgrep) | 17,822 / LGPL-2.1(엔진) | 언어별 파서 → 공통 AST | 패턴 매칭 + 함수 내 데이터 흐름(파일 간 분석은 유료판, 미확인) | 없음(오픈 엔진) | 없음 | 미확인 | 30개+(미확인) |
| [Kythe](https://github.com/kythe/kythe) | 2,171 / Apache-2.0 | 언어별 인덱서(컴파일러 연동) | 정의·참조·타입 그래프 | 없음 | 없음 | 미확인 | C++, Java, Go, TS 등 |

## 2. UI·상태 전용 정적 분석

| 도구 | 접근 | dui에 주는 것 |
|---|---|---|
| [React Compiler](https://github.com/react/react/blob/main/compiler/docs/DESIGN_GOALS.md) (MIT) | Babel 플러그인. 컴포넌트·훅 AST → **HIR**(기본 블록 제어 흐름 그래프) → **SSA** → 타입 추론(보수적) → 변경·별칭 효과 추론 → reactive scope → 코드 재생성. LLM 없음 | 훅·state·렌더 사이 **데이터 흐름을 AST만으로 정확히** 푸는 실제 사례. 주입 변환 위치도 같은 Babel 단계 |
| eslint-plugin-react-hooks | `rules-of-hooks`는 ESLint AST + 코드 경로 분석(TS 타입 미사용). 6.x의 `recommended-latest`에 React Compiler 기반 진단 포함 | 훅 호출 위치·조건부 호출 판별 로직 참고 |
| [react-docgen-typescript](https://github.com/styleguidist/react-docgen-typescript) (MIT, ★1,266) | `ts.createProgram` + `getTypeChecker()`로 props 타입 추출 | **props 타입 → 대표값 생성**의 기본 재료 |
| [ts-morph](https://github.com/dsherret/ts-morph) (MIT, ★6,231) | TS compiler API 래퍼(AST 탐색·수정, 타입 체커) | 분석 + 소스 변환을 한 도구로. 단 TS 7 대응은 진행 중(아래 위험 참고) |
| XState / Stately (MIT, ★30,520) | 사람이 상태 기계를 작성. [statelyai/agent](https://github.com/statelyai/agent)는 **LLM으로 기계를 생성**. 기존 코드에서 상태 기계를 정적으로 추론하는 공식 도구는 **찾지 못함** | 출력 형식(상태·이벤트·전환) 참고. 추론은 dui가 만들어야 함 |
| [Storybook 11 `write-stories`](https://storybook.js.org/docs/11/ai/skills/write-stories) | **에이전트(LLM)**가 컴포넌트마다 상태별 스토리(기본·로딩·빈·오류·비활성) + MSW 목을 작성 | 상태 나열을 LLM에 맡긴 예. 결과가 비결정적이고 커버리지 숫자 없음 → dui가 차별화할 지점 |
| rust-analyzer (MIT/Apache, ★17,008) | `ra_ap_*` 크레이트로 라이브러리 사용 가능(`ra_ap_hir` 등). **하위 호환 보장 없음** | Rust 타입·이름 해석. `rust-analyzer scip`으로 SCIP도 출력 |
| syn (Apache/MIT, ★3,448) | Rust 문법 파서. 타입 해석 없음 | proc-macro(주입) 작성용. 분석 주력으로는 부족 |
| Swift: SwiftSyntax, SourceKit-LSP | 문법 트리 + LSP(세부 미확인) | 나중 Swift 지원 시 |
| Kotlin: Kotlin Analysis API(K2), PSI | 컴파일러 기반 분석(세부 미확인) | 나중 Kotlin 지원 시 |

## 3. 학계: UI 전환 그래프를 코드에서 뽑기

| 연구 | 방법 | dui에 주는 것 |
|---|---|---|
| GATOR / [Static Window Transition Graphs for Android (ASE 2015)](https://dacongy.github.io/papers/yang-ase15.pdf) | 정적 분석으로 **창 = 노드, 이벤트로 일어나는 전환 = 선**인 그래프(WTG). 선에 창 스택 push/pop과 콜백 순서 표시 | dui의 `상태 + 이벤트 → 다음 상태`와 같은 모양. 정적 분석만으로 가능하다는 근거 |
| [On the unsoundness of static analysis for Android GUIs](https://doi.org/10.1145/2931021.2931026) | 정적 GUI 분석이 놓치는 경우 분석 | 정적 분석만으로는 100%가 안 된다 → **재생 검증 필요**의 근거 |
| [FlowDroid](https://github.com/secure-software-engineering/FlowDroid) (LGPL-2.1, ★1,275) | Android 생명주기 모델링 + taint 데이터 흐름 | 프레임워크 콜백(이벤트)을 모델링해야 흐름이 이어진다는 교훈 |
| 모델 기반 GUI 테스트(실행하며 상태 모델을 쌓는 방식, 예: DroidBot·Stoat 계열) | 앱을 실제로 돌리며 화면 상태 그래프 수집 | 정적 분석이 놓친 상태를 **실행으로 보충**. 도구별 세부·라이선스 미확인 |

## 4. 공통 패턴: 어느 층에 어떤 기술을 쓰나

| 층 | 거의 모든 도구가 쓰는 것 | LLM 사용 여부 |
|---|---|---|
| 1. 파싱 | tree-sitter(언어 많이, 빠르게, 타입 없음) | 안 씀 |
| 2. 이름·타입 해석 | 정확도가 필요하면 **언어 컴파일러**(TS compiler API, rust-analyzer) 또는 그 결과물인 **SCIP/LSP** | 안 씀 |
| 3. 그래프 | 호출·참조 그래프. 보안 도구는 데이터 흐름(CPG, CodeQL) | 안 씀 |
| 4. 순위·묶음 | PageRank(Aider), Leiden 커뮤니티(Graphify·GitNexus) | 안 씀(이름 붙이기만 선택) |
| 5. 검색 | grep/FTS(Cursor·CodeGraph), 임베딩(Continue·Windsurf·Cody) | 임베딩 모델 |
| 6. 해석·생성 | 요약, 답변, 재순위, 스토리 작성(Storybook) | **여기서만 LLM** |

관찰
- 그래프를 만드는 도구들(Graphify, CodeGraph, Aider, stack-graphs, Joern)은 **구조 추출에 LLM을 쓰지 않는다.** 결정적이고 재현 가능해야 해서다.
- 정밀도가 필요한 곳(SCIP, CodeQL, React Compiler)은 tree-sitter가 아니라 **컴파일러 수준 타입 해석**을 쓴다.
- 임베딩은 "비슷한 코드 찾기"용이지 상태·분기 추출용이 아니다. 최근 Cursor 공개 문서는 오히려 grep 중심이다.
- 상태 전환 추출(UI)은 범용 도구엔 없다. 학계(GATOR WTG)와 React Compiler 내부 분석이 가장 가깝다.
→ 준혁 결론(AST + 타입이 기본, AI는 보조)과 업계 방식이 일치한다.

## 5. dui 분석기 추천

### 5.1 TypeScript

| 후보 | 장점 | 단점 | 판단 |
|---|---|---|---|
| **TS compiler API (ts-morph 래핑)** | 타입 체커로 정확한 타입·심볼 해석. 판별 유니온·제네릭 해석 가능. 같은 도구로 변환도 가능 | 느림(프로그램 전체 로드). TS 7(Go 네이티브)에서 API가 바뀜 | **주력** |
| tree-sitter | 매우 빠름, 깨진 코드 허용, 증분 | 타입 없음 → 대표값·불가능 조합 판단 불가 | 빠른 1차 스캔(화면·파일 목록)만 |
| SCIP(scip-typescript) | 정의·참조가 정확, 언어 중립 형식 | 타입 **구조**(필드·유니온 갈래)와 데이터 흐름은 없음 | 쓰지 않음(필요 시 교차 참조 보조) |
| Babel/SWC AST | 주입 변환을 넣는 곳 | 타입 없음 | **주입기 전용**(Vite 플러그인) |

TS 버전 위험: InfoQ(2026-11-06)에 따르면 TS 7.0(2026-07, Go 네이티브)은 컴파일러 API를 뺐고, 7.1 베타가 `typescript/sync`·`typescript/async`·`typescript/ast` 등 **새 진입점으로 API를 되살렸다**(옛 `ts.createProgram`과 호환 아님, 원문 공식 블로그 미확인). obora는 도구용 `typescript ^5.9.3`을 유지하고 TS7은 tsgo로 따로 돌린다(pnpm-workspace.yaml 확인). → **dui는 분석 계층을 `TypeOracle` 인터페이스 뒤에 두고, v0은 TS 5.x/6 API(ts-morph), TS 7.1 안정판이 나오면 새 API 구현을 추가**한다.

데이터 흐름(state → JSX, 핸들러 → setState)은 React Compiler처럼 함수 단위 제어 흐름 + 단순 데이터 흐름을 **직접 구현**한다(React 훅 규칙 덕분에 범위가 좁다). React Compiler 내부를 라이브러리로 재사용할 수 있는지는 `미확인`(공개 API 아님) → M1에서 확인.

### 5.2 Rust

| 후보 | 장점 | 단점 | 판단 |
|---|---|---|---|
| **rust-analyzer 라이브러리(`ra_ap_hir` 등)** | 타입·trait·매크로 전개까지 해석 | 하위 호환 보장 없음 → 버전 고정 필요, 빌드 무거움 | **주력**(버전 고정) |
| `rust-analyzer scip` | 정의·참조를 안정된 형식으로 | 타입 구조·데이터 흐름 없음 | 교차 참조 보조 |
| syn | 가볍고 안정, proc-macro 표준 | 타입 해석 없음, 매크로 전개 못 봄 | **주입기(proc-macro)·빠른 스캔용** |
| tree-sitter-rust | 빠름 | 타입 없음 | Graphify가 이미 사용 → 그래프는 Graphify 결과 재사용 |

GPUI는 상태가 `Entity<T>` 모델 필드에 모이므로 "모델 타입 찾기 → 필드 타입 → 대표값"이 rust-analyzer 타입 정보로 결정적으로 된다. `cx.notify()`·`update` 호출 지점 찾기는 AST 패턴으로 충분.

### 5.3 재사용 vs 직접 만들기

| 항목 | 재사용 | 직접 |
|---|---|---|
| 파싱 | tree-sitter(MIT), Babel/SWC(MIT/Apache), syn | — |
| 타입 해석 | TS compiler API·ts-morph(MIT/Apache), rust-analyzer(MIT/Apache) | `TypeOracle` 인터페이스 |
| 호출·import 그래프 | Graphify(Apache-2.0) 1순위, 라우트·`navigates` 선은 CodeGraph(MIT) 방식 참고 또는 사용 | `CodeGraphPort` |
| props 타입 추출 | react-docgen-typescript(MIT) 참고 | — |
| **상태 위치 판별, 상태 → 렌더 데이터 흐름, 이벤트 → 상태 변경 규칙, 대표값·조합 생성, 커버리지 분모** | 없음(GATOR WTG·React Compiler 설계만 참고) | **dui 핵심 = 직접** |
| AI | — | 위에서 `추정됨`으로 남은 칸만, 이름 붙이기 |

### 5.4 라이선스 적합성 (dui는 상업용: MIT·Apache·BSD만)

| 대상 | 판단 |
|---|---|
| Graphify, CodeGraph, Axon, Descry, SCIP, scip-typescript, Aider, Continue, stack-graphs, Joern, Kythe, tree-sitter, ts-morph, react-docgen-typescript, React, rust-analyzer, syn, XState | 허용 |
| GitNexus (PolyForm Noncommercial) | 금지 |
| Serena 앱 (GPL-3.0+) | 금지. SolidLSP 부분(MIT)만 허용 |
| Semgrep 엔진, FlowDroid (LGPL-2.1) | 쓰지 않음(정책상 강한 카피레프트 아님이지만 정적 링크·배포 조건 검토 비용 → 피함) |
| CodeQL CLI | 금지(비공개 코드 분석은 유료 약관) |
| CodeGraph | 허용, 단 기본으로 익명 사용 통계를 보냄(`TELEMETRY.md`) → 내장 시 `CODEGRAPH_TELEMETRY=0` 기본 |

## 6. 결론 (추천 스택)

- **TS**: tree-sitter(빠른 스캔) → **ts-morph/TS compiler API(타입·심볼)** → dui 자체 데이터 흐름(React Compiler 방식) → Babel/SWC 변환(주입). 그래프는 Graphify.
- **Rust**: Graphify(tree-sitter) 그래프 → **rust-analyzer `ra_ap_*`(버전 고정)로 타입** → syn proc-macro(주입).
- AI는 `추정됨` 칸 채우기·이름 붙이기에만. 조합·커버리지는 결정적 알고리즘.
- 열린 확인: React Compiler 내부 재사용 가능성, TS 7.1 새 API로 타입 체커 기능이 충분한지, `ra_ap_hir` 버전 고정 비용.
