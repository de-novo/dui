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
    `import "./theme.css"; export const View = () => <p className="text-brand bg-loop rounded-card px-card gap-4">Preview</p>;`,
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
  --spacing-card: var(--spacing-edge);
  --spacing-edge: 0.75rem;
  --spacing-4: 6px;
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
  const spacingToken = themed.tokens.find((t: { name: string }) => t.name === "--spacing-card");
  assert.equal(spacingToken.group, "spacing");
  assert.equal(spacingToken.location.file, "theme.css");
  assert.deepEqual(spacingToken.value, {
    kind: "token",
    token: "theme.css#--spacing-edge",
    resolved: { kind: "length", unit: "rem", value: 0.75 },
  });
  const spacingStyles = themed.components[0].root.style.slice(3);
  assert.deepEqual(
    spacingStyles.map((s: { property: string }) => s.property),
    ["paddingStart", "paddingEnd", "gap"],
  );
  for (const side of spacingStyles.slice(0, 2))
    assert.deepEqual(side.value, {
      kind: "token",
      token: spacingToken.id,
      resolved: spacingToken.value.resolved,
    });
  assert.deepEqual(spacingStyles[2].value, {
    kind: "token",
    token: "theme.css#--spacing-4",
    resolved: { kind: "length", unit: "px", value: 6 },
  });
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
  writeFileSync(
    cssPath,
    ".box {width: auto; height: 0; min-width: 24px; max-width: 75%; min-height: 1em; max-height: 12rem;}",
  );
  writeFileSync(
    join(project, "Sizes.tsx"),
    `import sheet from "./View.module.css"; export function Sizes(){return <main><p className={sheet.box}/><p style={{width:"auto",height:0,minWidth:24,maxWidth:"75%",minHeight:"1em",maxHeight:"12rem"}}/></main>}`,
  );
  const sizes = JSON.parse(extract(project, "Sizes.tsx"));
  assert.ok(validate(sizes));
  for (const [index, child] of sizes.components[0].root.children.entries()) {
    const expected = [
      ["width", { kind: "keyword", value: "auto" }],
      ["height", { kind: "length", value: 0, unit: "px" }],
      ["minWidth", { kind: "length", value: 24, unit: "px" }],
      ["maxWidth", { kind: "length", value: 75, unit: "percent" }],
      ["minHeight", { kind: "length", value: 1, unit: "em" }],
      ["maxHeight", { kind: "length", value: 12, unit: "rem" }],
    ];
    assert.deepEqual(
      child.style.map((d: { property: string; value: unknown }) => [d.property, d.value]),
      expected,
    );
    for (const d of child.style) {
      const loc = d.from.location;
      assert.equal(loc.file, index === 0 ? "View.module.css" : "Sizes.tsx");
      assert.equal(
        readFileSync(join(project, loc.file)).subarray(loc.start, loc.end).toString(),
        d.from.text,
      );
    }
  }

  for (const [css, tsx, expected] of [
    ["0", "0", 0],
    ["2.5", "2.5", 2.5],
    [".5", '"0.5"', 0.5],
    ["-1", "-1", null],
    ["12px", '"12px"', null],
    ["var(--ratio)", "ratio", null],
  ] as const) {
    writeFileSync(cssPath, `.panel { flex-grow: ${css}; }`);
    writeFileSync(
      join(project, "Grow.tsx"),
      `import sheet from "./View.module.css"; export function Grow({ratio}) { return <main><p className={sheet.panel}/><p style={{flexGrow:${tsx}}}/></main>; }`,
    );
    const grow = JSON.parse(extract(project, "Grow.tsx"));
    assert.ok(validate(grow));
    for (const [index, child] of grow.components[0].root.children.entries()) {
      const decl = child.style?.[0];
      assert.equal(decl?.property, "grow");
      if (expected === null) {
        assert.equal(decl.value.kind, "unresolved");
        assert.equal(decl.value.text, index === 0 ? css : tsx);
      } else assert.deepEqual(decl.value, { kind: "number", value: expected });
      const loc = decl.from.location;
      assert.equal(loc.file, index === 0 ? "View.module.css" : "Grow.tsx");
      assert.equal(
        readFileSync(join(project, loc.file)).subarray(loc.start, loc.end).toString(),
        decl.from.text,
      );
    }
  }

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
  writeFileSync(
    cssPath,
    ".card {margin: 1rem auto -4px; padding-inline: 2px 5%; margin-inline: auto -3px;}",
  );
  writeFileSync(
    join(project, "View.tsx"),
    `import s from "./View.module.css"; export function View(){return <div><p className={s.card}/><p style={{margin:"1rem auto -4px",paddingInline:"2px 5%",marginInline:"auto -3px"}}/></div>}`,
  );
  const shorthand = JSON.parse(extract(project, "View.tsx"));
  assert.ok(validate(shorthand));
  for (const child of shorthand.components[0].root.children) {
    assert.deepEqual(
      child.style.map((d: { property: string; value: { value: unknown } }) => [
        d.property,
        d.value.value,
      ]),
      [
        ["marginTop", 1],
        ["marginEnd", "auto"],
        ["marginBottom", -4],
        ["marginStart", "auto"],
        ["paddingStart", 2],
        ["paddingEnd", 5],
        ["marginStart", "auto"],
        ["marginEnd", -3],
      ],
    );
  }
  for (const [css, jsx, expected] of [
    ["-0.75px", "-0.75", { kind: "length", value: -0.75, unit: "px" }],
    ["0", "0", { kind: "length", value: 0, unit: "px" }],
    ["0.08em", '"0.08em"', { kind: "length", value: 0.08, unit: "em" }],
    ["normal", '"normal"', null],
    ["var(--tracking)", "tracking", null],
  ] as const) {
    writeFileSync(cssPath, `.label { letter-spacing: ${css}; }`);
    writeFileSync(
      join(project, "Tracking.tsx"),
      `import s from "./View.module.css"; export function Tracking({tracking}){return <main><p className={s.label}/><p style={{letterSpacing:${jsx}}}/></main>}`,
    );
    const model = JSON.parse(extract(project, "Tracking.tsx"));
    assert.ok(validate(model));
    for (const [i, child] of model.components[0].root.children.entries()) {
      const decl = child.style?.[0];
      assert.equal(decl?.property, "letterSpacing");
      if (expected) assert.deepEqual(decl.value, expected);
      else {
        assert.equal(decl.value.kind, "unresolved");
        assert.equal(decl.value.text, i === 0 ? css : jsx);
      }
      const loc = decl.from.location;
      assert.equal(loc.file, i === 0 ? "View.module.css" : "Tracking.tsx");
      assert.equal(
        readFileSync(join(project, loc.file)).subarray(loc.start, loc.end).toString(),
        decl.from.text,
      );
    }
  }
  for (const [raw, inline, expected] of [
    ["1.5", "1.5", { kind: "number", value: 1.5 }],
    ["24px", '"24px"', { kind: "length", value: 24, unit: "px" }],
    ["120%", '"120%"', { kind: "length", value: 120, unit: "percent" }],
    ["0", "0", { kind: "number", value: 0 }],
    ["normal", '"normal"', null],
    ["-1", "-1", null],
    ["var(--leading)", "props.leading", null],
  ] as const) {
    writeFileSync(cssPath, `.sample {line-height: ${raw};}`);
    writeFileSync(
      join(project, "Leading.tsx"),
      `import s from "./View.module.css"; export function Leading(props){return <div><p className={s.sample}/><p style={{lineHeight:${inline}}}/></div>}`,
    );
    const leading = JSON.parse(extract(project, "Leading.tsx"));
    assert.ok(validate(leading));
    for (const [index, child] of leading.components[0].root.children.entries()) {
      const [decl] = child.style;
      assert.equal(decl.property, "lineHeight");
      if (expected) assert.deepEqual(decl.value, expected);
      else {
        assert.equal(decl.value.kind, "unresolved");
        assert.equal(decl.value.text, index === 0 ? raw : inline);
      }
      const loc = decl.from.location;
      assert.equal(loc.file, index === 0 ? "View.module.css" : "Leading.tsx");
      assert.equal(
        readFileSync(join(project, loc.file)).subarray(loc.start, loc.end).toString(),
        decl.from.text,
      );
    }
  }
  writeFileSync(
    cssPath,
    ".panel {flex-direction: column; align-items: flex-end; justify-content: space-evenly; flex-wrap: wrap;} .unknown {align-items: safe center; flex-direction: row-reverse;}",
  );
  writeFileSync(
    join(project, "Flex.tsx"),
    `import s from "./View.module.css"; export function Flex({center}){return <main><p className={s.panel}/><p style={{flexDirection:"column",alignItems:"flex-end",justifyContent:"space-evenly",flexWrap:"wrap"}}/><p className={s.unknown}/><p style={{alignItems:center,justifyContent:"stretch"}}/></main>}`,
  );
  const flex = JSON.parse(extract(project, "Flex.tsx"));
  assert.ok(validate(flex));
  const [sheet, inline, unsupported, dynamic] = flex.components[0].root.children;
  for (const [i, node] of [sheet, inline].entries()) {
    assert.deepEqual(
      node.style.map((d: { property: string; value: unknown }) => [d.property, d.value]),
      [
        ["direction", { kind: "keyword", value: "column" }],
        ["alignItems", { kind: "keyword", value: "end" }],
        ["justifyContent", { kind: "keyword", value: "spaceEvenly" }],
        ["wrap", { kind: "keyword", value: "wrap" }],
      ],
    );
    for (const d of node.style) {
      assert.equal(d.from.syntax, i === 0 ? "stylesheet" : "inline");
      const loc = d.from.location;
      assert.equal(
        readFileSync(join(project, loc.file)).subarray(loc.start, loc.end).toString(),
        d.from.text,
      );
    }
  }
  for (const [node, texts] of [
    [unsupported, ["safe center", "row-reverse"]],
    [dynamic, ["center", '"stretch"']],
  ] as const) {
    assert.deepEqual(
      node.style.map((d: { value: { kind: string; text: string } }) => [
        d.value.kind,
        d.value.text,
      ]),
      texts.map((t) => ["unresolved", t]),
    );
  }
  for (const [css, jsx, expected] of [
    ["2px", "2", { kind: "length", value: 2, unit: "px" }],
    ["0", "0", { kind: "length", value: 0, unit: "px" }],
    [
      "0.2em 0.2em 0.2em 0.2em",
      '"0.2em 0.2em 0.2em 0.2em"',
      { kind: "length", value: 0.2, unit: "em" },
    ],
    ["1px 2px", '"1px 2px"', null],
    ["10%", '"10%"', null],
    ["-2px", "-2", null],
    ["var(--edge)", "edge", null],
  ] as const) {
    writeFileSync(cssPath, `.panel { border-width: ${css}; }`);
    writeFileSync(
      join(project, "Border.tsx"),
      `import s from "./View.module.css"; export function Border({edge}){return <main><p className={s.panel}/><p style={{borderWidth:${jsx}}}/></main>}`,
    );
    const model = JSON.parse(extract(project, "Border.tsx"));
    assert.ok(validate(model));
    const children = model.components[0].root.children;
    assert.equal(children.length, 2);
    for (const [index, child] of children.entries()) {
      const decl = child.style[0];
      assert.equal(decl.property, "borderWidth");
      if (expected) assert.deepEqual(decl.value, expected);
      else {
        assert.equal(decl.value.kind, "unresolved");
        assert.equal(decl.value.text, index === 0 ? css : jsx);
      }
      const loc = decl.from.location;
      assert.equal(loc.file, index === 0 ? "View.module.css" : "Border.tsx");
      assert.equal(
        readFileSync(join(project, loc.file)).subarray(loc.start, loc.end).toString(),
        decl.from.text,
      );
    }
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
