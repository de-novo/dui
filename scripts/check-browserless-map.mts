// `dui map` starts no browser by default (AGENTS.md#never, #368): with no Chromium available it
// still maps a static build and a running URL from the HTML their servers send, with screens,
// edges and the limitation that links only scripts add are missing.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const binary = resolve(process.env.DUI_BIN ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-browserless-"));

type Report = {
  browser: unknown;
  output: string;
  cells: { path: string; status: string; reason?: string; image?: string }[];
  discoveredRoutes: string[];
  screenDiscovery: { limitations?: string[] };
};

const site = join(stage, "site");
mkdirSync(join(site, "docs"), { recursive: true });
const page = (file: string, title: string, body: string) =>
  writeFileSync(
    join(site, file),
    `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>${title}</title></head><body>${body}</body></html>`,
  );
page(
  "index.html",
  "Home",
  `<nav><a href="/">Home</a> <a href="docs/">Docs</a> <a href="/pricing.html?plan=team">Pricing</a>
<a href="https://example.com/out">Out</a> <a href="/manual.pdf">PDF</a></nav>
<script>const a = document.createElement("a"); a.href = "/from-script.html"; document.body.append(a);</script>`,
);
page("docs/index.html", "Docs", `<a href="../">Home</a> <a href="guide.html">Guide</a>`);
page("docs/guide.html", "Guide", `<a href="/docs/">Back</a>`);
page("pricing.html", "Pricing", `<a href="/">Home</a>`);
page("from-script.html", "Only scripts reach this", "");

// DUI_CHROMIUM replaces the browser search: no browser can start.
const map = async (args: string[], home: string) => {
  const started = Date.now();
  // Asynchronous: the --url server below runs in this process.
  const child = spawn(binary, ["map", ...args], {
    env: { ...process.env, HOME: join(stage, home), DUI_CHROMIUM: join(stage, "no-browser") },
  });
  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (d) => (stdout += d));
  child.stderr.on("data", (d) => (stderr += d));
  const status = await new Promise<number | null>((done) => child.on("close", done));
  assert.equal(status, 0, stderr.slice(-2000) + stdout.slice(0, 1500));
  assert.match(stderr, /planned without a browser/);
  return { report: JSON.parse(stdout) as Report, ms: Date.now() - started };
};

const check = (label: string, report: Report) => {
  assert.equal(report.browser, null, `${label}: no browser`);
  assert.deepEqual(
    [...report.discoveredRoutes].sort(),
    ["/docs/", "/docs/guide.html", "/pricing.html?plan=team"],
    `${label}: routes from the served HTML`,
  );
  assert.ok(
    report.cells.every((c) => c.status === "unverified" && c.reason === "not-rendered" && !c.image),
    `${label}: cells are planned, not captured`,
  );
  assert.deepEqual([...new Set(report.cells.map((c) => c.path))].sort(), [
    "/",
    "/docs/",
    "/docs/guide.html",
    "/pricing.html?plan=team",
  ]);
  assert.ok(
    report.screenDiscovery.limitations?.some((l) => /only scripts add/.test(l)),
    `${label}: the limitation is stated`,
  );
  const graph = JSON.parse(readFileSync(join(report.output, "graph.json"), "utf8")) as {
    nodes: { kind: string }[];
    edges: unknown[];
  };
  assert.ok(graph.nodes.filter((n) => n.kind === "screen").length >= 4, `${label}: screens`);
  assert.ok(graph.edges.length >= 4, `${label}: edges ${graph.edges.length}`);
};

const server = createServer((req, res) => {
  const path = new URL(req.url ?? "/", "http://x").pathname;
  const file = join(site, path.endsWith("/") ? path + "index.html" : path);
  try {
    const body = readFileSync(file);
    res.writeHead(200, { "content-type": extname(file) === ".html" ? "text/html" : "text/plain" });
    res.end(body);
  } catch {
    res.writeHead(404).end();
  }
});
try {
  const project = join(root, "fixtures/source-model");
  for (const entry of [[], ["--entry", "src/App.tsx"]]) {
    const report = JSON.parse(
      execFileSync(binary, ["map", project, ...entry], {
        encoding: "utf8",
        env: { ...process.env, HOME: join(stage, "tsx-home"), PATH: "", DUI_CHROMIUM: "/absent" },
      }),
    );
    const artifact = (file: string) => JSON.parse(readFileSync(join(report.output, file), "utf8"));
    assert.equal(report.screenDiscovery.kind, "tsx-source");
    assert.equal(report.browser, null);
    assert.deepEqual(report.cells, []);
    assert.deepEqual(
      artifact("source-model.json"),
      JSON.parse(execFileSync(binary, ["model", project, "src/App.tsx"], { encoding: "utf8" })),
    );
    const graph = artifact("graph.json");
    assert.deepEqual(
      graph.nodes.map((n: { label: string }) => n.label),
      ["/", "/settings"],
    );
    assert.deepEqual(
      graph.edges.map((e: { source: string; target: string }) => [e.source, e.target]),
      [
        ["route:/", "route:/settings"],
        ["route:/settings", "route:/"],
      ],
    );
  }
  const dynamic = join(stage, "dynamic");
  mkdirSync(dynamic);
  writeFileSync(
    join(dynamic, "App.tsx"),
    [
      'import { Link, Route } from "react-router";',
      "export function App(){return <><Route path={runtimePath} element={<App/>}/><Link to={runtimeTarget}>Open</Link></>}",
    ].join("\n"),
  );
  const dynamicReport = JSON.parse(
    execFileSync(binary, ["map", dynamic], {
      encoding: "utf8",
      env: { ...process.env, HOME: join(stage, "dynamic-home"), PATH: "", DUI_CHROMIUM: "/absent" },
    }),
  );
  assert.equal(dynamicReport.screenDiscovery.resolvedRelations, 0);
  assert.equal(dynamicReport.screenDiscovery.unresolvedRelations.length, 1);
  const unresolvedRoute = dynamicReport.screenDiscovery.diagnostics.find(
    (d: { code: string }) => d.code === "unresolved-route",
  );
  assert.equal(unresolvedRoute.location.file, "App.tsx");
  assert.equal(unresolvedRoute.location.line, 2);
  const built = await map([site, "--build-dir", site], "home-build");
  check("--build-dir", built.report);
  const again = await map([site, "--build-dir", site], "home-build");
  check("--build-dir again", again.report);

  await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
  const port = (server.address() as { port: number }).port;
  const url = await map([stage, "--url", `http://127.0.0.1:${port}/`], "home-url");
  check("--url", url.report);
  console.log(
    JSON.stringify({ ok: true, buildDirMs: built.ms, buildDirAgainMs: again.ms, urlMs: url.ms }),
  );
} finally {
  server.close();
  rmSync(stage, { recursive: true, force: true });
}
