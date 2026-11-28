import fs from "node:fs";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import { createInterface } from "node:readline";
import { gzipSync } from "node:zlib";
import { fileURLToPath } from "node:url";
import { buildSync } from "../web-basics/node_modules/esbuild/lib/main.js";
import { chromium } from "../web-basics/node_modules/playwright-core/index.mjs";
import { PNG } from "../web-basics/node_modules/pngjs/lib/png.js";
import { fixtures } from "../web-basics/fixtures.mjs";
const [flag, stage, chrome] = process.argv.slice(2);
if (flag !== "--capture" || !chrome) throw Error("run --capture STAGING_DIRECTORY CHROME");
const here = path.dirname(fileURLToPath(import.meta.url)),
  main = path.join(stage, "main"),
  admin = path.join(stage, "admin"),
  dui = path.join(stage, "target/release/dui"),
  out = path.join(stage, "web");
fs.mkdirSync(out, { recursive: true });
const read = (p) => fs.readFileSync(p, "utf8");
const env = { ...process.env, HOME: path.join(stage, "home"), DUI_CHROMIUM: "/missing" };
const base = path.join(main, "apps/canvas/src/features/model");
const bundle = (src) =>
  buildSync({
    stdin: { contents: src, resolveDir: here },
    bundle: true,
    minify: true,
    format: "esm",
    write: false,
    target: "es2022",
  }).outputFiles[0].text;
