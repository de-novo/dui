import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const binary = path.resolve("target/debug/dui");
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-design-audit-"));
const project = path.join(tmp, "project");
const config = path.join(project, "dui.json");
function run(args: string[], code = 0) {
  const result = spawnSync(binary, args, {
    encoding: "utf8",
    env: { ...process.env, HOME: path.join(tmp, "home") },
  });
  assert.equal(result.status, code, `${args.join(" ")}: ${result.stdout} ${result.stderr}`);
  return JSON.parse(result.stdout);
}

try {
  fs.cpSync("examples/design-audit", project, { recursive: true });
  const manifest = JSON.parse(fs.readFileSync(config, "utf8"));
  const specPath = path.resolve(project, manifest.designSpec);
  const spec = JSON.parse(fs.readFileSync(specPath, "utf8"));
  const [rule] = spec.rules;
  const token = spec.tokens[rule.token] as string;
  const changedToken =
    "#" + (parseInt(token.slice(1), 16) ^ 0xffffff).toString(16).padStart(6, "0");
  const baseline = run(["run", project, config]).output;
  const design = ["--design", config];
  const passed = run(["audit", baseline, ...design]).designAudit;
  assert.equal(passed.status, "passed");
  assert.equal(passed.auditedCells, manifest.widths.length);
  assert.deepEqual(passed.findings, []);

  const html = path.join(project, "index.html");
  fs.writeFileSync(html, fs.readFileSync(html, "utf8").replace(token, changedToken));
  const drifted = run(["run", project, config]).output;
  const archived = JSON.parse(fs.readFileSync(path.join(drifted, "report.json"), "utf8"));
  const drift = run(["audit", drifted, ...design, "--goal", "100"], 5).designAudit;
  assert.equal(drift.status, "findings");
  assert.equal(drift.findings.length, archived.cells.length);
  for (const [i, finding] of drift.findings.entries()) {
    const cell = archived.cells[i];
    const node = cell.observation.renderTree.find((n: { tag: string }) => n.tag === rule.tag);
    assert.equal(finding.cell.width, cell.width);
    assert.equal(finding.nodeId, node.id);
    assert.deepEqual(finding.rect, node.rect);
    assert.equal(finding.measured, node.style[rule.property]);
    assert.equal(finding.expected, token);
    assert.equal(finding.specRef, `${manifest.designSpec}#/rules/0`);
  }
  const markdown = "# Action color\n" + token;
  const markdownConfig = path.join(project, "markdown.json");
  fs.writeFileSync(path.join(project, "DESIGN.md"), markdown);
  fs.writeFileSync(
    path.join(project, "check-text.mjs"),
    `import { readFileSync } from "node:fs";
     const { version, designSpec } = JSON.parse(readFileSync(0, "utf8"));
     if (version !== 1 || designSpec.content !== ${JSON.stringify(markdown)}) process.exit(1);
     console.log(JSON.stringify({ findings: [] }));`,
  );
  fs.writeFileSync(
    markdownConfig,
    JSON.stringify({ designSpec: "DESIGN.md", plugins: ["check-text.mjs"] }),
  );
  assert.equal(run(["audit", drifted, "--design", markdownConfig]).designAudit.status, "passed");
  spec.tokens[rule.token] = changedToken;
  fs.writeFileSync(specPath, JSON.stringify(spec));
  assert.equal(run(["audit", drifted, ...design]).designAudit.status, "passed");

  rule.property = "missingMeasurement";
  fs.writeFileSync(specPath, JSON.stringify(spec));
  const missing = run(["audit", drifted, ...design], 5).designAudit;
  assert.equal(missing.status, "unverified");
  assert.equal(missing.unverified.length, archived.cells.length);

  const checker = path.resolve(project, manifest.plugins[0]);
  fs.writeFileSync(checker, 'console.log(JSON.stringify({findings:[{nodeId:"invented"}]}))');
  assert.match(run(["audit", drifted, ...design], 3).reason, /captured node evidence/);
  assert.equal(run(["audit", drifted]).designAudit, undefined);
  const incomplete = path.join(tmp, "incomplete");
  fs.mkdirSync(incomplete);
  fs.writeFileSync(
    path.join(incomplete, "report.json"),
    JSON.stringify({
      cells: archived.cells.map((cell: Record<string, unknown>) => ({ ...cell, observation: {} })),
    }),
  );
  const unavailable = run(["audit", incomplete, ...design], 5).designAudit;
  assert.equal(unavailable.auditedCells, 0);
  assert.equal(unavailable.unverified.length, archived.cells.length);
  console.log(
    JSON.stringify({
      baseline: passed.status,
      drift: drift.findings.map(({ nodeId, expected, measured }: Record<string, unknown>) => ({
        nodeId,
        expected,
        measured,
      })),
      updatedTokens: "passed",
      missingMeasurement: missing.status,
    }),
  );
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
