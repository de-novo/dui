import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const repoRoot = process.cwd();
const duiBin = path.join(repoRoot, "target", "debug", "dui");
assert.ok(fs.existsSync(duiBin), `missing built dui binary at ${duiBin}`);

interface RenderNode {
  id: string;
  tag: string;
  text: string;
  rect: { x: number; y: number; width: number; height: number };
}

interface ReportCell {
  path: string;
  width: number;
  status: string;
  image?: string;
  observation?: {
    renderTree?: RenderNode[];
    ios?: { udid: string; deviceName: string; deepLink?: string };
  };
}

interface RunReport {
  coverage: { verified: number; renderable: number };
  cells: ReportCell[];
}

interface DiffReport {
  summary: { changed: number };
  screens: Array<{
    path: string;
    cells: Array<{
      renderDiff?: {
        textChanged: number;
        changes: Array<{
          kind: string;
          id: string;
          beforeText?: string;
          afterText?: string;
        }>;
      };
    }>;
  }>;
}

function runDui(args: string[], envOverrides: Record<string, string>): unknown {
  const stdout = execFileSync(duiBin, args, {
    encoding: "utf8",
    env: { ...process.env, ...envOverrides },
  });
  return JSON.parse(stdout) as unknown;
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-ios-"));
const duiHome = path.join(tmp, ".dui-home");

try {
  const appDir = path.join(tmp, "ios-app");
  fs.mkdirSync(appDir, { recursive: true });
  const stateFile = path.join(tmp, "mock-simctl-state.txt");
  const swiftFile = path.join(appDir, "CheckoutView.swift");
  fs.writeFileSync(swiftFile, 'let payLabel = "Pay $29"\n');

  const pngA = Buffer.from(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
    "base64",
  );
  const pngB = Buffer.from(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
    "base64",
  );
  const pngAPath = path.join(tmp, "a.png");
  const pngBPath = path.join(tmp, "b.png");
  fs.writeFileSync(pngAPath, pngA);
  fs.writeFileSync(pngBPath, pngB);

  const mockSimctl = path.join(tmp, "mock-simctl");
  fs.writeFileSync(
    mockSimctl,
    `#!${process.execPath}
import fs from "node:fs";
const args = process.argv.slice(2);
const statePath = ${JSON.stringify(stateFile)};
const swiftPath = ${JSON.stringify(swiftFile)};
const pngA = ${JSON.stringify(pngAPath)};
const pngB = ${JSON.stringify(pngBPath)};

if (args[0] === "list") {
  process.stdout.write(JSON.stringify({
    devices: {
      "com.apple.CoreSimulator.SimRuntime.iOS-18-4": [
        { udid: "SIM-IPHONE-16-PRO", name: "iPhone 16 Pro", state: "Booted", isAvailable: true }
      ]
    }
  }));
  process.exit(0);
}

if (args[0] === "openurl") {
  fs.writeFileSync(statePath, args[2] ?? "app://home");
  process.exit(0);
}

if (args[0] === "io" && args[2] === "screenshot") {
  const outPath = args[3];
  const src = fs.readFileSync(swiftPath, "utf8");
  fs.copyFileSync(src.includes("Pay $39") ? pngB : pngA, outPath);
  process.exit(0);
}
process.exit(0);
`,
    { mode: 0o755 },
  );

  const mockTreeCmd = path.join(tmp, "mock-ios-tree");
  fs.writeFileSync(
    mockTreeCmd,
    `#!${process.execPath}
import fs from "node:fs";
const statePath = ${JSON.stringify(stateFile)};
const swiftPath = ${JSON.stringify(swiftFile)};
const link = fs.existsSync(statePath) ? fs.readFileSync(statePath, "utf8") : "app://home";
const src = fs.readFileSync(swiftPath, "utf8");
const payLabel = src.includes("Pay $39") ? "Pay $39" : "Pay $29";

if (link.includes("checkout")) {
  process.stdout.write(JSON.stringify({
    type: "Application",
    frame: { x: 0, y: 0, width: 393, height: 852 },
    children: [
      {
        type: "XCUIElementTypeStaticText",
        AXUniqueId: "title",
        AXLabel: "Checkout",
        frame: { x: 16, y: 80, width: 200, height: 28 }
      },
      {
        type: "XCUIElementTypeButton",
        AXUniqueId: "pay_btn",
        AXLabel: payLabel,
        enabled: true,
        frame: { x: 16, y: 740, width: 361, height: 50 }
      }
    ]
  }));
} else {
  process.stdout.write(JSON.stringify({
    type: "Application",
    frame: { x: 0, y: 0, width: 393, height: 852 },
    children: [
      {
        type: "XCUIElementTypeStaticText",
        AXUniqueId: "home_title",
        AXLabel: "iOS Home",
        frame: { x: 16, y: 80, width: 200, height: 28 }
      }
    ]
  }));
}
`,
    { mode: 0o755 },
  );

  const manifest = path.join(appDir, "dui.json");
  fs.writeFileSync(
    manifest,
    JSON.stringify({
      host: "ios",
      bundleId: "com.example.shop",
      path: "/home",
      deepLink: "app://home",
      widths: [393],
      settleMs: 0,
      deterministicStatusBar: true,
      scenarios: [{ path: "/checkout", deepLink: "app://checkout" }],
    }),
  );

  const env = { HOME: duiHome, DUI_SIMCTL: mockSimctl, DUI_IOS_TREE_CMD: mockTreeCmd };

  // 1. Capture /home and /checkout via IosMobileSurface
  const out1 = path.join(tmp, "out-1");
  runDui(["run", appDir, manifest, out1], env);
  const report1 = JSON.parse(fs.readFileSync(path.join(out1, "report.json"), "utf8")) as RunReport;
  assert.equal(report1.coverage.verified, 2, `expected 2 verified iOS cells`);
  const checkoutCell = report1.cells.find((c) => c.path === "/checkout");
  assert.ok(checkoutCell?.image && fs.existsSync(path.join(out1, checkoutCell.image)));
  assert.equal(checkoutCell.observation?.ios?.udid, "SIM-IPHONE-16-PRO");
  assert.equal(checkoutCell.observation?.ios?.deviceName, "iPhone 16 Pro");
  const payNode = checkoutCell.observation?.renderTree?.find((n) => n.id === "pay_btn#0");
  assert.equal(payNode?.text, "Pay $29");
  assert.equal(payNode?.tag, "button");

  // 2. Edit Swift source -> cache invalidates -> element-level renderDiff detects Pay $29 -> Pay $39
  fs.writeFileSync(swiftFile, 'let payLabel = "Pay $39"\n');
  const out2 = path.join(tmp, "out-2");
  runDui(["run", appDir, manifest, out2], env);
  const diff = runDui(["diff", out1, out2], env) as DiffReport;
  assert.ok(diff.summary.changed >= 1, `expected >=1 changed iOS cell`);
  const checkoutDiff = diff.screens
    .find((s) => s.path === "/checkout")
    ?.cells.find((c) => c.renderDiff);
  assert.ok(checkoutDiff?.renderDiff, "expected renderDiff on iOS /checkout");
  assert.ok(
    checkoutDiff.renderDiff.changes.some(
      (ch) => ch.id === "pay_btn#0" && ch.beforeText === "Pay $29" && ch.afterText === "Pay $39",
    ),
    `expected pay_btn#0 text change: ${JSON.stringify(checkoutDiff.renderDiff)}`,
  );

  // 3. Missing simctl honestly reports unrenderable (no-ios-simulator)
  const missingReply = runDui(["run", appDir, manifest, path.join(tmp, "out-missing")], {
    HOME: duiHome,
    DUI_SIMCTL: "/nonexistent/simctl",
  }) as { status: string; surface: string; reason: string };
  assert.equal(missingReply.status, "unrenderable");
  assert.equal(missingReply.surface, "ios-mobile");
  assert.equal(missingReply.reason, "no-ios-simulator");

  console.log(
    JSON.stringify({
      status: "ok",
      verifiedCells: report1.coverage.verified,
      checkoutNodes: checkoutCell.observation?.renderTree?.length ?? 0,
      diffChangedCells: diff.summary.changed,
      missingEnvReason: missingReply.reason,
    }),
  );
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
