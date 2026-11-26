# Capture reference

[Quickstart](../README.md) · [한국어](ko/reference.md)

## Capture manifest

Pass a JSON manifest to `dui run PROJECT MANIFEST [OUTPUT]`. `PROJECT` is the target app’s directory. The paths supplied on the command line are relative to the current working directory or absolute.

```json
{
  "path": "/",
  "widths": [375, 768, 1280],
  "readySelector": "main article",
  "routeSelector": "nav a[href]"
}
```

| Field | Meaning |
| --- | --- |
| `path` | Required. Same-origin entry path starting with `/`. Include the query string or hash for routes that use them. |
| `widths` | Required. One or more positive viewport widths in CSS pixels. |
| `readySelector` | Required for capture. Choose an element that appears when the intended content has loaded. An app shell can appear before its data. |
| `routeSelector` | Collect matching same-origin anchors recursively via loop-free BFS with visited-URL cycle blocking and dynamic ID pattern collapsing (`/orders/:id`). |
| `scenarios` | Array of overrides for extra cases. For example, `[{"path":"/settings","readySelector":"form"}]` adds a screen with its own readiness check. |
| `maxNoImprovementRounds` | Optional convergence guard. Stops exploring additional states in a scenario after N consecutive states produce no new branch outcomes or slots; remaining planned cells stay in the denominator as `unverified (not-rendered)`. |
| `concurrency` | Browsers capturing at once, 1 to 4. Default: half the logical CPUs, at most 4 (`DUI_CONCURRENCY` also sets it). The first state of each queued screen and the widths of a state run in parallel. The report keeps the same cells in the same order. |
| `requiredRequests` | Paths that must return a 2xx response. A required request with a generated mock must receive that mock. |
| `settleTimeoutMs` | Time budget for a screen to stop changing, in milliseconds (default `10000`). dui samples the page about every 100 ms and needs four identical samples in a row. Regions that change on their own are left out and listed in `observation.ignoredChanges`: `aria-busy`, progress bars, timers, endlessly animated elements, and nodes whose text or position changes in three samples in a row. A screen still changing at the budget is `unverified` with `unsettled: NODE CHANGE`, and `observation.unsettled` lists what changed last. |
| `expectedApiErrors` | API paths (or `prefix*`) whose 401, 403 or 5xx response is part of the screen, such as a signed-out state. Any other same-origin or `apiBaseUrl` Fetch/XHR response with those statuses leaves the cell `unverified` with the reason `api-error: STATUS PATH`. Failed calls are listed in `observation.apiErrors`. |
| `responseChecks` | Compare named fields in a JSON response, for example `{"/api/status":{"ready":true}}`. |
| `tsconfig` · `appRoot` | Paths relative to `PROJECT`. Defaults are `tsconfig.json` for analysis and `.` for Vite. |
| `rulePacks` | Additional rule files, resolved relative to the manifest. See the [rule pack schema](../rules/schema.json). |

## Mock responses

**Typed JSON.** dui recognizes same-origin GET requests with a static path and a typed `response.json()` result. It generates representative values for supported shapes, including three entries for arrays. Inspect `facts.json → autoMocks.generated` for payloads and `autoMocks.unresolved` for detected contracts it could not synthesize. Dynamic URLs, custom API clients, and server-side requests fall outside automatic JSON synthesis.

**Explicit JSON.** Set `mockResponses` to a map from request path to JSON value. An entry takes precedence over a generated response at that path. For example, `{"mockResponses":{"/api/shelf":{"books":[]}}}` exercises an empty shelf. Give that scenario a readiness selector for its empty state.

**CSV.** The request path, a sample inside the project, and column roles determine the generated feed. Column indices start at zero. This fragment preserves the header, assigns a stable name to each distinct nonempty label, and rotates pairs of values together by one row. Other columns retain their sample values.

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

## Live API

