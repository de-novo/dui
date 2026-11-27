// PINNED_RENDERER_DIR MODEL_JSON VIEWER_URL CHROME OUTPUT
const { chromium } = require("playwright-core"),
  { build } = require("esbuild");
const fs = require("node:fs"),
  path = require("node:path");
(async () => {
  const [repo, modelFile, site, executablePath, out] = process.argv.slice(2);
  const model = JSON.parse(fs.readFileSync(modelFile));
  const renderer = path.join(repo, "apps/canvas/src/features/model/modelDocument.ts");
  const Frame = path.join(repo, "apps/canvas/src/features/model/ApproximateModelFrame.tsx");
  const moduleCode = `import React from 'react';import{createRoot}from'react-dom/client';import{drawModel,decodeModel}from '${renderer}';import{ApproximateModelFrame}from '${Frame}';export function draw(m,width){return drawModel(decodeModel(m),m.components.find(c=>c.name==='ModelView').id,{width})}export function mount(node,render){createRoot(node).render(React.createElement(ApproximateModelFrame,{render,label:'Approximate'}))}`;
  const bundle = (
    await build({
      stdin: { contents: moduleCode, resolveDir: process.cwd() },
      jsx: "automatic",
      bundle: true,
      globalName: "probe",
      write: false,
      nodePaths: process.env.NODE_PATH.split(path.delimiter),
    })
  ).outputFiles[0].text;
  const browser = await chromium.launch({ executablePath });
  const data = {};
  try {
    for (const [width, height] of [
      [1440, 920],
      [390, 844],
    ]) {
      const page = await browser.newPage({ viewport: { width, height } });
      await page.goto(site + "/?view=model");
      await page.locator(".model-workbench h1").waitFor();
      const read = async (shadow) =>
        page.evaluate((shadow) => {
          const root = shadow
            ? document.querySelector(".approximate-model-content").shadowRoot
            : document;
          const sel = [
            "main",
            "main > header",
            "main > header > a",
            "main > header > h1",
            "main > div",
          ];
          return sel.map((selector) => {
            const el = root.querySelector(selector);
            if (!el) return null;
            const { x, y, width, height } = el.getBoundingClientRect();
            const cs = getComputedStyle(el);
            return { selector, x, y, width, height, display: cs.display, font: cs.fontSize };
          });
        }, shadow);
      const reference = await read(false);
      await page.screenshot({ path: path.join(out, `real-${width}-reference.png`) });
      const matches = await page.evaluate(() => {
        const el = document.querySelector(".model-workbench h1"),
          result = [];
        function visit(rules) {
          for (const r of rules) {
            if (r.selectorText && el.matches(r.selectorText) && r.style.fontSize)
              result.push([r.selectorText, r.style.fontSize]);
            if (r.cssRules) visit(r.cssRules);
          }
        }
        for (const sheet of document.styleSheets) visit(sheet.cssRules);
        return result;
      });
      await page.addScriptTag({ content: bundle });
      const result = await page.evaluate(({ model, width }) => probe.draw(model, width), {
        model,
        width,
      });
      await page.evaluate(
        ({ result }) => {
          document.body.innerHTML =
            '<div id="test-card" style="width:100%;position:relative;height:100vh"></div>';
          probe.mount(document.querySelector("#test-card"), result);
        },
        { result },
      );
      await page.locator(".approximate-model-content").locator("main").waitFor();
      const shadow = await read(true);
      await page.screenshot({ path: path.join(out, `real-${width}-shadow.png`) });
      await page.setContent(result.html);
      const standalone = await read(false);
      await page.goto(site + "/dialog.html");
      const dialog = page.getByRole("dialog");
      await dialog.waitFor();
      await page.waitForTimeout(500);
      const transform = await dialog.evaluate((e) => {
        const measure = () => {
          const r = e.getBoundingClientRect();
          return [r.x, r.y, r.width, r.height];
        };
        const original = measure(),
          matrix = getComputedStyle(e).transform;
        e.style.setProperty("transition", "none", "important");
        e.style.setProperty("animation", "none", "important");
        e.style.setProperty("transform", "none", "important");
        const ablated = measure();
        e.style.removeProperty("transform");
        return { matrix, original, ablated };
      });
      await page.screenshot({ path: path.join(out, `real-${width}-dialog.png`) });
      data[width] = {
        reference,
        shadow,
        standalone,
        matches,
        transform,
        unresolved: result.unresolved.length,
      };
      await page.close();
    }
  } finally {
    await browser.close();
  }
  console.log(JSON.stringify(data, null, 2));
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
