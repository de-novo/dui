// Probe the installed fixture on an explicitly selected disposable emulator.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

type Control = {
  text: string;
  rect: { x: number; y: number; width: number; height: number };
  attributes: Record<string, string>;
};

const root = resolve(import.meta.dirname, "..");
const fixture = join(root, "fixtures/android-compose");
const spec = JSON.parse(readFileSync(join(fixture, "dui.json"), "utf8"));
const serial = process.env.ANDROID_SERIAL;
assert.ok(
  serial,
  "Set ANDROID_SERIAL to a disposable emulator with the Compose fixture installed.",
);
const adbPath = process.env.DUI_ADB ?? "adb";
function adb(...args: string[]) {
  return execFileSync(adbPath, ["-s", serial!, ...args], {
    encoding: "utf8",
    timeout: 30000,
  }).trim();
}
assert.equal(adb("shell", "getprop", "ro.kernel.qemu"), "1", "Select an emulator.");
assert.ok(
  adb("shell", "pm", "path", spec.package).includes("package:"),
  "Install the fixture APK first.",
);

const output = mkdtempSync(join(tmpdir(), "dui-compose-"));
console.log("Compose captures:", output);
const home = join(output, "home");
mkdirSync(home);
const manifest = join(output, "dui.json");
const sizeBefore = adb("shell", "wm", "size");
const previous = sizeBefore.match(/Override size: (\d+x\d+)/)?.[1] ?? "reset";

function capture(width: number, phase: string) {
  const directory = join(output, width + "-" + phase);
  execFileSync(join(root, "target/debug/dui"), ["run", "--force", fixture, manifest, directory], {
    env: { ...process.env, HOME: home },
    timeout: 90000,
  });
  const report = JSON.parse(readFileSync(join(directory, "report.json"), "utf8"));
  assert.equal(report.cells?.length, 1, JSON.stringify(report.coverage ?? report));
  const [cell] = report.cells;
  assert.equal(cell.observation?.android?.serial, serial);
  assert.equal(cell.width, width);
  const png = readFileSync(join(directory, cell.image));
  assert.deepEqual(png.subarray(0, 8), Buffer.from("89504e470d0a1a0a", "hex"));
  assert.equal(png.readUInt32BE(16), width);
  assert.equal(png.readUInt32BE(20), spec.height);
  const nodes: Control[] = cell.observation.renderTree;
  const controls = nodes.filter(
    ({ attributes: a }) =>
      a?.package === spec.package && a.checkable === "true" && a.clickable === "true",
  );
  assert.equal(controls.length, 2, JSON.stringify(nodes));
  const byLabel = new Map<string, Control>();
  for (const control of controls) {
    const label = control.attributes.contentDesc || control.text;
    const { x, y, width: w, height: h } = control.rect;
    assert.ok(
      label && !byLabel.has(label),
      "Expected distinct switch names: " + JSON.stringify(controls),
    );
    assert.ok(w > 0 && h > 0 && x >= 0 && y >= 0 && x + w <= width && y + h <= spec.height);
    assert.match(control.attributes.checked, /^(true|false)$/);
    byLabel.set(label, control);
  }
  return byLabel;
}

function tap({ rect }: Control) {
  adb(
    "shell",
    "input",
    "tap",
    String(Math.floor(rect.x + rect.width / 2)),
    String(Math.floor(rect.y + rect.height / 2)),
  );
}

try {
  for (const width of spec.widths as number[]) {
    adb("shell", "wm", "size", width + "x" + spec.height);
    adb("shell", "am", "force-stop", spec.package);
    adb("shell", "am", "start", "-W", "-n", spec.activity);
    // Keep the same Activity alive for all three independent observations.
    const { activity: _activity, ...captureSpec } = spec;
    writeFileSync(manifest, JSON.stringify({ ...captureSpec, serial, widths: [width] }));
    const before = capture(width, "before");
    const [target, sibling] = [...before.keys()].sort();
    const initial = before.get(target)!;
    const initialSibling = before.get(sibling)!.attributes.checked;
    tap(initial);
    const changed = capture(width, "after");
    assert.equal(
      changed.get(target)?.attributes.checked,
      String(initial.attributes.checked !== "true"),
    );
    assert.equal(changed.get(sibling)?.attributes.checked, initialSibling);
    tap(changed.get(target)!);
    const reset = capture(width, "reset");
    for (const [label, node] of before)
      assert.equal(reset.get(label)?.attributes.checked, node.attributes.checked);
    console.log(
      JSON.stringify({
        width,
        target,
        sibling,
        bounds: initial.rect,
        checked: [
          initial.attributes.checked,
          changed.get(target)!.attributes.checked,
          reset.get(target)!.attributes.checked,
        ],
        siblingChecked: initialSibling,
      }),
    );
  }
} finally {
  adb("shell", "wm", "size", previous);
}
