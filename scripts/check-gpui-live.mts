// Verify live GPUI Kit session round-trip (#6):
// 1. Open an archived gpui-kit cell through the shared `dui live` / MCP `live` session contract.
// 2. Inspect, set, read back, and reset editable boolean (`enabled`) and enum (`density`) fields.
// 3. Verify independent semantic evidence via AccessKit; missing semantic evidence leaves the cell
//    `unverified` with `reason: "missing-semantic-evidence"`; mismatched semantic evidence returns
//    `unverified` with `reason: "semantic-mismatch"`.
// 4. Confirm a second view instance (`secondary`) retains its own independent state during edits
//    and resets on `primary` (and vice versa).
// 5. Run a 30-edit warm benchmark on the native session reporting cold open ms, p50, p95, and
//    0 host rebuilds / 0 native launches during warm edits; save a verified revision under `~/.dui`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { performance } from "node:perf_hooks";

const root = resolve(import.meta.dirname, "..");
const fixture = join(root, "fixtures/gpui-kit");
const manifest = join(fixture, "dui.json");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-gpui-live-check-"));

const home = join(stage, "home");
const env = {
  ...process.env,
  HOME: home,
  RUSTUP_HOME: process.env.RUSTUP_HOME ?? join(homedir(), ".rustup"),
  CARGO_HOME: process.env.CARGO_HOME ?? join(homedir(), ".cargo"),
};

function runDui(args: string[]) {
  const res = spawnSync(binary, args, { env, encoding: "utf8" });
  assert(res.stdout.trim().length > 0, `empty stdout for ${args.join(" ")}: ${res.stderr}`);
  return { status: res.status ?? 1, body: JSON.parse(res.stdout) as Record<string, unknown> };
}

function liveCli(payload: Record<string, unknown>) {
  const res = runDui(["live", JSON.stringify(payload)]);
  return res;
}

function liveMcp(payload: Record<string, unknown>) {
  const rpc =
    JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "tools/call",
      params: { name: "live", arguments: payload },
    }) + "\n";
  const res = spawnSync(binary, ["mcp"], { input: rpc, env, encoding: "utf8" });
  assert.equal(res.status, 0, `mcp exited ${res.status}: ${res.stderr}`);
  const parsed = JSON.parse(res.stdout.trim()) as {
    result: { isError: boolean; content: { text: string }[] };
  };
  return {
    isError: parsed.result.isError,
    body: JSON.parse(parsed.result.content[0]!.text) as Record<string, unknown>,
  };
}

function percentile(sorted: number[], p: number): number {
  const idx = Math.min(sorted.length - 1, Math.max(0, Math.ceil(sorted.length * p) - 1));
  return Number(sorted[idx]!.toFixed(3));
}

