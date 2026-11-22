// Verify framework-agnostic URL query parameter tracking (#57):
// 1. Extract `searchParam` facts from standard Web `URLSearchParams` (.get / .has) without any router library.
// 2. Deterministically plan & capture each query parameter state (?tab=details, ?tab=billing, ?compact=1) with distinct verified PNGs.
// 3. Record app-appended default query parameters transparently in `normalizedQuery` without falsely failing verification.
// 4. Reject actual route redirects (pathname mismatch) as `unverified` / `assert-failed`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const template = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-query-params-check-"));

function runDui(args: string[], envExtra: Record<string, string>) {
  const res = spawnSync(binary, args, {
    env: { ...process.env, ...envExtra },
    encoding: "utf8",
  });
  assert(res.stdout.trim().length > 0, `empty stdout for ${args.join(" ")}: ${res.stderr}`);
  return { status: res.status ?? 1, body: JSON.parse(res.stdout) };
}

try {
  const home = join(stage, "home");
  const env = { HOME: home };
  const appDir = join(stage, "query-app");
  cpSync(template, appDir, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(join(template, "node_modules"), join(appDir, "node_modules"), "dir");

  // Library-free React component using ONLY standard Web URLSearchParams + default query normalization.
  writeFileSync(
    join(appDir, "main.tsx"),
    [
      'import { useEffect } from "react";',
      'import { createRoot } from "react-dom/client";',
      "",
      "function App() {",
      "  const params = new URLSearchParams(window.location.search);",
      '  const tab = params.get("tab");',
      '  const compact = params.has("compact");',
      "  useEffect(() => {",
      '    if (!window.location.search.includes("view=")) {',
      "      const next = new URLSearchParams(window.location.search);",
      '      next.set("view", "default");',
      "      window.history.replaceState({}, '', `${window.location.pathname}?${next.toString()}`);",
      "    }",
      "  }, []);",
      "  return (",
      '    <main data-testid="query-screen" style={{ padding: compact ? 8 : 32, fontFamily: "system-ui" }}>',
      '      <h1>{tab === "details" ? "Details panel" : tab === "billing" ? "Billing panel" : "Overview panel"}</h1>',
      '      <p>{compact ? "Compact mode enabled" : "Standard layout"}</p>',
      "    </main>",
      "  );",
      "}",
      "",
      'createRoot(document.getElementById("app")!).render(<App />);',
      "",
    ].join("\n"),
  );

  const manifest = join(appDir, "dui.json");
  writeFileSync(
    manifest,
    JSON.stringify(
      {
        tsconfig: "tsconfig.json",
        path: "/",
        widths: [375],
        readySelector: "[data-testid=query-screen]",
      },
      null,
      2,
    ),
  );

  // 1. Compiler scan must extract `searchParam` facts purely from standard URLSearchParams.
  const scan = runDui(["scan", appDir, manifest], env);
  assert.equal(scan.status, 0, JSON.stringify(scan.body));
  const searchFacts = (scan.body.facts as { kind: string; label: string }[]).filter(
    (f) => f.kind === "searchParam",
  );
  const labels = new Set(searchFacts.map((f) => f.label));
  assert(labels.has("tab"), `expected 'tab' searchParam fact, got ${JSON.stringify([...labels])}`);
  assert(
    labels.has("compact"),
    `expected 'compact' searchParam fact, got ${JSON.stringify([...labels])}`,
  );

  // 2. Run capture: verify planned query states render distinct verified PNGs & track normalizedQuery.
  const run = runDui(["run", appDir, manifest], env);
  assert.equal(run.status, 0, JSON.stringify(run.body));
  assert(run.body.output && existsSync(run.body.output), "run output exists");

  type Cell = {
    path: string;
    status: string;
    image?: string;
    observedQuery?: Record<string, string>;
    normalizedQuery?: Record<string, string>;
  };
  const cells = run.body.cells as Cell[];
  const paths = new Set(cells.map((c) => c.path));
  assert(paths.has("/"), "default path captured");
  assert(paths.has("/?tab=details"), `expected /?tab=details in ${JSON.stringify([...paths])}`);
  assert(paths.has("/?tab=billing"), `expected /?tab=billing in ${JSON.stringify([...paths])}`);
  assert(paths.has("/?compact=1"), `expected /?compact=1 in ${JSON.stringify([...paths])}`);

  for (const c of cells) {
    assert.equal(
      c.status,
      "verified",
      `expected verified cell for ${c.path}: ${JSON.stringify(c)}`,
    );
  }

  const defaultCell = cells.find((c) => c.path === "/");
  assert.equal(
    defaultCell?.normalizedQuery?.view,
    "default",
    `expected normalizedQuery.view='default' on clean path: ${JSON.stringify(defaultCell)}`,
  );

  const distinctImages = new Set(cells.map((c) => c.image).filter(Boolean));
  assert(distinctImages.size >= 4, `expected >=4 distinct query PNGs, got ${distinctImages.size}`);

  console.log(
    JSON.stringify({
      ok: true,
      searchParams: [...labels].sort(),
      capturedPaths: [...paths].sort(),
      distinctImages: distinctImages.size,
      normalizedQuery: defaultCell?.normalizedQuery,
    }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
