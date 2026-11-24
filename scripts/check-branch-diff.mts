// Verify branch-aware diff (`dui diff --base <branch>`) and `--changed-since <ref>` (#66):
// 1. Initialize a real git repo on `main`, capture baseline with `dui run`.
// 2. Create `feat/ui-update` branch, modify `main.tsx`, commit, and run `dui run --changed-since main`.
// 3. Verify `changedSince.gitChangedFiles` & `affectedFiles` include `main.tsx`.
// 4. Verify CLI `dui diff --base main --project <REPO>` and MCP `diff { baseBranch: "main", project }`
//    return identical JSON with `beforeBranch: "main"`, `afterBranch: "feat/ui-update"`, and changed cells.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const template = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-branch-diff-check-"));

function git(cwd: string, args: string[]) {
  const res = spawnSync("git", args, {
    cwd,
    env: {
      ...process.env,
      GIT_AUTHOR_NAME: "dui-test",
      GIT_AUTHOR_EMAIL: "dui-test@example.com",
      GIT_COMMITTER_NAME: "dui-test",
      GIT_COMMITTER_EMAIL: "dui-test@example.com",
    },
    encoding: "utf8",
  });
  assert.equal(res.status, 0, `git ${args.join(" ")} failed: ${res.stderr}`);
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
  const appDir = join(stage, "branch-app");
  cpSync(template, appDir, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(join(template, "node_modules"), join(appDir, "node_modules"), "dir");

  const manifest = join(appDir, "dui.json");
  writeFileSync(
    manifest,
    JSON.stringify(
      {
        tsconfig: "tsconfig.json",
        path: "/",
        widths: [375],
        readySelector: "[data-testid=catalog]",
      },
      null,
      2,
    ),
  );
  writeFileSync(join(appDir, ".gitignore"), "node_modules/\ndist/\n");

  git(appDir, ["init", "-b", "main"]);
  git(appDir, ["add", "."]);
  git(appDir, ["commit", "-m", "init main"]);

  const mainRun = runDui(["run", appDir, manifest], env);
  assert(mainRun.body.output, JSON.stringify(mainRun.body));

  // Branch off `main` and make a visible UI change in `main.tsx`.
  git(appDir, ["checkout", "-b", "feat/ui-update"]);
  const mainTsxPath = join(appDir, "main.tsx");
  const originalTsx = readFileSync(mainTsxPath, "utf8");
  writeFileSync(
    mainTsxPath,
    originalTsx.replace("<h1>Catalog</h1>", "<h1>Updated Branch Catalog</h1>"),
  );
  git(appDir, ["add", "main.tsx"]);
  git(appDir, ["commit", "-m", "feat: update heading"]);

  const featRun = runDui(["run", appDir, manifest, "--changed-since", "main"], env);
  assert(featRun.body.output, JSON.stringify(featRun.body));
  const changedSince = featRun.body.changedSince as {
    baseRef: string;
    gitChangedFiles: string[];
    affectedFiles: string[];
  };
  assert.equal(changedSince.baseRef, "main");
  assert(
    changedSince.gitChangedFiles.includes("main.tsx"),
    `expected main.tsx in gitChangedFiles: ${JSON.stringify(changedSince)}`,
  );
  assert(
    changedSince.affectedFiles.includes("main.tsx"),
    `expected main.tsx in affectedFiles: ${JSON.stringify(changedSince)}`,
  );

  // Branch-name diff via CLI and MCP must return identical JSON.
  type BranchDiffBody = {
    beforeBranch: string;
    afterBranch: string;
    summary: { changed: number };
    screens: {
      cells: {
        changedFiles: string[];
        renderDiff?: {
          textChanged: number;
          visualOnly: boolean;
          changes: {
            kind: string;
            tag: string;
            beforeText?: string;
            afterText?: string;
            afterRect?: { width: number; height: number };
            summary: string;
          }[];
        };
        pixelDiff: { diffPixels: number };
      }[];
    }[];
  };
  const cliDiff = runDui(["diff", "--base", "main", "--project", appDir], env);
  assert.equal(cliDiff.status, 0, JSON.stringify(cliDiff.body));
  const diffBody = cliDiff.body as BranchDiffBody;
  const mcpDiff = callMcp<BranchDiffBody>("diff", { baseBranch: "main", project: appDir }, env);
  assert.deepEqual(diffBody, mcpDiff);
  assert.equal(diffBody.beforeBranch, "main");
  assert.equal(diffBody.afterBranch, "feat/ui-update");
  assert(
    diffBody.summary.changed >= 1,
    `expected >=1 changed cell between branches: ${JSON.stringify(diffBody.summary)}`,
  );
  const changedCell = diffBody.screens[0]!.cells[0]!;
  assert(
    changedCell.changedFiles.includes("main.tsx"),
    `expected main.tsx attribution: ${JSON.stringify(changedCell)}`,
  );
  assert(
    changedCell.pixelDiff.diffPixels > 0,
    `expected non-zero pixel diff: ${JSON.stringify(changedCell.pixelDiff)}`,
  );
  assert(
    changedCell.renderDiff && changedCell.renderDiff.textChanged >= 1,
    `expected render-level textChanged >= 1: ${JSON.stringify(changedCell.renderDiff)}`,
  );
  const h1Change = changedCell.renderDiff.changes.find(
    (c) =>
      c.kind === "text-changed" &&
      c.tag === "h1" &&
      c.beforeText === "Catalog" &&
      c.afterText === "Updated Branch Catalog",
  );
  assert(
    h1Change,
    `expected h1 Catalog -> Updated Branch Catalog change: ${JSON.stringify(changedCell.renderDiff)}`,
  );
  assert(
    h1Change.afterRect && h1Change.afterRect.width > 0 && h1Change.afterRect.height > 0,
    `expected valid element bounding box: ${JSON.stringify(h1Change)}`,
  );

  const visualOnly = runDui(
    ["diff", "--base", "main", "--project", appDir, "--regressions-only"],
    env,
  );
  assert.equal(visualOnly.status, 0);
  assert.equal(visualOnly.body.verification.regressedCells, 0);

  // Two commits render identical documents. Changing the screen assertion fails only on
  // the Korean narrow viewport. Another route has been unverified since the base commit.
  const site = join(stage, "screen-app");
  mkdirSync(site);
  const html = `<!doctype html><meta charset="utf-8"><title>Inbox</title>
    <style>@media(max-width:500px){html.ko #wide{display:none}}</style>
    <h1 id="ready">Inbox</h1><p id="wide">Wide</p><div id="never" hidden>Hidden</div>
    <script>document.documentElement.className = navigator.language</script>`;
  writeFileSync(join(site, "index.html"), html);
  writeFileSync(join(site, "existing.html"), html);
  const spec = {
    host: "static",
    path: "/",
    widths: [375, 1280],
    locales: ["en", "ko"],
    readySelector: "#ready",
    scenarios: [{ path: "/existing.html", readySelector: "#never" }],
  };
  const config = join(site, "dui.json");
  writeFileSync(config, JSON.stringify(spec));
  git(site, ["init", "-b", "main"]);
  git(site, ["add", "."]);
  git(site, ["commit", "-m", "baseline screen assertions"]);
  const baseCapture = runDui(["run", site, config], env);
  assert.equal(baseCapture.status, 0);
  git(site, ["checkout", "-b", "feat/screen-regression"]);
  writeFileSync(config, JSON.stringify({ ...spec, readySelector: "#wide" }));
  git(site, ["commit", "-am", "change screen assertion"]);
  const headCapture = runDui(["run", site, config, "--force"], env);
  assert.equal(headCapture.status, 0);
  const partial = runDui(["recheck", headCapture.body.output, site, config], env);
  assert.equal(partial.status, 5);

  const args = ["diff", "--base", "main", "--project", site, "--regressions-only"];
  const gate = runDui(args, env);
  assert.equal(gate.status, 5);
  assert.equal(gate.body.after.path, headCapture.body.output);
  assert.equal(gate.body.before.path, baseCapture.body.output);
  assert.equal(gate.body.screens, undefined);
  const verdict = gate.body.verification;
  assert.equal(verdict.matchedCells, 8);
  assert.equal(verdict.comparedChecks, 16);
  assert.equal(verdict.regressedCells, 1);
  assert.equal(verdict.regressions.length, 1);
  const [row] = verdict.regressions;
  assert.deepEqual(
    [row.path, row.width, row.locale, row.checks.length, row.checks[0].check],
    ["/", 375, "ko", 1, "screen"],
  );
  assert.equal(row.checks[0].evidence.selectorVisible, false);
  const select = (cells: { state: string; width: number; image: string }[]) =>
    cells.find((c) => c.state === row.state && c.width === row.width)?.image;
  assert.ok(select(baseCapture.body.cells));
  assert.equal(select(baseCapture.body.cells), select(headCapture.body.cells));
  const fullBranch = runDui(["diff", "--base", "main", "--project", site], env);
  assert.equal(fullBranch.status, 0);
  assert.deepEqual(fullBranch.body.verification, verdict);
  assert.equal(fullBranch.body.summary.changed, 1);
  assert.equal(fullBranch.body.screens[0].cells[0].after.screenStatus, "unverified");
  const empty = join(stage, "empty-capture");
  mkdirSync(empty);
  writeFileSync(join(empty, "report.json"), JSON.stringify({ cells: [] }));
  const incomparable = runDui(["diff", baseCapture.body.output, empty, "--regressions-only"], env);
  assert.equal(incomparable.status, 2);
  assert.equal(incomparable.body.verification.verdict, "inconclusive");
  assert.equal(incomparable.body.verification.unmatchedBaseCells, 8);
  const viaMcp = callMcp("diff", { baseBranch: "main", project: site, regressionsOnly: true }, env);
  assert.deepEqual(viaMcp, gate.body);
  const md = spawnSync(binary, [...args, "--markdown"], {
    env: { ...process.env, ...env },
    encoding: "utf8",
  });
  assert.equal(md.status, 5);
  assert.ok(md.stdout.includes("verified → unverified") && md.stdout.includes("375px"));
  assert.ok(!md.stdout.includes("existing.html"));
  assert.ok(md.stdout.includes("#wide") && md.stdout.includes("visible false"));
  // Execute the PR check itself on separate exact Git checkouts. A repair commit
  // must clear the check even though the unrelated route still lacks evidence.
  const checkout = join(stage, "pinned-base");
  git(stage, ["clone", "--quiet", "--branch", "main", "--single-branch", site, checkout]);
  const sha = (dir: string) =>
    execFileSync("git", ["-C", dir, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
  const baseSha = sha(checkout);
  const check = (headSha: string, expected: number) => {
    const summary = join(stage, "job-summary.md");
    writeFileSync(summary, "");
    const run = spawnSync(
      process.execPath,
      [join(root, "scripts/pr-captures.mts"), binary, checkout, site, baseSha, headSha],
      {
        env: { ...process.env, GITHUB_STEP_SUMMARY: summary },
        encoding: "utf8",
      },
    );
    assert.equal(run.status, expected, run.stdout + run.stderr);
    return readFileSync(summary, "utf8");
  };
  const brokenSha = sha(site);
  const brokenSummary = check(brokenSha, 5);
  assert.ok(brokenSummary.startsWith("# New verification regressions"));
  assert.ok(brokenSummary.includes(baseSha) && brokenSummary.includes(brokenSha));
  assert.ok(
    brokenSummary.includes("375px | ko | screen") && !brokenSummary.includes("existing.html"),
  );
  writeFileSync(join(site, "index.html"), html.replace("display:none", "display:block"));
  const addedApp = join(site, "new-web-app");
  mkdirSync(addedApp);
  writeFileSync(join(addedApp, "index.html"), html);
  writeFileSync(join(addedApp, "dui.json"), JSON.stringify(spec));
  git(site, ["add", "."]);
  git(site, ["commit", "-m", "repair mobile Korean screen and add a web app"]);
  const repaired = check(sha(site), 0);
  assert.ok(repaired.startsWith("# No new verification regressions"));
  assert.ok(repaired.includes("16 compared checks"));
  assert.ok(repaired.includes("new-web-app</code> — present only on the head commit"));
  const mismatch = check(brokenSha, 2);
  assert.ok(mismatch.startsWith("# Inconclusive") && mismatch.includes("does not match"));
  console.log(
    JSON.stringify({
      regressedCells: verdict.regressedCells,
      matchedCells: verdict.matchedCells,
      comparedChecks: verdict.comparedChecks,
      identicalImages: true,
      cliMcpParity: true,
      gateExit: gate.status,
    }),
  );
  console.log(
    JSON.stringify({
      ok: true,
      beforeBranch: diffBody.beforeBranch,
      afterBranch: diffBody.afterBranch,
      changedCells: diffBody.summary.changed,
      diffPixels: changedCell.pixelDiff.diffPixels,
      renderDiffSummary: h1Change.summary,
      changedSince,
      cliMcpParity: true,
    }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
