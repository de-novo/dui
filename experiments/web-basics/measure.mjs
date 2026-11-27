import { jsx } from "./lower.mjs";
import { fixtures } from "./fixtures.mjs";
import { project } from "./static-tree.mjs";
import { probe, deviations } from "./probes.mjs";
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { buildSync } from "esbuild";
import { chromium } from "playwright-core";
import { compile } from "tailwindcss";
import { PNG } from "pngjs";
const [flag, main, dui, chrome, out, ...filter] = process.argv.slice(2);
if (flag !== "--capture" || !out)
  throw Error("Expected --capture MAIN_CHECKOUT DUI CHROME OUT [fixture IDs]");
const here = path.dirname(fileURLToPath(import.meta.url)),
  root = path.resolve(out),
  modules = path.join(here, "node_modules"),
  env = { ...process.env, HOME: path.join(root, "home"), DUI_CHROMIUM: "/missing-chrome" };
fs.mkdirSync(env.HOME, { recursive: true });
const drawer = buildSync({
  stdin: {
    contents:
      'import React from "react";import {createRoot} from "react-dom/client";import {ApproximateModelFrame} from ' +
      JSON.stringify(path.join(main, "apps/canvas/src/features/model/ApproximateModelFrame.tsx")) +
      ';export function mount(host,r){createRoot(host).render(React.createElement(ApproximateModelFrame,{render:r,label:"",showUnresolved:false}));}export {drawModel,decodeModel} from ' +
      JSON.stringify(path.join(main, "apps/canvas/src/features/model/modelDocument.ts")),
    resolveDir: main,
  },
  jsx: "automatic",
  bundle: true,
  globalName: "dui",
  write: false,
  nodePaths: [path.join(main, "apps/canvas/node_modules")],
}).outputFiles[0].text;
const bytes = (name, mime) =>
  "data:" + mime + ";base64," + fs.readFileSync(path.join(here, "assets", name)).toString("base64");
const fontURL = bytes("triangle.ttf", "font/ttf"),
  imageURL = bytes("mark.png", "image/png");
