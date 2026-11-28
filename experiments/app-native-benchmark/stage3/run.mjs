#!/usr/bin/env node
import { createRequire } from "node:module";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const here = path.dirname(new URL(import.meta.url).pathname);
const repo = path.resolve(here, "../../..");
const shared = createRequire(import.meta.url);
const { chromium } = shared("playwright-core");
const esbuild = shared("esbuild");
const { PNG } = shared("pngjs");

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? process.argv[i + 1] : fallback;
}
const out = path.resolve(arg("out", "/tmp/dui-stage3-results"));
const dui = path.resolve(arg("dui", path.join(repo, "target/debug/dui")));
const chrome = process.env.CHROME_PATH || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });

const fixtures = JSON.parse(fs.readFileSync(path.join(here, "fixtures.json"), "utf8"));
if (process.env.STAGE3_FIXTURE_OVERLAY) {
  const overlay = JSON.parse(fs.readFileSync(process.env.STAGE3_FIXTURE_OVERLAY, "utf8"));
  Object.assign(fixtures.targets, overlay.targets ?? {});
}
const roots = {};
for (const target of Object.keys(fixtures.targets)) {
  roots[target] = process.env[`${target.toUpperCase().replaceAll("-", "_")}_WEB_ROOT`];
}
const viewports = [
  { name: "desktop", width: 1440, height: 920 },
  { name: "mobile", width: 390, height: 844 },
];
const bundleOut = path.join(out, "browser-bundle.js");
esbuild.buildSync({
  entryPoints: [path.join(here, "browser-entry.ts")],
  bundle: true,
  format: "iife",
  platform: "browser",
  outfile: bundleOut,
  loader: { ".json": "json" },
  logLevel: "silent",
});
const bundle = fs.readFileSync(bundleOut, "utf8");

function scan(target, root) {
  const modelPath = path.join(out, `${target}.model.json`);
  const args = ["model", root, fixtures.targets[target].entry];
  if (fixtures.targets[target].builtCss) args.push("--built-css", fixtures.targets[target].builtCss);
  try {
    const text = execFileSync(dui, args, { encoding: "utf8", maxBuffer: 100 * 1024 * 1024 });
    fs.writeFileSync(modelPath, text);
    return { model: JSON.parse(text), modelPath };
  } catch (error) {
    return { error: String(error.stderr || error.message), modelPath };
  }
}
function component(model, wanted) {
  return model.components?.find((c) => c.name === wanted) ?? model.components?.[0];
}
function valuesFor(comp, byName) {
  const values = {};
  const missing = [];
  for (const [name, value] of Object.entries(byName ?? {})) {
    if (name.includes("#")) {
      values[name] = value;
      continue;
    }
    const slot = comp?.slots?.find((s) => s.name === name);
    if (slot) values[slot.id] = value;
    else missing.push(name);
  }
  return { values, missing };
}
function comparePng(aPath, bPath, diffPath) {
  if (!fs.existsSync(aPath) || !fs.existsSync(bPath)) return undefined;
  const a = PNG.sync.read(fs.readFileSync(aPath));
  const b = PNG.sync.read(fs.readFileSync(bPath));
  if (a.width !== b.width || a.height !== b.height) return { error: `size mismatch ${a.width}x${a.height} vs ${b.width}x${b.height}` };
  let exact = 0, within2 = 0;
  const diff = new PNG({ width: a.width, height: a.height });
  for (let i = 0; i < a.data.length; i += 4) {
    const dr = Math.abs(a.data[i] - b.data[i]);
    const dg = Math.abs(a.data[i + 1] - b.data[i + 1]);
    const db = Math.abs(a.data[i + 2] - b.data[i + 2]);
    const da = Math.abs(a.data[i + 3] - b.data[i + 3]);
    const max = Math.max(dr, dg, db, da);
    if (max === 0) exact += 1;
    if (max <= 2) within2 += 1;
    diff.data[i] = max === 0 ? a.data[i] : 255;
    diff.data[i + 1] = max === 0 ? a.data[i + 1] : 0;
    diff.data[i + 2] = max === 0 ? a.data[i + 2] : 0;
    diff.data[i + 3] = 255;
  }
  fs.writeFileSync(diffPath, PNG.sync.write(diff));
  const pixels = a.width * a.height;
  return { exactRatio: exact / pixels, within2Ratio: within2 / pixels };
}

