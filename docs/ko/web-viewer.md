# 웹 뷰어 아키텍처

[English](../en/web-viewer.md) | [简体中文](../zh-CN/web-viewer.md) | 한국어

## 1. 범위와 진입점

이 문서는 v0.0.2의 구현 기준이다. 현재는 `crates/dui`가 독립 HTML 갤러리를 쓰고 `dui open`이 기록 파일을 연다. 새 뷰어는 `apps/canvas`에 둔다. TypeScript 7, React 19, TanStack Start SPA 모드, shadcn/ui, Tailwind CSS, React Flow를 사용한다. 이 의존성은 뷰어에만 둔다. 캡처 대상 프로젝트는 자체 스택을 사용한다.

dui 빌드에서 클라이언트를 빌드하고 진입 페이지와 해시가 붙은 리소스를 Rust 바이너리에 내장한다. 소스 설치와 릴리스 빌드는 같은 리소스 빌드 경로를 사용한다. 저장된 기록은 dui와 브라우저만으로 열 수 있게 한다. 기존 독립 HTML 갤러리도 계속 열 수 있게 한다. 라이브 세션에는 대상 앱의 툴체인과 렌더러가 필요하다. [코어 아키텍처](architecture.md) · [마일스톤](roadmap.md#v002)

**유료 배포.** 지도는 MIT 라이선스인 `@xyflow/react`를 사용하고 기본 출처 표시를 유지한다. dui 유료판에도 Pro 구독 없이 사용할 수 있다. 개발사는 화면 표시를 숨기는 프로젝트에 구독을 요청한다. 뷰어를 내장한 바이너리를 배포할 때에는 화면 표시와 별도로 실제 사용한 패키지 버전의 저작권 고지와 MIT 라이선스 전문을 동봉한다. 잠금 파일을 갱신할 때 번들에 포함되는 의존성도 확인한다. 지도 기능은 공개 API로 구현하고 Pro 예제와 템플릿은 저장소에서 제외한다. [MIT](https://github.com/xyflow/xyflow/blob/main/LICENSE) · [ProOptions](https://reactflow.dev/api-reference/types/pro-options) · [React Flow Pro](https://reactflow.dev/pro/content)

### React Flow 검증

개발용 뷰어는 저장이 끝난 실행에서 진입 화면 캡처와 발견된 링크를 읽는다. 저장소 루트에서 Node 26으로 아래 명령을 실행하고 출력된 로컬 주소를 연다. 마지막 경로는 캡처 결과의 출력 디렉터리로 바꾼다.

```sh
npm ci --prefix apps/canvas
npm --prefix apps/canvas run preview:archive -- /path/to/run
```

뷰어는 [Vite](../../dependency-catalog.json)과 공식 Tailwind Vite 플러그인으로 빌드한다. `npm --prefix apps/canvas run build`는 브라우저 자산을 `apps/canvas/dist`에 저장한다. CSS 빌드 도구에는 [라이선스 예외](../en/architecture.md#62-automated-checks-ci-fails-on-any-violation)를 적용한다.

npmgraph 기록에서 화면 노드 2개, 출처와 선택자 근거가 붙은 발견 링크 1개(재생 미검증 표시), 375px·1280px 캡처 총 4장을 확인했다. Chrome에서 상태와 폭 펼치기, 선택한 화면에서 라이브 세션 열기, 두 개 조작 요소 픽스처의 `boolean`·`enum` 값 편집 및 검증된 버전 저장, 선택한 화면을 유지한 채 전체 지도로 복귀, 키보드 포커스 복귀, 새로고침, 뒤로 가기와 기본 출처 표시를 검증했다. 설치된 CLI는 기존 독립 갤러리를 연다.

새 지도에서는 캡처한 화면의 DOM에서 다른 캡처 주소를 가리키는 같은 출처 링크를 찾으면 방향이 있는 선을 그린다. 같은 링크가 반복돼도 한 선으로 표시하며, 해당 링크를 관측한 폭에서만 보여준다. 선은 이동할 수 있는 링크가 있었다는 근거다. 실제 이동 성공은 재생으로 확인한다.

프리뷰는 실행 폴더를 하나 이상 받는다. 인자가 없으면 `~/.dui/projects`에서 저장된 화면 지도를 찾는다. **Select workspace**에서 프로젝트·기록된 브랜치·버전을 선택한다. 레일에는 작업 선택, 테마, 접기만 이 순서로 둔다. 화면 찾기·연결 방식·캡처 폭은 **Map options**에 둔다. 카드를 선택하면 해당 화면의 상태·폭·검증 결과·누락 이미지 사유·관측된 링크 근거와 **Open live session** 동작을 보여준다. 전체 지도로 돌아와도 선택한 화면이 유지된다. 카드에는 기록된 상태와 폭 전체의 미검증·렌더 불가 칸 수를 표시한다.

같은 프로젝트 ID는 모든 브랜치·워크트리·버전에서 같은 이름을 쓴다. 첫 캡처를 저장할 때 기본 프로젝트의 `package.json` 이름을 기록하고, 이름이 없으면 루트 폴더명을 쓴다. 기존 로컬 기록은 `dui history`가 이름을 등록한다.

### 뷰어 디자인 시스템

웹 뷰어는 Tailwind CSS 4.3.5와 shadcn/ui의 Base UI 프리셋(`base-nova`)을 쓴다. 공통 버튼과 대화상자는 shadcn CLI 4.23.0으로 생성하고 `@base-ui/react` 1.9.0으로 동작한다. Tailwind와 화면 지도는 같은 테마 토큰을 쓴다.

[tokens.css](../../apps/canvas/src/styles/tokens.css) · [Button](../../apps/canvas/src/components/ui/button.tsx) · [Dialog](../../apps/canvas/src/components/ui/dialog.tsx) · [CaptureCard](../../apps/canvas/src/features/map/CaptureCard.tsx)

- **색과 재질** — 두 테마 모두 캔버스 배경은 흑백 단색으로 한다. 오렌지 `--primary`(#EF803F)는 dui 마크와 주요 동작에 쓴다. 레일과 상태·상세 패널은 지도 위에 띄운 반투명 글래스 표면(라이트 모드는 반투명 흰색, 다크 모드는 반투명 차콜)으로 두고, 캡처 카드와 함께 `--blur`(16 px)를 적용한다.
- **글꼴과 간격** — 조작 요소는 시스템 산세리프, 경로는 시스템 고정폭 글꼴로 한다. 기본 글자 크기에서 메타 정보·본문·패널 제목은 12 / 14 / 20 px, 간격은 4 / 8 / 12 / 16 / 24 px를 쓴다. 모서리 반경은 컨트롤 6 px, 패널 12 px로 한다.
- **조작 요소** — 패널 동작은 공통 `Button`의 기본·주요·아이콘 유형을 쓴다. 아이콘 전용 조작에는 접근 가능한 이름을 준다. 외곽선은 키보드 포커스에 쓰고, 선택은 위치와 굵기로 구분한다.
- **캡처와 판정** — 전체 지도와 선택 화면에 같은 `CaptureCard`를 쓴다. 원본 비율로 이미지를 맞추고 아래에 경로 이름을 둔다. 카드에는 검토할 칸 수, 상세 패널에는 기록된 판정과 이유를 표시한다.
- **탐색** — 레일 순서는 작업 선택·테마·접기로 고정한다. 화면 찾기는 지도에 둔다. 선택하면 해당 화면을 크게 표시하고 관측된 이웃 화면을 두 개씩 넘겨본다. 상세를 닫으면 전체 지도의 이전 위치로 돌아간다.

실제 뷰어에서 375 px와 1280 px, 두 테마의 전체 지도와 선택 화면을 확인한다. 연결된 화면을 모두 열 수 있는지, 화살표가 기록된 방향과 일치하는지, 캡처 원색과 키보드 포커스 복귀가 유지되는지 검증한다.

## 2. 모듈과 의존 방향

```text
apps/canvas/src/
  routes/                        # TanStack Start
  features/{history,map,inspector}/
  model/                         # selection, records, projections
  data/                          # HTTP, archive decoding, session client
  components/ui/                 # shadcn/ui primitives
  styles/                        # shared Tailwind tokens
packages/schemas/                # archive and session JSON Schemas
```

라우트는 기능을 조립하고 URL 매개변수를 검사한다. 기능은 공통 모델과 UI 기본 요소를 가져오며, 선택 항목이나 타입이 정해진 콜백으로 통신한다. 모델은 순수 함수와 불변 값으로 구성하며 React와 I/O에 의존하지 않는다. 기록을 React Flow의 노드와 선으로 바꾸는 곳은 지도뿐이다. 지도와 상세 보기는 처음 열 때 로드한다. 생성된 shadcn/ui 코드와 테마 토큰은 뷰어 안에 둔다.

Rust가 검증, 커버리지 계산, 기록 저장을 맡는다. CLI와 MCP는 같은 작업 함수를 호출한다. 로컬 HTTP 어댑터는 뷰어에 데이터를 전달한다. 코어 아키텍처에 정한 크레이트 분리 전까지 이 책임은 `crates/dui`에 둔다. 기존 포트 세 개를 유지한다. 기능 모듈은 첫 사용처를 구현할 때 만든다.

### 2.1 클린 아키텍처 경계

변경하려는 규칙의 소유자부터 정한다. 소스 코드의 의존 방향은 안쪽을 향한다. 실행 중 바깥 도구가 필요하면 안쪽에서 정한 포트를 통해 호출한다.

| 경계 | 맡는 일 | 허용하는 의존성 |
|---|---|---|
| 도메인 | 타입이 있는 관측값, 칸 판정, 세션 결정. 같은 입력은 같은 결과를 낸다. | `dui-core`의 순수 값과 함수. |
| 애플리케이션 | 조회·편집·저장 유스케이스. 명령 순서를 정하고 포트를 호출한 뒤 관측값을 도메인에 넘겨 타입이 있는 결과를 반환한다. | 도메인과 `TypeOracle`, `Host`, `Renderer` 계약. |
| 어댑터와 조립 | CLI·MCP·HTTP가 입력과 오류를 변환한다. 컴파일러와 Chromium 어댑터가 근거를 수집한다. 바이너리 진입점이 구체 구현을 연결한다. | 애플리케이션 또는 포트 계약, 도메인 값, 각 어댑터가 사용하는 도구. |
| 브라우저 표현 | URL 선택, 화면 모델, 조작 요소. 라우트 로더는 `data/`를 통해 읽고 컴포넌트는 모델 값과 콜백을 받는다. | `data/`는 전송 타입을 `model/` 값으로 바꾸고 `features/`는 그 값을 그린다. 모델은 양쪽에 의존하지 않는다. |

**편집 한 건의 흐름.** CLI와 MCP는 요청을 같은 편집 명령으로 바꾼다. 유스케이스가 도메인에 판단을 요청한다. 오래된 변경 번호는 충돌로 끝나며 렌더러 호출은 0회다. 허용된 편집은 실행할 효과를 낸다. 애플리케이션이 렌더러 포트로 실행하고 관측 결과를 도메인 값으로 바꿔 판정을 반환한다. 충돌을 각 프로토콜의 오류로 바꾸는 곳은 전송 어댑터다. 거절 경로는 메모리 렌더러로, 판정은 관측값만 넣어 테스트한다.

**기존 코드 이행 지점.** 현재 `ports.rs`는 JSON 값을 전달하고 `analysis.rs`는 코어 검증이 끝난 뒤 `missing-screen-assertion`을 추가한다. 라이브 검증 구현 시 이 판정을 공통 검증기로 옮기고 해당 JSON 필드는 어댑터 경계에서 변환한다. CLI와 MCP의 기존 결과는 호환되게 유지한다. React Flow 노드, CDP 응답, HTTP 상태 코드는 도메인 입력으로 쓰지 않는다. 순수 작업은 일반 함수로 두고 trait은 기존 포트 경계에서만 사용한다.

## 3. 데이터와 탐색

`packages/schemas`의 공개 JSON Schema를 통신 계약으로 삼는다. Rust 출력을 스키마와 대조하고 TypeScript 전송 타입은 스키마에서 생성한다. 외부 JSON은 한 곳에서 뷰어 모델로 변환한다. 추가 필드는 허용한다. 현재의 버전 없는 기록은 이전 형식 디코더로 읽고 새 기록에는 스키마 버전을 남긴다. 지원하지 않는 버전이나 잘못된 필수 필드는 해당 실행의 데이터 오류로 표시한다. 다른 실행 기록은 계속 열 수 있다.

저장한 실행 기록은 불변이다. 식별자와 Git 정보는 `snapshot.json`, 칸의 근거는 `report.json`, 연결 구조는 `graph.json`에서 읽는다. ID의 내부 형식을 해석하지 않고 선택한 실행 안에서 조회한다. 브랜치와 워크트리는 기록 필터로 사용한다. 라우트는 `/projects/$project/runs/$run`이고 검색 키는 `screen`, `state`, `width`, `view`다. 키를 생략하면 개요를 열 수 있다. 목록·지도·상세 보기는 같은 선택을 사용한다. 뒤로 가기·새로고침·키보드 복귀 후 선택을 복원한다. 사라진 칸은 누락으로 표시한다.

Rust 서버는 루프백에 바인딩한다. `/api/runs/$project/$run`에서 저장이 끝난 실행을 제공하고, 이미지는 해당 실행 기록에 있는 참조로 조회한다. 뷰어 탐색에는 정적 진입 페이지를 반환한다. 없는 API 기록과 리소스에는 각각 오류를 반환한다. 라이브 변경 권한은 해당 로컬 세션으로 제한한다. 탐색 URL에 제어 자격 증명이나 파일 경로를 넣지 않는다.

### 캡처된 상태 조회

실행 결과의 `report.json`에서 한 칸의 `state`와 `width`를 선택한다.

```sh
dui inspect RUN_DIRECTORY '{"state":"STATE_ID","width":768}'
```

MCP 도구 `inspect`에는 `directory`와 같은 `selection` 객체를 전달한다. 각 항목에는 소스 위치, 소유 함수, 인스턴스, 컴파일러 타입, 관찰된 값이 들어간다. 인스턴스 ID는 해당 칸에서만 유효하다. `slot`과 `instance`를 추가해 하나를 선택하고 `value`로 불리언이나 열거형 후보 값을 검증한다. 이 작업은 값을 변경하지 않는다. `editable`은 캡처 시점에 setter와 지원 타입이 있었는지를 나타낸다. 인스턴스 기록이 없는 이전 캡처는 `instance-evidence-unavailable`을 반환한다.

## 4. 라이브 편집과 실패 처리

로컬 dui 프로세스 하나가 라이브 세션의 호스트와 렌더러를 소유한다. CLI와 MCP는 세션 ID로 그 프로세스에 요청한다. 뷰어는 루프백을 통해 같은 세션의 변경 번호와 최근 관측 화면을 읽는다. 저장 이미지는 기록으로 표시한다. 라이브 상세 보기에는 실행 중인 대상에서 마지막으로 관측한 프레임과 편집 상태를 표시한다. 변경 번호가 바뀌면 새로 읽고 뒤늦은 이전 응답은 버린다.

코어의 Decider(명령 → 사건·효과)와 셸의 자원 정리 가드를 사용한다. 작업은 `open → inspect → set/reset → save → close`이며 조회·편집·저장은 반복할 수 있다. 조회 결과에 소유자, 인스턴스, 소스, 타입, 현재 값, 편집 가능 여부를 담는다. 편집 요청에는 세션·칸·인스턴스·예상 변경 번호·요청 ID를 담는다. 같은 세션의 편집은 순서대로 처리한다. 타입 오류, 모호한 인스턴스, 오래된 변경 번호는 변경 전에 거절한다. 같은 요청을 재시도하면 기록한 결과를 반환하고, 같은 ID로 다른 내용을 보내면 거절한다.

변경 후 값을 다시 읽고 DOM 또는 접근성 트리에서 실제 변화도 확인한다. 쓰기 후 검증이 실패하면 관측한 값과 이유를 반환하고 변경 번호를 올려 미검증으로 남긴다. 렌더러 연결이 끊기면 세션을 무효화하고 저장된 칸에서 다시 열 수 있게 한다. 저장은 새 실행 기록을 원자적으로 게시한다. 닫기는 소유한 렌더러와 호스트를 정리한다. 다른 칸으로 이동해도 처리 중인 편집의 대상은 바뀌지 않는다.

첫 클라이언트가 같은 바이너리의 백그라운드 인스턴스를 시작한다. `~/.dui`의 사용자 전용 잠금으로 시작을 조율하고 바인딩에 성공한 루프백 주소를 기록한다. 이후 클라이언트는 기존 인스턴스를 사용한다. 세션 ID가 만료되면 칸을 다시 연다. 뷰어는 같은 출처에서 JSON을 읽고 CLI와 MCP는 인증된 로컬 명령을 보낸다. 상세 보기를 연 동안 세션의 변경 번호를 폴링하고 닫으면 중단한다.

```text
open(project, run, cell, requestId) -> session, revision
inspect(session, cell) -> revision, controls[], observation
set(session, cell, instance, slot, value, expectedRevision, requestId) -> editResult
reset(session, cell, instance, slot, expectedRevision, requestId) -> editResult
save(session, cell, expectedRevision, requestId) -> project, run, cell
close(session) -> closed
editResult = {session, cell, instance, slot, revision, observedValue, status, reason?, evidence}
```

초기화하면 선택한 조작 요소를 세션 시작 시의 값으로 되돌린다. 저장 시 최근 관측한 변경 번호와 검증 판정을 함께 기록한다. 요청 ID를 예상 변경 번호보다 먼저 검사하므로 응답 유실 후 재시도해도 첫 결과를 받을 수 있다. 요청 결과는 세션을 닫을 때까지 유지한다. 소유 프로세스 재시작 시 기존 세션은 만료된다. 대상 앱의 리비전이 기록과 다르면 라이브 실행의 출처를 별도로 표시한다.

### CLI·MCP에서 라이브 세션 사용

`dui live '{…}'`에 JSON 객체를 전달한다. MCP에서는 도구 `live`의 인자로 같은 객체를 전달한다. 저장된 실행 디렉터리와 보고서의 칸을 선택한 뒤 아래 예시에 응답으로 받은 세션·인스턴스·슬롯 ID를 넣는다. `dui map`으로 만든 실행은 `manifest`를 생략하면 프로젝트를 다시 분석한다.

```jsonl
{"op":"open","project":"PROJECT","manifest":"MANIFEST","run":"RUN_DIRECTORY","cell":{"state":"STATE_ID","width":768},"requestId":"open-1"}
{"op":"inspect","session":"SESSION","cell":{"state":"STATE_ID","width":768}}
{"op":"set","session":"SESSION","cell":{"state":"STATE_ID","width":768},"instance":"INSTANCE","slot":"SLOT","value":true,"expectedRevision":0,"requestId":"edit-1","expect":{"selector":"#menu-toggle","text":"Close"}}
{"op":"reset","session":"SESSION","cell":{"state":"STATE_ID","width":768},"instance":"INSTANCE","slot":"SLOT","expectedRevision":1,"requestId":"reset-1","expect":{"selector":"#menu-toggle","text":"Open"}}
{"op":"save","session":"SESSION","cell":{"state":"STATE_ID","width":768},"expectedRevision":2,"requestId":"save-1"}
{"op":"close","session":"SESSION"}
```

편집마다 고유한 요청 ID를 쓰고 응답을 잃으면 같은 요청을 재전송한다. set과 reset에서 `expect.selector`는 표시된 HTML 요소 하나와 일치해야 하며, 양끝 공백을 뺀 텍스트가 `expect.text`와 같아야 한다. 쓰기 후 검증에 실패해도 리비전은 증가하고 관찰값·텍스트와 함께 `unverified`를 반환한다. reset은 세션을 열었을 때의 값으로 복원한다. close는 브라우저와 호스트를 종료한다. 유휴 세션은 30분 후 만료된다. save는 `~/.dui` 아래 새 실행 경로를 반환한다. `dui open`(또는 `dui open RUN_DIRECTORY`)으로 내장 뷰어를 열거나, 저장된 디렉터리와 칸을 `dui inspect`에 전달한다. PNG와 SHA-256 해시, 관찰된 상태값과 텍스트 단언, 편집 순서와 Git 출처가 함께 저장된다. 다시 라이브로 편집할 때는 원본 칸을 연다.

### 편집 지연 시간 기준값

고정된 React inspect 픽스처에서 메뉴를 30번 번갈아 열고 닫았다. 소유 프로세스가 요청을 받은 때부터 값과 화면 문구를 검증할 때까지 **p50 111.4 ms**, **p95 112.9 ms**였다. Apple M1 Pro, Darwin 25.5.0, Node 26.13.0, TypeScript 7.0.2, Chrome 157.0.8103.79에서 새 세션을 여는 데 **1.72 s**가 걸렸다. 최초 호스트 빌드와 브라우저 실행은 각각 1회, 편집 중에는 모두 0회였다. `npm ci`, `npm ci --prefix fixtures/replay`, `npm run check:inspect` 순서로 재현한다. 실행마다 측정 환경과 픽스처 해시를 출력한다.

## 5. 패턴과 완료 조건

| 패턴 | 확인할 결과 |
|---|---|
| 경계 디코더와 순수 변환 | 이전 기록과 현재 기록을 읽어 지도와 목록이 같은 칸을 가리키는지 확인한다. 알 수 없는 상태는 데이터 오류로 표시한다. 세 가지 판정과 그 이유는 Rust에서 받는다. |
| 기능별 임시 상태와 공통 선택 | 키보드만으로 화면에서 상세 보기로 이동한다. 뒤로 가면 포커스와 선택이 돌아오고 새로고침해도 선택한 폭이 유지된다. |
| 접근 가능한 조작 요소의 조합 | 검증됨·미검증·렌더 불가 칸을 확인한다. 375px와 1280px에서 상태와 이유를 읽을 수 있고, 로딩 화면 이미지도 원래 판정을 유지한다. |
| 세션 하나에서 명령 순차 처리 | CLI와 MCP로 불리언과 열거형을 변경한다. 독립된 조작 요소, 중복 요청, 오래된 요청, 연결 끊김, 초기화를 확인한다. 다시 읽은 값과 화면 근거를 모두 대조한다. |
| 내장 클라이언트와 불변 기록 | 깨끗한 체크아웃에서 설치하고 대상 앱을 꺼둔 채 상세 주소를 바로 연다. 없는 이미지는 오류를 표시한다. 라이브 상태를 저장하고 이전 기록과 새 기록을 모두 다시 연다. |
| 다국어 캡처와 뷰어 i18n ([#12](https://github.com/de-novo/dui/issues/12)) | 매니페스트 `locales` 또는 자동 감지된 메시지 파일(`locales/*.json` / `messages/*.json`)에서 `screen × state × width × locale`을 캡처한다. 로케일별 요약(`report.locales`), 번역 키 누락, 텍스트 넘침 선택자, RTL 불일치를 보고한다. 워크스페이스 레일에서 뷰어 UI 언어를 `en`, `zh-CN`, `ko`로 전환하고 아카이브 칸을 로케일별로 필터링한다(`npm run check:i18n`). |
| 캡처 비교와 픽셀 차이 오버레이 ([#11](https://github.com/de-novo/dui/issues/11)) | `dui diff BEFORE AFTER [--overlay-dir DIR]`, MCP `diff`, 뷰어의 `?view=diff` 화면(`/api/diff`)으로 두 캡처를 비교한다. 콘텐츠 해시가 같은 미변경 칸은 숨기고 추가·삭제·변경된 칸만 화면·상태·폭 순으로 묶어 칸별 변경 소스 파일을 표시한다. 검증된 칸은 이전·이후·픽셀 차이 오버레이를 제공하고, 미검증·렌더 불가 칸은 픽셀 차이 대신 상태 변경만 보고한다(`npm run check:diff`). |

구현 순서는 기록·스키마 픽스처 → 라우터와 키보드 탐색 목록 → 지도 → 라이브 조회·편집 → 패키징·측정으로 한다. 실제 npmgraph 화면 두 개와 재배포 가능한 독립 조작 요소 두 개짜리 픽스처를 사용한다. 준비된 세션에서 30번 편집하며 호스트 재빌드와 브라우저 재시작이 모두 0회인지 확인한다. 최초 시작 시간과 편집의 p50·p95를 따로 보고한다. [shadcn/ui](../../.agents/skills/shadcn/SKILL.md)
