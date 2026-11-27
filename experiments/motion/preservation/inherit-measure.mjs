import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { chromium } from "playwright-core";
import { scopes, encode } from "./inherited.mjs";
const [extractor, chrome, out] = process.argv.slice(2);
const cases = {
  parent: "#outer{--shift:24px}#box{transform:translateX(var(--shift))}",
  nearest: "#outer{--shift:24px}#group{--shift:10px}#box{transform:translateX(var(--shift))}",
  override: "#outer{--shift:24px}#box{--shift:6px;transform:translateX(var(--shift))}",
  alias:
    "#outer{--base:12px;--shift:var(--base)}#group{--base:30px}#box{transform:translateX(var(--shift))}",
  fallback:
    "#outer{--shift:var(--missing,17px)}#box{--missing:33px;transform:translateX(var(--shift))}",
  cycle:
    "#outer{--a:var(--b);--b:var(--a)}#group{--b:35px}#box{transform:translateX(var(--a,7px))}",
};
const modes = ["leaf", "flatten", "scopes"],
  rows = {},
  inputs = [];
fs.writeFileSync(
  path.join(out, "App.tsx"),
  'export function App(){return <div id="outer"><div id="group"><div id="box"/></div></div>}',
);
const browser = await chromium.launch({ executablePath: chrome });
try {
  for (const [name, css] of Object.entries(cases)) {
    fs.writeFileSync(path.join(out, "built.css"), css);
    const model = JSON.parse(execFileSync(extractor, [out]));
    const s = scopes(model, css);
    inputs.push({ model, css });
    const cssModel = Object.fromEntries(modes.map((m) => [m, encode(s, m)]));
    rows[name] = { css, cssModel, views: {} };
    for (const w of [1440, 390]) {
      const page = await browser.newPage({
        viewport: { width: w, height: w === 1440 ? 920 : 844 },
      });
      const v = (rows[name].views[w] = {});
      for (const m of ["Chrome", ...modes]) {
        await page.setContent(
          "<style>body{margin:0;padding:12px}#box{width:56px;height:18px;background:#215c50}" +
            (m === "Chrome" ? css : "") +
            '</style><div id="outer"><div id="group"><div id="box"></div></div></div>',
        );
        v[m] = await page.evaluate((payload) => {
          const box = document.getElementById("box");
          if (payload) {
            const els =
              payload.levels.length === 1
                ? [box]
                : [document.getElementById("outer"), document.getElementById("group"), box];
            payload.levels.forEach((vs, i) => {
              for (const [k, val] of Object.entries(vs)) els[i].style.setProperty(k, val);
            });
            box.style.transform = payload.transform;
          }
          const { x, y, width, height } = box.getBoundingClientRect();
          return [x, y, width, height];
        }, cssModel[m] ?? null);
        await page.screenshot({
          path: path.join(out, name + "-" + w + "-" + m + ".png"),
          clip: { x: 0, y: 0, width: 115, height: 42 },
        });
      }
      await page.close();
    }
  }
} finally {
  await browser.close();
}
fs.writeFileSync(path.join(out, "inherit.json"), JSON.stringify(rows, null, 2));
fs.writeFileSync(path.join(out, "inputs.json"), JSON.stringify(inputs));
