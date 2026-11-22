// Verify content-addressed cell caching (#8):
// 1. Second run with no changes renders 0 cells and matches cells/results identically.
// 2. Changing one component re-renders affected cells and names the changed input (`source-files`).
// 3. Two worktrees with identical screens store each PNG once (verified via inode & unique bytes).
// 4. Changing browser or dui version invalidates affected cells (`renderer-environment` / `dui-version`).
// 5. `dui prune` removes unreferenced blobs while leaving every listed capture openable.
// 6. Reports cold vs warm run time and disk usage before vs after.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";

type CacheCell = {
  scenario: string;
  path: string;
  state: string;
  width: number;
  status: string;
  reason?: string | null;
  image?: string;
  cache?: {
    result: "hit" | "miss";
    key: string;
    changed?: string[];
  };
};

type CacheReport = {
  output: string;
  cache: {
    hits: number;
    misses: number;
    rendered: number;
    forced: boolean;
  };
  coverage: {
    verified: number;
    renderable: number;
    unrenderable: number;
    branchOutcomes: number;
    observedBranchOutcomes: number;
  };
  cells: CacheCell[];
};

const root = resolve(import.meta.dirname, "..");
const fixture = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-cache-check-"));

function runDui(
  args: string[],
  envExtra: Record<string, string>,
): { status: number; body: unknown; elapsedMs: number } {
  const start = performance.now();
  const res = spawnSync(binary, args, {
    env: { ...process.env, ...envExtra },
    encoding: "utf8",
  });
  const elapsedMs = performance.now() - start;
  assert(res.stdout.trim().length > 0, `empty stdout for ${args.join(" ")}: ${res.stderr}`);
  return { status: res.status ?? 1, body: JSON.parse(res.stdout), elapsedMs };
}

function normalizeCells(cells: CacheCell[]) {
  return cells.map((c) => ({
    scenario: c.scenario,
    path: c.path,
    state: c.state,
    width: c.width,
    status: c.status,
    reason: c.reason ?? null,
    image: c.image ?? null,
  }));
}

function measurePngDiskUsage(dirs: string[]): {
  logicalFiles: number;
  logicalBytes: number;
  uniqueInodes: number;
  uniqueBytes: number;
} {
  let logicalFiles = 0;
  let logicalBytes = 0;
  const seen = new Map<string, number>();
  for (const dir of dirs) {
    if (!existsSync(dir)) continue;
    for (const entry of readdirSync(dir)) {
      if (!entry.endsWith(".png")) continue;
      const st = statSync(join(dir, entry));
      logicalFiles += 1;
      logicalBytes += st.size;
      const key = `${st.dev}:${st.ino}`;
      if (!seen.has(key)) seen.set(key, st.size);
    }
  }
  let uniqueBytes = 0;
  for (const sz of seen.values()) uniqueBytes += sz;
  return { logicalFiles, logicalBytes, uniqueInodes: seen.size, uniqueBytes };
}

