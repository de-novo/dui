// Exercise the installed CLI against an app with no node_modules or tsconfig.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Ajv2020 } from "ajv/dist/2020.js";

const cli = resolve("target/debug/dui");
const fixture = resolve("fixtures/source-model");
const validate = new Ajv2020({ strictRequired: false }).compile<Record<string, any>>(
  JSON.parse(readFileSync("schemas/source-model.v1.schema.json", "utf8")),
);
const extract = (project: string, entry: string) =>
  execFileSync(cli, ["model", project, entry], {
    encoding: "utf8",
    env: { ...process.env, PATH: "", DUI_CHROMIUM: "/absent" },
  });
const output = extract(fixture, "src/App.tsx");
assert.equal(output, extract(fixture, "src/App.tsx"));
const model = JSON.parse(output);
assert.ok(validate(model), JSON.stringify(validate.errors));
const reference = JSON.parse(readFileSync(join(fixture, "model.json"), "utf8"));
assert.deepEqual(model.files, reference.files);
assert.deepEqual(model.screens, reference.screens);
assert.deepEqual(model.edges, reference.edges);
const orders = model.components.find((c: { name: string }) => c.name === "Orders");
assert.equal(orders.slots[0].type.item.schema, "src/api.ts#Order");
assert.deepEqual(orders.slots[1].type.values, ["all", "open"]);
const branch = orders.root.children[2];
assert.deepEqual(branch.on.path, ["length"]);
assert.deepEqual(branch.arms[0].when, { equals: 0 });
const list = branch.arms[1].children[0].children[0];
assert.deepEqual(list.key, { kind: "item", list: list.id, path: ["id"] });
const row = list.children[0];
assert.equal(row.children[0].children[0].parts[0].list, list.id);
assert.equal(
  row.style.find((s: { property: string }) => s.property === "borderRadius").value.value,
  8,
);
assert.equal(row.children[1].unresolvedStyle[0].text, "text-transform: uppercase;");
assert.equal(row.children[1].unresolvedStyle[0].location.file, "src/Orders.module.css");
assert.ok(orders.root.style.some((s: { from: { text: string } }) => s.from.text === "gap-4"));
assert.equal(orders.root.children[0].style.at(-1).value.token, "src/theme.css#--color-brand");

