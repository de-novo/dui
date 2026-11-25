import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync } from "node:fs";
import { cpus, release, tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const binary = join(root, "target/debug/dui");
const fixture = join(root, "fixtures/replay");
const temp = mkdtempSync(join(tmpdir(), "dui-inspect-"));
const project = join(temp, "project");
const directory = join(temp, "capture");
try {
  mkdirSync(project);
  for (const file of [
    "index.html",
    "tsconfig.json",
    "dui.json",
    "package.json",
    "package-lock.json",
  ])
    copyFileSync(join(fixture, file), join(project, file));
  copyFileSync(join(import.meta.dirname, "fixtures/inspect.tsx"), join(project, "main.tsx"));
  symlinkSync(join(fixture, "node_modules"), join(project, "node_modules"), "dir");
  const env = { ...process.env, HOME: temp };
  const git = (args: string[]) =>
    execFileSync("git", args, { cwd: project, env, encoding: "utf8" }).trim();
  git(["init", "--quiet"]);
  git([
    "add",
    "main.tsx",
    "package.json",
    "package-lock.json",
    "dui.json",
    "index.html",
    "tsconfig.json",
  ]);
  git([
    "-c",
    "user.name=dui fixture",
    "-c",
    "user.email=fixture@example.invalid",
    "-c",
    "commit.gpgsign=false",
    "commit",
    "-qm",
    "fixture",
  ]);
  const commit = git(["rev-parse", "HEAD"]);
  const run = spawnSync(binary, ["run", project, join(project, "dui.json"), directory], {
    env,
    encoding: "utf8",
  });
  assert.ok(run.status === 0 || run.status === 5, run.stderr + run.stdout);
  const originalReport = readFileSync(join(directory, "report.json"), "utf8");
  const [cell] = JSON.parse(originalReport).cells;
  assert.ok(
    cell.observation?.instances,
    JSON.stringify({
      status: cell.status,
      reason: cell.reason,
      path: cell.observation?.path,
      ready: cell.observation?.ready,
      requests: cell.observation?.requests,
    }),
  );
  const base = { state: cell.state, width: cell.width };
  function inspect(extra: Record<string, unknown> = {}, reason?: string) {
    const selection = { ...base, ...extra };
    const cli = spawnSync(binary, ["inspect", directory, JSON.stringify(selection)], {
      env,
      encoding: "utf8",
    });
    const value = JSON.parse(cli.stdout);
    assert.equal(cli.status, reason ? 2 : 0, cli.stdout);
    if (reason) assert.equal(value.reason, reason);
    const request = {
      jsonrpc: "2.0",
      id: 1,
      method: "tools/call",
      params: { name: "inspect", arguments: { directory, selection } },
    };
    const mcp = spawnSync(binary, ["mcp"], {
      input: JSON.stringify(request) + "\n",
      env,
      encoding: "utf8",
    });
    const result = JSON.parse(mcp.stdout).result;
    assert.equal(result.isError, Boolean(reason));
    assert.deepEqual(JSON.parse(result.content[0].text), value);
    return value.controls as {
      slot: string;
      instance: string;
      owner: string;
      label: string;
      value: unknown;
      editable: boolean;
      type: string;
      source: { file: string };
    }[];
  }
  const controls = inspect();
  const menus = controls.filter((c) => c.label === "open");
  assert.equal(menus.length, 2);
  assert.deepEqual(new Set(menus.map((c) => c.value)), new Set([true, false]));
  assert.equal(new Set(menus.map((c) => c.instance)).size, 2);
  assert.equal(new Set(controls.map((c) => c.instance)).size, 2);
  for (const menu of menus) {
    assert.equal(menu.owner, "Menu");
    assert.equal(menu.source.file, "main.tsx");
    assert.equal(menu.type, "boolean");
    assert.deepEqual(inspect({ slot: menu.slot, instance: menu.instance, value: !menu.value }), [
      menu,
    ]);
  }
  inspect({ slot: menus[0].slot }, "ambiguous-instance");
  const mode = controls.find((c) => c.label === "mode")!;
  inspect({ slot: mode.slot, instance: mode.instance, value: "edit" });
  inspect({ slot: mode.slot, instance: mode.instance, value: "invalid" }, "invalid-value");
  const unresolved = controls.find((c) => c.label === "metadata")!;
  assert.equal(unresolved.editable, false);
  inspect({ slot: unresolved.slot, instance: unresolved.instance, value: null }, "read-only");
  assert.deepEqual(inspect(), controls);
  function live(request: Record<string, unknown>, via: "cli" | "mcp" = "cli", reason?: string) {
    const mcp = via === "mcp";
    const result = spawnSync(binary, mcp ? ["mcp"] : ["live", JSON.stringify(request)], {
      env,
      encoding: "utf8",
      timeout: 120000,
      input: mcp
        ? JSON.stringify({
            jsonrpc: "2.0",
            id: 1,
            method: "tools/call",
            params: { name: "live", arguments: request },
          }) + "\n"
        : undefined,
    });
    assert.equal(
      result.status,
      mcp || !reason ? 0 : 2,
      result.stdout + result.stderr + String(result.error ?? ""),
    );
    const decoded = JSON.parse(result.stdout);
    if (mcp) assert.equal(decoded.result.isError, Boolean(reason), result.stdout);
    const body = mcp ? JSON.parse(decoded.result.content[0].text) : decoded;
    if (reason) assert.equal(body.reason, reason);
    return body;
  }
  const opening = {
    op: "open",
    project,
    manifest: join(project, "dui.json"),
    run: directory,
    cell: base,
    requestId: "first-open",
  };
  const opened = live(opening);
  const target = { session: opened.session, cell: base };
  try {
    assert.deepEqual(live(opening, "mcp"), opened);
    const current = () => live({ ...target, op: "inspect" }, "mcp").controls as typeof controls;
    const before = current();
    const first = before.find((c) => c.label === "open" && c.value === false)!;
    const other = before.find((c) => c.label === "open" && c.instance !== first.instance)!;
    const mode = before.find((c) => c.label === "mode" && c.instance === first.instance)!;
    const change = {
      ...target,
      op: "set",
      slot: first.slot,
      instance: first.instance,
      value: true,
      expectedRevision: 0,
      requestId: "open-menu",
      expect: { selector: "section:first-of-type button:first-child", text: "Close" },
    };
    live({ ...change, requestId: "bad-type", value: "true" }, "mcp", "invalid-value");
    live(
      { ...change, requestId: "wrong-cell", cell: { ...base, width: 1 } },
      "cli",
      "cell-mismatch",
    );
    const readOnly = before.find((c) => c.label === "metadata" && c.instance === first.instance)!;
    live({ ...change, slot: readOnly.slot, requestId: "read-only" }, "cli", "read-only");
    assert.deepEqual(current(), before);
    const one = live(change);
    assert.equal(one.status, "verified", JSON.stringify(one));
    assert.equal(one.observedValue, true);
    assert.deepEqual(live(change, "mcp"), one);
    live({ ...change, value: false }, "cli", "request-id-reused");
    live({ ...change, requestId: "stale" }, "mcp", "stale-revision");
    assert.deepEqual(
      current().find((c) => c.slot === other.slot && c.instance === other.instance),
      other,
    );
    const modeChange = {
      ...change,
      slot: mode.slot,
      value: "edit",
      expectedRevision: 1,
      requestId: "mode",
      expect: {
        selector: "section:first-of-type button:nth-child(2)",
        text: "intentionally wrong",
      },
    };
    const two = live(modeChange, "mcp");
    assert.equal(two.status, "unverified");
    assert.equal(two.reason, "semantic-mismatch");
    assert.equal(two.observedValue, "edit");
    const mismatchSave = live(
      { ...target, op: "save", expectedRevision: 2, requestId: "save-mismatch" },
      "mcp",
    );
    assert.equal(mismatchSave.status, "unverified");
    assert.equal(mismatchSave.reason, "semantic-mismatch");
    const modeReset = live({
      ...target,
      op: "reset",
      slot: mode.slot,
      instance: mode.instance,
      expectedRevision: 2,
      requestId: "reset-mode",
      expect: { selector: modeChange.expect.selector, text: "view" },
    });
    assert.equal(modeReset.status, "verified");
    assert.equal(modeReset.observedValue, "view");
    const reset = live(
      {
        ...target,
        op: "reset",
        slot: first.slot,
        instance: first.instance,
        expectedRevision: 3,
        requestId: "reset-menu",
        expect: { selector: change.expect.selector, text: "Open" },
      },
      "mcp",
    );
    assert.equal(reset.status, "verified");
    assert.equal(reset.observedValue, false);
    assert.deepEqual(current(), before);
    assert.equal(live({ ...change, requestId: "last", expectedRevision: 4 }).status, "verified");
    const save = { ...target, op: "save", expectedRevision: 5, requestId: "save-verified" };
    const saved = live(save);
    assert.equal(saved.status, "verified");
    assert.notEqual(mismatchSave.imageSha256, saved.imageSha256);
    assert.equal(readFileSync(join(directory, "report.json"), "utf8"), originalReport);
    assert.deepEqual(live(save, "mcp"), saved);
    live({ ...save, requestId: "stale-save", expectedRevision: 4 }, "cli", "stale-revision");
    const archived = JSON.parse(readFileSync(join(saved.output, "report.json"), "utf8"));
    const snapshot = JSON.parse(readFileSync(join(saved.output, "snapshot.json"), "utf8"));
    assert.equal(snapshot.commit, commit);
    assert.equal(snapshot.id, saved.run);
    const history = JSON.parse(execFileSync(binary, ["history"], { env, encoding: "utf8" }));
    assert.ok(
      history.snapshots.some(
        (s: { id: string; output: string }) => s.id === saved.run && s.output === saved.output,
      ),
    );
    assert.ok(snapshot.worktree);
    const [savedCell] = archived.cells;
    assert.equal(savedCell.assertion.visibleText, "Close");
    assert.equal(savedCell.assertion.observedValue, true);
    const png = readFileSync(join(saved.output, saved.image));
    assert.deepEqual(png.subarray(0, 8), Buffer.from("89504e470d0a1a0a", "hex"));
    assert.equal(png.readUInt32BE(16), base.width);
    assert.equal(createHash("sha256").update(png).digest("hex"), saved.imageSha256);
    assert.ok(readFileSync(join(saved.output, "graph.json"), "utf8").includes(saved.image));
    const inspected = JSON.parse(
      execFileSync(binary, ["inspect", saved.output, JSON.stringify(saved.cell)], {
        env,
        encoding: "utf8",
      }),
    );
    assert.ok(
      inspected.controls.some(
        (c: typeof first) =>
          c.slot === first.slot && c.instance === first.instance && c.value === true,
      ),
    );
    const resized = live({
      ...target,
      op: "resize",
      width: base.width + 13,
      expectedRevision: saved.revision,
      requestId: "resize-after-save",
    });
    live(
      { ...save, requestId: "save-without-state-assertion", expectedRevision: resized.revision },
      "mcp",
      "no-state-assertion-for-revision",
    );
    const edited = live({
      ...change,
      requestId: "state-at-new-width",
      expectedRevision: resized.revision,
    });
    const updated = live({
      ...save,
      requestId: "save-new-width",
      expectedRevision: edited.revision,
    });
    assert.equal(updated.cell.width, resized.layout.width);
    const resizedReport = JSON.parse(readFileSync(join(updated.output, "report.json"), "utf8"));
    assert.deepEqual(resizedReport.live.sourceCell, base);
    assert.equal(resizedReport.cells[0].height, resized.layout.height);
  } finally {
    live({ ...target, op: "close" });
  }
  live({ ...target, op: "inspect" }, "cli", "session-expired");
  const start = performance.now();
  const next = live({ ...opening, requestId: "second-open" }, "mcp");
  try {
    const coldOpenMs = performance.now() - start;
    assert.notEqual(next.session, target.session);
    assert.equal(next.revision, 0);
    const items = live({ op: "inspect", session: next.session, cell: base })
      .controls as typeof controls;
    assert.deepEqual(
      items
        .filter((c) => c.label === "open")
        .map((c) => c.value)
        .sort(),
      [false, true],
    );
    const menu = items.find((c) => c.label === "open" && c.value === false)!;
    const samples: number[] = [];
    assert.deepEqual(next.launches, { hostBuilds: 1, browserLaunches: 1 });
    let counts = next.launches;
    for (let i = 0; i < 30; i++) {
      const value = i % 2 === 0;
      const result = live({
        op: "set",
        session: next.session,
        cell: base,
        slot: menu.slot,
        instance: menu.instance,
        value,
        expectedRevision: i,
        requestId: "benchmark-" + i,
        expect: {
          selector: "section:first-of-type button:first-child",
          text: value ? "Close" : "Open",
        },
      });
      assert.equal(result.status, "verified");
      assert.equal(result.observedValue, value);
      assert.ok(Number.isFinite(result.elapsedMs) && result.elapsedMs > 0);
      samples.push(result.elapsedMs);
      counts = result.launches;
    }
    assert.deepEqual(counts, next.launches);
    const sorted = samples.sort((a, b) => a - b);
    console.log(
      JSON.stringify({
        fixture: "replay/inspect",
        sourceSha256: createHash("sha256")
          .update(readFileSync(join(project, "main.tsx")))
          .digest("hex"),
        lockSha256: createHash("sha256")
          .update(readFileSync(join(project, "package-lock.json")))
          .digest("hex"),
        node: process.version,
        osRelease: release(),
        cpu: cpus()[0]?.model,
        ...next.environment,
        coldOpenMs,
        samples: samples.length,
        p50Ms: sorted[Math.ceil(samples.length * 0.5) - 1],
        p95Ms: sorted[Math.ceil(samples.length * 0.95) - 1],
        cold: next.launches,
        warm: {
          hostBuilds: counts.hostBuilds - next.launches.hostBuilds,
          browserLaunches: counts.browserLaunches - next.launches.browserLaunches,
        },
      }),
    );
  } finally {
    live({ op: "close", session: next.session }, "mcp");
  }
  console.log(
    "Live CLI/MCP edits: boolean and enum set/reset, independent visible text, revision replay, cleanup and fresh reopen passed.",
  );
  console.log(
    "CLI and MCP agree on two Menu instances, boolean and enum values, and read-only state.",
  );
} finally {
  rmSync(temp, { recursive: true, force: true });
}
