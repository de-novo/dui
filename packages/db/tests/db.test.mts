import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "node:test";
import { openDuiDatabase, schema } from "../src/index.ts";

test("fresh database applies forward-only migrations once and supports Drizzle queries", async () => {
  const dir = mkdtempSync(join(tmpdir(), "dui-db-test-"));
  const dbPath = join(dir, "dui.db");
  try {
    const first = openDuiDatabase(dbPath);
    assert.deepEqual(first.appliedMigrations, ["0001_initial"]);

    const projectId = "a".repeat(64);
    const worktreeId = "b".repeat(64);
    const commitId = "c".repeat(64);
    await first.db.insert(schema.projects).values({
      id: projectId,
      displayName: "demo",
      primaryRoot: "/repo/demo",
    });
    await first.db.insert(schema.worktrees).values({
      id: worktreeId,
      projectId,
      path: "/repo/demo",
      appRelative: "",
      branch: "main",
    });
    await first.db.insert(schema.commits).values({
      id: commitId,
      projectId,
      gitSha: "abcdef1",
      dirty: false,
      treeDigest: null,
    });
    await first.db.insert(schema.runs).values({
      id: "0001-run",
      projectId,
      worktreeId,
      commitId,
      kindJson: { kind: "capture" },
      createdAtMs: 1_795_200_000_000,
      environmentJson: { os: "macos", duiVersion: "0.0.2", versions: {} },
      sourceFilesJson: ["src/App.tsx"],
    });
    first.close();

    const second = openDuiDatabase(dbPath);
    assert.deepEqual(second.appliedMigrations, []);
    const history = await second.listHistory();
    assert.equal(history.length, 1);
    assert.equal(history[0].projectName, "demo");
    assert.equal(history[0].branch, "main");
    assert.equal(history[0].dirty, false);
    second.close();
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("Drizzle reads a SQLite database initialized and imported by the Rust dui binary", async (t) => {
  const repoRoot = resolve(import.meta.dirname, "..", "..", "..");
  const binary = join(repoRoot, "target", "debug", "dui");
  if (!existsSync(binary)) {
    t.skip("target/debug/dui not built");
    return;
  }
  const home = mkdtempSync(join(tmpdir(), "dui-db-bridge-"));
  try {
    const projectRoot = "/repo/sample-app";
    const projectId = createHash("sha256").update(projectRoot).digest("hex");
    const runDir = join(home, ".dui", "projects", projectId, "runs", "0001795200000000-r1");
    mkdirSync(runDir, { recursive: true });
    writeFileSync(
      join(home, ".dui", "projects", projectId, "project.json"),
      JSON.stringify({ name: "sample-app" }),
    );
    writeFileSync(
      join(runDir, "snapshot.json"),
      JSON.stringify({
        id: "0001795200000000-r1",
        projectId,
        projectName: "sample-app",
        projectRoot,
        worktree: projectRoot,
        app: "",
        branch: "main",
        commit: "1234567",
        dirty: false,
        timestampMs: 1795200000000,
      }),
    );
    writeFileSync(
      join(runDir, "report.json"),
      JSON.stringify({
        os: "macos",
        compiler: "7.0.2",
        cells: [
          {
            path: "/",
            scenario: "default",
            state: "s0",
            width: 1280,
            planIndex: 0,
            status: "verified",
          },
        ],
      }),
    );

    const out = execFileSync(binary, ["history"], {
      env: { ...process.env, HOME: home },
      encoding: "utf8",
    });
    const parsed = JSON.parse(out) as { snapshots: { id: string; projectName: string }[] };
    assert.equal(parsed.snapshots.length, 1);
    assert.equal(parsed.snapshots[0].projectName, "sample-app");

    const dbPath = join(home, ".dui", "dui.db");
    assert.ok(existsSync(dbPath));
    const handle = openDuiDatabase(dbPath);
    assert.deepEqual(handle.appliedMigrations, []);
    const rows = await handle.listHistory();
    assert.equal(rows.length, 1);
    assert.equal(rows[0].runId, "0001795200000000-r1");
    assert.equal(rows[0].projectName, "sample-app");
    assert.equal(rows[0].branch, "main");
    handle.close();

    // Deleting dui.db and re-running dui history restores history losslessly from disk.
    for (const suffix of ["", "-wal", "-shm"]) {
      rmSync(dbPath + suffix, { force: true });
    }
    const rebuilt = JSON.parse(
      execFileSync(binary, ["history"], {
        env: { ...process.env, HOME: home },
        encoding: "utf8",
      }),
    ) as { snapshots: { id: string }[] };
    assert.equal(rebuilt.snapshots.length, 1);
    assert.equal(rebuilt.snapshots[0].id, "0001795200000000-r1");
  } finally {
    rmSync(home, { recursive: true, force: true });
  }
});
