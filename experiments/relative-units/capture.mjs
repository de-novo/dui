// Opt-in visual verification: node capture.mjs --capture CHROME PINNED_CHECKOUT EXTRACTOR
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { pathToFileURL, fileURLToPath } from "node:url";
import { build } from "esbuild";
import { chromium } from "playwright-core";
import { renderToStaticMarkup } from "react-dom/server";
import { mediaControl } from "./media.mjs";
import { extractExtension } from "./extension.mjs";

const [flag, executablePath, checkout, extractor] = process.argv.slice(2);
assert.equal(flag, "--capture");
const dir = path.dirname(fileURLToPath(import.meta.url));
const fixture = path.join(dir, "fixture");
const out = path.join(dir, "out");
fs.mkdirSync(out, { recursive: true });
const model = JSON.parse(execFileSync(extractor, [fixture]));
const css = fs.readFileSync(path.join(fixture, "built.css"), "utf8");
const extension = extractExtension(model, css);
fs.writeFileSync(path.join(out, "model.json"), JSON.stringify(model, null, 2));
fs.writeFileSync(path.join(out, "extension.json"), JSON.stringify(extension, null, 2));
const baseline = await build({
  entryPoints: [path.join(checkout, "apps/canvas/src/features/model/modelDocument.ts")],
  bundle: true,
  write: false,
  globalName: "baseline",
  nodePaths: [path.join(dir, "node_modules")],
});
const prototype = await build({
  entryPoints: [path.join(dir, "prototype.mjs")],
  bundle: true,
  write: false,
  globalName: "prototypeRenderer",
});
await build({
  entryPoints: [path.join(fixture, "App.tsx")],
  jsx: "automatic",
  platform: "node",
  format: "esm",
  bundle: true,
  packages: "external",
  outfile: path.join(out, "fixture.mjs"),
});
const { App } = await import(pathToFileURL(path.join(out, "fixture.mjs")));
const truth =
  "<!doctype html><style>*{box-sizing:border-box} body{margin:0}" +
  css +
  "</style>" +
  renderToStaticMarkup(App());
const ink =
  "main>section{height:72px} main>section div:not(:has(div)){height:24px;background:#16765d}";
const browser = await chromium.launch({ executablePath, headless: true });
const results = {};
try {
  for (const [width, height] of [
    [390, 844],
    [1440, 920],
  ]) {
    const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
    await page.addScriptTag({ content: baseline.outputFiles[0].text });
    const drawing = await page.evaluate(
      ({ model, width }) =>
        window.baseline.drawModel(window.baseline.decodeModel(model), model.components[0].id, {
          width,
        }),
      { model, width },
    );
    results[width] = {};
    for (const [name, html] of [
      ["reference", truth],
      ["baseline", drawing.html],
      ["prototype", drawing.html],
    ]) {
      await page.setContent(html);
      if (name === "prototype") {
        await page.evaluate(
          prototype.outputFiles[0].text + ";window.applyProbe = prototypeRenderer.applyExtension;",
        );
        await page.evaluate((e) => window.applyProbe(document, e), extension);
      }
      await page.evaluate((css) => {
        const s = document.createElement("style");
        s.textContent = css;
        document.head.append(s);
      }, ink);
      results[width][name] = await page.evaluate(() => ({
        rootFont: parseFloat(getComputedStyle(document.documentElement).fontSize),
        rows: [...document.querySelectorAll("main>section")].map((section) => {
          const r = section.querySelector("div:not(:has(div))").getBoundingClientRect();
          const p = section.getBoundingClientRect();
          return [r.x - p.x, r.y - p.y, r.width, r.height];
        }),
      }));
      await page.screenshot({ path: path.join(out, `${width}-${name}.png`) });
    }
    assert.deepEqual(results[width].prototype, results[width].reference);
    results[width].media = await mediaControl(page, model, width);
    for (const row of results[width].media) assert.equal(row.prototype, row.truth);
    await page.close();
  }
} finally {
  await browser.close();
}
fs.writeFileSync(path.join(out, "results.json"), JSON.stringify(results, null, 2));
console.log(JSON.stringify(results));
