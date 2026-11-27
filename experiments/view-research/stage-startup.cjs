// Usage: node stage-startup.cjs CHECKOUT OPTIMIZED_GPUI SITE_ROOT
const fs = require("node:fs"),
  path = require("node:path"),
  esbuild = require("esbuild");
const [checkout, bundle, site] = process.argv.slice(2).map((p) => path.resolve(p));
const dir = (name) => path.join(site, "startup", name);
fs.cpSync(bundle, dir("gpui"), { recursive: true });
for (const name of ["dom", "truth"]) fs.mkdirSync(dir(name), { recursive: true });
const index = path.join(dir("gpui"), "index.html");
fs.writeFileSync(
  index,
  fs
    .readFileSync(index, "utf8")
    .replace("const [, model, font]", 'performance.mark("init-start");\n  const [, model, font]')
    .replace(
      "run(model, new Uint8Array(font));",
      'performance.mark("assets-ready");\n  run(model, new Uint8Array(font));\n  performance.mark("run-returned");',
    ),
);
fs.copyFileSync(path.join(bundle, "model.json"), path.join(dir("dom"), "model.json"));
fs.writeFileSync(
  path.join(dir("dom"), "index.html"),
  '<!doctype html><body style="margin:0"><script type="module" src="./app.js"></script></body>\n',
);
const renderer = path.join(checkout, "apps/canvas/src/features/model/modelDocument.ts");
const contents =
  "import {drawModel,decodeModel} from " +
  JSON.stringify(renderer) +
  ';const response=await fetch("./model.json");const model=decodeModel(await response.json());const render=drawModel(model,model.components[0].id,{width:innerWidth});document.body.attachShadow({mode:"open"}).innerHTML=render.markup;performance.mark("dom-mounted");';
esbuild.buildSync({
  stdin: { contents, resolveDir: checkout },
  bundle: true,
  format: "esm",
  minify: true,
  target: "es2022",
  outfile: path.join(dir("dom"), "app.js"),
  nodePaths: (process.env.NODE_PATH || "").split(path.delimiter).filter(Boolean),
});
const source = fs.readFileSync(path.join(__dirname, "../gpui-web/Probe.tsx"), "utf8");
const fixture = esbuild.transformSync(source, {
  loader: "tsx",
  format: "cjs",
  jsx: "automatic",
}).code;
const mod = { exports: {} };
new Function("module", "exports", "require", fixture)(mod, mod.exports, require);
const markup = require("react-dom/server").renderToStaticMarkup(
  require("react").createElement(mod.exports.Probe),
);
fs.writeFileSync(path.join(dir("truth"), "index.html"), "<!doctype html>" + markup);
