import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

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