const reset = "*{box-sizing:border-box}body{margin:0;font:16px system-ui,sans-serif}";
function command(args) {
  const start = performance.now(),
    s = execFileSync("/usr/bin/time", ["-l", dui, ...args], {
      env,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
  return { json: JSON.parse(s), ms: performance.now() - start };
}
function alternate(args) {
  try {
    return command(args).json;
  } catch (e) {
    const details = JSON.parse(e.stdout);
    if (details.status !== "error") throw e;
    return { failure: details.reason };
  }
}
function hasKey(m) {
  function find(n) {
    return (n.kind === "list" && n.key?.kind === "item") || n.children?.some(find);
  }
  return m.components.some((c) => find(c.root));
}
function materialize(doc, tree) {
  function node(x, svg = false) {
    if (typeof x !== "object") return doc.createTextNode(String(x));
    svg ||= x.tag === "svg";
    const e = svg
      ? doc.createElementNS("http://www.w3.org/2000/svg", x.tag)
      : doc.createElement(x.tag);
    for (const [k, v] of Object.entries(x.attrs)) {
      if (k === "key" || k === "defaultValue" || k === "defaultChecked") continue;
      if (k === "style") {
        for (const [prop, value] of Object.entries(v))
          e.style[prop] =
            typeof value === "number" && !["fontWeight", "opacity", "lineHeight"].includes(prop)
              ? value + "px"
              : value;
      } else if (typeof v === "boolean") {
        if (v) e.setAttribute(k, "");
      } else e.setAttribute(k === "className" ? "class" : k, v);
    }
    x.children.forEach((c) => e.append(node(c, svg)));
    if ("defaultValue" in x.attrs) e.value = x.attrs.defaultValue;
    if ("defaultChecked" in x.attrs) e.checked = x.attrs.defaultChecked;
    return e;
  }
  doc.body.append(node(tree));
}
const b = await chromium.launch({ executablePath: chrome });
const result = {
  chrome: b.version(),
  platform: process.platform,
  locale: "en-US",
  dpr: 1,
  cases: {},
};
try {
  for (const f of fixtures.filter((f) => !filter.length || filter.includes(f.id))) {
    const name = f.id + "--" + f.scenario,
      dir = path.join(root, name);
    fs.mkdirSync(dir, { recursive: true });
    const write = (p, s) => fs.writeFileSync(path.join(dir, p), s);
    const css = f.tailwind
      ? (await compile("@tailwind utilities;")).build(f.tailwind)
      : (f.css ?? "");
    const imports = Object.entries(f.modules ?? {})
      .map(([k, s]) => {
        write(k + ".module.css", s);
        return "import " + k + ' from "./' + k + '.module.css";';
      })
      .join("\n");
    const source =
      imports +
      (css ? 'import "./style.css";' : "") +
      (f.prefix ?? "") +
      "\nexport default function App(){" +
      (f.setup ?? "") +
      "return (" +
      f.jsx +
      ")}";
    write("App.tsx", source);
    write("style.css", css);
    for (const asset of ["mark.png", "triangle.ttf"])
      fs.copyFileSync(path.join(here, "assets", asset), path.join(dir, asset));
    const build = (contents, more = {}) =>
      buildSync({
        stdin: { contents, resolveDir: dir, loader: "tsx" },
        jsx: "automatic",
        bundle: true,
        outdir: "out",
        write: false,
        nodePaths: [modules],
        external: ["./triangle.ttf"],
        ...more,
      });
    const assets = build(
      'import {createRoot} from "react-dom/client";import App from "./App";createRoot(document.querySelector("#app")).render(<App/>);',
    ).outputFiles;
    const built = assets.find((a) => a.path.endsWith(".css"))?.text ?? "";
    write("built.css", built);
    write("ref.js", assets.find((a) => a.path.endsWith(".js")).text);
    write(
      "ref.html",
      "<!doctype html><meta charset=utf-8><style>" +
        reset +
        built.replaceAll("./triangle.ttf", fontURL) +
        '</style><div id="app"></div><script src="ref.js"></script>',
    );
    let bindings = {};
    if (f.modules) {
      const artifact = build(
        imports + "export default {" + Object.keys(f.modules).join(",") + "}",
        { format: "cjs" },
      ).outputFiles.find((a) => a.path.endsWith(".js")).text;
      const mod = { exports: {} };
      new Function("module", "exports", artifact)(mod, mod.exports);
      bindings = mod.exports.default;
    }
    const archive = command(["map", dir]);
    const m = JSON.parse(fs.readFileSync(path.join(archive.json.output, "source-model.json")));
    write("baseline.json", JSON.stringify(m));
    const tree = project(source, bindings);
    write("bindings.json", JSON.stringify(bindings));
    const lowerDir = path.join(dir, "normalized");
    fs.mkdirSync(lowerDir, { recursive: true });
    fs.writeFileSync(
      path.join(lowerDir, "App.tsx"),
      "export default function App(){return (" + jsx(tree) + ")}",
    );
    fs.writeFileSync(path.join(lowerDir, "built.css"), built);
    const sidecar = {
      tree,
      keyDeclaration: f.key
        ? (m.components[0].root.children ?? []).find((n) => n.kind === "list")?.key
        : null,
      css: built,
      assets: f.id === "image" ? { "./mark.png": imageURL } : {},
      font: f.id === "font" ? fontURL : null,
    };
    write("sidecar.json", JSON.stringify(sidecar));
    const row = (result.cases[name] = {
      id: f.id,
      scenario: f.scenario,
      fields: f.probes,
      mapMs: archive.ms,
      hasScreen: !!m.screens?.length,
      keyAssertion: f.key ? !!hasKey(m) : null,
      views: {},
    });
    for (const width of [1440, 390]) {
      const height = width === 390 ? 844 : 920;
      const page = await b.newPage({
        viewport: { width: 1440, height: 920 },
        locale: "en-US",
        deviceScaleFactor: 1,
      });
      await page.setContent(
        '<style>*{box-sizing:border-box}body{margin:0}iframe{display:block;border:0}</style><iframe width="' +
          width +
          '" height="' +
          height +
          '"></iframe>',
      );
      const frame = page.frames()[1];
      await frame.goto(pathToFileURL(path.join(dir, "ref.html")).href);
      await frame.waitForSelector("#app > *", { state: "attached" });
      const view = (row.views[width] = {});
      const plus = built
        ? alternate([
            "model",
            dir,
            "App.tsx",
            "--built-css",
            "built.css",
            "--viewport-width",
            String(width),
          ])
        : m;
      for (const [mode, model] of [
        ["Chrome", null],
        ["map", m],
        ["sheet", plus],
        [
          "lower",
          alternate([
            "model",
            lowerDir,
            "App.tsx",
            ...(built ? ["--built-css", "built.css", "--viewport-width", String(width)] : []),
          ]),
        ],
        ["sidecar", null],
      ]) {
        if (model?.failure) {
          view[mode] = { values: null, errors: { extractor: model.failure } };
          continue;
        }
        if (mode !== "Chrome") {
          await page
            .locator("iframe")
            .evaluate((e, w) => (e.width = w), mode === "sidecar" ? width : 1440);
          await frame.goto("about:blank");
          await frame.evaluate(() => {
            document.body.style.margin = "0";
          });
          if (mode === "sidecar") {
            await frame.addScriptTag({
              content: "window.materializeScene = " + materialize.toString(),
            });
            await frame.evaluate(
              ({ s, reset }) => {
                const style = document.head.appendChild(document.createElement("style"));
                style.textContent =
                  reset + (s.font ? s.css.replaceAll("./triangle.ttf", s.font) : s.css);
                const replace = (n) => {
                  if (!n || typeof n !== "object") return;
                  if (s.assets[n.attrs.src]) n.attrs.src = s.assets[n.attrs.src];
                  n.children.forEach(replace);
                };
                replace(s.tree);
                window.materializeScene(document, s.tree);
              },
              {
                s: JSON.parse(fs.readFileSync(path.join(dir, "sidecar.json"))),
                reset,
              },
            );
          } else {
            await frame.addScriptTag({ content: drawer });
            await frame.evaluate(
              ({ m, width, height }) => {
                const r = dui.drawModel(
                  dui.decodeModel(m),
                  m.components.find((c) => c.name === "App").id,
                  { width, height },
                );
                const host = document.body.appendChild(document.createElement("div"));
                host.style.width = width + "px";
                host.style.height = height + "px";
                dui.mount(host, r);
              },
              { m: model, width, height },
            );
            await frame.waitForFunction(
              () =>
                document.querySelector(".approximate-model-content")?.shadowRoot?.childElementCount,
            );
          }
        }
        await frame.evaluate(async () => {
          document.body.getBoundingClientRect();
          await document.fonts.ready;
          const root = document.querySelector(".approximate-model-content")?.shadowRoot ?? document;
          await Promise.all(
            [...root.querySelectorAll("img")].map((i) => i.decode().catch(() => {})),
          );
        });
        await frame.addScriptTag({ content: "window.readProbes = " + probe.toString() });
        const values = await frame.evaluate(
          ({ probes, mode }) =>
            window.readProbes(
              mode === "Chrome"
                ? document.querySelector("#app")
                : (document.querySelector(".approximate-model-content")?.shadowRoot ?? document),
              probes,
            ),
          { probes: f.probes, mode },
        );
        const shot = await page.screenshot({
          path: path.join(dir, mode + "-" + width + ".png"),
          clip: { x: 0, y: 0, width: Math.min(width, 360), height: 116 },
        });
        for (const v of Object.values(values))
          if (v?.pixel) {
            const [x, y] = v.pixel.map(Math.floor),
              im = PNG.sync.read(shot);
            v.pixel = [...im.data.subarray(4 * (y * im.width + x), 4 * (y * im.width + x) + 3)];
          }
        view[mode] = { values };
        if (mode === "Chrome") {
          if (f.expected !== undefined && values.section.text !== f.expected)
            throw Error("bad scenario " + name);
          if (
            (f.id === "font" && !values.section.fontLoaded) ||
            (f.id === "image" && !values.img.loaded)
          )
            throw Error("reference asset not loaded " + name);
        } else {
          const errors = deviations(view.Chrome.values, values, f.id);
          if (f.key && !hasKey(model ?? m)) errors.modelKey = "missing";
          if (!row.hasScreen && mode === "map") errors.screen = "missing";
          view[mode].errors = errors;
        }
      }
      await page.close();
    }
    fs.writeFileSync(path.join(root, "results.json"), JSON.stringify(result, null, 2));
  }
} finally {
  await b.close();
}
