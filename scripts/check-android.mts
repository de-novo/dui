import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
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
  attributes?: Record<string, string>;
}

interface ReportCell {
  path: string;
  width: number;
  status: string;
  image?: string;
  observation?: {
    renderTree?: RenderNode[];
    android?: { serial: string; deepLink?: string };
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

// A missing environment captures nothing, so dui exits 5 with the unrenderable reply.
function runDuiMissingEnv(args: string[], envOverrides: Record<string, string>): unknown {
  const result = spawnSync(duiBin, args, {
    encoding: "utf8",
    env: { ...process.env, ...envOverrides },
  });
  assert.equal(result.status, 5, result.stderr);
  return JSON.parse(result.stdout) as unknown;
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-android-"));
const duiHome = path.join(tmp, ".dui-home");

try {
  const appDir = path.join(tmp, "android-app");
  fs.mkdirSync(appDir, { recursive: true });
  const stateFile = path.join(tmp, "mock-adb-state.txt");
  const versionFile = path.join(appDir, "MainScreen.kt");
  fs.writeFileSync(versionFile, 'const val PAY_LABEL = "Pay $29"\n');

  // Minimal valid 1x1 PNG (RGBA)
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

  const mockAdb = path.join(tmp, "mock-adb");
  fs.writeFileSync(
    mockAdb,
    `#!${process.execPath}
import fs from "node:fs";
const args = process.argv.slice(2);
const statePath = ${JSON.stringify(stateFile)};
const versionPath = ${JSON.stringify(versionFile)};
const pngA = ${JSON.stringify(pngAPath)};
const pngB = ${JSON.stringify(pngBPath)};

if (args[0] === "devices") {
  process.stdout.write("List of devices attached\\nemulator-5554\\tdevice\\n");
  process.exit(0);
}

const joined = args.join(" ");
if (joined.includes("am start")) {
  const dIdx = args.indexOf("-d");
  const link = dIdx >= 0 ? args[dIdx + 1] : "app://home";
  fs.writeFileSync(statePath, link);
  process.exit(0);
}

if (joined.includes("screencap -p")) {
  const src = fs.readFileSync(versionPath, "utf8");
  const file = src.includes("Pay $39") ? pngB : pngA;
  process.stdout.write(fs.readFileSync(file));
  process.exit(0);
}

if (joined.includes("uiautomator dump")) {
  const link = fs.existsSync(statePath) ? fs.readFileSync(statePath, "utf8") : "app://home";
  const src = fs.readFileSync(versionPath, "utf8");
  const payLabel = src.includes("Pay $39") ? "Pay $39" : "Pay $29";
  if (link.includes("checkout")) {
    process.stdout.write(\`<?xml version='1.0' encoding='UTF-8' standalone='yes' ?>
<hierarchy rotation="0">
  <node index="0" text="" resource-id="com.example.shop:id/root" class="android.widget.FrameLayout" package="com.example.shop" clickable="false" enabled="true" bounds="[0,0][1080,2400]">
    <node index="0" text="Checkout" resource-id="com.example.shop:id/title" class="android.widget.TextView" package="com.example.shop" clickable="false" enabled="true" bounds="[48,120][640,200]" />
    <node index="1" text="\${payLabel}" resource-id="com.example.shop:id/pay_btn" class="android.widget.Button" package="com.example.shop" clickable="true" enabled="true" bounds="[48,2100][1032,2244]" />
  </node>
</hierarchy>\`);
  } else {
    process.stdout.write(\`<?xml version='1.0' encoding='UTF-8' standalone='yes' ?>
<hierarchy rotation="0">
  <node index="0" text="" resource-id="com.example.shop:id/root" class="android.widget.FrameLayout" package="com.example.shop" clickable="false" enabled="true" bounds="[0,0][1080,2400]">
    <node index="0" text="Shop Home" resource-id="com.example.shop:id/home_title" class="android.widget.TextView" package="com.example.shop" clickable="false" enabled="true" bounds="[48,120][640,200]" />
  </node>
</hierarchy>\`);
  }
  process.exit(0);
}
process.exit(0);
`,
    { mode: 0o755 },
  );

  const manifest = path.join(appDir, "dui.json");
  fs.writeFileSync(
    manifest,
    JSON.stringify({
      host: "android",
      package: "com.example.shop",
      path: "/home",
      deepLink: "app://home",
      widths: [1080],
      settleMs: 0,
      scenarios: [{ path: "/checkout", deepLink: "app://checkout" }],
    }),
  );

  // 1. Capture /home and /checkout via AndroidSurface
  const out1 = path.join(tmp, "out-1");
  runDui(["run", appDir, manifest, out1], { HOME: duiHome, DUI_ADB: mockAdb });
  const report1 = JSON.parse(fs.readFileSync(path.join(out1, "report.json"), "utf8")) as RunReport;
  assert.equal(
    report1.coverage.verified,
    2,
    `expected 2 verified Android cells, got ${report1.coverage.verified}`,
  );
  const checkoutCell = report1.cells.find((c) => c.path === "/checkout");
  assert.ok(checkoutCell?.image && fs.existsSync(path.join(out1, checkoutCell.image)));
  assert.equal(checkoutCell.observation?.android?.serial, "emulator-5554");
  const payNode = checkoutCell.observation?.renderTree?.find((n) => n.id === "pay_btn#0");
  assert.equal(payNode?.text, "Pay $29");
  assert.equal(payNode?.tag, "button");

  // 2. Edit Kotlin source -> cache invalidates -> element-level renderDiff catches Pay $29 -> Pay $39
  fs.writeFileSync(versionFile, 'const val PAY_LABEL = "Pay $39"\n');
  const out2 = path.join(tmp, "out-2");
  runDui(["run", appDir, manifest, out2], { HOME: duiHome, DUI_ADB: mockAdb });
  const diff = runDui(["diff", out1, out2], { HOME: duiHome, DUI_ADB: mockAdb }) as DiffReport;
  assert.ok(diff.summary.changed >= 1, `expected >=1 changed Android cell`);
  const checkoutDiff = diff.screens
    .find((s) => s.path === "/checkout")
    ?.cells.find((c) => c.renderDiff);
  assert.ok(checkoutDiff?.renderDiff, "expected renderDiff on Android /checkout screen");
  assert.ok(
    checkoutDiff.renderDiff.changes.some(
      (ch) => ch.id === "pay_btn#0" && ch.beforeText === "Pay $29" && ch.afterText === "Pay $39",
    ),
    `expected pay_btn#0 text change: ${JSON.stringify(checkoutDiff.renderDiff)}`,
  );

  // 3. Missing adb / emulator reports unrenderable (no-android-emulator)
  const missingReply = runDuiMissingEnv(["run", appDir, manifest, path.join(tmp, "out-missing")], {
    HOME: duiHome,
    DUI_ADB: "/nonexistent/adb",
  }) as { status: string; surface: string; reason: string };
  assert.equal(missingReply.status, "unrenderable");
  assert.equal(missingReply.surface, "android-mobile");
  assert.equal(missingReply.reason, "no-android-emulator");

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
