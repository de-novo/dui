import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const repoRoot = process.cwd();
const duiBin = path.join(repoRoot, "target", "debug", "dui");
assert.ok(fs.existsSync(duiBin), `missing built dui binary at ${duiBin}`);

interface RenderNode {
  id: string;
  tag: string;
  text: string;
  rect: { x: number; y: number; width: number; height: number };
}

interface ReportCell {
  path: string;
  width: number;
  status: string;
  image?: string;
  observation?: {
    text?: string;
    renderTree?: RenderNode[];
    terminal?: { cols: number; rows: number };
  };
}

interface RunReport {
  coverage: { verified: number; renderable: number };
  cells: ReportCell[];
}

interface DiffReport {
  summary: { changed: number };
  screens: Array<{
    path: string;
    cells: Array<{
      renderDiff?: {
        textChanged: number;
        changes: Array<{
          kind: string;
          tag: string;
          beforeText?: string;
          afterText?: string;
        }>;
      };
    }>;
  }>;
}

function runDui(args: string[], envOverrides: Record<string, string>): unknown {
  const stdout = execFileSync(duiBin, args, {
    encoding: "utf8",
    env: { ...process.env, ...envOverrides },
  });
  return JSON.parse(stdout) as unknown;
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-tui-"));
const duiHome = path.join(tmp, ".dui-home");

try {
  const appDir = path.join(tmp, "tui-app");
  fs.mkdirSync(appDir, { recursive: true });

  const tuiScript = (jobsLabel: string): string => `
const cols = process.stdout.columns || 80;
let tab = 0;

function draw() {
  const border = "─".repeat(Math.max(10, cols - 2));
  process.stdout.write("\\x1b[2J\\x1b[1;1H");
  process.stdout.write("\\x1b[1;36m┌" + border + "┐\\x1b[0m\\r\\n");
  if (tab === 0) {
    process.stdout.write("\\x1b[1;32m│ Dashboard  [Tab: Settings]  cols=" + cols + "\\x1b[0m\\r\\n");
    process.stdout.write("│ ${jobsLabel}\\r\\n");
  } else {
    process.stdout.write("\\x1b[1;33m│ Settings   [Tab: Dashboard] cols=" + cols + "\\x1b[0m\\r\\n");
    process.stdout.write("│ Theme: Dark Monochrome\\r\\n");
  }
  process.stdout.write("\\x1b[1;36m└" + border + "┘\\x1b[0m\\r\\n");
}

if (process.stdin.isTTY) {
  process.stdin.setRawMode(true);
}
process.stdin.resume();
draw();
process.stdin.on("data", (chunk) => {
  const s = chunk.toString("utf8");
  if (s === "\\t") {
    tab = tab === 0 ? 1 : 0;
    draw();
  } else if (s === "q" || s === "\\x03") {
    process.exit(0);
  }
});
`;

  fs.writeFileSync(path.join(appDir, "app.mjs"), tuiScript("Active Jobs: 3"));

  const manifest = path.join(appDir, "dui.json");
  fs.writeFileSync(
    manifest,
    JSON.stringify({
      host: "pty",
      command: [process.execPath, "app.mjs"],
      path: "/dashboard",
      widths: [80, 120],
      rows: 12,
      settleMs: 120,
      scenarios: [{ path: "/settings", keys: ["Tab"] }],
    }),
  );

  // 1. Capture both terminal widths (80 & 120) x 2 screens (/dashboard & /settings via Tab)
  const out1 = path.join(tmp, "out-1");
  runDui(["run", appDir, manifest, out1], { HOME: duiHome });

  const report1 = JSON.parse(fs.readFileSync(path.join(out1, "report.json"), "utf8")) as RunReport;
  assert.equal(
    report1.coverage.verified,
    4,
    `expected 4 verified TUI cells, got ${report1.coverage.verified}`,
  );
  const dash80 = report1.cells.find((c) => c.path === "/dashboard" && c.width === 80);
  const settings120 = report1.cells.find((c) => c.path === "/settings" && c.width === 120);
  assert.ok(
    dash80?.observation?.text?.includes("Active Jobs: 3"),
    "dashboard captures initial state",
  );
  assert.ok(dash80?.observation?.text?.includes("cols=80"), "dashboard reflects 80 cols");
  assert.ok(
    settings120?.observation?.text?.includes("Theme: Dark Monochrome"),
    "Tab key switches to Settings screen",
  );
  assert.ok(settings120?.observation?.text?.includes("cols=120"), "settings reflects 120 cols");
  assert.ok(
    (dash80?.observation?.renderTree?.length ?? 0) >= 3,
    "TUI emits RenderNodeSnapshot tree",
  );
  assert.ok(dash80?.image && fs.existsSync(path.join(out1, dash80.image)), "TUI writes PNG image");

  // 2. Mutate TUI source and confirm cache invalidation + element-level renderDiff
  fs.writeFileSync(path.join(appDir, "app.mjs"), tuiScript("Active Jobs: 9"));
  const out2 = path.join(tmp, "out-2");
  runDui(["run", appDir, manifest, out2], { HOME: duiHome });
  const diff = runDui(["diff", out1, out2], { HOME: duiHome }) as DiffReport;
  assert.ok(
    diff.summary.changed >= 2,
    `expected >=2 changed TUI cells, got ${diff.summary.changed}`,
  );
  const dashScreen = diff.screens.find((s) => s.path === "/dashboard");
  const changedCell = dashScreen?.cells.find((c) => c.renderDiff);
  assert.ok(changedCell?.renderDiff, "expected element-level renderDiff for TUI");
  assert.ok(
    changedCell.renderDiff.changes.some(
      (ch) =>
        ch.tag === "tui-row" &&
        ch.beforeText?.includes("Active Jobs: 3") &&
        ch.afterText?.includes("Active Jobs: 9"),
    ),
    `expected tui-row text change in renderDiff: ${JSON.stringify(changedCell.renderDiff)}`,
  );

  // 3. Missing PTY runtime honestly reports unrenderable (no-pty-env)
  const missingEnvReply = runDui(["run", appDir, manifest, path.join(tmp, "out-missing")], {
    HOME: duiHome,
    DUI_PTY_PYTHON: "/nonexistent/python3",
  }) as { status: string; surface: string; reason: string };
  assert.equal(missingEnvReply.status, "unrenderable");
  assert.equal(missingEnvReply.surface, "universal-tui");
  assert.equal(missingEnvReply.reason, "no-pty-env");

  console.log(
    JSON.stringify({
      status: "ok",
      verifiedCells: report1.coverage.verified,
      renderNodesOn80: dash80.observation?.renderTree?.length ?? 0,
      diffChangedCells: diff.summary.changed,
      missingEnvReason: missingEnvReply.reason,
    }),
  );
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
