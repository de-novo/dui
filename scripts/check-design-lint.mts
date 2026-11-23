import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const repoRoot = process.cwd();
const duiBin = path.join(repoRoot, "target", "debug", "dui");
assert.ok(fs.existsSync(duiBin), `missing built dui binary at ${duiBin}`);

interface DesignFinding {
  kind: string;
  severity: string;
  nodeId: string;
  tag: string;
  summary: string;
  measured: string;
  expected: string;
  path: string;
  width: number;
}

interface AuditReport {
  meetsGoal: boolean;
  summary: { verifiedCells: number; totalCells: number };
  designAudit?: {
    total: number;
    errors: number;
    warnings: number;
    info: number;
    findings: DesignFinding[];
  };
}

function runDui(args: string[], homeDir: string): unknown {
  const stdout = execFileSync(duiBin, args, {
    encoding: "utf8",
    env: { ...process.env, HOME: homeDir },
  });
  return JSON.parse(stdout) as unknown;
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-design-lint-"));
const duiHome = path.join(tmp, ".dui-home");

try {
  const siteDir = path.join(tmp, "site");
  fs.mkdirSync(siteDir, { recursive: true });
  fs.writeFileSync(
    path.join(siteDir, "index.html"),
    `<!doctype html>
<html lang="en">
  <head><meta charset="utf-8" /><title>Design Defect Demo</title></head>
  <body style="margin:0;padding:16px;font-family:sans-serif;background:#ffffff">
    <h1 style="width:520px;margin:0;color:#111111;background:#ffffff">Overflowing Header</h1>
    <button id="bad-empty-btn" style="position:absolute;left:16px;top:80px;width:18px;height:18px;padding:0;border:1px solid #000"></button>
    <button id="low-contrast-btn" style="position:absolute;left:22px;top:84px;width:120px;height:48px;color:#b8b8b8;background:#ffffff;font-size:14px">Low Contrast</button>
    <button id="nested-span-btn" style="margin-top:120px;width:120px;height:44px;color:#111111;background:#ffffff"><span>Save Order</span></button>
    <label for="search-input" style="display:block;margin-top:12px">Search Orders</label>
    <input id="search-input" style="width:200px;height:44px" />
    <div style="width:320px;overflow-x:auto;margin-top:12px">
      <div id="scrolled-cell" style="width:600px;height:32px;color:#111111;background:#ffffff">Horizontally scrollable content</div>
    </div>
  </body>
</html>`,
  );

  const manifest = path.join(siteDir, "dui.json");
  fs.writeFileSync(
    manifest,
    JSON.stringify({
      host: "static",
      widths: [375, 768, 1280],
      concurrency: 2,
    }),
  );

  const outDir = path.join(tmp, "out");
  runDui(["run", siteDir, manifest, outDir], duiHome);
  const audit = runDui(["audit", outDir], duiHome) as AuditReport;

  assert.equal(audit.summary.verifiedCells, 3, "parallel capture verifies all 3 widths");
  assert.ok(audit.designAudit, "audit includes structured designAudit");
  const mobileFindings = audit.designAudit.findings.filter((f) => f.width === 375);
  const kinds = new Set(mobileFindings.map((f) => f.kind));

  assert.ok(
    kinds.has("horizontal-overflow"),
    `missing horizontal-overflow: ${[...kinds].join(",")}`,
  );
  assert.ok(kinds.has("small-touch-target"), `missing small-touch-target: ${[...kinds].join(",")}`);
  assert.ok(
    kinds.has("unlabeled-interactive"),
    `missing unlabeled-interactive: ${[...kinds].join(",")}`,
  );
  assert.ok(kinds.has("element-overlap"), `missing element-overlap: ${[...kinds].join(",")}`);
  assert.ok(kinds.has("low-contrast"), `missing low-contrast: ${[...kinds].join(",")}`);

  // False-positive regression checks: nested span button, <label for> input, and overflow-x:auto child must not trigger defects
  assert.ok(
    !mobileFindings.some(
      (f) => f.kind === "unlabeled-interactive" && f.nodeId === "#nested-span-btn",
    ),
    `false-positive unlabeled-interactive on #nested-span-btn: ${JSON.stringify(mobileFindings)}`,
  );
  assert.ok(
    !mobileFindings.some((f) => f.kind === "unlabeled-interactive" && f.nodeId === "#search-input"),
    `false-positive unlabeled-interactive on #search-input: ${JSON.stringify(mobileFindings)}`,
  );
  assert.ok(
    !mobileFindings.some((f) => f.kind === "horizontal-overflow" && f.nodeId === "#scrolled-cell"),
    `false-positive horizontal-overflow on #scrolled-cell: ${JSON.stringify(mobileFindings)}`,
  );

  console.log(
    JSON.stringify({
      status: "ok",
      parallelVerifiedCells: audit.summary.verifiedCells,
      designFindingsTotal: audit.designAudit.total,
      mobileDefectKinds: [...kinds].sort(),
    }),
  );
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