const results = { version: 1, dui: execFileSync(dui, ["--version"], { encoding: "utf8" }).trim(), viewports, targets: {} };
const browser = await chromium.launch({ executablePath: chrome, headless: true });
for (const [target, config] of Object.entries(fixtures.targets)) {
  const root = roots[target];
  const record = { states: {}, publicSafe: !config.private };
  results.targets[target] = record;
  if (!root || !fs.existsSync(root)) {
    record.skipped = `${target.toUpperCase()}_WEB_ROOT is not set or does not exist`;
    continue;
  }
  const scanned = scan(target, root);
  record.modelPath = scanned.modelPath;
  if (scanned.error) {
    record.extraction = { status: "error", error: scanned.error };
    continue;
  }
  const model = scanned.model;
  const comp = component(model, config.component);
  record.extraction = {
    status: comp ? "extracted" : "missing-component",
    component: comp?.id,
    rootKind: comp?.root?.kind,
    rootReason: comp?.root?.reason,
    diagnostics: model.diagnostics?.length ?? 0,
    components: model.components?.length ?? 0,
    slots: comp?.slots?.map((s) => ({ id: s.id, name: s.name, kind: s.kind })) ?? [],
  };
  if (!comp) continue;
  for (const state of fixtures.states) {
    const assignment = valuesFor(comp, config.slots?.[state]);
    const stateRecord = { missingSlots: assignment.missing, viewports: {}, assertions: config.assertions?.[state] ?? [] };
    record.states[state] = stateRecord;
    for (const viewport of viewports) {
      const context = await browser.newContext({ viewport, recordVideo: { dir: path.join(out, "videos"), size: viewport } });
      const page = await context.newPage();
      await page.setContent(`<!doctype html><meta charset=utf-8><style>html,body{margin:0;width:${viewport.width}px;height:${viewport.height}px;background:white}#host{width:${viewport.width}px;height:${viewport.height}px;overflow:hidden}</style><div id=host></div>`);
      await page.addScriptTag({ content: bundle });
      const render = await page.evaluate(({ model, component, values, viewport }) => {
        const rendered = window.duiStage3.render(model, component, { width: viewport.width, height: viewport.height, slotValues: values });
        const host = document.getElementById("host");
        const shadow = host.attachShadow({ mode: "open" });
        shadow.innerHTML = rendered.markup;
        return {
          unresolved: rendered.unresolved,
          text: host.shadowRoot.textContent.replace(/\s+/g, " ").trim(),
          elements: host.shadowRoot.querySelectorAll("*").length,
          placeholders: host.shadowRoot.querySelectorAll("[data-model-placeholder], .model-placeholder, [data-placeholder]").length,
        };
      }, { model, component: comp.id, values: assignment.values, viewport });
      await page.waitForTimeout(100);
      const png = path.join(out, `${target}-${state}-${viewport.name}.png`);
      await page.screenshot({ path: png });
      const video = page.video();
      await context.close();
      if (video) {
        const videoPath = await video.path().catch(() => undefined);
        if (videoPath) stateRecord.video = path.relative(out, videoPath);
      }
      const expected = config.assertions?.[state] ?? [];
      const missingText = expected.filter((text) => !render.text.includes(text));
      const mounted = render.elements > 0 && render.text.length > 0;
      let cause;
      if (comp.root.kind === "placeholder") cause = "extraction";
      else if (assignment.missing.length) cause = "mock-state";
      else if (!mounted || render.unresolved.length || missingText.length) cause = "render";
      else cause = "none";
      const envTarget = target.toUpperCase().replaceAll("-", "_");
      const reference = process.env[`${envTarget}_REFERENCE_${state.toUpperCase()}_${viewport.name.toUpperCase()}`] ?? config.references?.[state]?.[viewport.name];
      const diff = reference ? comparePng(reference, png, path.join(out, `${target}-${state}-${viewport.name}-diff.png`)) : undefined;
      stateRecord.viewports[viewport.name] = { png: path.basename(png), mounted, textLength: render.text.length, elements: render.elements, missingText, unresolved: render.unresolved.slice(0, 20), cause, pixel: diff };
    }
  }
}
await browser.close();
fs.writeFileSync(path.join(out, "results.json"), JSON.stringify(results, null, 2));
const summary = {};
for (const [name, target] of Object.entries(results.targets)) {
  summary[name] = { extraction: target.extraction, skipped: target.skipped, states: {} };
  for (const [state, record] of Object.entries(target.states ?? {})) {
    summary[name].states[state] = {};
    for (const [viewport, result] of Object.entries(record.viewports)) {
      summary[name].states[state][viewport] = {
        mounted: result.mounted,
        cause: result.cause,
        exactRatio: result.pixel?.exactRatio,
        within2Ratio: result.pixel?.within2Ratio,
        missingSlots: record.missingSlots,
      };
    }
  }
}
console.log(JSON.stringify(summary, null, 2));
