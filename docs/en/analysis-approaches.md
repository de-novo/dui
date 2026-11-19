# How code-analysis AI tools understand codebases (research before designing dui's analyzer)

English | [简体中文](../zh-CN/analysis-approaches.md) | [한국어](../ko/analysis-approaches.md)

> Written 2026-11-19. Compiled by reading each tool's README, source, and official docs directly. Anything not confirmed is `unconfirmed`.
> ★ and licenses are GitHub API values as of 2026-11-19 (when `spdx_id` is empty, confirmed from the LICENSE text or package metadata).
> Premise (준혁's conclusion): state locations, types, branches, and `event → setState` rules can mostly be derived from **AST + type info**. AI only fills gaps, and combinations are produced by deterministic algorithms.

## 0. Terms

| Term | Meaning |
|---|---|
| AST | Code read as a syntax tree. No types or name binding |
| tree-sitter | A parser that turns dozens of languages into ASTs the same fast way. Produces partial trees even for broken code. **No type info** |
| Compiler API | **Type and name resolution** from a language's official tooling, such as the TS compiler API or rust-analyzer |
| LSP | Language Server Protocol for editors (go to definition, find references). Queries are fine-grained |
| SCIP | A language-neutral index format by Sourcegraph. Stores definitions, references, and implementations **with types resolved** in one file |
| Data flow | Tracking where a value comes from and where it goes (e.g. state → JSX) |
| Embeddings / RAG | Turning code fragments into vectors to find similar ones. Results are probabilistic |

## 1. Code-graph and AI coding tools

| Tool | ★ / license | Parser | Graph | Where an LLM is used | Embeddings | Incremental updates | Languages |
|---|---|---|---|---|---|---|---|
| [Graphify](https://github.com/Graphify-Labs/graphify) | 150,438 / Apache-2.0 | tree-sitter ("deterministic, no LLM") | calls, imports, inherits; EXTRACTED/INFERRED on every edge; Leiden communities | Not for code. Only for semantic extraction from docs/PDFs/media and naming communities (optional) | **None** ("Not a vector index") | `graphify watch` | ~40 |
| [CodeGraph](https://github.com/colbymchenry/codegraph) | 78,088 / MIT | tree-sitter (Rust kernel, `codegraph-kernel/Cargo.toml`) | Symbols, calls, references + **framework routes and `navigates` edges** (React Router, TanStack Router, Next, Vue Router, etc.), Tauri `invoke` tracing | Per README, no LLM in graph building | None; SQLite FTS5 full-text search | Auto-sync via file watching | 20+ |
| [GitNexus](https://github.com/abhigyanpatwari/GitNexus) | 48,658 / **PolyForm Noncommercial** | tree-sitter queries (`ARCHITECTURE.md`) | 19-phase pipeline: symbols, MRO, DI, Leiden communities, execution flows (processes), stored in LadybugDB | "The CLI indexes code locally without an LLM"; wiki generation etc. optional | Optional (remote embeddings) | `--watch` | Many |
| [Axon](https://github.com/harshkedia177/axon) | 820 / MIT (pyproject) | tree-sitter (Python, TS, JS) | Calls, type relations, communities, execution flows; KuzuDB | None ("Zero cloud") | Local 384-dim (bge-small) + BM25 hybrid | `--watch` | 3 |
| [Descry](https://github.com/06chaynes/descry) | 1 / MIT | Regex (+ast-grep) + **SCIP overlay for type resolution** | Calls, imports, definitions; resolutions labelled resolved / unresolved / external | None | Optional (sentence-transformers) | Cache | All SCIP-supported languages |
| [SCIP](https://github.com/scip-code/scip) (+ scip-typescript etc.) | 892 / Apache-2.0 | Per-language compilers: scip-typescript uses the TS compiler API (`ts.createProgram`, `getTypeChecker`); Rust uses `rust-analyzer scip` | Definitions, references, implementations (precise) | None | None | Depends on indexer | TS/JS, Rust, Java/Kotlin, C#, C/C++, Dart, Go, etc. |
| Sourcegraph Cody | unconfirmed (repository visibility unconfirmed) | Keyword search + SCIP code graph | SCIP definitions / references | Answer generation, candidate reranking | Yes (embedding search) | unconfirmed | Many |
| [Aider repo map](https://aider.chat/docs/repomap.html) | 50,182 / Apache-2.0 | tree-sitter + per-language `*-tags.scm` queries (def/ref tags); Pygments fills refs for languages without them | **File graph** (referencing file → defining file) + **personalized PageRank** ranking | The map summarizes context for the LLM | None | File mtime cache | tree-sitter languages |
| [Continue](https://github.com/continuedev/continue) | 36,519 / Apache-2.0 | tree-sitter chunks per function / class (`core/indexing/chunk/code.ts`) | None (search-oriented) | Answers | **Yes**, LanceDB vector store | Cache-key based | Many |
| Cursor | Closed | Per public docs: Instant Grep (local regex index) + an explore subagent | unconfirmed | Summarizing search results, exploration | Current docs state it "does not store embeddings of your codebase for search" ([docs](https://cursor.com/docs/context/semantic-search)) | Local index | Language-agnostic |
| Windsurf (Codeium) | Closed | "Parses code into blocks using its structure" (parser type unconfirmed) | unconfirmed | Reranking, answers | **Yes** (local indexing: fragments sent to a server for embedding, stored locally) | Remote index re-indexed periodically | Language-agnostic |
| GitHub code navigation | [stack-graphs](https://github.com/github/stack-graphs) 877 / Apache·MIT, **archived 2025-09-09** | tree-sitter + a DSL for name-binding rules (stack graphs) | Definitions / references (built per file, merged at query time → incremental) | None | None | Per-file incremental | Python, TS, etc. |
| [Serena](https://github.com/oraios/serena) | 31,530 / **SolidLSP MIT, Serena app GPL-3.0+** | **LSP** (40+ language servers) or a JetBrains plugin | No stored graph; LSP queried on demand (definitions, references, symbols) | Used as tools by an agent | None | Handled by LSP | 40+ |
| [Joern](https://github.com/joernio/joern) | 3,655 / Apache-2.0 | Per-language frontends (including fuzzy parsers) | **Code Property Graph**: AST + CFG + PDG (data flow) in one graph | None | None | unconfirmed | C/C++, Java, JS, Python, Go, Kotlin, PHP, Ruby, Swift |
| [CodeQL](https://github.com/github/codeql) | 10,385 / queries MIT, **CLI under separate terms**: analyzing private code requires paid GitHub Code Security | Compiler-level extractors (observing builds) | Relational DB + data-flow / taint libraries | None | None | No (re-extraction) | ~10 |
| [Semgrep](https://github.com/semgrep/semgrep) | 17,822 / LGPL-2.1 (engine) | Per-language parsers → generic AST | Pattern matching + intra-function data flow (cross-file analysis in the paid edition, unconfirmed) | None (open engine) | None | unconfirmed | 30+ (unconfirmed) |
| [Kythe](https://github.com/kythe/kythe) | 2,171 / Apache-2.0 | Per-language indexers (compiler-integrated) | Definition, reference, and type graph | None | None | unconfirmed | C++, Java, Go, TS, etc. |

## 2. UI- and state-specific static analysis

| Tool | Approach | What it gives dui |
|---|---|---|
| [React Compiler](https://github.com/react/react/blob/main/compiler/docs/DESIGN_GOALS.md) (MIT) | Babel plugin. Component / hook AST → **HIR** (control-flow graph of basic blocks) → **SSA** → (conservative) type inference → mutation / aliasing effect inference → reactive scopes → code regeneration. No LLM | A real example of solving **data flow between hooks, state, and rendering precisely from the AST alone**. The injection transform lives in the same Babel stage |
| eslint-plugin-react-hooks | `rules-of-hooks` uses the ESLint AST + code-path analysis (no TS types). 6.x `recommended-latest` includes React Compiler–based diagnostics | Reference for locating hook calls and detecting conditional calls |
| [react-docgen-typescript](https://github.com/styleguidist/react-docgen-typescript) (MIT, ★1,266) | Extracts prop types with `ts.createProgram` + `getTypeChecker()` | Base ingredient for **prop types → representative values** |
| [ts-morph](https://github.com/dsherret/ts-morph) (MIT, ★6,231) | Wrapper over the TS compiler API (AST navigation / editing, type checker) | Analysis + source transforms in one tool. TS 7 support is in progress (see the risk below) |
| XState / Stately (MIT, ★30,520) | People write state machines. [statelyai/agent](https://github.com/statelyai/agent) **generates machines with an LLM**. No official tool found that statically infers a state machine from existing code | Reference for the output format (states, events, transitions). The inference must be built by dui |
| [Storybook 11 `write-stories`](https://storybook.js.org/docs/11/ai/skills/write-stories) | An **agent (LLM)** writes per-state stories for each component (default, loading, empty, error, disabled) + MSW mocks | An example of leaving state enumeration to an LLM. Results are non-deterministic with no coverage number → where dui differentiates |
| rust-analyzer (MIT/Apache, ★17,008) | Usable as a library via the `ra_ap_*` crates (`ra_ap_hir`, etc.). **No backward-compatibility guarantee** | Rust type and name resolution. Also outputs SCIP via `rust-analyzer scip` |
| syn (Apache/MIT, ★3,448) | Rust syntax parser. No type resolution | For writing proc-macros (injection). Not enough as the main analyzer |
| Swift: SwiftSyntax, SourceKit-LSP | Syntax tree + LSP (details unconfirmed) | For future Swift support |
| Kotlin: Kotlin Analysis API (K2), PSI | Compiler-based analysis (details unconfirmed) | For future Kotlin support |

## 3. Academia: extracting UI transition graphs from code

| Work | Method | What it gives dui |
|---|---|---|
| GATOR / [Static Window Transition Graphs for Android (ASE 2015)](https://dacongy.github.io/papers/yang-ase15.pdf) | Static analysis builds a graph where **windows = nodes and event-triggered transitions = edges** (WTG). Edges are annotated with window-stack push/pop and callback sequences | Same shape as dui's `state + event → next state`. Evidence that this is possible with static analysis alone |
| [On the unsoundness of static analysis for Android GUIs](https://doi.org/10.1145/2931021.2931026) | Analyzes cases static GUI analysis misses | Static analysis alone cannot reach 100% → evidence that **replay verification is needed** |
| [FlowDroid](https://github.com/secure-software-engineering/FlowDroid) (LGPL-2.1, ★1,275) | Android lifecycle modeling + taint data flow | Lesson: framework callbacks (events) must be modeled for flows to connect |
| Model-based GUI testing (building a state model while running the app, e.g. the DroidBot / Stoat family) | Run the app and collect a screen-state graph | **Supplement by execution** the states static analysis misses. Per-tool details and licenses unconfirmed |

## 4. Common pattern: which technology is used at which layer

| Layer | What almost every tool uses | LLM used? |
|---|---|---|
| 1. Parsing | tree-sitter (many languages, fast, no types) | No |
| 2. Name / type resolution | When precision matters, the **language compiler** (TS compiler API, rust-analyzer) or its output, **SCIP / LSP** | No |
| 3. Graph | Call / reference graphs. Security tools use data flow (CPG, CodeQL) | No |
| 4. Ranking / grouping | PageRank (Aider), Leiden communities (Graphify, GitNexus) | No (naming only, optional) |
| 5. Search | grep / FTS (Cursor, CodeGraph), embeddings (Continue, Windsurf, Cody) | Embedding models |
| 6. Interpretation / generation | Summaries, answers, reranking, writing stories (Storybook) | **The LLM is used only here** |

Observations
- Graph-building tools (Graphify, CodeGraph, Aider, stack-graphs, Joern) **do not use an LLM to extract structure**, because it must be deterministic and reproducible.
- Where precision is needed (SCIP, CodeQL, React Compiler), they use **compiler-level type resolution**, not tree-sitter.
- Embeddings are for "finding similar code", not for extracting state or branches. Cursor's recent public docs are, if anything, grep-centric.
- General-purpose tools do not extract (UI) state transitions. Academia (GATOR WTG) and React Compiler's internal analysis are the closest.
→ 준혁's conclusion (AST + types first, AI as a helper) matches how the industry works.

## 5. Recommendation for dui's analyzer

> **Updated 2026-11-19**: dui's core is now Rust and depends only on official compilers. ts-morph, Babel / SWC, and Graphify-as-engine below are superseded by the [design: Tech stack](design-v0.md#tech-stack-decided-2026-11-19). The tables below are kept as research.

### 5.1 TypeScript

| Candidate | Pros | Cons | Decision |
|---|---|---|---|
| **TS compiler API (wrapped by ts-morph)** | Precise type and symbol resolution via the type checker. Can resolve discriminated unions and generics. The same tool can also transform | Slow (loads the whole program). The API changes in TS 7 (Go native) | **Main** |
| tree-sitter | Very fast, tolerates broken code, incremental | No types → cannot decide representative values or impossible combinations | Only for a fast first scan (screen and file lists) |
| SCIP (scip-typescript) | Precise definitions / references, language-neutral format | No type **structure** (fields, union arms) and no data flow | Not used (cross-reference helper if needed) |
| Babel/SWC AST | Where the injection transform goes | No types | **Injector only** (Vite plugin) |

TS version risk: per InfoQ (2026-11-06), TS 7.0 (2026-07, Go native) dropped the compiler API, and the 7.1 beta **restored the API through new entry points** such as `typescript/sync`, `typescript/async`, and `typescript/ast` (not compatible with the old `ts.createProgram`; confirmed in the [official TS 7.1 beta post](https://devblogs.microsoft.com/typescript/announcing-typescript-7-1-beta/), which also lists `typescript/fs`). obora keeps `typescript ^5.9.3` for tooling and runs TS7 separately via tsgo (confirmed in pnpm-workspace.yaml). → **dui keeps the analysis layer behind a `TypeOracle` interface and supports only the current latest stable major, i.e. TS 7.x (policy decided 2026-11-20); obora will move to TS 7. Type queries use the TS 7.1 API.**

Data flow (state → JSX, handler → setState) is **implemented by dui itself**, like React Compiler, as per-function control flow + simple data flow (React's hook rules keep the scope narrow). Whether React Compiler internals can be reused as a library is `unconfirmed` (not a public API) → check in M1.

### 5.2 Rust

| Candidate | Pros | Cons | Decision |
|---|---|---|---|
| **rust-analyzer as a library (`ra_ap_hir`, etc.)** | Resolves types, traits, and even macro expansion | No backward-compatibility guarantee → version pinning needed; heavy build | **Main** (pinned version) |
| `rust-analyzer scip` | Definitions / references in a stable format | No type structure or data flow | Cross-reference helper |
| syn | Light and stable, the proc-macro standard | No type resolution, cannot see macro expansion | **Injector (proc-macro) and fast scans** |
| tree-sitter-rust | Fast | No types | Graphify already uses it → reuse Graphify's graph |

In GPUI, state gathers in `Entity<T>` model fields, so "find model types → field types → representative values" is deterministic with rust-analyzer type info. AST patterns are enough to find `cx.notify()` / `update` call sites.

### 5.3 Reuse vs build

| Item | Reuse | Build |
|---|---|---|
| Parsing | tree-sitter (MIT), Babel/SWC (MIT/Apache), syn | — |
| Type resolution | TS compiler API, ts-morph (MIT/Apache), rust-analyzer (MIT/Apache) | `TypeOracle` interface |
| Call / import graph | Graphify (Apache-2.0) first; routes and `navigates` edges: reference CodeGraph's approach (MIT) or use it | `CodeGraphPort` |
| Prop type extraction | Reference react-docgen-typescript (MIT) | — |
| **State-location detection, state → render data flow, event → state-change rules, representative values and combination generation, coverage denominator** | None (only GATOR WTG and React Compiler designs as references) | **dui's core = build ourselves** |
| AI | — | Only cells left as `inferred` above, and naming |

### 5.4 License fit (dui is Apache-2.0 open source; dependencies must allow commercial use — MIT, Apache, BSD, ISC — so users and companies can use dui commercially)

| Target | Decision |
|---|---|
| Graphify, CodeGraph, Axon, Descry, SCIP, scip-typescript, Aider, Continue, stack-graphs, Joern, Kythe, tree-sitter, ts-morph, react-docgen-typescript, React, rust-analyzer, syn, XState | Allowed |
| GitNexus (PolyForm Noncommercial) | Forbidden |
| Serena app (GPL-3.0+) | Forbidden. Only the SolidLSP part (MIT) is allowed |
| Semgrep engine, FlowDroid (LGPL-2.1) | Not used (not strong copyleft under the policy, but the cost of reviewing static-linking / distribution terms → avoided) |
| CodeQL CLI | Forbidden (private-code analysis is under paid terms) |
| CodeGraph | Allowed, but sends anonymous usage statistics by default (`TELEMETRY.md`) → default `CODEGRAPH_TELEMETRY=0` if embedded |

## 6. Conclusion (recommended stack)

Decided 2026-11-19 (details: [design: Tech stack](design-v0.md#tech-stack-decided-2026-11-19)):

- **Core**: Rust single binary (CLI, MCP via `rmcp`, analyzer, combination rules). Code graph built in-house on tree-sitter + oxc; Graphify and CodeGraph are reference only.
- **TS**: tree-sitter (fast scan) → oxc (parse, semantics) → **types from the project's official TypeScript compiler via `TypeOracle`** (TS 7.0+ only; TS 7.1 `typescript/sync` through a small Node sidecar) → dui's own data flow (React Compiler style) → oxc transform (injection).
- **Rust**: tree-sitter scan → **rust-analyzer `ra_ap_*` (pinned) for types** → syn proc-macro (injection).
- **Rendering**: Chromium over CDP from Rust (chromiumoxide); network mocks through the CDP Fetch domain.
- AI only fills `inferred` cells and names things. Combinations and coverage use deterministic algorithms.
- Open checks: whether React Compiler internals can be reused, whether the TS 7.1 API offers enough type-checker capability, a Node-free way to query TS 7 from Rust (`unconfirmed`), the cost of pinning `ra_ap_hir`.