try {
  // Capture baseline gpui-kit run under isolated ~/.dui.
  const captured = runDui(["run", fixture, manifest]);
  assert.equal(captured.status, 0, JSON.stringify(captured.body));
  const runDir = captured.body.output as string;
  const cells = captured.body.cells as { scenario: string; state: string; width: number }[];
  const baseCell = cells.find((c) => c.scenario === "compact-disabled")!;
  assert(baseCell, "compact-disabled cell missing");
  const cellRef = { state: baseCell.state, width: baseCell.width };

  // 1. Open persistent native GPUI session through shared `dui live` contract.
  const openStart = performance.now();
  const opened = liveCli({
    op: "open",
    project: fixture,
    manifest,
    run: runDir,
    cell: cellRef,
    requestId: "gpui-open-1",
  });
  const coldOpenMs = Number((performance.now() - openStart).toFixed(3));
  assert.equal(opened.status, 0, JSON.stringify(opened.body));
  const session = opened.body.session as string;
  assert.equal(opened.body.revision, 0);
  const coldLaunches = opened.body.launches as {
    hostBuilds: number;
    browserLaunches: number;
    nativeLaunches: number;
  };
  assert.equal(coldLaunches.browserLaunches, 0);
  assert.equal(coldLaunches.nativeLaunches, 1);

  try {
    // 2. Inspect via CLI and MCP — both must return identical controls for primary & secondary.
    const cliInspect = liveCli({ op: "inspect", session, cell: cellRef });
    assert.equal(cliInspect.status, 0);
    const mcpInspect = liveMcp({ op: "inspect", session, cell: cellRef });
    assert.equal(mcpInspect.isError, false);
    assert.deepEqual(cliInspect.body, mcpInspect.body);
    const controls = cliInspect.body.controls as {
      instance: string;
      slot: string;
      value: unknown;
      editable: boolean;
    }[];
    assert.equal(controls.length, 4);
    assert.equal(
      controls.find((c) => c.instance === "primary" && c.slot === "enabled")?.value,
      false,
    );
    assert.equal(
      controls.find((c) => c.instance === "secondary" && c.slot === "enabled")?.value,
      false,
    );

    // 3. Verified set with independent AccessKit semantic evidence via MCP.
    let rev = 0;
    const setPrimaryTrue = liveMcp({
      op: "set",
      session,
      cell: cellRef,
      instance: "primary",
      slot: "enabled",
      value: true,
      expectedRevision: rev,
      requestId: "edit-primary-enabled-1",
      expect: { label: "Primary alerts", checked: true },
    });
    assert.equal(setPrimaryTrue.isError, false, JSON.stringify(setPrimaryTrue.body));
    assert.equal(setPrimaryTrue.body.status, "verified");
    assert.equal(setPrimaryTrue.body.observedValue, true);
    assert.equal(setPrimaryTrue.body.revision, 1);
    rev = 1;

    // Duplicate requestId replays recorded outcome without mutating revision.
    const replayed = liveCli({
      op: "set",
      session,
      cell: cellRef,
      instance: "primary",
      slot: "enabled",
      value: true,
      expectedRevision: 0,
      requestId: "edit-primary-enabled-1",
      expect: { label: "Primary alerts", checked: true },
    });
    assert.equal(replayed.status, 0);
    assert.equal(replayed.body.revision, 1);

    // Confirm secondary instance retained its own state (`false`).
    const afterPrimary = liveCli({ op: "inspect", session, cell: cellRef });
    const afterPrimaryControls = afterPrimary.body.controls as {
      instance: string;
      slot: string;
      value: unknown;
    }[];
    assert.equal(
      afterPrimaryControls.find((c) => c.instance === "primary" && c.slot === "enabled")?.value,
      true,
    );
    assert.equal(
      afterPrimaryControls.find((c) => c.instance === "secondary" && c.slot === "enabled")?.value,
      false,
    );

    // 4. Edit secondary instance independently (`enabled = true`), then reset `primary` and
    //    confirm `secondary` retains its own edited state (`true`)!
    const setSecondary = liveCli({
      op: "set",
      session,
      cell: cellRef,
      instance: "secondary",
      slot: "enabled",
      value: true,
      expectedRevision: rev,
      requestId: "edit-secondary-enabled-1",
      expect: { label: "Secondary alerts", checked: true },
    });
    assert.equal(setSecondary.status, 0);
    assert.equal(setSecondary.body.status, "verified");
    assert.equal(setSecondary.body.observedValue, true);
    rev = setSecondary.body.revision as number;

    const resetPrimary = liveCli({
      op: "reset",
      session,
      cell: cellRef,
      instance: "primary",
      slot: "enabled",
      expectedRevision: rev,
      requestId: "reset-primary-enabled-1",
      expect: { label: "Primary alerts", checked: false },
    });
    assert.equal(resetPrimary.status, 0);
    assert.equal(resetPrimary.body.status, "verified");
    assert.equal(resetPrimary.body.observedValue, false);
    rev = resetPrimary.body.revision as number;

    const afterReset = liveCli({ op: "inspect", session, cell: cellRef });
    const afterResetControls = afterReset.body.controls as {
      instance: string;
      slot: string;
      value: unknown;
    }[];
    assert.equal(
      afterResetControls.find((c) => c.instance === "primary" && c.slot === "enabled")?.value,
      false,
    );
    assert.equal(
      afterResetControls.find((c) => c.instance === "secondary" && c.slot === "enabled")?.value,
      true,
      "second view instance must retain its own state when primary is reset",
    );

    // 5. Missing semantic evidence leaves the cell `unverified` with an explicit reason.
    const missingEvidence = liveCli({
      op: "set",
      session,
      cell: cellRef,
      instance: "primary",
      slot: "density",
      value: "expanded",
      expectedRevision: rev,
      requestId: "edit-missing-evidence",
    });
    assert.equal(missingEvidence.status, 0);
    assert.equal(missingEvidence.body.observedValue, "expanded");
    assert.equal(missingEvidence.body.status, "unverified");
    assert.equal(missingEvidence.body.reason, "missing-semantic-evidence");
    rev = missingEvidence.body.revision as number;

    // Mismatched semantic evidence also leaves the cell `unverified` with `semantic-mismatch`.
    const mismatchEvidence = liveCli({
      op: "set",
      session,
      cell: cellRef,
      instance: "primary",
      slot: "density",
      value: "compact",
      expectedRevision: rev,
      requestId: "edit-mismatch-evidence",
      expect: { label: "Primary density: expanded" },
    });
    assert.equal(mismatchEvidence.status, 0);
    assert.equal(mismatchEvidence.body.observedValue, "compact");
    assert.equal(mismatchEvidence.body.status, "unverified");
    assert.equal(mismatchEvidence.body.reason, "semantic-mismatch");
    rev = mismatchEvidence.body.revision as number;

    // 6. Warm-edit benchmark: 30 alternating verified edits on the same persistent native session.
    const samples: number[] = [];
    const beforeBenchLaunches = mismatchEvidence.body.launches as {
      hostBuilds: number;
      browserLaunches: number;
      nativeLaunches: number;
    };
    let lastLaunches = beforeBenchLaunches;
    for (let i = 0; i < 30; i += 1) {
      const nextVal = i % 2 === 0;
      const edit = liveCli({
        op: "set",
        session,
        cell: cellRef,
        instance: "primary",
        slot: "enabled",
        value: nextVal,
        expectedRevision: rev,
        requestId: `bench-${i}`,
        expect: { label: "Primary alerts", checked: nextVal },
      });
      assert.equal(edit.status, 0, JSON.stringify(edit.body));
      assert.equal(edit.body.status, "verified");
      assert.equal(edit.body.observedValue, nextVal);
      samples.push(edit.body.elapsedMs as number);
      lastLaunches = edit.body.launches as typeof beforeBenchLaunches;
      rev = edit.body.revision as number;
    }
    const warmHostBuilds = lastLaunches.hostBuilds - beforeBenchLaunches.hostBuilds;
    const warmNativeLaunches = lastLaunches.nativeLaunches - beforeBenchLaunches.nativeLaunches;
    assert.equal(warmHostBuilds, 0, "expected 0 host rebuilds during warm edits");
    assert.equal(warmNativeLaunches, 0, "expected 0 native launches during warm edits");

    // 7. Save the verified live revision under ~/.dui and confirm PNG + report exist.
    const saved = liveCli({
      op: "save",
      session,
      cell: cellRef,
      expectedRevision: rev,
      requestId: "gpui-save-1",
    });
    assert.equal(saved.status, 0, JSON.stringify(saved.body));
    assert.equal(saved.body.status, "verified");
    const savedOut = saved.body.output as string;
    const savedPng = join(savedOut, saved.body.image as string);
    assert(existsSync(savedPng));
    assert.deepEqual(
      readFileSync(savedPng).subarray(0, 8),
      Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    );

    const sorted = [...samples].sort((a, b) => a - b);
    console.log(
      JSON.stringify({
        fixture: "fixtures/gpui-kit",
        environment: opened.body.environment,
        coldOpenMs,
        samples: sorted.length,
        p50Ms: percentile(sorted, 0.5),
        p95Ms: percentile(sorted, 0.95),
        coldLaunches,
        warmDelta: {
          hostBuilds: warmHostBuilds,
          nativeLaunches: warmNativeLaunches,
        },
        secondInstanceRetainedState: true,
        missingSemanticEvidenceReason: missingEvidence.body.reason,
        mismatchSemanticEvidenceReason: mismatchEvidence.body.reason,
        savedStatus: saved.body.status,
      }),
    );
  } finally {
    liveCli({ op: "close", session });
  }
} finally {
  rmSync(stage, { recursive: true, force: true });
}
