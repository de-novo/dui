// Verify M1 deterministic planning, alias/re-export resolution, CLI/MCP equivalence,
// insufficient-budget denominator preservation, and fill-in loop termination.
import assert from "node:assert/strict";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { createInterface } from "node:readline";

type Fact = {
  file: string;
  owner: string;
  kind: string;
  label?: string;
  shape?: unknown;
  predicate?: { operator: string; value?: unknown };
  textWhenTrue?: string | null;
  textWhenFalse?: string | null;
};

type CellReport = {
  scenario: string;
  path: string;
  state: string;
  width: number;
  planIndex: number;
  status: string;
  reason?: string | null;
  input: Record<string, unknown>;
};

type RunReport = {
  compiler: string;
  budget: number;
  remainingPlans: number;
  scenarios: {
    scenario: string;
    stateBudget: number;
    plannedStates: number;
    remainingStates: number;
    termination: string;
  }[];
  coverage: {
    verified: number;
    renderable: number;
    attempted: number;
    unrenderable: number;
    branchOutcomes: number;
    observedBranchOutcomes: number;
  };
  unobservedBranches: string[];
  runtimeOnlyInstrumentedBranches: string[];
  cells: CellReport[];
};

const root = resolve(import.meta.dirname, "..");
const fixture = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-m1-gates-"));

function cloneFixture(name: string): string {
  const dir = join(stage, name);
  cpSync(fixture, dir, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(join(fixture, "node_modules"), join(dir, "node_modules"), "dir");
  return dir;
}

function normalizeFacts(facts: Fact[]) {
  return facts
    .map((f) => ({
      file: f.file,
      owner: f.owner,
      kind: f.kind,
      label: f.label ?? null,
      shape: f.shape ?? null,
      predicate: f.predicate
        ? { operator: f.predicate.operator, value: f.predicate.value ?? null }
        : null,
      textWhenTrue: f.textWhenTrue ?? null,
      textWhenFalse: f.textWhenFalse ?? null,
    }))
    .sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b)));
}

function normalizeRun(report: RunReport) {
  return {
    budget: report.budget,
    remainingPlans: report.remainingPlans,
    scenarios: report.scenarios.map((s) => ({
      scenario: s.scenario,
      stateBudget: s.stateBudget,
      plannedStates: s.plannedStates,
      remainingStates: s.remainingStates,
      termination: s.termination,
    })),
    coverage: report.coverage,
    unobservedBranches: report.unobservedBranches,
    runtimeOnlyInstrumentedBranches: report.runtimeOnlyInstrumentedBranches,
    cells: report.cells.map((c) => ({
      scenario: c.scenario,
      path: c.path,
      state: c.state,
      width: c.width,
      planIndex: c.planIndex,
      status: c.status,
      reason: c.reason ?? null,
      input: c.input,
    })),
  };
}

function runCli(args: string[], home: string): { status: number; body: unknown } {
  const res = spawnSync(binary, args, {
    env: { ...process.env, HOME: home },
    encoding: "utf8",
  });
  assert(res.stdout.trim().length > 0, `empty CLI stdout for ${args.join(" ")}: ${res.stderr}`);
  return { status: res.status ?? 1, body: JSON.parse(res.stdout) };
}

async function openMcp(home: string) {
  const child = spawn(binary, ["mcp"], {
    env: { ...process.env, HOME: home },
    stdio: ["pipe", "pipe", "pipe"],
  });
  const lines = createInterface({ input: child.stdout });
  const queue: string[] = [];
  const waiters: ((line: string) => void)[] = [];
  lines.on("line", (line) => {
    const next = waiters.shift();
    if (next) next(line);
    else queue.push(line);
  });
  let seq = 1;
  async function request(method: string, params: Record<string, unknown>) {
    const id = seq++;
    child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    const raw =
      queue.length > 0
        ? queue.shift()!
        : await new Promise<string>((resolveLine) => waiters.push(resolveLine));
    const reply = JSON.parse(raw) as {
      result?: { isError?: boolean; content?: { text: string }[] };
      error?: { message: string };
    };
    assert(!reply.error, `MCP error for ${method}: ${reply.error?.message}`);
    return reply.result!;
  }
  await request("initialize", {
    protocolVersion: "2025-06-18",
    capabilities: {},
    clientInfo: { name: "dui-m1-gates", version: "0.0.1" },
  });
  return {
    async call(name: string, args: Record<string, unknown>) {
      const result = await request("tools/call", { name, arguments: args });
      const text = result.content?.[0]?.text ?? "";
      return { isError: Boolean(result.isError), body: JSON.parse(text) };
    },
    close() {
      child.stdin.end();
      child.kill();
    },
  };
}