try {
  const home = join(stage, "home");
  const baseEnv = { HOME: home };

  // Create a git repository with two worktrees containing identical screens.
  const repo = join(stage, "repo");
  cpSync(fixture, repo, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(join(fixture, "node_modules"), join(repo, "node_modules"), "dir");

  execFileSync("git", ["init", "-q", "-b", "main"], { cwd: repo });
  execFileSync("git", ["config", "user.name", "Test"], { cwd: repo });
  execFileSync("git", ["config", "user.email", "test@example.com"], { cwd: repo });
  execFileSync(
    "git",
    ["add", "dui.json", "hooks.ts", "index.html", "main.tsx", "package.json", "tsconfig.json"],
    {
      cwd: repo,
    },
  );
  execFileSync("git", ["commit", "-q", "-m", "initial"], { cwd: repo });

  const worktree2 = join(stage, "repo-wt2");
  execFileSync("git", ["worktree", "add", "-q", "-b", "branch-two", worktree2], { cwd: repo });
  symlinkSync(join(fixture, "node_modules"), join(worktree2, "node_modules"), "dir");

  // 1. Cold run on worktree 1 -> renders all 18 cells.
  const cold = runDui(["run", repo, join(repo, "dui.json")], baseEnv);
  assert.equal(cold.status, 0);
  const coldReport = cold.body as CacheReport;
  assert.equal(coldReport.cache.hits, 0);
  assert.equal(coldReport.cache.rendered, 18);
  assert.equal(coldReport.coverage.verified, 18);

  // 2. Warm run on worktree 1 with no changes -> 0 cells rendered, identical cells and results.
  const warm = runDui(["run", repo, join(repo, "dui.json")], baseEnv);
  assert.equal(warm.status, 0);
  const warmReport = warm.body as CacheReport;
  assert.equal(warmReport.cache.rendered, 0);
  assert.equal(warmReport.cache.hits, 18);
  assert.deepEqual(normalizeCells(warmReport.cells), normalizeCells(coldReport.cells));
  assert.deepEqual(warmReport.coverage, coldReport.coverage);
  assert(warmReport.cells.every((c) => c.cache?.result === "hit"));

  // 3. Second worktree with identical screens -> 0 cells rendered, PNGs share the same inode on disk.
  const wt2 = runDui(["run", worktree2, join(worktree2, "dui.json")], baseEnv);
  assert.equal(wt2.status, 0);
  const wt2Report = wt2.body as CacheReport;
  assert.equal(wt2Report.cache.rendered, 0);
  assert.equal(wt2Report.cache.hits, 18);
  const diskUsage = measurePngDiskUsage([
    join(home, ".dui", "blobs"),
    coldReport.output,
    wt2Report.output,
  ]);
  assert(
    diskUsage.uniqueBytes < diskUsage.logicalBytes,
    `expected hard-linked blob dedup across worktrees: ${JSON.stringify(diskUsage)}`,
  );

  // 4. Changing one component re-renders affected cells and names `source-files` on each miss.
  const originalMain = readFileSync(join(repo, "main.tsx"), "utf8");
  writeFileSync(join(repo, "main.tsx"), originalMain.replace("<h1>Catalog</h1>", "<h1>Store</h1>"));
  const edited = runDui(["run", repo, join(repo, "dui.json")], baseEnv);
  assert.equal(edited.status, 0);
  const editedReport = edited.body as CacheReport;
  assert.equal(editedReport.cache.rendered, 18);
  assert(
    editedReport.cells.every(
      (c) => c.cache?.result === "miss" && c.cache.changed?.includes("source-files"),
    ),
    `expected source-files miss reason, got ${JSON.stringify(editedReport.cells[0]?.cache)}`,
  );
  writeFileSync(join(repo, "main.tsx"), originalMain);

  // 5. Changing renderer or dui version turns every affected cell into a miss naming the input.
  const rendererChanged = runDui(["run", repo, join(repo, "dui.json")], {
    ...baseEnv,
    DUI_CACHE_RENDERER: "Chrome/999.0.0",
  });
  assert.equal(rendererChanged.status, 0);
  const rendererReport = rendererChanged.body as CacheReport;
  assert.equal(rendererReport.cache.rendered, 18);
  assert(
    rendererReport.cells.every(
      (c) => c.cache?.result === "miss" && c.cache.changed?.includes("renderer-environment"),
    ),
  );

  const versionChanged = runDui(["run", repo, join(repo, "dui.json")], {
    ...baseEnv,
    DUI_CACHE_VERSION: "9.9.9",
  });
  assert.equal(versionChanged.status, 0);
  const versionReport = versionChanged.body as CacheReport;
  assert.equal(versionReport.cache.rendered, 18);
  assert(
    versionReport.cells.every(
      (c) => c.cache?.result === "miss" && c.cache.changed?.includes("dui-version"),
    ),
  );

  // 6. `--force` bypasses cache and marks every cell as `forced`.
  const forced = runDui(["run", "--force", repo, join(repo, "dui.json")], baseEnv);
  assert.equal(forced.status, 0);
  const forcedReport = forced.body as CacheReport;
  assert.equal(forcedReport.cache.forced, true);
  assert.equal(forcedReport.cache.rendered, 18);
  assert(
    forcedReport.cells.every(
      (c) => c.cache?.result === "miss" && c.cache.changed?.includes("forced"),
    ),
  );

  // 7. Selective module-graph invalidation: editing one component re-renders only dependent cells.
  const multiDir = join(stage, "multi");
  cpSync(fixture, multiDir, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(join(fixture, "node_modules"), join(multiDir, "node_modules"), "dir");
  writeFileSync(
    join(multiDir, "SharedBadge.tsx"),
    'export function SharedBadge() { return <span data-testid="badge">v1</span>; }\n',
  );
  writeFileSync(
    join(multiDir, "Catalog.tsx"),
    [
      'import { SharedBadge } from "./SharedBadge";',
      'export function Catalog() { return <main data-testid="catalog"><h1>Catalog</h1><SharedBadge /></main>; }',
      "",
    ].join("\n"),
  );
  writeFileSync(
    join(multiDir, "Settings.tsx"),
    [
      'import { SharedBadge } from "./SharedBadge";',
      'export function Settings() { return <main data-testid="settings"><h1>Settings</h1><SharedBadge /></main>; }',
      "",
    ].join("\n"),
  );
  writeFileSync(
    join(multiDir, "main.tsx"),
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
    join(multiDir, "dui.json"),
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
  const multiCold = runDui(["run", multiDir, join(multiDir, "dui.json")], baseEnv);
  assert.equal(multiCold.status, 0);
  const multiColdReport = multiCold.body as CacheReport;
  assert.equal(multiColdReport.cache.rendered, 2);
  assert.equal(multiColdReport.cache.hits, 0);

  writeFileSync(
    join(multiDir, "Catalog.tsx"),
    [
      'import { SharedBadge } from "./SharedBadge";',
      'export function Catalog() { return <main data-testid="catalog"><h1>Catalog Updated</h1><SharedBadge /></main>; }',
      "",
    ].join("\n"),
  );
  const multiCatalogEdit = runDui(["run", multiDir, join(multiDir, "dui.json")], baseEnv);
  assert.equal(multiCatalogEdit.status, 0);
  const multiCatalogReport = multiCatalogEdit.body as CacheReport;
  assert.equal(multiCatalogReport.cache.rendered, 1);
  assert.equal(multiCatalogReport.cache.hits, 1);
  const catalogCell = multiCatalogReport.cells.find((c) => c.path === "/catalog");
  const settingsCell = multiCatalogReport.cells.find((c) => c.path === "/settings");
  assert.equal(catalogCell?.cache?.result, "miss");
  assert(catalogCell?.cache?.changed?.includes("source-files"));
  assert.equal(settingsCell?.cache?.result, "hit");

  writeFileSync(
    join(multiDir, "SharedBadge.tsx"),
    'export function SharedBadge() { return <span data-testid="badge">v2</span>; }\n',
  );
  const multiSharedEdit = runDui(["run", multiDir, join(multiDir, "dui.json")], baseEnv);
  assert.equal(multiSharedEdit.status, 0);
  const multiSharedReport = multiSharedEdit.body as CacheReport;
  assert.equal(multiSharedReport.cache.rendered, 2);
  assert.equal(multiSharedReport.cache.hits, 0);
  assert(
    multiSharedReport.cells.every(
      (c) => c.cache?.result === "miss" && c.cache.changed?.includes("source-files"),
    ),
  );

  // 8. `dui prune` removes unreferenced blobs and leaves every listed capture openable.
  const orphanPath = join(home, ".dui", "blobs", `${"0".repeat(64)}.png`);
  writeFileSync(orphanPath, Buffer.from("orphan-blob-data"));
  const pruned = runDui(["prune"], baseEnv);
  assert.equal(pruned.status, 0);
  const pruneBody = pruned.body as {
    removedBlobs: number;
    freedBytes: number;
    remainingBlobs: number;
    remainingBytes: number;
  };
  assert(pruneBody.removedBlobs >= 1);
  assert(!existsSync(orphanPath));

  const history = runDui(["history"], baseEnv);
  assert.equal(history.status, 0);
  const snapshots = (history.body as { snapshots: { output: string }[] }).snapshots;
  assert(snapshots.length >= 6);
  for (const snap of snapshots) {
    assert(existsSync(join(snap.output, "report.json")));
    assert(existsSync(join(snap.output, "graph.json")));
    const r = JSON.parse(readFileSync(join(snap.output, "report.json"), "utf8")) as {
      cells: { image?: string }[];
    };
    for (const c of r.cells) {
      if (c.image) assert(existsSync(join(snap.output, c.image)));
    }
  }

  console.log(
    JSON.stringify({
      fixture: "fixtures/toggle",
      coldMs: Math.round(cold.elapsedMs),
      warmMs: Math.round(warm.elapsedMs),
      speedup: Number((cold.elapsedMs / Math.max(warm.elapsedMs, 1)).toFixed(2)),
      coldRendered: coldReport.cache.rendered,
      warmRendered: warmReport.cache.rendered,
      twoWorktreesDisk: diskUsage,
      pruned: pruneBody,
    }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