const engines = {};
for (const id of ["DOM", "Yoga"]) {
  const src =
    "import {drawModel,decodeModel} from " +
    JSON.stringify(base + "/modelDocument.ts") +
    ";" +
    (id === "Yoga" ? 'import {arrange} from "./yoga.mjs";' : "") +
    'window.show=(m,name,w,h)=>{const host=document.querySelector("#card");const shadow=host.shadowRoot??host.attachShadow({mode:"open"});shadow.innerHTML=drawModel(decodeModel(m),m.components.find(c=>c.name===name).id,{width:w,height:h}).markup;' +
    (id === "Yoga" ? "arrange(host,w);" : "") +
    "};";
  engines[id] = bundle(src);
}
const matrix = {
  sizes: Object.fromEntries(
    Object.entries(engines).map(([k, v]) => [k, [Buffer.byteLength(v), gzipSync(v).length]]),
  ),
  screens: {},
};
const browser = await chromium.launch({ executablePath: chrome });
const norm = (s) => s.replace(/\s+/g, " ").trim();
const staticAssets = path.join(admin, "dist");
const types = {
  ".js": "application/javascript",
  ".css": "text/css",
  ".woff2": "font/woff2",
  ".svg": "image/svg+xml",
  ".html": "text/html",
};
function diff(a, b) {
  const A = PNG.sync.read(a),
    B = PNG.sync.read(b);
  let ink = 0,
    bad = 0;
  for (let i = 0; i < A.data.length; i += 4) {
    const x = [0, 1, 2];
    if (x.some((k) => Math.min(A.data[i + k], B.data[i + k]) < 235)) {
      ink++;
      if (x.some((k) => Math.abs(A.data[i + k] - B.data[i + k]) > 20)) bad++;
    }
  }
  return { ink, bad, ratio: ink ? bad / ink : null };
}
try {
  matrix.chrome = browser.version();
  for (const [id, component, needle] of [
    ["503", "MaintenanceError", "503"],
    ["403", "ForbiddenError", "403"],
    ["workspace", "App", "dui map ."],
    ["inline", "App", "one two"],
    ["grid", "App", ">A<"],
  ]) {
    const dir = path.join(out, id);
    fs.mkdirSync(dir, { recursive: true });
    const real = id === "503" || id === "403";
    const workspace = id === "workspace";
    const workspaceFile = path.join(dir, "EmptyWorkspace.tsx");
    if (workspace) {
      fs.copyFileSync(
        path.join(main, "apps/canvas/src/features/workspace/EmptyWorkspace.tsx"),
        workspaceFile,
      );
      fs.copyFileSync(
        path.join(main, "apps/canvas/src/preview.css"),
        path.join(dir, "preview.css"),
      );
    }
    const source = workspace
      ? 'import {EmptyWorkspace} from "./EmptyWorkspace";import "./preview.css";export default function App(){return <EmptyWorkspace title="Empty workspace" openModel="Open model"/>}'
      : real
        ? read(
            path.join(
              admin,
              "src/features/errors",
              id === "503" ? "maintenance-error.tsx" : "forbidden.tsx",
            ),
          )
        : "export default function App(){return (" + fixtures.find((x) => x.id === id).jsx + ")}";
    const file = path.join(dir, "App.tsx");
    fs.writeFileSync(file, source);
    if (real) {
      fs.cpSync(path.join(admin, "src/components/ui"), path.join(dir, "src/components/ui"), {
        recursive: true,
      });
      fs.copyFileSync(path.join(admin, "tsconfig.json"), path.join(dir, "tsconfig.json"));
      fs.copyFileSync(path.join(admin, "tsconfig.app.json"), path.join(dir, "tsconfig.app.json"));
    }
    let archive;
    const times = [];
    for (let i = 0; i < 6; i++) {
      const start = performance.now();
      const run = spawnSync("/usr/bin/time", ["-l", dui, "map", dir], { env, encoding: "utf8" });
      if (run.status) throw Error(run.stderr + run.stdout);
      const ms = performance.now() - start;
      archive = JSON.parse(run.stdout);
      if (i)
        times.push({
          ms,
          rssBytes: Number(run.stderr.match(/(\d+)\s+maximum resident set size/)[1]),
        });
    }
    const model = JSON.parse(read(path.join(archive.output, "source-model.json")));
    const result = (matrix.screens[id] = { analysis: times, views: {} });
    const server = spawn(dui, ["serve", archive.output], {
      env,
      stdio: ["ignore", "pipe", "inherit"],
    });
    const [url] = await once(createInterface({ input: server.stdout }), "line");
    try {
      for (const width of [1440, 390]) {
        const height = width === 1440 ? 920 : 844;
        const ctx = await browser.newContext({
          viewport: { width, height },
          deviceScaleFactor: 1,
          colorScheme: "light",
          reducedMotion: "reduce",
          locale: "en-US",
        });
        const ref = await ctx.newPage();
        if (real) {
          await ref.route("**/*", (route) => {
            const u = new URL(route.request().url());
            if (u.hostname !== "fixture.local") return route.abort();
            const f = path.join(staticAssets, u.pathname === "/" + id ? "index.html" : u.pathname);
            if (!fs.existsSync(f) || !fs.statSync(f).isFile()) return route.abort();
            return route.fulfill({
              path: f,
              contentType: types[path.extname(f)] || "application/octet-stream",
            });
          });
          await ref.goto("http://fixture.local/" + id);
          await ref.getByRole("heading", { name: id, exact: true }).waitFor();
        } else {
          const js = buildSync({
            stdin: {
              contents:
                source.replace('import "./preview.css";', "") +
                ';import {createRoot} from "react-dom/client";createRoot(document.querySelector("main")).render(<App/>);',
              resolveDir: dir,
              loader: "tsx",
            },
            jsx: "automatic",
            nodePaths: [path.join(here, "../web-basics/node_modules")],
            bundle: true,
            write: false,
          }).outputFiles[0].text;
          await ref.setContent(
            "<style>*{box-sizing:border-box}body{margin:0;font:16px system-ui,sans-serif}</style><main></main>",
          );
          await ref.addScriptTag({ content: js });
          await ref.waitForSelector("section");
          if (workspace) await ref.addStyleTag({ content: read(path.join(dir, "preview.css")) });
        }
        await ref.evaluate(() => document.fonts.ready);
        const refText = norm(await ref.locator(real ? "#root" : "main").innerText());
        const reference = await ref.screenshot({
          path: path.join(dir, "Chrome-" + width + ".png"),
        });
        await ref.close();
        const views = (result.views[width] = {});
        for (const [engine, code] of Object.entries(engines)) {
          const p = await ctx.newPage();
          await p.goto(url);
          await p.waitForFunction(() =>
            document
              .querySelector(".approximate-model-content")
              ?.shadowRoot?.querySelector("[data-model-node]"),
          );
          // Keep the real viewer mounted and subscribed offscreen; copy the model card on each real watcher update.
          await p.evaluate(() => {
            const sheet = document.createElement("style");
            sheet.textContent =
              "body{margin:0!important;background:white!important}body>*:not(#card){position:fixed!important;left:-200vw!important}#card{position:fixed;inset:0;background:white;overflow:hidden;font:16px system-ui,sans-serif}";
            document.head.append(sheet);
            document.body.append(Object.assign(document.createElement("div"), { id: "card" }));
          });
          await p.addScriptTag({ type: "module", content: code });
          await p.waitForFunction(() => typeof window.show === "function");
          await p.evaluate(
            ([m, c, w, h]) => window.show(m, c, w, h),
            [model, component, width, height],
          );
          const text = norm(
            await p.evaluate(() =>
              document
                .querySelector("#card")
                .shadowRoot.textContent.replace(
                  document.querySelector("#card").shadowRoot.querySelector("style").textContent,
                  "",
                ),
            ),
          );
          const shot = await p.screenshot({ path: path.join(dir, engine + "-" + width + ".png") });
          const pixels = diff(reference, shot);
          const heap = await p.context().newCDPSession(p);
          await heap.send("HeapProfiler.collectGarbage");
          const heapBytes = (await heap.send("Runtime.getHeapUsage")).usedSize;
          const v = (views[engine] = {
            pixels,
            refText,
            text,
            heapBytes,
            match:
              pixels.ratio !== null &&
              pixels.ratio <= 0.01 &&
              text.replace(/\s/g, "") === refText.replace(/\s/g, ""),
            edits: [],
          });
          // Use the live viewer's own model endpoint and full snapshot after revision changes.
          const api = await p.evaluate(() =>
            performance
              .getEntriesByType("resource")
              .map((r) => r.name)
              .find((u) => u.includes("/api/source-model?")),
          );
          if (!api) throw Error("missing viewer model endpoint");
          await p.evaluate(
            async ([api, c, w, h]) => {
              const u = new URL(api);
              for (const key of ["revision", "statusRevision", "wait"]) u.searchParams.delete(key);
              let rev = (await (await fetch(u)).json()).revision;
              window.watching = true;
              void (async () => {
                while (window.watching) {
                  const next = new URL(u);
                  next.searchParams.set("revision", rev);
                  next.searchParams.set("wait", "1");
                  const r = await (await fetch(next)).json();
                  if (r.revision !== rev) {
                    const snap = await (await fetch(u)).json();
                    window.show(snap.model, c, w, h);
                    rev = snap.revision;
                  }
                }
              })().catch((e) => (window.watchError = String(e)));
            },
            [api, component, width, height],
          );
          for (let trial = 0; trial < 8; trial++) {
            const changed = trial % 2 === 0;
            const replacement = needle === ">A<" ? ">Altered<" : needle + " edited";
            const expected = changed ? replacement : needle;
            const wanted = expected.replace(/[<>]/g, "");
            const editFile = workspace ? workspaceFile : file;
            const original = workspace
              ? read(path.join(main, "apps/canvas/src/features/workspace/EmptyWorkspace.tsx"))
              : source;
            const start = performance.now();
            fs.writeFileSync(editFile, changed ? original.replace(needle, replacement) : original);
            try {
              await p.waitForFunction(
                ([word, changed]) => {
                  const root = document
                    .querySelector("#card")
                    .shadowRoot.querySelector(".dui-document-body");
                  const text = root.innerText;
                  return (
                    text.includes(word) &&
                    (changed || (!text.includes(" edited") && !text.includes("Altered")))
                  );
                },
                [wanted, changed],
                { timeout: 1500 },
              );
              await p.evaluate(
                () => new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok))),
              );
              v.edits.push(performance.now() - start);
            } catch {
              v.edits.push(null);
            }
          }
          await p.close();
        }
        await ctx.close();
      }
    } finally {
      server.kill();
    }
    fs.writeFileSync(path.join(out, "matrix.json"), JSON.stringify(matrix, null, 2));
  }
} finally {
  await browser.close();
}
