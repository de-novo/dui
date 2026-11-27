// Run from experiments/motion. The reference is the mounted, open production dialog.
import { parseSync, visitorKeys } from "oxc-parser";
import * as css from "css-tree";
import { buildSync } from "esbuild";
import { chromium } from "playwright-core";
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
const [src, sheet, extractor, repo, chrome, url, out] = process.argv.slice(2);
const source = fs.readFileSync(src, "utf8"),
  style = fs.readFileSync(sheet, "utf8");
const classes = {};
function visit(n) {
  if (!n?.type) return;
  if (
    n.type === "JSXOpeningElement" &&
    n.name.type === "JSXMemberExpression" &&
    n.name.object.name === "DialogPrimitive"
  ) {
    const a = n.attributes.find((a) => a.name?.name === "className"),
      v = a?.value?.expression?.arguments?.[0]?.value ?? a?.value?.value;
    if (v) classes[n.name.property.name] = v;
  }
  for (const k of visitorKeys[n.type] ?? []) {
    const v = n[k];
    if (Array.isArray(v)) v.forEach(visit);
    else visit(v);
  }
}
visit(parseSync("dialog.tsx", source).program);
if (!classes.Content || !classes.Title) throw Error("missing primitive source");
fs.writeFileSync(path.join(out, "built.css"), style);
const q = JSON.stringify;
fs.writeFileSync(
  path.join(out, "App.tsx"),
  "export function Panel(){return <div className=" +
    q(classes.Content) +
    "><h2 className=" +
    q(classes.Title) +
    ">Dialog</h2></div>}",
);
const model = JSON.parse(execFileSync(extractor, [out]));
const props = {},
  tokens = new Set(classes.Content.split(/\s+/));
const rules = css.parse(style, { positions: true }).children;
for (const r of rules) {
  if (r.type !== "Rule" || r.prelude.type !== "SelectorList") continue;
  const s = r.prelude.children.first.children.toArray();
  const utility =
    s.length === 1 && s[0].type === "ClassSelector" && tokens.has(css.ident.decode(s[0].name));
  const global = css.generate(r.prelude).split(",").includes("*");
  if (!global && !utility) continue;
  for (const d of r.block.children) {
    if (d.type !== "Declaration") continue;
    if (
      (global && /^--tw-(translate|rotate|skew|scale)/.test(d.property)) ||
      (utility &&
        [
          "transform",
          "left",
          "top",
          "position",
          "width",
          "max-width",
          "--tw-translate-x",
          "--tw-translate-y",
        ].includes(d.property))
    )
      props[d.property] = style.slice(d.value.loc.start.offset, d.value.loc.end.offset);
  }
}
const renderer = path.join(repo, "apps/canvas/src/features/model/modelDocument.ts");
const js = buildSync({
  stdin: { contents: "export {drawModel,decodeModel} from " + q(renderer), resolveDir: repo },
  bundle: true,
  globalName: "dui",
  write: false,
  nodePaths: [path.resolve("../relative-units/node_modules")],
}).outputFiles[0].text;
const browser = await chromium.launch({ executablePath: chrome }),
  results = {};
try {
  for (const width of [1440, 390]) {
    const p = await browser.newPage({ viewport: { width, height: width === 1440 ? 920 : 844 } });
    await p.goto(url);
    const panel = p.getByRole("dialog");
    await panel.waitFor();
    await p.waitForTimeout(500);
    const rect = (e) => {
      const { x, y, width, height } = e.getBoundingClientRect();
      return { x, y, width, height, transform: getComputedStyle(e).transform };
    };
    const ref = await panel.evaluate(rect);
    await p.screenshot({ path: path.join(out, "panel-" + width + "-source.png") });
    await p.setContent("<style>body{margin:0}</style><main></main>");
    await p.addScriptTag({ content: js });
    await p.evaluate(
      ({ model, width }) => {
        const s = document.querySelector("main").attachShadow({ mode: "open" });
        s.innerHTML = dui.drawModel(dui.decodeModel(model), model.components[0].id, {
          width,
        }).markup;
      },
      { model, width },
    );
    const el = p.locator("main").locator("div").first();
    const baseline = await el.evaluate(rect);
    await el.evaluate((e, props) => {
      for (const [k, v] of Object.entries(props)) e.style.setProperty(k, v);
    }, props);
    const candidate = await el.evaluate(rect);
    await p.screenshot({ path: path.join(out, "panel-" + width + "-after.png") });
    results[width] = { ref, baseline, candidate };
    await p.close();
  }
} finally {
  await browser.close();
}
fs.writeFileSync(
  path.join(out, "panel.json"),
  JSON.stringify({ classes, props, results }, null, 2),
);
