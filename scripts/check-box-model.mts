import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const repoRoot = process.cwd();
const duiBin = path.join(repoRoot, "target", "debug", "dui");
assert.ok(fs.existsSync(duiBin), `missing built dui binary at ${duiBin}`);

interface BoxSpacing {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

interface RenderNode {
  id: string;
  tag: string;
  parentId?: string;
  rect: { x: number; y: number; width: number; height: number };
  boxModel?: {
    margin?: BoxSpacing;
    padding?: BoxSpacing;
    border?: BoxSpacing;
    borderRadius?: string;
    gap?: string;
  };
  attributes?: Record<string, string>;
  style: Record<string, string>;
}

interface RunReport {
  coverage: { verified: number };
  cells: Array<{
    path: string;
    width: number;
    observation?: {
      renderTree?: RenderNode[];
    };
  }>;
}

function runDui(args: string[], homeDir: string): unknown {
  const stdout = execFileSync(duiBin, args, {
    encoding: "utf8",
    env: { ...process.env, HOME: homeDir },
  });
  return JSON.parse(stdout) as unknown;
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-box-model-"));
const duiHome = path.join(tmp, ".dui-home");

try {
  const siteDir = path.join(tmp, "site");
  fs.mkdirSync(siteDir, { recursive: true });
  fs.writeFileSync(
    path.join(siteDir, "index.html"),
    `<!doctype html>
<html lang="en">
  <head><meta charset="utf-8" /><title>Box Model Demo</title></head>
  <body style="margin:0;padding:24px;font-family:sans-serif">
    <section id="card" data-testid="pricing-card" style="display:flex;flex-direction:column;gap:12px;margin:16px 8px 20px 8px;padding:16px 20px;border:2px solid #222;border-radius:10px;width:280px">
      <h1 style="margin:0;font-size:24px;line-height:32px">Pro Plan</h1>
      <button id="cta" type="button" aria-label="Upgrade now" style="padding:10px 16px;border-radius:6px">Upgrade</button>
    </section>
  </body>
</html>`,
  );

  const manifest = path.join(siteDir, "dui.json");
  fs.writeFileSync(manifest, JSON.stringify({ host: "static", widths: [375] }));

  const out1 = path.join(tmp, "out-1");
  runDui(["run", siteDir, manifest, out1], duiHome);
  const report1 = JSON.parse(fs.readFileSync(path.join(out1, "report.json"), "utf8")) as RunReport;
  assert.equal(report1.coverage.verified, 1);

  const nodes = report1.cells[0]?.observation?.renderTree ?? [];
  const card = nodes.find((n) => n.id === "[data-testid=pricing-card]");
  const cta = nodes.find((n) => n.id === "#cta");

  assert.ok(card?.boxModel, `expected boxModel on pricing card: ${JSON.stringify(card)}`);
  assert.deepEqual(card.boxModel.margin, { top: 16, right: 8, bottom: 20, left: 8 });
  assert.deepEqual(card.boxModel.padding, { top: 16, right: 20, bottom: 16, left: 20 });
  assert.deepEqual(card.boxModel.border, { top: 2, right: 2, bottom: 2, left: 2 });
  assert.equal(card.boxModel.borderRadius, "10px");
  assert.equal(card.boxModel.gap, "12px");
  assert.equal(card.attributes?.["data-testid"], "pricing-card");

  assert.ok(cta, "expected #cta button node");
  assert.equal(cta.parentId, "[data-testid=pricing-card]");
  assert.equal(cta.attributes?.["aria-label"], "Upgrade now");
  assert.equal(cta.attributes?.type, "button");
  assert.deepEqual(cta.boxModel?.padding, { top: 10, right: 16, bottom: 10, left: 16 });

  // Second run hits cache -> confirm boxModel & attributes survive cache round-trip!
  const out2 = path.join(tmp, "out-2");
  runDui(["run", siteDir, manifest, out2], duiHome);
  const report2 = JSON.parse(fs.readFileSync(path.join(out2, "report.json"), "utf8")) as RunReport;
  const cachedCard = report2.cells[0]?.observation?.renderTree?.find(
    (n) => n.id === "[data-testid=pricing-card]",
  );
  assert.deepEqual(cachedCard?.boxModel?.padding, { top: 16, right: 20, bottom: 16, left: 20 });
  assert.equal(cachedCard?.boxModel?.gap, "12px");

  console.log(
    JSON.stringify({
      status: "ok",
      nodesCollected: nodes.length,
      cardMargin: card.boxModel.margin,
      cardPadding: card.boxModel.padding,
      cardGap: card.boxModel.gap,
      ctaParentId: cta.parentId,
      cachePreservedBoxModel: true,
    }),
  );
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
