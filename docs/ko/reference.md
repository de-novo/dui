# 캡처 레퍼런스

[빠른 시작](../../README.ko.md) · [English](../reference.md)

## 캡처 설정

`dui run PROJECT MANIFEST [OUTPUT]`에 JSON 설정 파일을 전달한다. `PROJECT`는 대상 앱의 디렉터리다. 명령줄 경로는 현재 작업 디렉터리 기준 또는 절대 경로로 지정한다.

```json
{
  "path": "/",
  "widths": [375, 768, 1280],
  "readySelector": "main article",
  "routeSelector": "nav a[href]"
}
```

| 필드 | 용도 |
| --- | --- |
| `path` | 필수. `/`로 시작하는 같은 출처의 진입 경로. 필요하면 쿼리 문자열이나 해시도 포함한다. |
| `widths` | 필수. 캡처할 화면 폭을 양의 정수로 나열한다. 단위는 CSS 픽셀이다. |
| `readySelector` | 캡처에 필수. 원하는 콘텐츠가 로드된 뒤 나타나는 요소를 고른다. 앱의 바깥 틀은 데이터보다 먼저 나타날 수 있다. |
| `routeSelector` | 선택자에 맞는 같은 출처 링크를 사이클 차단과 동적 ID 패턴 축약(`/orders/:id`)이 적용된 다단계 BFS로 재귀 탐색한다. |
| `scenarios` | 기본 설정에서 달라지는 값만 담은 배열. 예를 들어 `[{"path":"/settings","readySelector":"form"}]`은 폼이 준비될 때까지 기다리는 설정 화면을 추가한다. |
| `maxNoImprovementRounds` | 선택적 수렴 가드. 연속 N개 상태에서 새 분기나 새 슬롯이 관측되지 않으면 시나리오 탐색을 종료하며, 남은 계획 칸은 분모에 `unverified (not-rendered)`로 보존한다. |
| `requiredRequests` | 2xx 응답을 받아야 할 요청 경로. 생성한 목 응답을 지정한 요청은 실제로 그 응답을 받아야 한다. |
| `responseChecks` | JSON 응답의 지정 필드를 비교한다. 예를 들어 `{"/api/status":{"ready":true}}`로 준비 상태를 확인한다. |
| `tsconfig` · `appRoot` | `PROJECT` 기준 경로. 분석에 쓸 설정의 기본값은 `tsconfig.json`, Vite 앱 루트의 기본값은 `.`이다. |
| `rulePacks` | 추가 규칙 파일. 설정 파일을 기준으로 경로를 해석한다. [규칙 팩 스키마](../../rules/schema.json) |

## 목 응답

**타입이 있는 JSON.** 같은 출처의 고정 경로로 GET 요청을 보내고 `response.json()`의 반환 타입을 알 수 있으면 dui가 대표값을 만든다. 배열에는 요소 세 개를 넣는다. `facts.json`의 `autoMocks.generated`에서 생성된 응답을, `autoMocks.unresolved`에서 발견했지만 값을 만들지 못한 경로를 확인한다. 동적 URL, 별도 API 클라이언트, 서버 측 요청은 자동 JSON 생성 범위 밖이다.

**직접 지정한 JSON.** `mockResponses`에 요청 경로와 응답 값을 넣으면 해당 경로에서 우선 적용한다. 예를 들어 `{"mockResponses":{"/api/shelf":{"books":[]}}}`은 빈 책장을 재현한다. 이 시나리오의 `readySelector`는 빈 상태의 안내 요소를 가리켜야 한다.

**CSV.** 요청 경로, 프로젝트 안의 샘플 파일, 바꿀 열을 지정한다. 열 번호는 0부터 센다. 아래 설정은 제목 행을 보존하고 비어 있지 않은 같은 이름에 같은 대체 이름을 부여한다. 값 두 개는 다음 행에서 한 묶음으로 가져온다. 나머지 열에는 원래 값이 남는다.

```json
{
  "csvMocks": {
    "/usage.csv": {
      "sample": "public/usage.csv",
      "headers": true,
      "labelColumns": [0],
      "valueColumns": [1, 2]
    }
  }
}
```

## 결과 읽기

| 출력 파일 | 확인할 내용 |
| --- | --- |
| `graph.html` | 경로와 폭별 기본 화면, 각 상태와 검증 근거를 탐색하는 그래프. PNG와 같은 디렉터리에 두고 브라우저로 연다. |
| `report.json` | 칸의 상태와 이유, 이미지 경로, 컴파일러 진단, 커버리지. `cells[].observation.fulfilledMocks`에는 실제로 제공한 자동 목 응답의 경로가 남는다. |
| `facts.json` | 컴파일러에서 읽은 사실, 상태 후보의 소스 위치, 생성한 목 응답. |
| `graph.json` | 도구에서 읽을 수 있는 그래프 노드와 간선. |

**한 칸**은 시나리오 하나의 상태 하나를 한 가지 폭으로 캡처할 자리다. `verified`는 경로, 폭, 준비 조건, 필수 요청과 해당 상태에 대한 검사를 통과한 칸이다. `unverified`에는 검사에 실패한 칸과 예산 한도로 시도하지 못한 칸(`not-rendered`)이 포함된다. `unrenderable`에는 렌더 환경의 한계를 기록한다. 미지원 프로젝트는 캡처 전에 거절한다.

**칸 검증률**은 `coverage.verified / coverage.renderable`이다. 화면 여섯 개를 폭 세 가지에서 모두 캡처하면 기본 상태 18칸을 채운 것이다. 추가로 계획된 상태도 분모에 포함한다. 분기 커버리지는 `observedBranchOutcomes`와 `branchOutcomes`로 따로 확인한다.

## 로컬 캡처 기록

