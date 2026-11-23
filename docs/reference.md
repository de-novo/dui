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
| `requiredRequests` | Paths that must return a 2xx response. A required request with a generated mock must receive that mock. |
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

## Read a run

| File in `OUTPUT` | What to inspect |
| --- | --- |
| `graph.html` | Default captures by route and width, with a graph for exploring individual states and their evidence. Open locally alongside the PNG files. |
| `report.json` | Cell status and reason, screenshot path, compiler diagnostics, and coverage counts. `cells[].observation.fulfilledMocks` lists generated mocks actually served. |
| `facts.json` | Compiler facts, state candidates’ source locations, and generated responses. |
| `graph.json` | Nodes and edges for tools that consume the graph. |

A **cell** represents one scenario, state, and width. `verified` means the observed route, width, readiness, required requests, and applicable state checks passed. `unverified` includes failed checks and queued states left beyond the budget (`not-rendered`). `unrenderable` records an environment limitation. An unsupported project is rejected before capture.

**Cell coverage** is `coverage.verified / coverage.renderable`. If all six routes are captured at three widths, that accounts for 18 baseline cells. Additional planned states still count in the denominator. Use `observedBranchOutcomes` and `branchOutcomes` to inspect branch coverage separately.

## Local capture history

`run`, `exercise`, and `replay` save artifacts in `~/.dui/projects/<project-id>/runs/<snapshot-id>/`. Each run gets a new directory, including incomplete captures that return exit code 5. The CLI returns its absolute path as `output`. An optional output argument provides a filesystem link at the requested location. For an existing output directory, dui links the generated files individually.

Run `dui open` to choose a project and compare two snapshots in the browser. `dui history` refreshes the viewer and returns the snapshot list as JSON. MCP exposes the same `history()` operation. Linked Git worktrees share the primary checkout’s project identity. Apps within a monorepo are grouped by relative path. Independent clones and non-Git directories have separate histories. Detached HEAD captures retain the commit.

The viewer pairs cells by route, state ID, and width. It compares the stored PNG file hashes. A missing image is marked unavailable. Different state IDs appear as added or removed cells. Projects captured before home storage can be opened from their original galleries.

## CLI and MCP

The commands assume `dui` is on your `PATH`. With the default installation you can also use `~/.local/bin/dui`. `PLAN` is the original run’s `report.json`. Give `replay` a new output directory, then use that same directory with `replay-report`.

| CLI | MCP | Operation |
| --- | --- | --- |
| `dui map [PROJECT]` | `map(project)` | Auto-discover routes (TanStack Router, React Router, links), merge optional `dui.json` (`routeParams`, `storageState`, extra `scenarios`), and capture. |
| `dui run [PROJECT] [MANIFEST] [OUTPUT]` | `run(project, manifest, output?)` | Capture using `dui.json` directly (defaults to `.` / `dui.json`, or auto-discovers via `map` when omitted). |
| `dui scan [PROJECT] [MANIFEST]` | `scan(project, manifest)` | Print compiler facts, framework controls, and generated mocks without launching a browser. |
| `dui auth login URL [--out FILE]` | — | Launch Chromium to sign in (form / MFA / OAuth) and save Playwright-compatible `storageState.json` (`0600`). |
| `dui open [RUN_DIR]` | — | Open the embedded 2D canvas viewer in the browser. |
| `dui desktop` | — | Open the native GPUI desktop viewer over `~/.dui/dui.db`. |
| `dui diff BEFORE AFTER [--markdown]` | `diff(before, after)` | Compare two runs (element `renderDiff`, lazy pixel overlay, or `--base BRANCH --markdown` for PR comments). |
| `dui audit RUN_DIR [--goal PERCENT]` | `audit(run, goal?)` | Compute coverage percentage, gaps, and unresolved review comments. |
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
