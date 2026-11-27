import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { parseSync, visitorKeys } from "oxc-parser";
import * as css from "css-tree";
import { compile } from "tailwindcss";
import { buildSync, transformSync } from "esbuild";
import { chromium } from "playwright-core";
import { interpolateNumber } from "d3-interpolate";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
const [flag, chrome, repo, extractor, tmp] = process.argv.slice(2);
if (flag !== "--capture" || !tmp)
  throw Error("Expected --capture CHROME PINNED_CHECKOUT EXTRACTOR EMPTY_OUTPUT");
const read = (f) => fs.readFileSync(f, "utf8"),
  write = (f, x) => fs.writeFileSync(path.join(tmp, f), x);
const source = read("App.tsx"),
  ast = parseSync("App.tsx", source).program,
  targets = [];
function visit(n) {
  if (!n?.type) return;
  if (n.type === "JSXAttribute" && n.name.name === "className")
    targets.push({ classes: n.value.value?.split(/\s+/), span: [n.start, n.end] });
  for (const k of visitorKeys[n.type] ?? []) {
    const v = n[k];
    if (Array.isArray(v)) v.forEach(visit);
    else visit(v);
  }
}
visit(ast);
const built = (await compile("@tailwind utilities;")).build(
  targets.flatMap((t) => t.classes ?? []),
);
const sheet = read("probe.css") + "\n" + built;
write("built.css", sheet);
write("App.tsx", source);
const model = JSON.parse(execFileSync(extractor, [tmp], { encoding: "utf8" }));
const rules = css.parse(sheet, { positions: true }).children.toArray();
const keyframes = new Map(),
  styles = [];
function declarations(rule) {
  return Object.fromEntries(
    rule.block.children.toArray().map((d) => [d.property, css.generate(d.value)]),
  );
}
function translate(value) {
  const m = /^translateX\((-?[\d.]+)px\)$/.exec(value);
  if (!m) throw Error("Unsupported transform " + value);
  return Number(m[1]);
}
for (const r of rules) {
  if (r.type === "Atrule" && r.name === "keyframes") {
    keyframes.set(
      css.generate(r.prelude),
      r.block.children
        .toArray()
        .map((k) => ({
          offset: parseFloat(css.generate(k.prelude)) / 100,
          x: translate(declarations(k).transform),
          cssSpan: [k.loc.start.offset, k.loc.end.offset],
        })),
    );
  } else if (r.type === "Rule") {
    let sel = r.prelude.children.first.children.toArray();
    if (sel[0]?.type === "ClassSelector")
      styles.push({
        name: css.ident.decode(sel[0].name),
        pseudo: sel[1]?.name,
        decl: declarations(r),
        cssSpan: [r.loc.start.offset, r.loc.end.offset],
      });
  }
}
const root = model.components[0].root;
const tracks = [],
  unverified = [];
for (const [i, t] of targets.entries()) {
  const target = root.children[i].id;
  if (!t.classes) {
    unverified.push({ target, reason: "runtime class expression", span: t.span });
    continue;
  }
  const matching = styles.filter((r) => t.classes.includes(r.name)),
    attrs = Object.assign({}, ...matching.filter((r) => !r.pseudo).map((r) => r.decl));
  const [property, seconds, timing, fill] = (attrs.transition ?? attrs.animation ?? "").split(" ");
  if (!/^\d+(\.\d+)?s$/.test(seconds) || timing !== "linear") throw Error("Unsupported timing");
  const hover = matching.find((r) => r.pseudo === "hover");
  const frames = hover
    ? [
        { offset: 0, x: translate(attrs.transform) },
        { offset: 1, x: translate(hover.decl.transform) },
      ]
    : keyframes.get(property);
  if (!frames || (hover && property !== "transform")) throw Error("Unsupported track");
  tracks.push({
    target,
    trigger: hover ? "hover-enter" : "mount",
    durationMs: parseFloat(seconds) * 1000,
    timing,
    fill: fill ?? "transition-state",
    property: "translateX",
    unit: "px",
    frames,
    tsxSpan: t.span,
    cssSpans: matching.map((r) => r.cssSpan),
  });
}
write("motion.json", JSON.stringify({ tracks, unverified }, null, 2));
write("source-model.json", JSON.stringify(model));
const mod = { exports: {} };
new Function(
  "module",
  "exports",
  "require",
  transformSync(source, { loader: "tsx", jsx: "automatic", format: "cjs" }).code,
)(mod, mod.exports, createRequire(import.meta.url));
const real = renderToStaticMarkup(React.createElement(mod.exports.Probe, { dynamic: "" }));
const renderer = path.join(repo, "apps/canvas/src/features/model/modelDocument.ts");
const js = buildSync({
  stdin: {
    contents: "export {drawModel,decodeModel} from " + JSON.stringify(renderer),
    resolveDir: repo,
  },
  bundle: true,
  globalName: "dui",
  write: false,
  nodePaths: [path.resolve("../relative-units/node_modules")],
}).outputFiles[0].text;
const browser = await chromium.launch({ executablePath: chrome });
const measurements = [];
try {
  for (const width of [1440, 390]) {
    const ctx = await browser.newContext({
        viewport: { width, height: width === 390 ? 844 : 920 },
        hasTouch: false,
      }),
      ref = await ctx.newPage(),
      approx = await ctx.newPage();
    await ref.setContent("<style>body{margin:0}" + sheet + "</style>" + real);
    await approx.setContent('<style>body{margin:0}</style><main id="host"></main>');
    await approx.addScriptTag({ content: js });
    await approx.evaluate(
      ({ model, width }) => {
        document.querySelector("#host").attachShadow({ mode: "open" }).innerHTML = dui.drawModel(
          dui.decodeModel(model),
          model.components[0].id,
          { width },
        ).markup;
      },
      { model, width },
    );
    await ref.locator(".move").hover();
    await ref.waitForFunction(() => document.getAnimations().length === 3);
    await ref.evaluate(() => document.getAnimations().forEach((a) => a.pause()));
    for (let ms = 0; ms <= 1000; ms += 100) {
      const truth = await ref.evaluate((ms) => {
        document.getAnimations().forEach((a) => (a.currentTime = ms));
        return Array.from(document.querySelectorAll(".track"), (e) => {
          const r = e.getBoundingClientRect();
          return [r.x, r.y];
        });
      }, ms);
      const offsets = tracks.map((t) => {
        const p = ms / t.durationMs;
        const i = t.frames.findIndex((f, i) => i > 0 && p <= f.offset);
        const a = t.frames[i - 1],
          b = t.frames[i];
        return interpolateNumber(a.x, b.x)((p - a.offset) / (b.offset - a.offset));
      });
      const pred = await approx.evaluate(
        ({ tracks, offsets, ids }) => {
          const dom = document.querySelector("#host").shadowRoot.querySelector("main");
          return tracks.map((t, i) => {
            const e = dom.children[ids.indexOf(t.target)];
            e.style.transform = "translateX(" + offsets[i] + "px)";
            const r = e.getBoundingClientRect();
            return [r.x, r.y];
          });
        },
        { tracks, offsets, ids: root.children.map((n) => n.id) },
      );
      measurements.push({ width, ms, truth, pred });
      for (const [name, page] of [
        ["chrome", ref],
        ["model", approx],
      ])
        await page.screenshot({
          path: path.join(tmp, width + "-" + ms + "-" + name + ".png"),
          clip: { x: 0, y: 0, width: Math.min(width, 480), height: 190 },
          animations: "allow",
        });
    }
    await ctx.close();
  }
} finally {
  await browser.close();
}
write("measurements.json", JSON.stringify(measurements, null, 2));
