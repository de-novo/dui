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
}

interface ReportCell {
  path: string;
  width: number;
  status: string;
  image?: string;
  observation?: {
    text?: string;
    renderTree?: RenderNode[];
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

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-electron-"));
const duiHome = path.join(tmp, ".dui-home");

try {
  const appDir = path.join(tmp, "desktop-app");
  const rendererDir = path.join(appDir, "out", "renderer");
  fs.mkdirSync(rendererDir, { recursive: true });

  const renderIndexHtml = (title: string): string => `<!doctype html>
<html lang="en">
  <head><meta charset="utf-8" /><title>Electron Desktop</title></head>
  <body style="font-family:sans-serif;padding:24px">
    <h1>${title}</h1>
    <p id="version">loading...</p>
    <nav><a href="/preferences.html">Preferences</a></nav>
    <script>
      window.electronAPI.getAppVersion().then((v) => {
        document.getElementById("version").textContent = "Electron " + v;
      });
    </script>
  </body>
</html>`;

  fs.writeFileSync(path.join(rendererDir, "index.html"), renderIndexHtml("Workspace Studio"));
  fs.writeFileSync(
    path.join(rendererDir, "preferences.html"),
    `<!doctype html>
<html lang="en">
  <head><meta charset="utf-8" /><title>Preferences</title></head>
  <body style="font-family:sans-serif;padding:24px">
    <h1>Desktop Preferences</h1>
    <a href="/">Back to Workspace</a>
  </body>
</html>`,
  );

  const manifest = path.join(appDir, "dui.json");
  fs.writeFileSync(
    manifest,
    JSON.stringify({
      host: "electron",
      rendererRoot: "out/renderer",
      widths: [1024, 1440],
      routeSelector: "a[href]",
      ipcMocks: {
        getAppVersion: "45.0.0",
      },
    }),
  );

  // 1. Capture Electron renderer with injected window.electronAPI preload mocks
  const out1 = path.join(tmp, "out-1");
  runDui(["run", appDir, manifest, out1], { HOME: duiHome });
  const report1 = JSON.parse(fs.readFileSync(path.join(out1, "report.json"), "utf8")) as RunReport;
  assert.equal(
    report1.coverage.verified,
    4,
    `expected 4 verified Electron cells, got ${report1.coverage.verified}`,
  );
  const homeCell = report1.cells.find((c) => c.path === "/" && c.width === 1024);
  assert.ok(
    homeCell?.observation?.text?.includes("Electron 45.0.0"),
    `expected injected electronAPI.getAppVersion() result: ${homeCell?.observation?.text}`,
  );
  assert.ok((homeCell?.observation?.renderTree?.length ?? 0) >= 2, "expected renderTree");

  // 2. Edit renderer HTML -> cache invalidates -> renderDiff detects h1 change
  fs.writeFileSync(path.join(rendererDir, "index.html"), renderIndexHtml("Workspace Studio v2"));
  const out2 = path.join(tmp, "out-2");
  runDui(["run", appDir, manifest, out2], { HOME: duiHome });
  const diff = runDui(["diff", out1, out2], { HOME: duiHome }) as DiffReport;
  assert.ok(diff.summary.changed >= 2, `expected >=2 changed Electron cells`);
  const homeDiff = diff.screens.find((s) => s.path === "/")?.cells.find((c) => c.renderDiff);
  assert.ok(homeDiff?.renderDiff, "expected element-level renderDiff on Electron / screen");
  assert.ok(
    homeDiff.renderDiff.changes.some(
      (ch) =>
        ch.tag === "h1" &&
        ch.beforeText === "Workspace Studio" &&
        ch.afterText === "Workspace Studio v2",
    ),
    `expected h1 text change: ${JSON.stringify(homeDiff.renderDiff)}`,
  );

  // 3. Binary mode without Electron binary honestly reports unrenderable (no-electron-binary)
  const binManifest = path.join(appDir, "dui-bin.json");
  fs.writeFileSync(
    binManifest,
    JSON.stringify({
      host: "electron",
      mode: "binary",
      widths: [1024],
    }),
  );
  const missingReply = runDui(["run", appDir, binManifest, path.join(tmp, "out-missing")], {
    HOME: duiHome,
    DUI_ELECTRON: "/nonexistent/electron",
  }) as { status: string; surface: string; reason: string };
  assert.equal(missingReply.status, "unrenderable");
  assert.equal(missingReply.surface, "electron-desktop");
  assert.equal(missingReply.reason, "no-electron-binary");

  console.log(
    JSON.stringify({
      status: "ok",
      verifiedCells: report1.coverage.verified,
      preloadResolved: true,
      diffChangedCells: diff.summary.changed,
      missingBinaryReason: missingReply.reason,
    }),
  );
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
