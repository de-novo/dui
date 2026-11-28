// `dui map` starts no browser by default (AGENTS.md#never, #368): with no Chromium available it
// still maps a static build and a running URL from the HTML their servers send, with screens,
// edges and the limitation that links only scripts add are missing.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const binary = resolve(process.env.DUI_BIN ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-browserless-"));

type Report = {
  browser: unknown;
  output: string;
  coverage: {
    planned: number;
    captured: number;
    verified: number;
    capturedRenderable: number;
    renderable: number;
    attempted: number;
  };
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
  assert.equal(report.coverage.planned, report.cells.length, `${label}: planned cells`);
  assert.equal(report.coverage.captured, 0, `${label}: no captured cells`);
  assert.equal(report.coverage.attempted, 0, `${label}: legacy attempted count stays zero`);
  assert.equal(
    report.coverage.capturedRenderable,
    0,
    `${label}: no captured renderable denominator`,
  );
  assert.equal(
    report.coverage.renderable,
    report.cells.length,
    `${label}: planned renderable compatibility count`,
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
    assert.equal(graph.sourceModel, "source-model.json");
    assert.equal(graph.nodes[0].name, "Orders");
    assert.equal(graph.nodes[0].component, "src/Orders.tsx#Orders");
    assert.equal(graph.nodes[0].location.file, "src/App.tsx");
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
  // Regression for the Never rule: a default Next map neither resolves/starts `next dev` nor
  // launches Chromium, even when both executables are explicitly configured.
  const nextApp = join(stage, "next-app");
  mkdirSync(join(nextApp, "app", "pricing"), { recursive: true });
  mkdirSync(join(nextApp, "app", "orders", "[id]"), { recursive: true });
  writeFileSync(
    join(nextApp, "package.json"),
    JSON.stringify({ name: "next-app", private: true, dependencies: { next: "16.4.6" } }),
  );
  writeFileSync(join(nextApp, "app", "page.tsx"), "export default function Home(){return <main/>}");
  writeFileSync(
    join(nextApp, "app", "pricing", "page.tsx"),
    "export default function Pricing(){return <main/>}",
  );
  writeFileSync(
    join(nextApp, "app", "orders", "[id]", "page.tsx"),
    "export default function Order(){return <main/>}",
  );
  const fakebin = join(stage, "fakebin");
  mkdirSync(fakebin);
  const nextLog = join(stage, "next-launched.log");
  const chromeLog = join(stage, "chrome-launched.log");
  writeFileSync(
    join(fakebin, "next"),
    `#!/usr/bin/env node
require("node:fs").appendFileSync(process.env.DUI_FAKE_NEXT_LOG, process.argv.slice(2).join(" ") + "\n");
if (process.argv.includes("dev")) {
  require("node:http").createServer((_req, res) => res.end("<html><body>fake</body></html>"))
    .listen(Number(process.env.PORT), "127.0.0.1");
}
`,
  );
  writeFileSync(
    join(fakebin, "chrome"),
    `#!/bin/sh
echo called >> "$DUI_FAKE_CHROME_LOG"
exit 1
`,
  );
  chmodSync(join(fakebin, "next"), 0o755);
  chmodSync(join(fakebin, "chrome"), 0o755);
  const nextReport = JSON.parse(
    execFileSync(binary, ["map", nextApp], {
      encoding: "utf8",
      env: {
        ...process.env,
        HOME: join(stage, "next-home"),
        DUI_CHROMIUM: join(fakebin, "chrome"),
        DUI_NEXT_BIN: join(fakebin, "next"),
        DUI_FAKE_NEXT_LOG: nextLog,
        DUI_FAKE_CHROME_LOG: chromeLog,
      },
    }),
  ) as Report & { nextAppRoutes: unknown[] };
  assert.equal(nextReport.browser, null);
  assert.equal(nextReport.nextAppRoutes.length, 3);
  assert.ok(nextReport.cells.length >= 4, "static Next routes are planned");
  assert.ok(
    nextReport.cells.every(
      (c) => c.status === "unverified" && c.reason === "not-rendered" && !c.image,
    ),
    "Next cells are planned without evidence",
  );
  assert.equal(existsSync(nextLog), false, "default Next map must not invoke next");
  assert.equal(existsSync(chromeLog), false, "default Next map must not invoke Chromium");

  // A host command is the app's dev server. It is explicit capture infrastructure, not input to
  // the default lightweight map.
  const commandApp = join(stage, "command-app");
  mkdirSync(commandApp);
  const commandLog = join(stage, "command-started.log");
  writeFileSync(
    join(commandApp, "server.mjs"),
    `import fs from "node:fs";
fs.writeFileSync(process.env.DUI_COMMAND_LOG, "started");
`,
  );
  writeFileSync(
    join(commandApp, "dui.json"),
    JSON.stringify({
      host: "command",
      command: [process.execPath, "server.mjs"],
      env: { DUI_COMMAND_LOG: commandLog },
      path: "/",
      widths: [375],
    }),
  );
  const commandReport = JSON.parse(
    execFileSync(binary, ["map", commandApp], {
      encoding: "utf8",
      env: {
        ...process.env,
        HOME: join(stage, "command-home"),
        DUI_CHROMIUM: join(stage, "no-browser"),
      },
    }),
  ) as Report;
  assert.equal(commandReport.browser, null);
  assert.deepEqual(
    commandReport.cells.map((c) => [c.path, c.status, c.reason]),
    [["/", "unverified", "not-rendered"]],
  );
  assert.equal(
    existsSync(commandLog),
    false,
    "default command-host map must not start the command",
  );

  const filters = join(stage, "filters");
  mkdirSync(filters);
  writeFileSync(
    join(filters, "Links.tsx"),
    `import {Component} from "react";
class Menu extends Component { render(){return <nav><a href="#/one">{this.props.title}</a><a href="#/two">Two</a></nav>} }
export {Menu as Links};`,
  );
  const classEntry = `import * as UI from "react"; import {Links} from "./Links";
declare var Router;
export class Filter extends UI.Component {
  componentDidMount(){const routes=Router({'/one': selectOne, '/two': selectTwo}); routes.init('/one');}
  render(){let menu; if(this.props.enabled){menu=<Links title="One"/>;} return <main>{menu}</main>;}
}`;
  function extractFilters(source: string) {
    writeFileSync(join(filters, "App.tsx"), source);
    return JSON.parse(
      execFileSync(binary, ["map", filters, "--entry", "App.tsx"], {
        encoding: "utf8",
        env: {
          ...process.env,
          HOME: join(stage, "filter-home"),
          PATH: "",
          DUI_CHROMIUM: "/absent",
        },
      }),
    ).screenDiscovery;
  }
  assert.equal(extractFilters(classEntry).declared, 2);
  assert.equal(extractFilters(classEntry).resolvedRelations, 4);
  for (const source of [
    classEntry.replace("declare var Router;", "function Router(x){return x}"),
    classEntry.replace("routes.init('/one');", ""),
    classEntry.replace("'/two': selectTwo", "'/unknown': selectTwo"),
    classEntry.replace("return <main>{menu}", "menu = null; return <main>{menu}"),
  ]) {
    assert.equal(extractFilters(source).resolvedRelations, 0);
  }
  const built = await map([site, "--build-dir", site], "home-build");
  check("--build-dir", built.report);
  const audit = JSON.parse(
    execFileSync(binary, ["audit", built.report.output], {
      encoding: "utf8",
      env: { ...process.env, HOME: join(stage, "home-build") },
    }),
  ) as {
    meetsGoal: boolean;
    summary: {
      plannedCells: number;
      capturedCells: number;
      capturedRenderableCells: number;
      notCapturedCells: number;
      unverifiedCells: number;
    };
    gaps: unknown[];
    notCaptured: { status: string; reason: string }[];
  };
  assert.equal(audit.meetsGoal, false);
  assert.equal(audit.summary.plannedCells, built.report.cells.length);
  assert.equal(audit.summary.capturedCells, 0);
  assert.equal(audit.summary.capturedRenderableCells, 0);
  assert.equal(audit.summary.notCapturedCells, built.report.cells.length);
  assert.equal(audit.summary.unverifiedCells, 0);
  assert.equal(audit.gaps.length, 0);
  assert.equal(audit.notCaptured.length, built.report.cells.length);
  assert.ok(
    audit.notCaptured.every((c) => c.status === "not-captured" && c.reason === "not-rendered"),
  );
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