Set `"apiMode": "live"` (or `"hybrid"` to keep `mockResponses` for some paths) and `apiBaseUrl` (an `http://` URL, or `env:NAME`) to load data from a running API. Requests under `apiProxyPrefix` (or `apiProxyPrefixes`), such as `"/api/"`, are sent to `apiBaseUrl` with the page's own headers, including `Authorization` and cookies, and its request body. `apiHeaderEnv` adds headers read from environment variables, such as `{"Authorization": "API_TOKEN"}`; they replace a page header of the same name and never appear in archives. The response, including `Set-Cookie`, goes back to the page. Each proxied request runs on its own worker, so a slow endpoint delays only the screen that waits for it, and several requests can be in flight. Requests other than `GET`, `HEAD` and `OPTIONS` are blocked unless `allowMutations` is `true`.

A proxied request still waiting when the screen is captured is listed in `observation.apiPending`. One that failed is listed in `observation.apiFailures` with its reason, for example `timed out after 10 s waiting for GET http://127.0.0.1:8080/api/orders`.

## Read a run

| File in `OUTPUT` | What to inspect |
| --- | --- |
| `graph.html` | Default captures by route and width, with a graph for exploring individual states and their evidence. Open locally alongside the PNG files. |
| `report.json` | Cell status and reason, screenshot path, compiler diagnostics, and coverage counts. `cells[].observation.fulfilledMocks` lists generated mocks actually served. |
| `facts.json` | Compiler facts, state candidates’ source locations, and generated responses. |
| `graph.json` | Nodes and edges for tools that consume the graph. |

A **cell** represents one scenario, state, and width. `verified` means the observed route, width, readiness, required requests, and applicable state checks passed. `unverified` includes failed checks and queued states left beyond the budget (`not-rendered`). `unrenderable` records an environment limitation. An unsupported project is rejected before capture.

**Unverified reasons.** `reason` starts with the kind of failure, followed by what failed: `http-error` (the document returned 4xx/5xx), `api-error: 401 /api/me`, `redirected: /a → /b`, `not-ready: SELECTOR` (with `+ unsettled: NODE CHANGE` when the page also kept changing), `unsettled: NODE CHANGE`, `width-mismatch: observed W (expected W)`, `request-failed: /path STATUS`, `not-analyzed: compiler diagnostics`, `assert-failed: slot NAME is VALUE (expected VALUE)` (or a failed state assertion, or a branch probe that did not run), `missing-state-assertion`, `missing-screen-assertion` and `not-rendered`. `dui map` counts unverified cells by kind in its stderr summary.

**Cell coverage** is `coverage.verified / coverage.renderable`. If all six routes are captured at three widths, that accounts for 18 baseline cells. Additional planned states still count in the denominator. Use `observedBranchOutcomes` and `branchOutcomes` to inspect branch coverage separately.

**Timing.** Each captured cell has `timing` in milliseconds: `navigateMs`, `settleMs` with `settlePolls`, `screenshotMs`, `overlaysMs`, `keyboardMs` and `totalMs`. Cells served from the cache have none. `timings` in the report gives `wallMs`, per-phase `totals`, the `slowestPhase` and the five `slowestCells`. `dui map` also prints one line to stderr with the cell count, the wall time and the slowest phase.

## Local capture history

`run`, `exercise`, and `replay` save artifacts in `~/.dui/projects/<project-id>/runs/<snapshot-id>/`. Each run gets a new directory, including incomplete captures that return exit code 5. The CLI returns its absolute path as `output`. An optional output argument provides a filesystem link at the requested location. For an existing output directory, dui links the generated files individually. A run writes into `runs/.pending-*` until it finishes. SIGINT, SIGTERM or SIGHUP stops the run's browsers and servers, removes that directory and exits with 128 + the signal number. A `.pending-*` directory whose process is gone, or that has no owner and is older than an hour, is removed when the next run starts.

Run `dui open` to choose a project and compare two snapshots in the browser. `dui history` refreshes the viewer and returns the snapshot list as JSON. MCP exposes the same `history()` operation. Linked Git worktrees share the primary checkout’s project identity. Apps within a monorepo are grouped by relative path. Independent clones and non-Git directories have separate histories. Detached HEAD captures retain the commit.

