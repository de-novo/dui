// Verify v0.0.5 GPUI + gpui-kit 0.10.0 native desktop viewer (#40):
// 1. Run `cargo test` for `apps/desktop` (SQLite read/write round-trip).
// 2. Capture a fixture run into an isolated `~/.dui/dui.db` and attach a review comment.
// 3. Invoke `dui desktop --home <HOME>/.dui --snapshot-out <PNG>` to render the real GPUI desktop view offscreen.
// 4. Verify non-blank PNG output, SQLite run/screen/cell/comment counts, and AccessKit semantic elements.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, symlinkSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const template = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const desktopManifest = join(root, "apps/desktop/Cargo.toml");
const stage = mkdtempSync(join(tmpdir(), "dui-desktop-check-"));

function selectToolchain(): string | null {
  if (process.env.DUI_RUST_TOOLCHAIN) return process.env.DUI_RUST_TOOLCHAIN;
  const list = spawnSync("rustup", ["toolchain", "list"], { encoding: "utf8" });
  if (list.status !== 0) return null;
  let best: { minor: number; name: string } | null = null;
  for (const line of list.stdout.split(/\r?\n/)) {
    const name = line.trim().split(/\s+/)[0] ?? "";
    const match = name.match(/^1\.(\d+)\./);
    if (match) {
      const minor = Number(match[1]);
      if (minor >= 96 && (!best || minor > best.minor)) {
        best = { minor, name };
      }
    }
  }
  return best?.name ?? null;
}

function runDui(args: string[], envExtra: Record<string, string>) {
  const res = spawnSync(binary, args, {
    env: { ...process.env, ...envExtra },
    encoding: "utf8",
  });
  assert(res.stdout.trim().length > 0, `empty stdout for ${args.join(" ")}: ${res.stderr}`);
  return { status: res.status ?? 1, body: JSON.parse(res.stdout) };
}

try {
  const tc = selectToolchain();
  const cargoArgs = tc
    ? [`+${tc}`, "test", "--manifest-path", desktopManifest]
    : ["test", "--manifest-path", desktopManifest];
  const unit = spawnSync("cargo", cargoArgs, { encoding: "utf8", stdio: "inherit" });
  assert.equal(unit.status, 0, "apps/desktop unit tests must pass");

  const home = join(stage, "home");
  const realHome = homedir();
  const env: Record<string, string> = {
    HOME: home,
    RUSTUP_HOME: process.env.RUSTUP_HOME ?? join(realHome, ".rustup"),
    CARGO_HOME: process.env.CARGO_HOME ?? join(realHome, ".cargo"),
  };
  if (tc) env.DUI_RUST_TOOLCHAIN = tc;
  const appDir = join(stage, "desktop-fixture");
  cpSync(template, appDir, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(join(template, "node_modules"), join(appDir, "node_modules"), "dir");

  const manifest = join(appDir, "dui.json");
  const capture = runDui(["run", appDir, manifest], env);
  assert(capture.body.output, JSON.stringify(capture.body));
  const runDir = capture.body.output as string;

  const added = runDui(
    [
      "comment",
      "add",
      JSON.stringify({
        run: runDir,
        target: { kind: "screen", path: "/" },
        author: "reviewer",
        body: "Desktop viewer comment check",
      }),
    ],
    env,
  );
  assert.equal(added.status, 0, JSON.stringify(added.body));

  const snapshotPng = join(stage, "desktop.png");
  const desktop = runDui(
    ["desktop", "--home", join(home, ".dui"), "--snapshot-out", snapshotPng],
    env,
  );
  assert.equal(desktop.status, 0, JSON.stringify(desktop.body));
  assert.equal(desktop.body.ok, true);
  assert(existsSync(snapshotPng), "desktop PNG written");
  const pngBytes = readFileSync(snapshotPng);
  assert(pngBytes.length > 4096, `desktop PNG too small (${pngBytes.length} bytes)`);
  assert.equal(desktop.body.summary.runs, 1);
  assert.equal(desktop.body.summary.comments, 1);
  assert(desktop.body.summary.cells >= 2, "at least 2 widths loaded from SQLite");

  const labels = (desktop.body.elements as { label?: string }[]).map((e) => e.label ?? "");
  assert(
    labels.some((l) => l.includes("Refresh")) && labels.some((l) => l.includes("Resolve")),
    `expected desktop controls in AccessKit tree, got: ${JSON.stringify(labels)}`,
  );

  console.log(
    JSON.stringify({
      ok: true,
      renderer: desktop.body.renderer,
      pixelWidth: desktop.body.pixelWidth,
      pixelHeight: desktop.body.pixelHeight,
      pngBytes: pngBytes.length,
      summary: desktop.body.summary,
    }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
