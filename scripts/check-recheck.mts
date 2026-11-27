import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const bin = resolve(import.meta.dirname, "../target/debug/dui");
const temp = mkdtempSync(join(tmpdir(), "dui-recheck-"));
const project = join(temp, "site");
mkdirSync(project);

async function dui(args: string[], code = 0, input?: string) {
  const child = spawn(bin, args, {
    cwd: project,
    env: { ...process.env, HOME: temp },
    stdio: [input === undefined ? "ignore" : "pipe", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  child.stdout!.on("data", (chunk) => (stdout += chunk));
  child.stderr!.on("data", (chunk) => (stderr += chunk));
  const exit = await new Promise<number | null>((done, reject) => {
    child.on("error", reject);
    child.on("close", done);
    if (input !== undefined) {
      child.stdin?.on("error", reject);
      child.stdin?.end(input);
    }
  });
  assert.equal(exit, code, stderr + stdout.slice(0, 1000));
  return JSON.parse(stdout);
}

const visits: string[] = [];
let fixed = false;
const server = createServer((req, res) => {
  const url = new URL(req.url ?? "/", "http://localhost");
  if (url.pathname === "/visited") {
    visits.push(url.searchParams.get("cell")!);
    res.writeHead(204).end();
    return;
  }
  const hide = !fixed && url.pathname === "/details" && req.headers["accept-language"] === "ko";
  res.writeHead(200, { "Content-Type": "text/html", "Cache-Control": "no-store" });
  res.end(`<!doctype html><meta charset="utf-8"><title>Recheck fixture</title>
    <style>${hide ? "@media(max-width: 500px){h1{display:none}}" : ""}</style>
    <h1>Ready</h1><a href="/details">Details</a>
    <script>new Image().src = "/visited?cell=" + encodeURIComponent(
      [location.pathname, innerWidth, navigator.language].join(" "));</script>`);
});
try {
  await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
  const address = server.address();
  assert.ok(address && typeof address === "object");
  const manifest = {
    host: "url",
    url: `http://127.0.0.1:${address.port}`,
    widths: [375, 1280],
    locales: ["en", "ko"],
    routeSelector: "a[href]",
    readySelector: "h1",
  };
  writeFileSync(join(project, "dui.json"), JSON.stringify(manifest));
  const original = await dui(["run"]);
  assert.equal(original.cells.length, 8);
  const originalFile = join(original.output, "report.json");
  const before = readFileSync(originalFile);
  const failed = original.cells.filter(
    (cell: { observation: { screenEvidence: { screenStatus: string } } }) =>
      cell.observation.screenEvidence.screenStatus === "unverified",
  );
  assert.equal(failed.length, 1);
  assert.equal(failed[0].status, "verified");
  assert.equal(failed[0].locale, "ko");
  assert.equal(failed[0].width, 375);
  visits.length = 0;

  const remaining = await dui(["recheck", original.output], 5);
  assert.deepEqual(visits, ["/details 375 ko"]);
  assert.deepEqual(
    [remaining.recheck.selected, remaining.recheck.skipped, remaining.recheck.remaining],
    [1, 7, 1],
  );
  assert.equal(remaining.cells[0].state, failed[0].state);
  assert.equal(remaining.cells[0].observation.screenEvidence.selectorVisible, false);
  assert.notEqual(remaining.output, original.output);
  visits.length = 0;

  // Removing a configured ancestor must never verify its linked state under another scenario.
  writeFileSync(join(project, "dui.json"), JSON.stringify({ ...manifest, path: "/other-entry" }));
  const missing = await dui(["recheck", remaining.output], 5);
  assert.equal(missing.recheck.remaining, 1);
  assert.equal(missing.cells[0].reason, "target-not-produced");
  assert.equal(missing.coverage.planned, 1);
  assert.equal(missing.coverage.captured, 0);
  assert.equal(missing.coverage.capturedRenderable, 0);
  assert.equal(missing.coverage.verified, 0);
  assert.equal(missing.coverage.attempted, 0);
  assert.deepEqual(visits, []);
  writeFileSync(join(project, "dui.json"), JSON.stringify(manifest));
  fixed = true;
  const rpc = await dui(
    ["mcp"],
    0,
    JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "tools/call",
      params: { name: "recheck", arguments: { run: missing.output, project } },
    }) + "\n",
  );
  assert.equal(rpc.result.isError, false);
  const resolved = JSON.parse(rpc.result.content[0].text);
  assert.equal(resolved.recheck.resolved, 1);
  assert.equal(resolved.recheck.remaining, 0);
  assert.equal(resolved.recheck.results[0].previousStatus, "unverified");
  assert.equal(resolved.recheck.results[0].screenStatus, "verified");
  assert.equal(resolved.cells.length, 1);
  assert.deepEqual(visits, ["/details 375 ko"]);
  assert.deepEqual(readFileSync(originalFile), before);
  visits.length = 0;

  const runs = resolve(resolved.output, "..");
  const archives = readdirSync(runs);
  const noop = await dui(["recheck", resolved.output, join(temp, "nonexistent-project")]);
  assert.equal(noop.recheck.selected, 0);
  assert.equal(noop.recheck.skipped, 1);
  assert.equal(noop.output, undefined);
  assert.deepEqual(readdirSync(runs), archives);
  assert.deepEqual(visits, []);
  console.log(
    "8 captured; exactly 1 browser visit per retry; 7 skipped; 1 resolved; no-op created no run.",
  );
} finally {
  server.closeAllConnections();
  server.close();
  rmSync(temp, { recursive: true, force: true });
}
