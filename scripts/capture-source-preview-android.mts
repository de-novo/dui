// Capture fixtures/source-preview/Settings.kt, compiled with Jetpack Compose, on a disposable
// emulator through dui's Android host. Writes android.png and android.json (provenance).
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

type Node = { text: string; attributes: Record<string, string> };

const root = resolve(import.meta.dirname, "..");
const source = join(root, "fixtures/source-preview/Settings.kt");
const host = join(
  root,
  "fixtures/source-preview-native/android/src/main/java/sh/dui/sourcepreview/HostActivity.kt",
);
const fixture = join(root, "fixtures/source-preview-native/android");
const spec = JSON.parse(readFileSync(join(fixture, "dui.json"), "utf8"));
const output = resolve(process.argv[2] ?? "");
assert.ok(process.argv[2], "Pass a new output directory");
const serial = process.env.ANDROID_SERIAL;
assert.ok(serial, "Set ANDROID_SERIAL to a disposable emulator with the host APK installed.");

const sha256 = (path: string) => createHash("sha256").update(readFileSync(path)).digest("hex");
function adb(...args: string[]) {
  return execFileSync("adb", ["-s", serial!, ...args], { encoding: "utf8", timeout: 30000 }).trim();
}
assert.equal(adb("shell", "getprop", "ro.kernel.qemu"), "1", "Select an emulator.");

mkdirSync(output);
const home = join(output, ".home");
mkdirSync(home);
const [width] = spec.widths as number[];
const previous = adb("shell", "wm", "size").match(/Override size: (\d+x\d+)/)?.[1] ?? "reset";
let cell;
try {
  adb("shell", "wm", "size", width + "x" + spec.height);
  adb("shell", "am", "force-stop", spec.package);
  adb("shell", "am", "start", "-W", "-n", spec.activity);
  const { activity: _activity, ...captureSpec } = spec;
  const manifest = join(output, ".dui.json");
  writeFileSync(manifest, JSON.stringify({ ...captureSpec, serial }));
  const run = join(output, ".run");
  execFileSync(join(root, "target/debug/dui"), ["run", "--force", fixture, manifest, run], {
    env: { ...process.env, HOME: home },
    timeout: 90000,
  });
  const report = JSON.parse(readFileSync(join(run, "report.json"), "utf8"));
  assert.equal(report.cells?.length, 1, JSON.stringify(report.coverage ?? report));
  cell = report.cells[0];
  assert.equal(cell.observation?.android?.serial, serial);
  copyFileSync(join(run, cell.image), join(output, "android.png"));
} finally {
  adb("shell", "wm", "size", previous);
}

// The component must be on screen in its declared initial state.
const nodes: Node[] = cell.observation.renderTree;
const ours = nodes.filter((n) => n.attributes?.package === spec.package);
const switches = ours.filter((n) => n.attributes.checkable === "true");
assert.deepEqual(
  switches.map((n) => n.attributes.checked),
  ["false", "true"],
  "Expected Notifications off and Reminders on: " + JSON.stringify(ours),
);
const texts = new Set(ours.map((n) => n.text.trim()).filter(Boolean));
for (const text of ["Notifications", "Reminders", "Notifications off"])
  assert.ok(texts.has(text), `Missing "${text}" in ${JSON.stringify([...texts])}`);

const png = readFileSync(join(output, "android.png"));
assert.deepEqual(png.subarray(0, 8), Buffer.from("89504e470d0a1a0a", "hex"));
const imageWidth = png.readUInt32BE(16);
const imageHeight = png.readUInt32BE(20);
const density = Number(
  adb("shell", "wm", "density").match(/(?:Override|Physical) density: (\d+)/)?.[1],
);
assert.ok(density > 0, "Missing display density");
const prop = (name: string) => adb("shell", "getprop", name);
const env = process.env;
const record = {
  runtime: "android",
  source: "android/Settings.kt",
  entry: "example.Settings",
  fixture: "fixtures/source-preview/Settings.kt",
  sourceSha256: sha256(source),
  host: "fixtures/source-preview-native/android/src/main/java/sh/dui/sourcepreview/HostActivity.kt",
  hostSha256: sha256(host),
  image: "android.png",
  imageWidth,
  imageHeight,
  imageSha256: sha256(join(output, "android.png")),
  width: Math.round((imageWidth * 160) / density),
  scale: density / 160,
  device: `${prop("ro.product.model")} emulator`,
  os: `Android ${prop("ro.build.version.release")} (API ${prop("ro.build.version.sdk")})`,
  toolchain: "Kotlin 2.4.21, Compose BOM 2026.11.01, Material 3",
  witness: "Notifications off",
  commit:
    env.GITHUB_SHA ?? execFileSync("git", ["-C", root, "rev-parse", "HEAD"]).toString().trim(),
  workflowRun:
    env.GITHUB_SERVER_URL && env.GITHUB_REPOSITORY && env.GITHUB_RUN_ID
      ? `${env.GITHUB_SERVER_URL}/${env.GITHUB_REPOSITORY}/actions/runs/${env.GITHUB_RUN_ID}`
      : null,
};
writeFileSync(join(output, "android.json"), JSON.stringify(record, null, 2) + "\n");
const summary = `Native Compose capture on ${record.device} (${record.os}): ${imageWidth} × ${imageHeight} px, ${record.width} dp wide, image SHA-256 \`${record.imageSha256}\`.`;
console.log(summary);
if (env.GITHUB_STEP_SUMMARY) writeFileSync(env.GITHUB_STEP_SUMMARY, summary + "\n", { flag: "a" });
