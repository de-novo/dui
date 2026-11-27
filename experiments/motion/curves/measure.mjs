import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import bezier from "bezier-easing";
import { parse, generate } from "css-tree";
import { chromium } from "playwright-core";
import { buildSync, transformSync } from "esbuild";
import { createRequire } from "node:module";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
const [repo, extractor, chrome, out] = process.argv.slice(2),
  root = path.resolve("curves");
const css = fs.readFileSync(root + "/built.css", "utf8"),
  source = fs.readFileSync(root + "/App.tsx", "utf8");
const model = JSON.parse(execFileSync(extractor, [root]));
const mod = { exports: {} };
new Function(
  "module",
  "exports",
  "require",
  transformSync(source, { loader: "tsx", jsx: "automatic", format: "cjs" }).code,
)(mod, mod.exports, createRequire(import.meta.url));
const html = renderToStaticMarkup(React.createElement(mod.exports.Curves));
const rules = parse(css).children.toArray();
const tracks = rules
  .filter((r) => r.type === "Rule")
  .flatMap((r) => {
    const d = r.block.children.toArray().find((d) => d.property === "animation");
    if (!d) return [];
    const [name, duration, curve] = d.value.children.toArray().map(generate);
    return [
      {
        className: generate(r.prelude).slice(1),
        name,
        duration: parseFloat(duration) * 1000,
        curve,
      },
    ];
  });
const frames = rules
  .find((r) => r.type === "Atrule" && r.name === "keyframes")
  .block.children.toArray()
  .map((r) =>
    r.block.children.first.value.children
      .toArray()
      .map((fn) => ({ op: fn.name, value: parseFloat(generate(fn.children.first)) })),
  );
const ease = bezier(0.25, 0.1, 0.25, 1);
function progress(curve, p) {
  p = Math.max(0, Math.min(1, p));
  if (curve === "ease") return ease(p);
  if (curve.startsWith("cubic-bezier("))
    return bezier(...curve.slice(13, -1).split(",").map(Number))(p);
  if (curve === "steps(4,end)") return Math.floor(4 * p) / 4;
  throw Error(curve);
}
const js = buildSync({
  stdin: {
    contents:
      "export {drawModel,decodeModel} from " +
      JSON.stringify(path.join(repo, "apps/canvas/src/features/model/modelDocument.ts")),
    resolveDir: repo,
  },
  bundle: true,
  globalName: "dui",
  write: false,
  nodePaths: [path.resolve("../relative-units/node_modules")],
}).outputFiles[0].text;
const browser = await chromium.launch({ executablePath: chrome }),
  stats = {};
try {
  for (const width of [1440, 390]) {
    const ctx = await browser.newContext({
        viewport: { width, height: width === 1440 ? 920 : 844 },
      }),
      ref = await ctx.newPage(),
      pred = await ctx.newPage();
    await ref.setContent("<style>body{margin:0}" + css + "</style>" + html);
    await ref.evaluate(() => document.getAnimations().forEach((a) => a.pause()));
    await pred.setContent("<style>body{margin:0}</style><aside></aside>");
    await pred.addScriptTag({ content: js });
    await pred.evaluate(
      ({ model, width }) =>
        (document.querySelector("aside").attachShadow({ mode: "open" }).innerHTML = dui.drawModel(
          dui.decodeModel(model),
          model.components[0].id,
          { width },
        ).markup),
      { model, width },
    );
    // Reverse a running CSS transition in a real animation frame. Record only its elapsed clock as input.
    const reversal = await ref.evaluate(async () => {
      const e = document.querySelector(".reverse");
      getComputedStyle(e).transform;
      e.classList.add("go");
      getComputedStyle(e).transform;
      const a = e.getAnimations()[0];
      await a.ready;
      return await new Promise((resolve) => {
        function tick() {
          if (a.currentTime < 400) {
            requestAnimationFrame(tick);
            return;
          }
          const elapsed = a.currentTime;
          e.classList.remove("go");
          getComputedStyle(e).transform;
          const reverse = e.getAnimations()[0];
          reverse.pause();
          resolve({ elapsed, actualDuration: reverse.effect.getTiming().duration });
        }
        requestAnimationFrame(tick);
      });
    });
    const factor = ease(reversal.elapsed / 1000),
      duration = 1000 * factor;
    const rows = [];
    for (let ms = 0; ms <= 1000; ms += 100) {
      const truth = await ref.evaluate(
        (ms) =>
          Array.from(document.querySelectorAll(".box"), (e) => {
            const a = e.getAnimations()[0];
            if (a) a.currentTime = ms;
            const { x, y, width, height } = e.getBoundingClientRect();
            return [x, y, width, height];
          }),
        ms,
      );
      const transforms = tracks.map((t) => {
        const v = progress(t.curve, ms / t.duration);
        return frames[0]
          .map(({ op, value }, i) => {
            const n = value + (frames[1][i].value - value) * v;
            return op + "(" + n + (op === "translateX" ? "px" : op === "rotate" ? "deg" : "") + ")";
          })
          .join(" ");
      });
      transforms.push("translateX(" + 96 * factor * (1 - ease(Math.min(1, ms / duration))) + "px)");
      const candidate = await pred.evaluate(
        (transforms) =>
          Array.from(
            document.querySelector("aside").shadowRoot.querySelector("main").children,
            (e, i) => {
              e.style.transform = transforms[i];
              const { x, y, width, height } = e.getBoundingClientRect();
              return [x, y, width, height];
            },
          ),
        transforms,
      );
      rows.push({ ms, truth, candidate });
      for (const [name, page] of [
        ["chrome", ref],
        ["model", pred],
      ])
        await page.screenshot({
          path: path.join(out, width + "-" + ms + "-" + name + ".png"),
          clip: { x: 0, y: 0, width: Math.min(width, 480), height: 190 },
        });
    }
    stats[width] = { reversal, predictedDuration: duration, rows };
    await ctx.close();
  }
} finally {
  await browser.close();
}
fs.writeFileSync(path.join(out, "curves.json"), JSON.stringify({ tracks, frames, stats }, null, 2));
