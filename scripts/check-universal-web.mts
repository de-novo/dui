import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

const repoRoot = process.cwd();
const duiBin = path.join(repoRoot, "target", "debug", "dui");
assert.ok(fs.existsSync(duiBin), `missing built dui binary at ${duiBin}`);

interface ReportCell {
  path: string;
  width: number;
  status: string;
  image?: string;
  state: string;
  observation?: {
    screenEvidence?: Record<string, unknown>;
    redirectedTo?: string;
    renderTree?: Array<{ id: string; tag: string; text: string }>;
  };
}

interface RunReport {
  coverage: {
    verified: number;
    renderable: number;
  };
  cells: ReportCell[];
}

interface GraphEdge {
  kind: string;
  source: string;
  target: string;
}

interface GraphView {
  edges?: GraphEdge[];
}

interface DiffCell {
  path: string;
  width: number;
  renderDiff?: {
    textChanged: number;
    changes: Array<{ kind: string; tag: string; beforeText?: string; afterText?: string }>;
  };
}

interface DiffReport {
  summary: { changed: number };
  screens: Array<{ path: string; cells: DiffCell[] }>;
}

async function runDui(args: string[], homeDir: string): Promise<unknown> {
  const { stdout } = await execFileAsync(duiBin, args, {
    encoding: "utf8",
    env: { ...process.env, HOME: homeDir },
  });
  return JSON.parse(stdout) as unknown;
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-universal-web-"));
const duiHome = path.join(tmp, ".dui-home");

try {
  // 1. host: "static" — plain multi-page HTML/CSS site (zero React/TS/Vite)
  const staticDir = path.join(tmp, "static-site");
  fs.mkdirSync(staticDir, { recursive: true });
  fs.writeFileSync(
    path.join(staticDir, "index.html"),
    `<!doctype html>
<html lang="en">
  <head><meta charset="utf-8" /><title>Home</title></head>
  <body style="font-family:sans-serif;padding:24px">
    <h1>Universal Home</h1>
    <nav>
      <a href="/public.JSON?v=2#raw">Manifest</a>
      <a href="/pricing.html">Pricing</a>
      <a href="/about.html">About</a>
      <button id="more-toggle" type="button" aria-expanded="false">More</button>
      <div id="more-menu" hidden></div>
    </nav>
    <main>
      <p>Plain HTML rendered with zero bundler.</p>
      <button type="button" aria-haspopup="dialog" onclick="document.querySelector('#lightbox').showModal()">View image</button>
      <dialog id="lightbox"><img alt="Preview" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='200' height='160'/%3E"><button type="button" aria-label="Close image preview">×</button></dialog>
      <button id="open-modal-btn" type="button" aria-haspopup="dialog">Create Item</button>
      <dialog id="create-dialog" aria-label="Create Item">
        <h2>Create Item</h2>
        <p>Modal content captured automatically.</p>
        <button id="close-modal-btn" type="button" aria-label="Close">Close</button>
      </dialog>
    </main>
    <script>
      const btn = document.getElementById("more-toggle");
      const menu = document.getElementById("more-menu");
      btn.addEventListener("click", () => {
        const open = btn.getAttribute("aria-expanded") === "true";
        btn.setAttribute("aria-expanded", open ? "false" : "true");
        menu.hidden = open;
        menu.innerHTML = open ? "" : '<a href="/more.html">More Info</a>';
      });
      const dlg = document.getElementById("create-dialog");
      document.getElementById("open-modal-btn").addEventListener("click", () => dlg.showModal());
      document.getElementById("close-modal-btn").addEventListener("click", () => dlg.close());
    </script>
  </body>
</html>`,
  );
  fs.writeFileSync(
    path.join(staticDir, "more.html"),
    `<!doctype html>
<html lang="en">
  <head><meta charset="utf-8" /><title>More</title></head>
  <body style="font-family:sans-serif;padding:24px">
    <h1>More Info</h1>
    <a href="/">Back Home</a>
  </body>
</html>`,
  );
  fs.writeFileSync(
    path.join(staticDir, "pricing.html"),
    `<!doctype html>
<html lang="en">
  <head><meta charset="utf-8" /><title>Pricing</title><link rel="canonical" href="/pricing.html"></head>
  <body style="font-family:sans-serif;padding:24px">
    <h1>Pricing Plans</h1>
    <a href="/">Back Home</a>
  </body>
</html>`,
  );
  fs.writeFileSync(
    path.join(staticDir, "about.html"),
    `<!doctype html>
<html lang="en">
  <head><meta charset="utf-8" /><title>About</title></head>
  <body style="font-family:sans-serif;padding:24px">
    <h1>About Us</h1>
    <a href="/">Back Home</a>
  </body>
</html>`,
  );

  const staticManifest = path.join(staticDir, "dui.json");
  fs.writeFileSync(
    staticManifest,
    JSON.stringify({
      host: "static",
      widths: [375, 1280],
      routeSelector: "a[href]",
      discoverOverlays: true,
    }),
  );

  const staticOut1 = path.join(tmp, "out-static-1");
  await runDui(["run", staticDir, staticManifest, staticOut1], duiHome);

  const report1 = JSON.parse(
    fs.readFileSync(path.join(staticOut1, "report.json"), "utf8"),
  ) as RunReport;
  assert.equal(
    report1.coverage.verified,
    10,
    `expected 10 verified static cells (incl. /more.html + /#overlay:create-item), got ${report1.coverage.verified}`,
  );
  const pricing = report1.cells.find((c) => c.path === "/pricing.html");
  assert.ok(pricing);
  const evidence = pricing.observation?.screenEvidence;
  assert.deepEqual(evidence, {
    screenStatus: "unverified",
    settled: true,
    canonicalPath: "/pricing.html",
    canonicalMatches: true,
    requestedPath: "/pricing.html",
    observedPath: "/pricing.html",
    documentStatus: 200,
    routeMatches: true,
    heading: "Pricing Plans",
    selector: "body",
    selectorMatched: true,
    selectorVisible: true,
    selectorSource: "default",
  });
  const inspected = (await runDui(
    [
      "inspect",
      staticOut1,
      JSON.stringify({
        state: pricing.state,
        width: pricing.width,
      }),
    ],
    duiHome,
  )) as { screenEvidence: unknown };
  assert.deepEqual(inspected.screenEvidence, evidence);
  const discoveredPaths = new Set(report1.cells.map((c) => c.path));
  assert.ok(discoveredPaths.has("/"), "static run includes /");
  assert.ok(discoveredPaths.has("/pricing.html"), "static run discovers /pricing.html");
  assert.ok(discoveredPaths.has("/about.html"), "static run discovers /about.html");
  assert.ok(
    discoveredPaths.has("/more.html"),
    "static run discovers /more.html via WAI-ARIA aria-expanded disclosure expansion",
  );
  assert.ok(
    discoveredPaths.has("/#overlay:create-item"),
    "static run auto-discovers and captures native <dialog> overlay child node",
  );
  for (const cell of report1.cells) {
    assert.ok(
      (cell.observation?.renderTree?.length ?? 0) > 0,
      `expected renderTree on ${cell.path}@${cell.width}`,
    );
  }
  const graph1 = JSON.parse(
    fs.readFileSync(path.join(staticOut1, "graph.json"), "utf8"),
  ) as GraphView;
  const linkEdges = (graph1.edges ?? []).filter((e) => e.kind === "linksTo");
  assert.ok(
    linkEdges.length >= 2,
    `expected linksTo edges on static site, got ${linkEdges.length}`,
  );

  // Edit index.html and confirm cache invalidation + element-level renderDiff
  fs.writeFileSync(
    path.join(staticDir, "index.html"),
    fs
      .readFileSync(path.join(staticDir, "index.html"), "utf8")
      .replace("Universal Home", "Universal Home v2"),
  );
  const staticOut2 = path.join(tmp, "out-static-2");
  await runDui(["run", staticDir, staticManifest, staticOut2], duiHome);
  const diff = (await runDui(["diff", staticOut1, staticOut2], duiHome)) as DiffReport;
  assert.ok(
    diff.summary.changed >= 1,
    `expected >=1 changed cell: ${JSON.stringify(diff.summary)}`,
  );
  const homeScreen = diff.screens.find((s) => s.path === "/");
  const homeChange = homeScreen?.cells.find((c) => c.renderDiff);
  assert.ok(homeChange?.renderDiff, "expected renderDiff on modified / screen");
  assert.ok(
    homeChange.renderDiff.changes.some(
      (ch) =>
        ch.tag === "h1" &&
        ch.beforeText === "Universal Home" &&
        ch.afterText === "Universal Home v2",
    ),
    "expected h1 text change in universal web renderDiff",
  );

  // 2. host: "url" — attach to a running local HTTP server
  const server = http.createServer((req, res) => {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    if (req.url === "/docs") {
      res.end("<html><body><h1>Live Docs</h1><a href='/'>Home</a></body></html>");
      return;
    }
    res.end(`<html><body><div id="app"></div><script>
      setTimeout(() => {
        history.replaceState(null, "", "/dashboard");
        document.querySelector("#app").innerHTML = "<h1>Live Server</h1><a href='/docs'>Docs</a><a href='/dashboard'>Current screen</a>";
      }, 900);
    </script></body></html>`);
  });
  await new Promise<void>((resolve) => {
    server.listen(0, "127.0.0.1", () => resolve());
  });
  const addr = server.address();
  assert.ok(addr && typeof addr === "object", "expected bound server address");

  try {
    const urlManifest = path.join(tmp, "url-dui.json");
    fs.writeFileSync(
      urlManifest,
      JSON.stringify({
        host: "url",
        url: `http://127.0.0.1:${addr.port}`,
        readySelector: "h1",
        widths: [375],
        routeSelector: "a[href]",
      }),
    );
    const urlOut = path.join(tmp, "out-url");
    await runDui(["run", tmp, urlManifest, urlOut], duiHome);
    const urlReport = JSON.parse(
      fs.readFileSync(path.join(urlOut, "report.json"), "utf8"),
    ) as RunReport;
    assert.equal(
      urlReport.cells.find((cell) => cell.path === "/")?.observation?.redirectedTo,
      "/dashboard",
    );
    const entryEvidence = urlReport.cells.find((c) => c.path === "/")?.observation?.screenEvidence;
    assert.equal(entryEvidence?.observedPath, "/dashboard");
    assert.equal(entryEvidence?.routeMatches, false);
    assert.equal(entryEvidence?.screenStatus, "verified");
    assert.equal(entryEvidence?.selectorSource, "project");
    assert.equal(entryEvidence?.selectorMatched, true);
    const urlGraph = JSON.parse(
      fs.readFileSync(path.join(urlOut, "graph.json"), "utf8"),
    ) as GraphView;
    assert.ok(
      urlGraph.edges?.some(
        (edge) =>
          edge.kind === "linksTo" && edge.source === "route:/" && edge.target === "route:/docs",
      ),
      "entry alias retains outbound edges",
    );
    assert.equal(
      urlReport.cells.length,
      2,
      "the discovered landing route already has entry evidence",
    );
    assert.equal(
      urlReport.coverage.verified,
      2,
      `expected 2 verified url cells, got ${urlReport.coverage.verified}`,
    );
  } finally {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }

  // 3. host: "command" — spawn an arbitrary local server command
  const cmdDir = path.join(tmp, "cmd-site");
  fs.mkdirSync(cmdDir, { recursive: true });
  fs.writeFileSync(
    path.join(cmdDir, "serve.mjs"),
    `import http from "node:http";
const port = Number(process.env.PORT);
http.createServer((_req, res) => {
  res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
  res.end("<html><body><h1>Command Server</h1><aside hidden></aside><p>Spawned on demand</p></body></html>");
}).listen(port, "127.0.0.1");
`,
  );
  const cmdManifest = path.join(cmdDir, "dui.json");
  fs.writeFileSync(
    cmdManifest,
    JSON.stringify({
      host: "command",
      command: [process.execPath, "serve.mjs"],
      readySelector: "aside",
      widths: [768],
    }),
  );
  const cmdOut = path.join(tmp, "out-cmd");
  await runDui(["run", cmdDir, cmdManifest, cmdOut], duiHome);
  const cmdReport = JSON.parse(
    fs.readFileSync(path.join(cmdOut, "report.json"), "utf8"),
  ) as RunReport;
  const hidden = cmdReport.cells[0]?.observation?.screenEvidence;
  assert.equal(hidden?.selectorMatched, true);
  assert.equal(hidden?.selectorVisible, false);
  assert.equal(hidden?.screenStatus, "unverified");
  assert.equal(
    cmdReport.coverage.verified,
    1,
    `expected 1 verified command cell, got ${cmdReport.coverage.verified}`,
  );

  console.log(
    JSON.stringify({
      status: "ok",
      staticVerified: report1.coverage.verified,
      staticLinks: linkEdges.length,
      renderDiffChanges: homeChange.renderDiff.changes.length,
      urlVerified: 2,
      commandVerified: cmdReport.coverage.verified,
    }),
  );
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
