// Verify branch-aware diff (`dui diff --base <branch>`) and `--changed-since <ref>` (#66):
// 1. Initialize a real git repo on `main`, capture baseline with `dui run`.
// 2. Create `feat/ui-update` branch, modify `main.tsx`, commit, and run `dui run --changed-since main`.
// 3. Verify `changedSince.gitChangedFiles` & `affectedFiles` include `main.tsx`.
// 4. Verify CLI `dui diff --base main --project <REPO>` and MCP `diff { baseBranch: "main", project }`
//    return identical JSON with `beforeBranch: "main"`, `afterBranch: "feat/ui-update"`, and changed cells.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
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

  console.log(
    JSON.stringify({
      ok: true,
      beforeBranch: diffBody.beforeBranch,
      afterBranch: diffBody.afterBranch,
      changedCells: diffBody.summary.changed,
      diffPixels: changedCell.pixelDiff.diffPixels,
      changedSince,
      cliMcpParity: true,
    }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