The viewer pairs cells by route, state ID, and width. It compares the stored PNG file hashes. A missing image is marked unavailable. Different state IDs appear as added or removed cells. Projects captured before home storage can be opened from their original galleries.

## CLI and MCP

The commands assume `dui` is on your `PATH`. With the default installation you can also use `~/.local/bin/dui`. `PLAN` is the original run’s `report.json`. Give `replay` a new output directory, then use that same directory with `replay-report`.

| CLI | MCP | Operation |
| --- | --- | --- |
| `dui map [PROJECT]` | `map(project)` | Auto-discover routes (TanStack Router, React Router, links), merge optional `dui.json` (`routeParams`, `storageState`, extra `scenarios`), and capture. A React Router route with children but no `element`, `Component` or `index` is a group, and one whose element is a static `<Navigate to>` is a redirect. Neither is captured; both are listed in `screenDiscovery.layouts` and `screenDiscovery.redirects`. Children that cannot be read statically appear in `screenDiscovery.unresolved` as `PATH/*`. |
| `dui run [PROJECT] [MANIFEST] [OUTPUT]` | `run(project, manifest, output?)` | Capture using `dui.json` directly (defaults to `.` / `dui.json`, or auto-discovers via `map` when omitted). |
| `dui scan [PROJECT] [MANIFEST]` | `scan(project, manifest)` | Print compiler facts, framework controls, and generated mocks without launching a browser. |
| `dui auth login URL [--out FILE]` | — | Launch Chromium to sign in (form / MFA / OAuth) and save Playwright-compatible `storageState.json` (`0600`). |
| `dui open [RUN_DIR]` | — | Open the embedded 2D canvas viewer in the browser. |
| `dui desktop` | — | Open the native GPUI desktop viewer over `~/.dui/dui.db`. |
| `dui diff BEFORE AFTER [--markdown]` | `diff(before, after)` | Compare two runs (element `renderDiff`, lazy pixel overlay, or `--base BRANCH --markdown` for PR comments). |
| `dui audit RUN_DIR [--goal PERCENT]` | `audit(run, goal?)` | Compute coverage percentage, gaps, and unresolved review comments. |
| `dui audit RUN_DIR --design MANIFEST [--goal PERCENT]` | — | Compare recorded nodes with a project specification and return findings under `designAudit`. [Plugin contract](en/architecture.md#63-project-owned-design-audit) |
| `dui comment <add\|list\|resolve\|reopen>` | `comment_*` | Manage review comments anchored to screens, states, widths, or transitions. |
| `dui transitions PROJECT MANIFEST [RUN]` | `transitions(...)` | Replay actions and verify extracted/inferred screen transitions. |
| `dui replay PROJECT MANIFEST PLAN [OUTPUT]` | `replay(project, manifest, plan, output?)` | Attempt every cell in a saved plan, including pending states. |
| `dui replay-report MANIFEST PLAN OUTPUT` | `replay_report(manifest, plan, output)` | Recompute the report from saved evidence. |
| `dui toolchains` | — | Probe installed official compiler toolchains (`tsc`, `rustc`, `go`, `swiftc`, `kotlinc`, `dart`). |

The CLI also provides `check-ts PROJECT` to check the installed compiler, `plan FIXTURE` to expand explicit states and widths into jobs, and `exercise PROJECT MANIFEST OUTPUT PORT` to capture against an already running local host.

For MCP, configure a stdio server with the binary’s absolute path as the command and `["mcp"]` as its arguments. Tool paths are strings. A call returns a JSON-encoded result in `content[].text` and sets `isError` when the operation reports failure or incomplete verification.

The CLI writes JSON to standard output. A capture can write a usable report and exit with code 5.

| Exit | Meaning |
| --- | --- |
| 0 | Command completed successfully. |
| 2 | Invalid input, manifest, or replay plan. |
| 3 | Execution or I/O failure. |
| 4 | Unsupported project or toolchain. |
| 5 | Some cells are unverified or unrenderable. Inspect `report.json` for reasons. |
