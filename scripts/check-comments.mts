// Verify v0.0.5 review comments across SQLite, CLI, MCP, and embedded viewer (#38):
// 1. Capture a fixture run and attach comments to screen, cell (width), and transition targets via CLI and MCP.
// 2. Confirm CLI `dui comment list` and MCP `comment_list` return identical JSON from `~/.dui/dui.db`.
// 3. Resolve and reopen comments via MCP/CLI and verify lifecycle transitions persist.
// 4. Capture a second run on the same project and confirm project-level comments remain attached at the same location.
// 5. Open the embedded `dui open --no-open` viewer and verify `/api/comments` GET/POST and Chrome inspector rendering.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

type CommentRecord = {
  id: string;
  projectId: string;
  runId: string;
  author: string;
  body: string;
  status: "open" | "resolved";
  location: {
    kind: string;
    path?: string | null;
    width?: number | null;
    to?: string | null;
  };
};

type CommentList = {
  summary: { total: number; open: number; resolved: number };
  comments: CommentRecord[];
};

const root = resolve(import.meta.dirname, "..");
const template = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-comments-check-"));

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
  const appDir = join(stage, "review-app");
  cpSync(template, appDir, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(join(template, "node_modules"), join(appDir, "node_modules"), "dir");

  const manifest = join(appDir, "dui.json");

  const run1 = runDui(["run", appDir, manifest], env);
  assert(run1.body.output, JSON.stringify(run1.body));
  const run1Dir = run1.body.output as string;
  assert(run1Dir && existsSync(run1Dir), "first run archive exists");

  // 1. Add comments anchored to screen, cell (width), and transition via CLI and MCP.
  const screenAdded = runDui(
    [
      "comment",
      "add",
      JSON.stringify({
        run: run1Dir,
        target: { kind: "screen", path: "/" },
        author: "alice",
        body: "Clarify header contrast on home screen",
      }),
    ],
    env,
  );
  assert.equal(screenAdded.status, 0, JSON.stringify(screenAdded.body));
  const screenComment = screenAdded.body.comment as CommentRecord;
  assert.equal(screenComment.status, "open");
  assert.equal(screenComment.location.kind, "screen");
  assert.equal(screenComment.location.path, "/");

  const cellAdded = callMcp<{ comment: CommentRecord }>(
    "comment_add",
    {
      run: run1Dir,
      target: { kind: "cell", path: "/", width: 375 },
      author: "bob",
      body: "Mobile CTA wraps at 375px",
    },
    env,
  );
  assert.equal(cellAdded.comment.location.kind, "cell");
  assert.equal(cellAdded.comment.location.width, 375);

  const edgeAdded = runDui(
    [
      "comment",
      "add",
      JSON.stringify({
        run: run1Dir,
        target: { kind: "transition", path: "/", to: "/details" },
        author: "alice",
        body: "Verify navigation transition",
      }),
    ],
    env,
  );
  assert.equal(edgeAdded.status, 0, JSON.stringify(edgeAdded.body));
  const edgeComment = edgeAdded.body.comment as CommentRecord;
  assert.equal(edgeComment.location.kind, "transition");
  assert.equal(edgeComment.location.to, "/details");

  // 2. CLI and MCP list responses must be identical.
  const cliList = runDui(["comment", "list", JSON.stringify({ run: run1Dir })], env);
  assert.equal(cliList.status, 0);
  const mcpList = callMcp<CommentList>("comment_list", { run: run1Dir }, env);
  assert.deepEqual(cliList.body, mcpList);
  assert.equal(mcpList.summary.total, 3);
  assert.equal(mcpList.summary.open, 3);

  // 3. Resolve via MCP, then reopen via CLI.
  const resolved = callMcp<{ comment: CommentRecord }>(
    "comment_resolve",
    { id: cellAdded.comment.id },
    env,
  );
  assert.equal(resolved.comment.status, "resolved");

  const reopened = runDui(["comment", "reopen", cellAdded.comment.id], env);
  assert.equal(reopened.status, 0, JSON.stringify(reopened.body));
  assert.equal(reopened.body.comment.status, "open");

  // 4. Second run on the same project still sees all project comments at the same location.
  const run2 = runDui(["run", appDir, manifest], env);
  assert(run2.body.output, JSON.stringify(run2.body));
  const run2Dir = run2.body.output as string;
  const acrossRuns = callMcp<CommentList>("comment_list", { run: run2Dir, path: "/" }, env);
  assert.equal(acrossRuns.summary.total, 3, "project comments survive across new runs");

  // 5. Embedded `dui open --no-open` viewer serves `/api/comments` and renders in Chrome.
  const viewer = spawn(binary, ["open", "--no-open", run2Dir], {
    env: { ...process.env, ...env, NO_COLOR: "1" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let viewerUrl = "";
  let stderr = "";
  const urlReady = new Promise<string>((resolveUrl, rejectUrl) => {
    const timer = setTimeout(
      () => rejectUrl(new Error(`dui open timed out; stderr: ${stderr}`)),
      30_000,
    );
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
    viewer.on("exit", (code) => {
      if (!viewerUrl) {
        clearTimeout(timer);
        rejectUrl(new Error(`dui open exited ${code}: ${stderr}`));
      }
    });
  });

  try {
    viewerUrl = await urlReady;
    const runsRes = await fetch(`${viewerUrl}/archive/runs.json`);
    assert.equal(runsRes.status, 200);
    const runs = (await runsRes.json()) as { key: string; projectId: string }[];
    assert(runs.length >= 1, "viewer exposes run");

    const apiListRes = await fetch(
      `${viewerUrl}/api/comments?projectId=${encodeURIComponent(runs[0]!.projectId)}&path=%2F`,
    );
    assert.equal(apiListRes.status, 200);
    const apiList = (await apiListRes.json()) as CommentList;
    assert.equal(apiList.summary.total, 3);

    const apiAddRes = await fetch(`${viewerUrl}/api/comments`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        op: "add",
        run: runs[0]!.key,
        target: { kind: "screen", path: "/" },
        author: "viewer-user",
        body: "Posted from viewer API",
      }),
    });
    assert.equal(apiAddRes.status, 200);
    const apiAdded = (await apiAddRes.json()) as { comment: CommentRecord };
    assert.equal(apiAdded.comment.body, "Posted from viewer API");

    // Verify Chrome renders the inspector comments panel and the posted comment.
    const graphJson = JSON.parse(readFileSync(join(run2Dir, "graph.json"), "utf8")) as {
      overview?: { path?: string }[];
    };
    const rootScreenPath = graphJson.overview?.[0]?.path ?? "/";
    const detailUrl = `${viewerUrl}/?run=${encodeURIComponent(runs[0]!.key)}&screen=${encodeURIComponent(`route:${rootScreenPath}`)}&uiLang=en`;
    const chrome = spawnSync(
      chromeExecutable(),
      [
        "--headless=new",
        "--disable-gpu",
        "--no-sandbox",
        "--virtual-time-budget=6000",
        "--dump-dom",
        detailUrl,
      ],
      { encoding: "utf8", timeout: 30_000 },
    );
    assert.equal(chrome.status, 0, chrome.stderr);
    assert(
      chrome.stdout.includes("Clarify header contrast on home screen") &&
        chrome.stdout.includes("Posted from viewer API"),
      "viewer DOM renders review comments from SQLite",
    );
  } finally {
    viewer.kill("SIGTERM");
  }

  console.log(
    JSON.stringify({
      ok: true,
      comments: 4,
      cliMcpParity: true,
      crossRunPersistence: true,
      viewerVerified: true,
    }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
