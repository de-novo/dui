// Verify v0.0.6 99% coverage audit report across CLI, MCP, SQLite comments, and embedded viewer (#44):
// 1. Capture a fixture run and attach one open and one resolved review comment.
// 2. Run `dui audit <RUN_DIR>` and MCP `audit` and confirm identical JSON output.
// 3. Verify verified/renderable cell counts, missing branch metrics, gap reasons, and open comments attached to the screen.
// 4. Open `dui open --no-open` and verify `/api/audit` + Chrome renders the coverage audit summary pill and open comment badge.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

type AuditReport = {
  goalPercent: number;
  meetsGoal: boolean;
  summary: {
    totalCells: number;
    verifiedCells: number;
    renderableCells: number;
    unverifiedCells: number;
    unrenderableCells: number;
    verifiedPercent: number;
    openComments: number;
    resolvedComments: number;
  };
  gaps: { path: string; width: number; status: string; reason: string }[];
  screens: {
    path: string;
    openComments: number;
    unresolvedComments: { id: string; body: string }[];
  }[];
};

const root = resolve(import.meta.dirname, "..");
const template = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-audit-check-"));

function chromeExecutable(): string {
  const candidates = [
    process.env.DUI_CHROMIUM,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/chromium",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium-browser",
  ];
  for (const path of candidates) {
    if (path && existsSync(path)) return path;
  }
  throw new Error("Chrome/Chromium executable not found");
}

function runDui(args: string[], envExtra: Record<string, string>) {
  const res = spawnSync(binary, args, {
    env: { ...process.env, ...envExtra },
    encoding: "utf8",
  });
  assert(res.stdout.trim().length > 0, `empty stdout for ${args.join(" ")}: ${res.stderr}`);
  return { status: res.status ?? 1, body: JSON.parse(res.stdout) };
}

function callMcp<T>(name: string, args: Record<string, unknown>, envExtra: Record<string, string>) {
  const rpc =
    JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "tools/call",
      params: { name, arguments: args },
    }) + "\n";
  const res = spawnSync(binary, ["mcp"], {
    input: rpc,
    env: { ...process.env, ...envExtra },
    encoding: "utf8",
  });
  assert.equal(res.status, 0, `mcp exited ${res.status}: ${res.stderr}`);
  const parsed = JSON.parse(res.stdout.trim()) as {
    result: { isError: boolean; content: { text: string }[] };
  };
  assert.equal(parsed.result.isError, false, parsed.result.content[0]?.text);
  return JSON.parse(parsed.result.content[0]!.text) as T;
}

try {
  const home = join(stage, "home");
  const env = { HOME: home };
  const appDir = join(stage, "audit-app");
  cpSync(template, appDir, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(join(template, "node_modules"), join(appDir, "node_modules"), "dir");

  const manifest = join(appDir, "dui.json");
  const run = runDui(["run", appDir, manifest], env);
  assert(run.body.output, JSON.stringify(run.body));
  const runDir = run.body.output as string;

  // Add one open comment and one resolved comment.
  const openComment = runDui(
    [
      "comment",
      "add",
      JSON.stringify({
        run: runDir,
        target: { kind: "screen", path: "/" },
        author: "reviewer",
        body: "Audit open feedback",
      }),
    ],
    env,
  );
  assert.equal(openComment.status, 0);

  const resolvedComment = runDui(
    [
      "comment",
      "add",
      JSON.stringify({
        run: runDir,
        target: { kind: "screen", path: "/" },
        author: "reviewer",
        body: "Audit resolved feedback",
      }),
    ],
    env,
  );
  assert.equal(resolvedComment.status, 0);
  const resolved = runDui(["comment", "resolve", resolvedComment.body.comment.id], env);
  assert.equal(resolved.status, 0);

  // CLI and MCP audit must match identically.
  const cliAudit = runDui(["audit", runDir], env);
  assert.equal(cliAudit.status, 0, JSON.stringify(cliAudit.body));
  const mcpAudit = callMcp<AuditReport>("audit", { run: runDir }, env);
  assert.deepEqual(cliAudit.body, mcpAudit);

  assert.equal(mcpAudit.goalPercent, 99);
  assert(mcpAudit.summary.totalCells >= 2, "audited cells present");
  assert.equal(mcpAudit.summary.openComments, 1);
  assert.equal(mcpAudit.summary.resolvedComments, 1);
  assert.equal(mcpAudit.screens[0]?.openComments, 1);
  assert.equal(mcpAudit.screens[0]?.unresolvedComments[0]?.body, "Audit open feedback");

  // Verify embedded `dui open --no-open` exposes `/api/audit` and renders audit summary + comment badge in Chrome.
  const viewer = spawn(binary, ["open", "--no-open", runDir], {
    env: { ...process.env, ...env, NO_COLOR: "1" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let viewerUrl = "";
  let stderr = "";
  const ready = new Promise<string>((resolveUrl, rejectUrl) => {
    const timer = setTimeout(() => rejectUrl(new Error(`dui open timed out: ${stderr}`)), 30_000);
    viewer.stdout.setEncoding("utf8");
    viewer.stdout.on("data", (chunk: string) => {
      const m = chunk.match(/http:\/\/127\.0\.0\.1:\d+/);
      if (m) {
        clearTimeout(timer);
        resolveUrl(m[0]);
      }
    });
    viewer.stderr.setEncoding("utf8");
    viewer.stderr.on("data", (chunk: string) => {
      stderr += chunk;
    });
  });

  try {
    viewerUrl = await ready;
    const runsRes = await fetch(`${viewerUrl}/archive/runs.json`);
    const runs = (await runsRes.json()) as { key: string }[];
    const auditRes = await fetch(`${viewerUrl}/api/audit?run=${encodeURIComponent(runs[0]!.key)}`);
    assert.equal(auditRes.status, 200);
    const apiAudit = (await auditRes.json()) as AuditReport;
    assert.equal(apiAudit.summary.openComments, 1);

    const chrome = spawnSync(
      chromeExecutable(),
      [
        "--headless=new",
        "--disable-gpu",
        "--no-sandbox",
        "--virtual-time-budget=6000",
        "--dump-dom",
        `${viewerUrl}/?run=${encodeURIComponent(runs[0]!.key)}&uiLang=en`,
      ],
      { encoding: "utf8", timeout: 30_000 },
    );
    assert.equal(chrome.status, 0, chrome.stderr);
    assert(
      chrome.stdout.includes("audit-summary-pill") && chrome.stdout.includes("1 unresolved"),
      "viewer renders coverage audit pill and unresolved comment count",
    );
  } finally {
    viewer.kill("SIGTERM");
  }

  console.log(
    JSON.stringify({
      ok: true,
      goalPercent: mcpAudit.goalPercent,
      meetsGoal: mcpAudit.meetsGoal,
      summary: mcpAudit.summary,
      cliMcpParity: true,
      viewerAuditVerified: true,
    }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