try {
  const home = join(stage, "home");

  // 1. Alias and re-export resolution must produce identical semantic facts under TS 7.
  const reexportDir = cloneFixture("reexport");
  const directDir = cloneFixture("direct");
  const aliasDir = cloneFixture("alias");
  const baseMain = readFileSync(join(fixture, "main.tsx"), "utf8");
  writeFileSync(
    join(directDir, "main.tsx"),
    baseMain
      .replace(
        'import { choose as useChoice } from "./hooks";',
        'import { useState } from "react";',
      )
      .replaceAll("useChoice", "useState"),
  );
  writeFileSync(
    join(aliasDir, "main.tsx"),
    baseMain.replace(
      'import { choose as useChoice } from "./hooks";',
      'import { useState as useChoice } from "react";',
    ),
  );

  const scans = [directDir, aliasDir, reexportDir].map((dir) => {
    const out = execFileSync(binary, ["scan", dir, join(dir, "dui.json")], {
      encoding: "utf8",
      env: { ...process.env, HOME: home },
    });
    return JSON.parse(out) as { compiler: string; diagnostics: unknown[]; facts: Fact[] };
  });
  for (const scan of scans) {
    assert.deepEqual(scan.diagnostics, []);
    assert(
      scan.compiler.startsWith("7.") || scan.compiler.startsWith("6."),
      `expected stable TS 6/7 compiler, got ${scan.compiler}`,
    );
  }
  const [directNorm, aliasNorm, reexportNorm] = scans.map((s) => normalizeFacts(s.facts));
  assert.deepEqual(directNorm, aliasNorm, "aliased import facts diverged from direct import");
  assert.deepEqual(directNorm, reexportNorm, "re-export import facts diverged from direct import");
  assert.equal(directNorm.filter((f) => f.kind === "slot").length, 2);
  assert.equal(directNorm.filter((f) => f.kind === "branch").length, 2);

  // 2. Compare CLI and MCP plans, cell IDs, verdicts, and separate cell/branch coverage (full budget).
  const mcp = await openMcp(home);
  try {
    const cliScan = runCli(["scan", reexportDir, join(reexportDir, "dui.json")], home);
    const mcpScan = await mcp.call("scan", {
      project: reexportDir,
      manifest: join(reexportDir, "dui.json"),
    });
    assert.equal(cliScan.status, 0);
    assert.equal(mcpScan.isError, false);
    assert.deepEqual(cliScan.body, mcpScan.body, "CLI and MCP scan outputs diverged");

    const cliFullOut = join(stage, "cli-full");
    const mcpFullOut = join(stage, "mcp-full");
    const cliFull = runCli(["run", reexportDir, join(reexportDir, "dui.json"), cliFullOut], home);
    const mcpFull = await mcp.call("run", {
      project: reexportDir,
      manifest: join(reexportDir, "dui.json"),
      output: mcpFullOut,
    });
    assert.equal(cliFull.status, 0);
    assert.equal(mcpFull.isError, false);
    const cliFullReport = cliFull.body as RunReport;
    const mcpFullReport = mcpFull.body as RunReport;
    assert.deepEqual(
      normalizeRun(cliFullReport),
      normalizeRun(mcpFullReport),
      "CLI and MCP full-budget plans/verdicts diverged",
    );
    assert.equal(cliFullReport.coverage.verified, cliFullReport.coverage.renderable);
    assert.equal(cliFullReport.coverage.unrenderable, 0);
    assert.equal(cliFullReport.coverage.branchOutcomes, 4);
    assert.equal(cliFullReport.coverage.observedBranchOutcomes, 4);
    assert.deepEqual(cliFullReport.unobservedBranches, []);
    assert.deepEqual(cliFullReport.runtimeOnlyInstrumentedBranches, []);
    assert.equal(cliFullReport.scenarios[0].termination, "exhausted");

    // 3. Insufficient budget: CLI and MCP agree, every planned cell is preserved in report and graph.
    const lowBudgetManifest = join(reexportDir, "dui-low-budget.json");
    const spec = JSON.parse(readFileSync(join(reexportDir, "dui.json"), "utf8")) as Record<
      string,
      unknown
    >;
    writeFileSync(lowBudgetManifest, JSON.stringify({ ...spec, stateBudget: 2 }, null, 2));

    const cliLowOut = join(stage, "cli-low");
    const mcpLowOut = join(stage, "mcp-low");
    const cliLow = runCli(["run", reexportDir, lowBudgetManifest, cliLowOut], home);
    const mcpLow = await mcp.call("run", {
      project: reexportDir,
      manifest: lowBudgetManifest,
      output: mcpLowOut,
    });
    assert.equal(cliLow.status, 5);
    assert.equal(mcpLow.isError, true);
    const cliLowReport = cliLow.body as RunReport;
    const mcpLowReport = mcpLow.body as RunReport;
    assert.deepEqual(
      normalizeRun(cliLowReport),
      normalizeRun(mcpLowReport),
      "CLI and MCP insufficient-budget plans/verdicts diverged",
    );
    assert.equal(cliLowReport.scenarios[0].termination, "budget-limit");
    assert.equal(cliLowReport.scenarios[0].plannedStates, 2);
    assert(cliLowReport.remainingPlans > 0);
    assert.equal(cliLowReport.coverage.attempted, 4);
    assert(cliLowReport.coverage.renderable > cliLowReport.coverage.attempted);
    assert.equal(cliLowReport.cells.length, cliLowReport.coverage.renderable);
    const unattempted = cliLowReport.cells.filter((c) => c.reason === "not-rendered");
    assert.equal(unattempted.length, cliLowReport.coverage.renderable - 4);
    const lowGraph = JSON.parse(readFileSync(join(cliLowOut, "graph.json"), "utf8")) as {
      nodes: { kind: string }[];
    };
    assert.equal(
      lowGraph.nodes.filter((n) => n.kind === "cell").length,
      cliLowReport.cells.length,
      "graph.json dropped planned cells under an insufficient budget",
    );

    // 4. Stagnation termination: fill-in loop stops on no-improvement and preserves remaining cells.
    const stagnantManifest = join(reexportDir, "dui-stagnant.json");
    writeFileSync(
      stagnantManifest,
      JSON.stringify({ ...spec, stateBudget: 16, maxNoImprovementRounds: 1 }, null, 2),
    );
    const stagnantOut = join(stage, "cli-stagnant");
    const stagnant = runCli(["run", reexportDir, stagnantManifest, stagnantOut], home);
    assert.equal(stagnant.status, 5);
    const stagnantReport = stagnant.body as RunReport;
    assert.equal(stagnantReport.scenarios[0].termination, "no-improvement");
    assert.equal(stagnantReport.coverage.observedBranchOutcomes, 4);
    assert(stagnantReport.remainingPlans > 0);

    const summary = {
      fixture: "fixtures/toggle",
      sourceSha256: createHash("sha256").update(baseMain).digest("hex"),
      compiler: scans[0].compiler,
      aliasReexportMatch: true,
      fullBudget: {
        cellsVerified: cliFullReport.coverage.verified,
        cellsRenderable: cliFullReport.coverage.renderable,
        branchesObserved: cliFullReport.coverage.observedBranchOutcomes,
        branchesTarget: cliFullReport.coverage.branchOutcomes,
        termination: cliFullReport.scenarios[0].termination,
        cliMcpIdentical: true,
      },
      insufficientBudget: {
        budget: 2,
        attemptedCells: cliLowReport.coverage.attempted,
        renderableCells: cliLowReport.coverage.renderable,
        remainingPlans: cliLowReport.remainingPlans,
        termination: cliLowReport.scenarios[0].termination,
        cliMcpIdentical: true,
      },
      stagnationGuard: {
        plannedStates: stagnantReport.scenarios[0].plannedStates,
        remainingPlans: stagnantReport.remainingPlans,
        branchesObserved: stagnantReport.coverage.observedBranchOutcomes,
        termination: stagnantReport.scenarios[0].termination,
      },
    };
    console.log(JSON.stringify(summary));
  } finally {
    mcp.close();
  }
} finally {
  rmSync(stage, { recursive: true, force: true });
}