`run`, `exercise`, `replay`는 결과를 사용자 홈의 `~/.dui/projects/<프로젝트 ID>/runs/<캡처 ID>/`에 저장한다. 종료 코드 5인 부분 캡처도 매번 새 폴더에 남는다. CLI 결과의 `output`이 이 폴더의 절대 경로다. 출력 경로를 따로 지정하면 그 위치에 링크를 만든다. 기존 출력 폴더에는 생성한 파일별로 링크를 둔다.

`dui open`으로 브라우저에서 프로젝트와 비교할 캡처 두 개를 고른다. `dui history`는 뷰어를 갱신하고 캡처 목록을 JSON으로 돌려준다. MCP에서는 `history()`를 호출한다. 같은 저장소의 연결된 워크트리는 기본 체크아웃 경로를 기준으로 묶고, 모노레포의 앱은 상대 경로로 구분한다. 별도로 클론한 저장소와 Git을 쓰지 않는 폴더는 각각 독립된 기록을 갖는다. 브랜치 없이 체크아웃한 경우에도 커밋을 기록한다.

뷰어는 경로·상태 ID·폭이 같은 칸의 PNG 파일 해시를 비교한다. 이미지가 없으면 ‘이미지 비교 불가’로 표시한다. 상태 ID가 달라지면 삭제·추가된 칸으로 표시한다. 이전 버전에서 만든 캡처는 원래 갤러리에서 열 수 있다.

## CLI와 MCP

아래 명령은 `dui`가 `PATH`에 있을 때 쓴다. 기본 경로에 설치했다면 `~/.local/bin/dui`로 실행해도 된다. `PLAN`은 최초 실행의 `report.json`이다. `replay`에는 새 출력 디렉터리를 지정하고, `replay-report`에는 그 디렉터리를 다시 전달한다.

| CLI | MCP | 동작 |
| --- | --- | --- |
| `dui map [PROJECT]` | `map(project)` | 라우터(TanStack Router · React Router)와 링크에서 화면을 자동 발견하고, `dui.json`(`routeParams`, `storageState`, 추가 `scenarios`)을 병합해 캡처한다. |
| `dui run [PROJECT] [MANIFEST] [OUTPUT]` | `run(project, manifest, output?)` | `dui.json` 설정대로 직접 캡처한다(인자 생략 시 `.` / `dui.json` 사용, 설정이 없으면 `map`으로 자동 발견). |
| `dui scan [PROJECT] [MANIFEST]` | `scan(project, manifest)` | 브라우저 없이 컴파일러 사실, 프레임워크 컨트롤, 생성한 목 응답을 출력한다. |
| `dui auth login URL [--out FILE]` | — | Chromium 창에서 로그인(폼·MFA·OAuth) 후 `0600` 권한의 `storageState.json`으로 저장한다. |
| `dui open [RUN_DIR]` | — | 내장 2D 캔버스 뷰어를 브라우저에서 연다. |
| `dui desktop` | — | `~/.dui/dui.db`를 직접 읽는 네이티브 GPUI 데스크톱 뷰어를 연다. |
| `dui diff BEFORE AFTER [--markdown]` | `diff(before, after)` | 두 캡처를 비교한다(요소 `renderDiff`, 지연 픽셀 오버레이, `--base BRANCH --markdown` PR 코멘트 출력). |
| `dui audit RUN_DIR [--goal PERCENT]` | `audit(run, goal?)` | 검증률·누락 사유·미해결 리뷰 코멘트를 계산한다. |
| `dui audit RUN_DIR --design MANIFEST [--goal PERCENT]` | — | 캡처한 노드를 프로젝트 명세와 비교하고 결과를 `designAudit`에 기록한다. [플러그인 규약](architecture.md#63-프로젝트-디자인-검사) |
| `dui comment <add\|list\|resolve\|reopen>` | `comment_*` | 화면·상태·폭·전이에 고정된 리뷰 코멘트를 관리한다. |
| `dui transitions PROJECT MANIFEST [RUN]` | `transitions(...)` | 액션을 재실행해 화면 전이를 검증한다. |
| `dui replay PROJECT MANIFEST PLAN [OUTPUT]` | `replay(project, manifest, plan, output?)` | 저장한 계획의 모든 칸을 시도한다. |
| `dui replay-report MANIFEST PLAN OUTPUT` | `replay_report(manifest, plan, output)` | 저장한 증거로 보고서를 다시 계산한다. |
| `dui toolchains` | — | 설치된 6개 공식 컴파일러(`tsc`, `rustc`, `go`, `swiftc`, `kotlinc`, `dart`) 지원 상태를 진단한다. |

CLI 전용 명령도 있다. `check-ts PROJECT`는 설치된 컴파일러를 확인하고, `plan FIXTURE`는 명시한 상태와 폭을 칸 목록으로 변환한다. `exercise PROJECT MANIFEST OUTPUT PORT`는 실행 중인 로컬 호스트에서 캡처한다.

MCP 클라이언트에 stdio 서버를 등록한다. 실행 명령은 바이너리의 절대 경로, 인자는 `["mcp"]`다. 도구의 경로 인자는 문자열이다. 호출 결과는 `content[].text`에 JSON 문자열로 담긴다. 실행에 실패하거나 미검증 칸 등이 남으면 `isError`가 설정된다.

CLI는 표준 출력에 JSON을 쓴다. 종료 코드 5여도 캡처 보고서를 열 수 있다.

| 종료 코드 | 의미 |
| --- | --- |
| 0 | 성공 |
| 2 | 입력, 설정 또는 재실행 계획 오류 |
| 3 | 실행 또는 입출력 실패 |
| 4 | 미지원 프로젝트 또는 도구 체인 |
| 5 | 미검증 또는 렌더 불가 칸이 남음. 이유는 `report.json`에서 확인한다. |
