// Verify capture diff (#11):
// 1. Two captures of the same commit produce an empty diff (CLI and MCP agree identically).
// 2. Changing one component shows only dependent cells with changed source files and a pixel-diff overlay.
// 3. Unrenderable / unverified cells show a status change and no pixel diff.
// 4. The viewer (/api/diff + CaptureDiffView in headless Chrome) lists the exact same changed cells as CLI/MCP.
import assert from "node:assert/strict";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

type DiffCell = {
  kind: "added" | "removed" | "changed";
  path: string;
  state: string;
  width: number;
  changedFiles: string[];
  statusChange?: { from: string; to: string; fromReason?: string; toReason?: string };
  pixelDiff?: {
    diffPixels: number;
    totalPixels: number;
    diffRatio: number;
    width: number;
    height: number;
    changedBounds?: { x: number; y: number; width: number; height: number };
    overlayImage?: string;
  };
};

type DiffReport = {
  summary: {
    totalChanged: number;
    added: number;
    removed: number;
    changed: number;
    unchanged: number;
    screensAffected: number;
  };
  screens: { path: string; cells: DiffCell[] }[];
};

const root = resolve(import.meta.dirname, "..");
const template = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-diff-check-"));

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

function callMcpDiff(
  before: string,
  after: string,
  envExtra: Record<string, string>,
  overlayDir?: string,
): DiffReport {
  const payload =
    JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "tools/call",
      params: {
        name: "diff",
        arguments: overlayDir ? { before, after, overlayDir } : { before, after },
      },
    }) + "\n";
  const res = spawnSync(binary, ["mcp"], {
    input: payload,
    env: { ...process.env, ...envExtra },
    encoding: "utf8",
  });
  assert.equal(res.status, 0, `mcp exited ${res.status}: ${res.stderr}`);
  const rpc = JSON.parse(res.stdout.trim()) as {
    result: { isError: boolean; content: { text: string }[] };
  };
  assert.equal(rpc.result.isError, false);
  return JSON.parse(rpc.result.content[0]!.text) as DiffReport;
}

