import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { chromium } from "../web-basics/node_modules/playwright-core/index.mjs";
import { buildSync } from "../web-basics/node_modules/esbuild/lib/main.js";
import { snapshot, observe, render } from "./page.mjs";
const [flag, main, admin, stage, chrome, modules] = process.argv.slice(2);
if (flag !== "--capture" || !modules)
  throw Error("--capture MAIN ADMIN STAGE CHROME VIEWER_NODE_MODULES");
const here = path.dirname(fileURLToPath(import.meta.url)),
  read = (p) => fs.readFileSync(p, "utf8"),
  sha = (b) => crypto.createHash("sha256").update(b).digest("hex");
const drawer = path.join(main, "apps/canvas/src/features/model/modelDocument.ts");
const direct = Object.fromEntries(
  [
    ...read(drawer)
      .match(/const directCss:[^=]+= \{([\s\S]*?)\n\};/)[1]
      .matchAll(/(\w+): "([^"]+)"/g),
  ].map((m) => [m[1], m[2]]),
);
const js = buildSync({
  stdin: {
    contents:
      "import * as drawer from " +
      JSON.stringify(drawer) +
      ';import {arrange} from "../screen-engines/yoga.mjs";window.drawer=drawer;window.arrange=arrange;',
    resolveDir: here,
  },
  bundle: true,
  format: "esm",
  write: false,
  nodePaths: [modules],
}).outputFiles[0].text;
const browser = await chromium.launch({ executablePath: chrome });
const all = { chrome: browser.version(), screens: {}, resources: {} };
const loc = { file: "oracle", line: 1, column: 1, start: 0, end: 1 };
function model(tree) {
  function walk(n) {
    if ("text" in n)
      return { kind: "text", id: n.id, location: loc, parts: [{ kind: "literal", value: n.text }] };
    const style = Object.entries(direct)
      .map(([property, k]) => {
        const v = n.typed[k] ?? n.used[k];
        if (!v) return null;
        return {
          property,
          value: /^-?[\d.]+px$/.test(v)
            ? { kind: "length", value: parseFloat(v), unit: "px" }
            : { kind: "keyword", value: v },
          from: { syntax: "inline", text: k, location: loc },
        };
      })
      .filter(Boolean);
    return {
      kind: "element",
      id: n.id,
      tag: n.tag,
      location: loc,
      style,
      props: Object.fromEntries(
        Object.entries(n.attrs).map(([k, v]) => [k, { kind: "literal", value: v }]),
      ),
      children: n.children.map(walk),
    };
  }
  return {
    schemaVersion: 1,
    extractor: { name: "oracle", version: "1" },
    files: [{ path: "oracle", language: "typescript", sha256: "0".repeat(64) }],
    components: [{ id: "oracle#Screen", name: "Screen", location: loc, root: walk(tree) }],
    screens: [],
  };
}
try {
  for (const route of ["403", "503"])
    for (const width of [1440, 390]) {
      const height = width === 1440 ? 920 : 844,
        key = route + "-" + width,
        ctx = await browser.newContext({
          viewport: { width, height },
          deviceScaleFactor: 1,
          colorScheme: "light",
          reducedMotion: "reduce",
        });
      const p = await ctx.newPage();
      await p.route("**/*", async (r) => {
        const u = new URL(r.request().url());
        let f;
        if (u.hostname === "fixture.local")
          f = path.join(admin, "dist", u.pathname === "/" + route ? "index.html" : u.pathname);
        else if (u.hostname === "fonts.googleapis.com") f = path.join(stage, "cache/fonts.css");
        else if (u.hostname === "fonts.gstatic.com") {
          f = path.join(stage, "cache", path.basename(u.pathname));
          if (!fs.existsSync(f)) execFileSync("curl", ["-fsSL", u.href, "-o", f]);
        } else return r.abort();
        if (!fs.existsSync(f) || !fs.statSync(f).isFile()) return r.abort();
        if (u.hostname.startsWith("fonts.")) all.resources[u.href] = sha(fs.readFileSync(f));
        return r.fulfill({
          path: f,
          headers: { "access-control-allow-origin": "*" },
          contentType:
            {
              ".css": "text/css",
              ".woff2": "font/woff2",
              ".js": "application/javascript",
              ".html": "text/html",
              ".svg": "image/svg+xml",
            }[path.extname(f)] ?? "application/octet-stream",
        });
      });
      await p.goto("http://fixture.local/" + route, { waitUntil: "domcontentloaded" });
      await p.getByRole("heading", { name: route, exact: true }).waitFor();
      await p.evaluate(() => document.fonts.ready);
      await p.addScriptTag({ type: "module", content: js });
      await p.waitForFunction(() => window.drawer);
      await p.addScriptTag({
        content:
          "window.snap=" +
          snapshot.toString() +
          ";window.observe=" +
          observe.toString() +
          ";window.renderProbe=" +
          render.toString(),
      });
      const tree = await p.evaluate(() => window.snap(document.querySelector(".h-svh")));
      const m = model(tree);
      const validation = await p.evaluate((m) => {
        try {
          window.drawer.decodeModel(m);
          return true;
        } catch {
          return false;
        }
      }, m);
      if (!validation) {
        fs.writeFileSync(path.join(stage, "failed-model.json"), JSON.stringify(m));
        throw Error("model schema");
      }
      const data = (all.screens[key] = { input: sha(JSON.stringify(tree)), modes: {} });
      fs.writeFileSync(path.join(stage, key + "-tree.json"), JSON.stringify(tree));
      const cd = await ctx.newCDPSession(p);
      await cd.send("DOM.enable");
      await cd.send("CSS.enable");
      await cd.send("DOM.getDocument", { depth: -1, pierce: true });
      for (const mode of [
        "reference",
        "identity",
        "stock",
        "shadow-only",
        "typed",
        "used",
        "yoga",
      ]) {
        console.log(key, mode);
        if (mode !== "reference")
          await p.evaluate(
            (args) => {
              window.probeRoot = window.renderProbe(args);
            },
            { tree, model: m, mode, width, height },
          );
        else await p.evaluate(() => (window.probeRoot = document.querySelector(".h-svh")));
        const inp = await p.evaluate((t) => JSON.stringify(t), tree);
        if (sha(inp) !== data.input) throw Error("input drift");
        await p.evaluate(async () => {
          await document.fonts.ready;
          await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
        });
        const geom = await p.evaluate(() => window.observe(window.probeRoot));
        const actual = await p.evaluate(() => window.snap(window.probeRoot));
        const differences = {};
        let usedDiff = 0,
          geomTypedDiff = 0,
          textMismatch = 0,
          attrMismatch = 0,
          pseudoMismatch = 0;
        function compare(a, b) {
          if (a.tag !== b.tag || a.text !== b.text) textMismatch++;
          if (!a.tag) return;
          for (const [k, v] of Object.entries(a.attrs)) if (b.attrs[k] !== v) attrMismatch++;
          if (JSON.stringify(a.pseudo) !== JSON.stringify(b.pseudo)) pseudoMismatch++;
          for (const [k, v] of Object.entries(a.used))
            if (b.used[k] !== v) {
              usedDiff++;
              differences[k] ??= [v, b.used[k]];
            }
          for (const k of [
            "width",
            "height",
            "min-width",
            "min-height",
            "max-width",
            "max-height",
            "margin-left",
            "margin-right",
            "flex-basis",
            "line-height",
          ])
            if (a.typed[k] !== b.typed[k]) geomTypedDiff++;
          a.children.forEach((n, i) => compare(n, b.children[i]));
        }
        compare(tree, actual);
        console.log("geometry", Object.keys(geom.runs).length);
        const fonts = {};
        for (const id of Object.keys(geom.runs)) {
          const idx = id.split(".").slice(1, -1).map(Number);
          const o = await cd.send("Runtime.evaluate", {
            expression: "[" + idx + "].reduce((e,i)=>e.childNodes[i],window.probeRoot)",
          });
          const { nodeId } = await cd.send("DOM.requestNode", { objectId: o.result.objectId });
          if (!nodeId) {
            await cd.send("DOM.getDocument", { depth: -1, pierce: true });
          }
          const node = await cd.send("DOM.requestNode", { objectId: o.result.objectId });
          fonts[id] = (await cd.send("CSS.getPlatformFontsForNode", { nodeId: node.nodeId })).fonts;
        }
        data.modes[mode] = {
          geom,
          differences,
          usedDiff,
          geomTypedDiff,
          textMismatch,
          attrMismatch,
          pseudoMismatch,
          fonts,
        };
        await p.screenshot({ path: path.join(stage, key + "-" + mode + ".png") });
      }
      await ctx.close();
      fs.writeFileSync(path.join(stage, "raw.json"), JSON.stringify(all));
      console.log(key);
    }
} finally {
  await browser.close();
}
