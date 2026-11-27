// Run with node reload.mjs /absolute/path/to/dui after building the release probe.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, watch, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const project = mkdtempSync(join(tmpdir(), "dui-reload-"));
const renderer = fileURLToPath(new URL("target/release/renderer-geometry", import.meta.url));
const screens = ["Alpha", "Beta"];
const dependencies = new Map();
const cards = new Map();
const contents = new Map();
function draw(screen) {
  const model = execFileSync(process.argv[2], [
    "model",
    project,
    screen + ".tsx",
    "--built-css",
    screen + ".css",
  ]);
  dependencies.set(screen, new Set(JSON.parse(model).files.map((file) => file.path)));
  const rect = JSON.parse(execFileSync(renderer, { input: model })).rect;
  const [x, y, width, height] = rect;
  cards.set(
    screen,
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 390 844"><rect x="${x}" y="${y}" width="${width}" height="${height}"/></svg>`,
  );
  return rect;
}
function edit(screen, width) {
  writeFileSync(join(project, screen + ".css"), "#box{width:" + width + "px;height:24px}");
}
try {
  for (const screen of screens) {
    writeFileSync(
      join(project, screen + ".tsx"),
      "export function " + screen + '(){return <div id="box"/>}',
    );
    edit(screen, 80);
    draw(screen);
  }
  for (const files of dependencies.values()) {
    for (const file of files) contents.set(file, readFileSync(join(project, file)));
  }
  const initialBeta = cards.get("Beta");
  const durations = [];
  for (let width = 81; width <= 101; width++) {
    let watcher;
    let deadline;
    const event = new Promise((resolve, reject) => {
      deadline = setTimeout(() => reject(Error("No changed dependency reported within 5 s")), 5000);
      watcher = watch(project, (_event, file) => {
        if (!contents.has(file)) return;
        const current = readFileSync(join(project, file));
        if (current.equals(contents.get(file))) return;
        contents.set(file, current);
        resolve(file);
      });
      watcher.on("error", reject);
    });
    const start = performance.now();
    try {
      edit("Alpha", width);
      const file = await event;
      const affected = screens.filter((screen) => dependencies.get(screen).has(file));
      assert.deepEqual(affected, ["Alpha"]);
      assert.deepEqual(draw(affected[0]), [0, 0, width, 24]);
      assert.ok(cards.get("Alpha").includes(`width="${width}"`));
      assert.equal(cards.get("Beta"), initialBeta);
      durations.push(performance.now() - start);
    } finally {
      clearTimeout(deadline);
      watcher.close();
    }
  }
  durations.sort((a, b) => a - b);
  console.log(
    JSON.stringify({
      edits: durations.length,
      medianMs: durations[10],
      p95Ms: durations[19],
      redrawnPerEdit: 1,
    }),
  );
} finally {
  rmSync(project, { recursive: true, force: true });
}