try {
  const home = join(stage, "home");
  const baseEnv = { HOME: home };
  const appDir = join(stage, "multi");
  cpSync(template, appDir, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(join(template, "node_modules"), join(appDir, "node_modules"), "dir");

  writeFileSync(
    join(appDir, "SharedBadge.tsx"),
    'export function SharedBadge() { return <span data-testid="badge">v1</span>; }\n',
  );
  writeFileSync(
    join(appDir, "Catalog.tsx"),
    [
      'import { SharedBadge } from "./SharedBadge";',
      'export function Catalog() { return <main data-testid="catalog" style={{ padding: "24px", fontFamily: "sans-serif" }}><h1>Catalog</h1><SharedBadge /></main>; }',
      "",
    ].join("\n"),
  );
  writeFileSync(
    join(appDir, "Settings.tsx"),
    [
      'import { SharedBadge } from "./SharedBadge";',
      'export function Settings() { return <main data-testid="settings" style={{ padding: "24px", fontFamily: "sans-serif" }}><h1>Settings</h1><SharedBadge /></main>; }',
      "",
    ].join("\n"),
  );
  writeFileSync(
    join(appDir, "main.tsx"),
    [
      'import { createRoot } from "react-dom/client";',
      'import { Catalog } from "./Catalog";',
      'import { Settings } from "./Settings";',
      "function App() {",
      '  return window.location.pathname === "/settings" ? <Settings /> : <Catalog />;',
      "}",
      'createRoot(document.getElementById("app")!).render(<App />);',
      "",
    ].join("\n"),
  );
  writeFileSync(
    join(appDir, "dui.json"),
    JSON.stringify(
      {
        tsconfig: "tsconfig.json",
        path: "/catalog",
        sourceFile: "Catalog.tsx",
        widths: [375],
        readySelector: "[data-testid=catalog]",
        scenarios: [
          {
            path: "/settings",
            sourceFile: "Settings.tsx",
            readySelector: "[data-testid=settings]",
          },
        ],
      },
      null,
      2,
    ),
  );

  // Capture 1 & 2 on the exact same commit.
  const run1 = runDui(["run", appDir, join(appDir, "dui.json")], baseEnv);
  assert.equal(run1.status, 0);
  const run1Dir = (run1.body as { output: string }).output;

  const run2 = runDui(["run", appDir, join(appDir, "dui.json")], baseEnv);
  assert.equal(run2.status, 0);
  const run2Dir = (run2.body as { output: string }).output;

  // AC 1: Two captures of the same commit show an empty diff, and CLI/MCP match identically.
  const sameCli = runDui(["diff", run1Dir, run2Dir], baseEnv);
  assert.equal(sameCli.status, 0);
  const sameCliDiff = sameCli.body as DiffReport;
  const sameMcpDiff = callMcpDiff(run1Dir, run2Dir, baseEnv);
  assert.deepEqual(sameCliDiff, sameMcpDiff);
  assert.equal(sameCliDiff.summary.totalChanged, 0);
  assert.equal(sameCliDiff.summary.unchanged, 2);
  assert.deepEqual(sameCliDiff.screens, []);

  // Edit only Catalog.tsx and capture run 3.
  writeFileSync(
    join(appDir, "Catalog.tsx"),
    [
      'import { SharedBadge } from "./SharedBadge";',
      'export function Catalog() { return <main data-testid="catalog" style={{ padding: "24px", fontFamily: "sans-serif", background: "#111", color: "#fff" }}><h1>Catalog Updated</h1><SharedBadge /></main>; }',
      "",
    ].join("\n"),
  );
  const run3 = runDui(["run", appDir, join(appDir, "dui.json")], baseEnv);
  assert.equal(run3.status, 0);
  const run3Dir = (run3.body as { output: string }).output;

  // AC 2: Changing one component shows only the dependent cell, changed files, and pixel-diff overlay.
  const overlayDir = join(stage, "overlays");
  const changedCli = runDui(["diff", run1Dir, run3Dir, "--overlay-dir", overlayDir], baseEnv);
  assert.equal(changedCli.status, 0);
  const changedCliDiff = changedCli.body as DiffReport;
  const changedMcpDiff = callMcpDiff(run1Dir, run3Dir, baseEnv, overlayDir);
  assert.deepEqual(changedCliDiff, changedMcpDiff);
  assert.equal(changedCliDiff.summary.totalChanged, 1);
  assert.equal(changedCliDiff.summary.changed, 1);
  assert.equal(changedCliDiff.summary.unchanged, 1);
  assert.equal(changedCliDiff.screens.length, 1);
  assert.equal(changedCliDiff.screens[0]?.path, "/catalog");
  const changedCell = changedCliDiff.screens[0]!.cells[0]!;
  assert.equal(changedCell.kind, "changed");
  assert.deepEqual(changedCell.changedFiles, ["Catalog.tsx"]);
  assert(changedCell.pixelDiff && changedCell.pixelDiff.diffPixels > 0);
  assert(changedCell.pixelDiff.changedBounds);
  assert(changedCell.pixelDiff.overlayImage);
  const overlayPng = readFileSync(join(overlayDir, changedCell.pixelDiff.overlayImage));
  assert.deepEqual(overlayPng.subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));

  // Status-change check: unrenderable cell shows statusChange and no pixelDiff.
  const brokenDir = join(stage, "broken-run");
  mkdirSync(brokenDir, { recursive: true });
  const baseReport = JSON.parse(readFileSync(join(run1Dir, "report.json"), "utf8")) as {
    cells: Record<string, unknown>[];
  };
  const brokenReport = structuredClone(baseReport);
  brokenReport.cells[0] = {
    ...brokenReport.cells[0],
    status: "unrenderable",
    reason: "no-render-env",
    image: null,
  };
  writeFileSync(join(brokenDir, "report.json"), JSON.stringify(brokenReport));
  const statusDiff = runDui(["diff", run1Dir, brokenDir], baseEnv).body as DiffReport;
  assert.equal(statusDiff.summary.totalChanged, 1);
  const brokenCell = statusDiff.screens[0]!.cells[0]!;
  assert.equal(brokenCell.pixelDiff, undefined);
  assert.equal(brokenCell.statusChange?.from, "verified");
  assert.equal(brokenCell.statusChange?.to, "unrenderable");
  assert.equal(brokenCell.statusChange?.toReason, "no-render-env");

  // AC 3: Viewer and CLI/MCP JSON list the exact same changed cells in real Chrome.
  execFileSync("npm", ["--prefix", join(root, "apps/canvas"), "run", "build"], {
    stdio: "pipe",
  });
  const server = spawn(
    process.execPath,
    [join(root, "apps/canvas/preview.mts"), run1Dir, run2Dir, run3Dir],
    {
      env: { ...process.env, HOME: home, PORT: "0", NO_COLOR: "1" },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  try {
    const ansiPattern = new RegExp(`${String.fromCharCode(0x1b)}\\[[0-9;]*m`, "g");
    const url = await new Promise<string>((resolveUrl, rejectUrl) => {
      let buf = "";
      let errBuf = "";
      const timer = setTimeout(() => {
        rejectUrl(new Error(`timed out waiting for preview URL: ${buf}\n${errBuf}`));
      }, 30_000);
      server.stdout.on("data", (chunk) => {
        buf += chunk.toString();
        const plain = buf.replaceAll(ansiPattern, "");
        const match = plain.match(/http:\/\/127\.0\.0\.1:\d+/);
        if (match) {
          clearTimeout(timer);
          resolveUrl(match[0]);
        }
      });
      server.stderr.on("data", (chunk) => {
        errBuf += chunk.toString();
      });
      server.on("exit", (code) => {
        clearTimeout(timer);
        rejectUrl(new Error(`preview exited early (${code}): ${errBuf}`));
      });
    });

    const runs = (await (await fetch(`${url}/archive/runs.json`)).json()) as { key: string }[];
    assert.equal(runs.length, 3);
    const [key1, key2, key3] = runs.map((r) => r.key) as [string, string, string];

    const apiSame = (await (
      await fetch(`${url}/api/diff?before=${key1}&after=${key2}`)
    ).json()) as DiffReport;
    assert.deepEqual(apiSame.summary, sameCliDiff.summary);
    assert.deepEqual(apiSame.screens, sameCliDiff.screens);

    const apiChanged = (await (
      await fetch(`${url}/api/diff?before=${key1}&after=${key3}`)
    ).json()) as DiffReport;
    assert.deepEqual(apiChanged.summary, changedCliDiff.summary);
    assert.deepEqual(
      apiChanged.screens.map((s) => ({
        path: s.path,
        cells: s.cells.map((c) => ({
          kind: c.kind,
          state: c.state,
          width: c.width,
          changedFiles: c.changedFiles,
        })),
      })),
      changedCliDiff.screens.map((s) => ({
        path: s.path,
        cells: s.cells.map((c) => ({
          kind: c.kind,
          state: c.state,
          width: c.width,
          changedFiles: c.changedFiles,
        })),
      })),
    );

    const overlayRes = await fetch(
      `${url}/api/diff/overlay?before=${key1}&after=${key3}&file=${changedCell.pixelDiff.overlayImage}`,
    );
    assert.equal(overlayRes.status, 200);
    assert.equal(overlayRes.headers.get("content-type"), "image/png");

    const chrome = chromeExecutable();
    const emptyDom = execFileSync(
      chrome,
      [
        "--headless=new",
        "--disable-gpu",
        "--no-sandbox",
        "--virtual-time-budget=3000",
        "--dump-dom",
        `${url}/?view=diff&before=${encodeURIComponent(key1)}&after=${encodeURIComponent(key2)}`,
      ],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], timeout: 30_000 },
    );
    assert(emptyDom.includes('data-diff-empty="true"'), "expected empty diff state in viewer DOM");

    const changedDom = execFileSync(
      chrome,
      [
        "--headless=new",
        "--disable-gpu",
        "--no-sandbox",
        "--virtual-time-budget=3000",
        "--dump-dom",
        `${url}/?view=diff&before=${encodeURIComponent(key1)}&after=${encodeURIComponent(key3)}`,
      ],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], timeout: 30_000 },
    );
    assert(
      changedDom.includes('data-diff-screen="/catalog"'),
      "expected /catalog changed cell in viewer DOM",
    );
    assert(
      !changedDom.includes('data-diff-screen="/settings"'),
      "unchanged /settings cell must stay hidden in viewer DOM",
    );
    assert(
      !changedDom.includes("Catalog.tsx"),
      "human visual diff DOM should not show source code file chips",
    );
    assert(changedDom.includes("/api/diff/overlay"), "expected pixel diff overlay in viewer DOM");

    console.log(
      JSON.stringify({
        sameCommitEmptyDiff: sameCliDiff.summary,
        singleComponentDiff: {
          summary: changedCliDiff.summary,
          screen: changedCell.path,
          changedFiles: changedCell.changedFiles,
          pixelDiff: changedCell.pixelDiff,
        },
        statusChangeNoPixelDiff: brokenCell.statusChange,
        viewerMatchesCliMcp: true,
      }),
    );
  } finally {
    server.kill();
  }
} finally {
  rmSync(stage, { recursive: true, force: true });
}