const project = mkdtempSync(join(tmpdir(), "dui-tsx-"));
try {
  writeFileSync(
    join(project, "App.tsx"),
    `import { View as Aliased } from "./View";\nexport default function App() { return <Aliased />; }`,
  );
  writeFileSync(
    join(project, "View.tsx"),
    `import "./App";\nexport const View = () => <p className="p-6">안녕🌿</p>;`,
  );
  const alternate = JSON.parse(extract(project, "App.tsx"));
  assert.ok(validate(alternate));
  assert.equal(alternate.files.length, 2);
  assert.equal(alternate.screens[0].component, "App.tsx#App");
  assert.equal(alternate.components[0].root.component, "View.tsx#View");
  const view = alternate.components[1].root;
  assert.equal(view.style, undefined);
  assert.equal(view.unresolvedStyle[0].text, "p-6");
  const { start, end } = view.children[0].location;
  assert.equal(readFileSync(join(project, "View.tsx")).subarray(start, end).toString(), "안녕🌿");
  writeFileSync(
    join(project, "View.tsx"),
    `import "./theme.css"; export const View = () => <p className="text-brand bg-loop rounded-card">Preview</p>;`,
  );
  writeFileSync(
    join(project, "theme.css"),
    `@import "tailwindcss";
@theme {
  --color-brand: var(--color-primary);
  --color-primary: var(--color-base);
  --color-base: #AbC;
  --color-loop: var(--color-cycle);
  --color-cycle: var(--color-loop);
  --radius-card: var(--radius-missing);
}`,
  );
  const themed = JSON.parse(extract(project, "View.tsx"));
  assert.ok(validate(themed));
  const brand = themed.tokens.find((t: { name: string }) => t.name === "--color-brand");
  assert.deepEqual(brand.value, {
    kind: "token",
    token: "theme.css#--color-primary",
    resolved: { kind: "color", value: "#aabbcc" },
  });
  const [color, background, radius] = themed.components[0].root.style;
  assert.deepEqual(color.value, { kind: "token", token: brand.id, resolved: brand.value.resolved });
  assert.equal(background.value.resolved, undefined);
  assert.equal(radius.value.resolved, undefined);
  assert.equal(
    themed.tokens.find((t: { name: string }) => t.name === "--color-loop").value.text,
    "var(--color-cycle)",
  );
  writeFileSync(
    join(project, "View.tsx"),
    `import sheet from "./View.module.css";
export function Shadow({sheet}: {sheet: {card: string}}){return <p className={sheet.card}/>;}
export default function View(){return <p style={{color:"#ffffff"}} className={sheet.card}/>;}`,
  );
  const cssPath = join(project, "View.module.css");
  writeFileSync(cssPath, ".card {gap: 1.5rem 1.5rem;} .other {gap: 2px 5%; row-gap: 3px;}");
  writeFileSync(
    join(project, "Gaps.tsx"),
    `import s from "./View.module.css"; export function Gaps(){return <div><p className={s.card}/><p style={{gap:"1.5rem 1.5rem"}}/><p className={s.other}/><p style={{gap:"2px 5%",columnGap:3}}/></div>}`,
  );
  const gaps = JSON.parse(extract(project, "Gaps.tsx"));
  assert.ok(validate(gaps));
  const [cssGap, inlineGap, mixedCss, mixedInline] = gaps.components[0].root.children;
  for (const child of [cssGap, inlineGap]) {
    assert.equal(child.style[0].property, "gap");
    assert.deepEqual(child.style[0].value, { kind: "length", value: 1.5, unit: "rem" });
  }
  assert.equal(mixedCss.style[0].value.kind, "unresolved");
  assert.equal(mixedCss.unresolvedStyle[0].text, "row-gap: 3px;");
  assert.equal(mixedInline.style, undefined);
  assert.equal(mixedInline.unresolvedStyle.length, 2);
  writeFileSync(
    cssPath,
    `p.card { color: #321; }
.card { color: #abc; }
@media (min-width: 40rem) { .card { gap: 12px; } }
.card { width: var(--runtime-width); }
aside.card { height: 40px; }`,
  );
  const cssModel = JSON.parse(extract(project, "View.tsx"));
  assert.ok(validate(cssModel));
  const [shadow, card] = cssModel.components.map((c: { root: unknown }) => c.root);
  assert.equal(shadow.style, undefined);
  assert.ok(shadow.unresolvedStyle[0].text.includes("sheet.card"));
  assert.deepEqual(
    card.style.map((d: { property: string }) => d.property),
    ["color", "gap", "width", "color", "color"],
  );
  assert.deepEqual(
    card.style.map((d: { value: { value?: unknown } }) => d.value.value),
    ["#aabbcc", 12, undefined, "#332211", "#ffffff"],
  );
  assert.deepEqual(card.style[1].when, { minWidth: { value: 40, unit: "rem" } });
  assert.equal(card.style[2].value.kind, "unresolved");
  const cssSource = card.style[1].from;
  assert.equal(cssSource.syntax, "stylesheet");
  assert.equal(cssSource.location.file, "View.module.css");
  assert.equal(
    readFileSync(cssPath).subarray(cssSource.location.start, cssSource.location.end).toString(),
    "gap: 12px;",
  );
  for (const unsupported of [
    ".wrapper .card {color:blue}",
    ".card {color:red!important}",
    "@media print {.card {color:blue}}",
  ]) {
    writeFileSync(cssPath, `.card {padding:2px;} ${unsupported}`);
    const fallback = JSON.parse(extract(project, "View.tsx")).components[1].root;
    assert.ok(fallback.unresolvedStyle[0].text.includes("sheet.card"));
    assert.equal(fallback.style.length, 1);
  }
  writeFileSync(
    cssPath,
    `.card {padding-inline-start: 12px; padding-bottom: 1rem; margin-inline-end: auto; margin-top: -4px;}`,
  );
  writeFileSync(
    join(project, "View.tsx"),
    `import sheet from "./View.module.css";
export default function View(){return <section><p className={sheet.card}/><p style={{paddingInlineStart:12, paddingBottom:"1rem", marginInlineEnd:"auto", marginTop:-4}}/></section>;}`,
  );
  const paired = JSON.parse(extract(project, "View.tsx"));
  assert.ok(validate(paired));
  for (const [index, child] of paired.components[0].root.children.entries()) {
    assert.deepEqual(
      child.style.map((d: { property: string; value: unknown }) => [d.property, d.value]),
      [
        ["paddingStart", { kind: "length", value: 12, unit: "px" }],
        ["paddingBottom", { kind: "length", value: 1, unit: "rem" }],
        ["marginEnd", { kind: "keyword", value: "auto" }],
        ["marginTop", { kind: "length", value: -4, unit: "px" }],
      ],
    );
    assert.ok(
      child.style.every(
        (d: { from: { syntax: string } }) =>
          d.from.syntax === (index === 0 ? "stylesheet" : "inline"),
      ),
    );
  }
} finally {
  rmSync(project, { recursive: true, force: true });
}
console.log(
  JSON.stringify({
    screens: model.screens.length,
    components: model.components.length,
    edges: model.edges.length,
    tokens: model.tokens.length,
  }),
);
