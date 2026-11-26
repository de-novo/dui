import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { withChromiumPage } from "./chromium-page.mts";

const repoRoot = process.cwd();
const duiBin = path.join(repoRoot, "target", "debug", "dui");
assert.ok(fs.existsSync(duiBin), `missing built dui binary at ${duiBin}`);

interface ReportCell {
  path: string;
  width: number;
  status: string;
  reason?: string | null;
}

interface RunReport {
  coverage: { verified: number; renderable: number };
  cells: ReportCell[];
}

function runDuiRaw(args: string[], homeDir: string): { status: number | null; json: unknown } {
  const res = spawnSync(duiBin, args, {
    encoding: "utf8",
    env: { ...process.env, HOME: homeDir },
  });
  return {
    status: res.status,
    json: res.stdout.trim() ? (JSON.parse(res.stdout) as unknown) : null,
  };
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-reliability-"));
const duiHome = path.join(tmp, ".dui-home");

try {
  const appDir = path.join(tmp, "server-app");
  fs.mkdirSync(appDir, { recursive: true });
  fs.writeFileSync(
    path.join(appDir, "serve.mjs"),
    `import http from "node:http";
const port = Number(process.env.PORT);
http.createServer((req, res) => {
  if (req.url === "/not-found") {
    res.writeHead(404, { "content-type": "text/html; charset=utf-8" });
    res.end("<html><body><h1>404 Not Found</h1></body></html>");
    return;
  }
  if (req.url === "/api/me") {
    const ok = req.headers.authorization === "Bearer ok";
    res.writeHead(ok ? 200 : 401, { "content-type": "application/json" });
    res.end(JSON.stringify(ok ? { name: "Ada" } : { error: "unauthorized" }));
    return;
  }
  if (req.url === "/account") {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(\`<html><body><h1>Account</h1><p id="who">loading</p><script>
      fetch("/api/me", { headers: { authorization: "Bearer " + (localStorage.token || "") } })
        .then((r) => r.json()).then((me) => { document.getElementById("who").textContent = me.name || me.error; });
    </script></body></html>\`);
    return;
  }
  if (req.url === "/clock") {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(\`<html><body><style>.spin{animation:spin 1s linear infinite}@keyframes spin{to{transform:rotate(360deg)}}</style>
      <h1>Orders</h1><p>Updated <span id="clock">0</span></p>
      <svg class="spin" width="40" height="40" aria-label="Loading"><rect width="40" height="12"/></svg>
      <script>setInterval(() => { clock.textContent = String(Date.now()); }, 50);</script></body></html>\`);
    return;
  }
  if (req.url === "/feed") {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(\`<html><body><h1>Feed</h1><ul id="feed"></ul><script>
      let n = 0; setInterval(() => { const li = document.createElement("li"); li.textContent = "item " + n++; feed.append(li); }, 150);
    </script></body></html>\`);
    return;
  }
  if (req.url === "/moved") {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(\`<html><body><script>history.replaceState(null, "", "/landing");</script><h1>Landing</h1></body></html>\`);
    return;
  }
  if (req.url === "/shell") {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(\`<html><body><nav>Menu</nav><ul id="log"></ul><script>
      let n = 0; setInterval(() => { const li = document.createElement("li"); li.textContent = "tick " + n++; log.append(li); }, 150);
    </script></body></html>\`);
    return;
  }
  if (req.url === "/server-error") {
    res.writeHead(500, { "content-type": "text/html; charset=utf-8" });
    res.end("<html><body><h1>500 Error</h1></body></html>");
    return;
  }
  res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
  res.end(\`<html><body style="margin:0;padding:16px">
    <h1>OK Page</h1>
    <button id="tiny" style="width:18px;height:18px;padding:0"></button>
  </body></html>\`);
}).listen(port, "127.0.0.1");
`,
  );

  // 1. 404 & 500 pages MUST be marked unverified with reason "http-error", while 200 OK passes.
  const errManifest = path.join(appDir, "dui-errors.json");
  fs.writeFileSync(
    errManifest,
    JSON.stringify({
      host: "command",
      command: [process.execPath, "serve.mjs"],
      path: "/ok",
      widths: [375],
      scenarios: [{ path: "/not-found" }, { path: "/server-error" }],
    }),
  );
  const errOut = path.join(tmp, "out-errors");
  const errRun = runDuiRaw(["run", appDir, errManifest, errOut], duiHome);
  assert.equal(errRun.status, 5, "run with 404/500 pages must not exit 0");

  const errReport = JSON.parse(
    fs.readFileSync(path.join(errOut, "report.json"), "utf8"),
  ) as RunReport;
  const okCell = errReport.cells.find((c) => c.path === "/ok");
  const notFoundCell = errReport.cells.find((c) => c.path === "/not-found");
  const serverErrCell = errReport.cells.find((c) => c.path === "/server-error");
  assert.equal(okCell?.status, "verified", "/ok should be verified");
  assert.equal(notFoundCell?.status, "unverified", "/not-found must be unverified");
  assert.equal(notFoundCell?.reason, "http-error", "/not-found reason must be http-error");
  assert.equal(serverErrCell?.status, "unverified", "/server-error must be unverified");
  assert.equal(serverErrCell?.reason, "http-error", "/server-error reason must be http-error");

  // 1b. A screen whose own API call returns 401 is not verified; a valid session, or an
  // endpoint the manifest expects to fail, is.
  const account = (extra: Record<string, unknown>, name: string) => {
    const manifest = path.join(appDir, `dui-${name}.json`);
    fs.writeFileSync(
      manifest,
      JSON.stringify({
        host: "command",
        command: [process.execPath, "serve.mjs"],
        path: "/account",
        widths: [375],
        readySelector: "h1",
        ...extra,
      }),
    );
    const out = path.join(tmp, `out-${name}`);
    const run = runDuiRaw(["run", appDir, manifest, out], duiHome);
    const [cell] = JSON.parse(fs.readFileSync(path.join(out, "report.json"), "utf8")).cells;
    return { status: run.status, cell };
  };
  const signedOut = account({}, "signed-out");
  assert.equal(signedOut.status, 5, "a 401 API response must not exit 0");
  assert.equal(signedOut.cell.status, "unverified");
  assert.equal(signedOut.cell.reason, "api-error: 401 /api/me");
  assert.deepEqual(signedOut.cell.observation.apiErrors, { "/api/me": 401 });
  const signedIn = account({ localStorage: { token: "ok" } }, "signed-in");
  assert.equal(signedIn.cell.status, "verified", JSON.stringify(signedIn.cell.reason));
  assert.equal(signedIn.status, 0);
  const expected = account({ expectedApiErrors: ["/api/*"] }, "expected-401");
  assert.equal(expected.cell.status, "verified", JSON.stringify(expected.cell.reason));

  // 1c. A ticking clock and a spinner do not hold a screen back; a feed that keeps growing
  // ends unverified at the settle budget and names the node that kept changing.
  const clockStarted = Date.now();
  const clock = account({ path: "/clock", settleTimeoutMs: 8000 }, "clock");
  const clockMs = Date.now() - clockStarted;
  assert.equal(clock.cell.status, "verified", JSON.stringify(clock.cell.reason));
  assert.equal(clock.cell.observation.settled, true);
  assert.deepEqual(
    clock.cell.observation.ignoredChanges.map((c: { node: string }) => c.node).sort(),
    ["#clock", "svg#0"],
  );
  assert.ok(clockMs < 8000, `clock page settled before the budget (${clockMs} ms)`);
  const feed = account({ path: "/feed", settleTimeoutMs: 1500 }, "feed");
  assert.equal(feed.status, 5);
  assert.equal(feed.cell.status, "unverified");
  assert.match(feed.cell.reason, /^unsettled: li#\d+ added/);
  assert.equal(feed.cell.observation.settled, false);

  // 1d. An unverified cell names what failed: a redirect with both routes, and a selector that
  // never matched on a page that also kept changing.
  const moved = account({ path: "/moved" }, "moved");
  assert.equal(moved.cell.reason, "redirected: /moved → /landing");
  const shell = account({ path: "/shell", settleTimeoutMs: 1500 }, "shell");
  assert.match(shell.cell.reason, /^not-ready: h1 \+ unsettled: li#\d+ added/);

  // 1e. SIGTERM during a run removes its staging directory and exits 128 + 15.
  const interruptHome = path.join(tmp, ".dui-home-interrupt");
  const feedManifest = path.join(appDir, "dui-interrupt.json");
  fs.writeFileSync(
    feedManifest,
    JSON.stringify({
      host: "command",
      command: [process.execPath, "serve.mjs"],
      path: "/feed",
      widths: [375, 1280],
      readySelector: "h1",
      settleTimeoutMs: 30000,
    }),
  );
  const pending = () =>
    fs
      .globSync(path.join(interruptHome, ".dui/projects/*/runs/.pending-*"))
      .filter((p) => fs.statSync(p).isDirectory());
  const running = spawn(duiBin, ["run", appDir, feedManifest, path.join(tmp, "out-interrupt")], {
    env: { ...process.env, HOME: interruptHome, DUI_CONCURRENCY: "2" },
    stdio: "ignore",
  });
  const exited = new Promise<number | null>((done) => running.on("exit", (code) => done(code)));
  for (let i = 0; i < 200 && pending().length === 0; i++) {
    await new Promise((r) => setTimeout(r, 100));
  }
  assert.equal(pending().length, 1, "the run staged its output");
  await new Promise((r) => setTimeout(r, 1500));
  running.kill("SIGTERM");
  assert.equal(await exited, 143);
  assert.deepEqual(pending(), [], "the interrupted run left no staging directory");

  // 2. Valid 200 scenarios pass audit cleanly with 100% verified cells.
  const okManifest = path.join(appDir, "dui-ok.json");
  fs.writeFileSync(
    okManifest,
    JSON.stringify({
      host: "command",
      command: [process.execPath, "serve.mjs"],
      path: "/ok",
      widths: [375],
      scenarios: [
        { path: "/ok", queryParams: { tab: "a" } },
        { path: "/ok", queryParams: { tab: "b" } },
      ],
    }),
  );
  const okOut = path.join(tmp, "out-ok");
  const okRun = runDuiRaw(["run", appDir, okManifest, okOut], duiHome);
  assert.equal(okRun.status, 0, "valid 200 scenarios exit 0");

  const auditRes = runDuiRaw(["audit", okOut], duiHome);
  assert.equal(auditRes.status, 0);

  // On a tall page a classic scrollbar can wrap a row that would fit with hidden scrollbars.
  // Sample the actual PNG at recorded control centers, including one below the viewport.
  const tall = path.join(tmp, "tall");
  fs.mkdirSync(tall);
  fs.writeFileSync(
    path.join(tall, "index.html"),
    `<!doctype html>
    <style>
      body { margin: 0 } header { display: flex; flex-wrap: wrap }
      button { flex: none; width: 50vw; height: 48px; border: 0; background: #cc3344 }
      button + button { background: #44aa55 } main { height: 180vh } footer button { background: #3355cc }
    </style>
    <header><button aria-label="First"></button><button aria-label="Second"></button></header>
    <main></main><footer><button aria-label="Last"></button></footer>`,
  );
  const manifest = path.join(tall, "dui.json");
  fs.writeFileSync(
    manifest,
    JSON.stringify({ host: "static", path: "/", widths: [600], readySelector: "button" }),
  );
  const output = path.join(tmp, "out-tall");
  assert.equal(runDuiRaw(["run", tall, manifest, output], duiHome).status, 0);
  const [capture] = JSON.parse(fs.readFileSync(path.join(output, "report.json"), "utf8")).cells;
  const png = fs.readFileSync(path.join(output, capture.image)).toString("base64");
  type SampleNode = {
    tag: string;
    rect: { x: number; y: number; width: number; height: number };
    style: { backgroundColor: string };
  };
  const samples = (capture.observation.renderTree as SampleNode[])
    .filter((node) => node.tag === "button")
    .map(({ rect, style }) => ({
      x: Math.floor(rect.x + rect.width / 2),
      y: Math.floor(rect.y + rect.height / 2),
      color: style.backgroundColor,
    }));
  assert.equal(samples.length, 3);
  await withChromiumPage(async (page) => {
    const result = await page.call("Runtime.evaluate", {
      awaitPromise: true,
      returnByValue: true,
      expression: `(async () => {
      const img = new Image(); img.src = "data:image/png;base64,${png}"; await img.decode();
      const canvas = document.createElement("canvas"); canvas.width = img.width; canvas.height = img.height;
      const ctx = canvas.getContext("2d"); ctx.drawImage(img, 0, 0);
      return ${JSON.stringify(samples)}.map(({x, y}) => {
        const [r, g, b] = ctx.getImageData(x, y, 1, 1).data; return "rgb(" + [r, g, b].join(", ") + ")";
      });
    })()`,
    });
    assert.deepEqual(
      (result.result as { value: string[] }).value,
      samples.map((s) => s.color),
      "Full-page PNG paint must match recorded geometry",
    );
  });
  console.log(
    JSON.stringify({
      status: "ok",
      notFoundReason: notFoundCell.reason,
      serverErrorReason: serverErrCell.reason,
    }),
  );
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
