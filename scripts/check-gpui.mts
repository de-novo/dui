// Verify real GPUI Kit (gpui-pre 0.3.12 / gpui-kit 0.10.0) offscreen rendering (#5):
// 1. Run one real GPUI screen against the documented gpui-pre baseline and record OS, toolchain, render path, and width.
// 2. Render two supplied typed states through the shared Renderer contract and save PNGs + structured evidence under ~/.dui.
// 3. Exercise the missing-render-environment path, surface its reason, and record whether element-tree assertions are available.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

type GpuiCell = {
  scenario: string;
  path: string;
  width: number;
  status: string;
  reason?: string | null;
  image?: string;
  elementTreeAvailable?: boolean;
  observation?: {
    renderPath?: string;
    pixelWidth?: number;
    pixelHeight?: number;
    elementTreeAvailable?: boolean;
    elements?: { role?: string; label?: string; checked?: boolean; visible?: boolean }[];
    assertions?: { label: string; passed: boolean }[];
  };
};

type GpuiReport = {
  output: string;
  runtime: string;
  os: string;
  arch: string;
  toolchain: { rustc: string; cargo: string };
  versions: Record<string, string>;
  renderPath: string;
  elementTreeAvailable: boolean;
  widths: number[];
  coverage: {
    verified: number;
    renderable: number;
    attempted: number;
    unrenderable: number;
  };
  cells: GpuiCell[];
};

const root = resolve(import.meta.dirname, "..");
const fixture = join(root, "fixtures/gpui-kit");
const manifest = join(fixture, "dui.json");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-gpui-check-"));

function runDui(args: string[], envExtra: Record<string, string>) {
  const res = spawnSync(binary, args, {
    env: { ...process.env, ...envExtra },
    encoding: "utf8",
  });
  assert(res.stdout.trim().length > 0, `empty stdout for ${args.join(" ")}: ${res.stderr}`);
  return { status: res.status ?? 1, body: JSON.parse(res.stdout) };
}

try {
  const home = join(stage, "home");
  const baseEnv = {
    HOME: home,
    RUSTUP_HOME: process.env.RUSTUP_HOME ?? join(homedir(), ".rustup"),
    CARGO_HOME: process.env.CARGO_HOME ?? join(homedir(), ".cargo"),
  };

  // 1 & 2. Render two supplied states through the shared Renderer contract and save under ~/.dui.
  const run = runDui(["run", fixture, manifest], baseEnv);
  assert.equal(run.status, 0, `gpui run failed (${run.status}): ${JSON.stringify(run.body)}`);
  const report = run.body as GpuiReport;

  assert.equal(report.runtime, "gpui-kit");
  assert(report.os.length > 0);
  assert(report.toolchain.rustc.startsWith("rustc "));
  assert(report.toolchain.cargo.startsWith("cargo "));
  assert.equal(report.versions["gpui-kit"], "0.10.0");
  assert(report.versions["gpui-pre"]?.startsWith("0.3."));
  assert(["metal", "wgpu"].includes(report.renderPath));
  assert.deepEqual(report.widths, [480]);
  assert.equal(report.elementTreeAvailable, true);
  assert.equal(report.coverage.verified, 2);
  assert.equal(report.coverage.unrenderable, 0);
  assert.equal(report.cells.length, 2);

  // Verify saved archive structure under ~/.dui and distinct real PNGs.
  assert(report.output.includes(join(".dui", "projects")));
  assert(existsSync(join(report.output, "report.json")));
  assert(existsSync(join(report.output, "graph.json")));
  assert(existsSync(join(report.output, "snapshot.json")));

  const images = report.cells.map((c) => {
    assert.equal(c.status, "verified");
    assert(c.image, `missing image for ${c.scenario}`);
    const pngPath = join(report.output, c.image);
    assert(existsSync(pngPath), `missing png ${pngPath}`);
    const bytes = readFileSync(pngPath);
    assert.deepEqual(bytes.subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
    assert.equal(c.elementTreeAvailable, true);
    assert(c.observation?.assertions?.every((a) => a.passed));
    return c.image;
  });
  assert.notEqual(images[0], images[1], "two distinct GPUI states must produce distinct PNGs");

  // Confirm archived capture is listed by `dui history`.
  const history = runDui(["history"], baseEnv);
  assert.equal(history.status, 0);
  const snapshots = (history.body as { snapshots: { output: string }[] }).snapshots;
  assert(snapshots.some((s) => s.output === report.output));

  // 3. Exercise the missing-render-environment path and surface its reason + element-tree flag.
  const noRender = runDui(["run", fixture, manifest], {
    ...baseEnv,
    DUI_GPUI_NO_RENDER: "1",
  });
  assert.equal(noRender.status, 5);
  const noRenderReport = noRender.body as GpuiReport;
  assert.equal(noRenderReport.coverage.verified, 0);
  assert.equal(noRenderReport.coverage.unrenderable, 2);
  assert.equal(noRenderReport.elementTreeAvailable, false);
  for (const cell of noRenderReport.cells) {
    assert.equal(cell.status, "unrenderable");
    assert.equal(cell.reason, "no-render-env");
    assert.equal(cell.image, undefined);
    assert.equal(cell.elementTreeAvailable, false);
  }

  console.log(
    JSON.stringify({
      runtime: report.runtime,
      os: report.os,
      arch: report.arch,
      toolchain: report.toolchain,
      versions: report.versions,
      renderPath: report.renderPath,
      widths: report.widths,
      elementTreeAvailable: report.elementTreeAvailable,
      states: report.cells.map((c) => ({
        scenario: c.scenario,
        status: c.status,
        image: c.image,
        pixelWidth: c.observation?.pixelWidth,
        pixelHeight: c.observation?.pixelHeight,
        assertionsPassed: c.observation?.assertions?.length ?? 0,
      })),
      missingRenderEnv: {
        status: noRender.status,
        unrenderable: noRenderReport.coverage.unrenderable,
        reason: noRenderReport.cells[0]?.reason,
        elementTreeAvailable: noRenderReport.elementTreeAvailable,
      },
    }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
