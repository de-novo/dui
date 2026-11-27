// Exercise the installed CLI against an app with no node_modules or tsconfig.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  mkdtempSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { pathToFileURL } from "node:url";
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

const queryProject = mkdtempSync(join(tmpdir(), "dui-query-view-"));
try {
  mkdirSync(join(queryProject, "src"));
  const entrySource = `import { lazy } from "react";
const DiffView = lazy(() => import("./DiffView").then((m) => ({ default: m.DiffView })));
function Viewer() {
  const params = new URLSearchParams(location.search);
  const isDiff = params.get("view") === "diff";
  function showDiff() { history.pushState(null, "", "?view=diff"); }
  return <main>
    <a href="?view=model">Model</a>
    <button onClick={showDiff}>Diff</button>
    {isDiff && document.readyState === "complete" ? <DiffView /> : <p>Overview</p>}
  </main>;
}
if (new URLSearchParams(location.search).get("view") === "model") {
  const { ModelView } = await import("./ModelView");
  root.render(<ModelView />);
} else {
  root.render(<Viewer />);
}`;
  writeFileSync(join(queryProject, "src/preview.tsx"), entrySource);
  writeFileSync(
    join(queryProject, "src/ModelView.tsx"),
    `export function ModelView() { return <main><a href="?">Map</a></main>; }`,
  );
  writeFileSync(
    join(queryProject, "src/DiffView.tsx"),
    `export function DiffView() { return <main>Diff</main>; }`,
  );
  const mapOutput = execFileSync(cli, ["map", queryProject, "--entry", "src/preview.tsx"], {
    encoding: "utf8",
    env: { ...process.env, HOME: join(queryProject, "home"), PATH: "", DUI_CHROMIUM: "/absent" },
  });
  const mapBody = JSON.parse(mapOutput);
  assert.equal(mapBody.screenDiscovery.declared, 3, JSON.stringify(mapBody.screenDiscovery));
  assert.equal(
    mapBody.screenDiscovery.resolvedRelations,
    2,
    JSON.stringify(mapBody.screenDiscovery),
  );
  const graph = JSON.parse(readFileSync(join(mapBody.output, "graph.json"), "utf8"));
  assert.deepEqual(graph.nodes.map((n: { label: string }) => n.label).sort(), [
    "/",
    "/?view=diff",
    "/?view=model",
  ]);
  assert.deepEqual(
    graph.edges.map((e: { source: string; target: string }) => [e.source, e.target]).sort(),
    [
      ["route:/", "route:/?view=model"],
      ["route:/?view=model", "route:/"],
    ],
  );
  const queryModel = JSON.parse(extract(queryProject, "src/preview.tsx"));
  assert.ok(
    queryModel.edges.every(
      (edge: { to: { kind: string; screen?: string }; kind: string }) =>
        edge.kind !== "navigate" &&
        !(edge.to.kind === "screen" && edge.to.screen === "route:/?view=diff"),
    ),
    "runtime pushState must not become a source-proven diff navigation edge",
  );

  writeFileSync(
    join(queryProject, "src/preview.tsx"),
    entrySource.replace('href="?view=model"', 'href="?view=other"'),
  );
  const uncorroborated = JSON.parse(extract(queryProject, "src/preview.tsx"));
  const other = uncorroborated.edges.find((edge: { location: { file: string } }) =>
    edge.location.file.endsWith("preview.tsx"),
  );
  assert.equal(other.to.kind, "opaque", "query links without a matching view stay opaque");

  writeFileSync(
    join(queryProject, "src/preview.tsx"),
    entrySource.replace(
      "root.render(<ModelView />);",
      "root.render(<><ModelView /><DiffView /></>);",
    ),
  );
  const ambiguous = JSON.parse(extract(queryProject, "src/preview.tsx"));
  assert.ok(
    !ambiguous.screens?.some((screen: { route?: string }) => screen.route === "/?view=model"),
    "a branch with two possible view components is not a source-proven query view",
  );
} finally {
  rmSync(queryProject, { recursive: true, force: true });
}

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
  for (const [base, expected] of [
    ["3px", { kind: "length", value: 6, unit: "px" }],
    ["var(--spacing-unit)", { kind: "length", value: 1, unit: "rem" }],
    ["var(--unknown)", null],
    ["2px 4px", null],
    ["0", { kind: "length", value: 0, unit: "px" }],
  ] as const) {
    writeFileSync(
      join(project, "theme.css"),
      `@import "tailwindcss"; @theme { --spacing: ${base}; --spacing-unit: 0.5rem; --spacing-4: 11px; }`,
    );
    writeFileSync(
      join(project, "Scale.tsx"),
      `import "./theme.css"; export function Scale(){return <main className="gap-2 px-4"/>;}`,
    );
    const scaled = JSON.parse(extract(project, "Scale.tsx"));
    assert.ok(validate(scaled));
    const [gap, start, end] = scaled.components[0].root.style;
    assert.equal(gap.property, "gap");
    if (expected) assert.deepEqual(gap.value, expected);
    else assert.equal(gap.value.kind, "unresolved");
    assert.deepEqual(start.value, {
      kind: "token",
      token: "theme.css#--spacing-4",
      resolved: { kind: "length", value: 11, unit: "px" },
    });
    assert.deepEqual(end.value, start.value);
  }
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
// Build an actual utility stylesheet. The extractor only receives its finished artifact.
// Neutral property, computed CSS property and expected values for each rendered card.
const cardStyles = [
  ["gap", "gap", "6px", "1px", "8px"],
  ["color", "color", "rgb(18, 52, 86)", "rgb(34, 68, 102)", "rgb(51, 85, 119)"],
  [
    "background",
    "background-color",
    "rgb(210, 221, 232)",
    "rgb(196, 213, 230)",
    "rgb(179, 196, 213)",
  ],
  [
    "borderColor",
    "border-top-color",
    "rgb(69, 103, 137)",
    "rgb(69, 103, 137)",
    "rgb(69, 103, 137)",
  ],
  ["borderWidth", "border-top-width", "2px", "3px", "4px"],
  ["borderRadius", "border-top-left-radius", "7px", "5px", "9px"],
  ["width", "width", "160px", "140px", "120px"],
  ["height", "height", "90px", "80px", "70px"],
  ["minWidth", "min-width", "80px", "80px", "80px"],
  ["maxWidth", "max-width", "220px", "220px", "220px"],
  ["minHeight", "min-height", "40px", "40px", "40px"],
  ["maxHeight", "max-height", "120px", "120px", "120px"],
  ["opacity", "opacity", "0.91", "0.82", "0.73"],
  ["fontSize", "font-size", "16px", "14px", "12px"],
  ["lineHeight", "line-height", "24px", "20px", "18px"],
  ["letterSpacing", "letter-spacing", "1px", "2px", "3px"],
  ["direction", "flex-direction", "column", "row", "row"],
  ["wrap", "flex-wrap", "wrap", "wrap", "wrap"],
  ["alignItems", "align-items", "center", "center", "center"],
  ["justifyContent", "justify-content", "center", "center", "center"],
  ["grow", "flex-grow", "2", "2", "2"],
  ...["Top", "Bottom", "Start", "End"].flatMap((side) => {
    const suffix =
      side === "Start" || side === "End" ? "inline-" + side.toLowerCase() : side.toLowerCase();
    return [
      ["padding" + side, "padding-" + suffix, "12px", "10px", "10px"],
      ["margin" + side, "margin-" + suffix, "6px", "4px", "4px"],
    ];
  }),
];
const builtProject = mkdtempSync(resolve("apps/canvas/.dui-style-"));
try {
  writeFileSync(
    join(builtProject, "vite.config.mjs"),
    'import tw from "@tailwindcss/vite"; export default {base:"./",plugins:[tw()]};',
  );
  writeFileSync(
    join(builtProject, "style.css"),
    `@import "tailwindcss"; @theme { --spacing: 3px; }
:root { --card-ink:#123456; }
@layer components {
.card { display:flex; color:var(--card-ink); background-color:#d2dde8; border-style:solid; border-color:#456789; border-width:2px; border-radius:7px;
 width:160px; height:90px; min-width:80px; max-width:220px; min-height:40px; max-height:120px;
 opacity:.91; font-size:16px; line-height:24px; letter-spacing:1px;
 flex-direction:column; flex-wrap:wrap; align-items:center; justify-content:center; flex-grow:2; padding:12px; margin:6px; }
}
main { padding-top:3px; }
@media (max-width:768px) { main { padding-top:7px; } }
@media (width<768px) { main { padding-top:11px; } }
[data-dense] { gap:1px; color:#224466; background-color:#c4d5e6; border-width:3px; border-radius:5px; width:140px; height:80px; opacity:.82; font-size:14px; line-height:20px; letter-spacing:2px; flex-direction:row; padding:10px; margin:4px; }`,
  );
  writeFileSync(
    join(builtProject, "index.html"),
    '<style>output{display:none}</style><div id="app"></div><output></output><output id="responsive"></output><script type="module" src="./entry.tsx"></script>',
  );
  writeFileSync(
    join(builtProject, "entry.tsx"),
    `import React from "react";
import {createRoot} from "react-dom/client";
import {flushSync} from "react-dom";
import {App} from "./App";
flushSync(()=>createRoot(document.querySelector("#app")).render(<App/>));
document.querySelector("#responsive").textContent=[innerWidth,getComputedStyle(document.querySelector("main")).paddingTop].join(",");
const props=${JSON.stringify(cardStyles.map(([, css]) => css))};
document.querySelector("output").textContent=JSON.stringify([...document.querySelectorAll("main div")].map(el=>props.map(p=>getComputedStyle(el).getPropertyValue(p))));`,
  );
  writeFileSync(
    join(builtProject, "App.tsx"),
    'import React from "react"; import "./style.css"; export function App(){return <main><div className="card gap-2">Base</div><div className="card gap-2" data-dense="yes">Stylesheet</div><div className="card" data-dense style={{gap:8,color:"#335577",backgroundColor:"#b3c4d5",borderWidth:4,borderRadius:9,width:120,height:70,opacity:.73,fontSize:12,lineHeight:"18px",letterSpacing:3}}>Inline</div></main>}',
  );
  execFileSync(process.execPath, [resolve("apps/canvas/node_modules/vite/bin/vite.js"), "build"], {
    cwd: builtProject,
  });
  const asset = readdirSync(join(builtProject, "dist/assets")).find((f) => f.endsWith(".css"));
  assert.ok(asset);
  const bundle = "dist/assets/" + asset;
  const readBuilt = (...options: string[]) =>
    JSON.parse(
      execFileSync(cli, ["model", builtProject, "App.tsx", "--built-css", bundle, ...options], {
        encoding: "utf8",
        env: { ...process.env, PATH: "" },
      }),
    );
  const output = readBuilt();
  assert.ok(validate(output), JSON.stringify(validate.errors));
  assert.equal(
    output.components[0].root.style.find((d: { property: string }) => d.property === "paddingTop")
      .value.kind,
    "unresolved",
  );
  for (const [width, expected] of [
    [767, 11],
    [768, 7],
    [769, 3],
  ]) {
    const root = readBuilt("--viewport-width", String(width)).components[0].root;
    assert.deepEqual(
      root.style.find((d: { property: string }) => d.property === "paddingTop").value,
      { kind: "length", value: expected, unit: "px" },
    );
    const cards = root.children;
    const modelValues = cards.map((card: { style: any[] }, index: number) =>
      cardStyles.map(([property], k) => {
        const matches = card.style.filter((d) => d.property === property);
        assert.equal(matches.length, 1, `Card ${index} ${property}`);
        const { value, from } = matches[0];
        let cssValue = String(value.value);
        if (value.kind === "color") {
          const channels = value.value
            .slice(1)
            .match(/../g)
            .map((h: string) => parseInt(h, 16));
          cssValue = "rgb(" + channels.join(", ") + ")";
        } else if (value.kind === "length") cssValue += value.unit;
        assert.equal(
          cssValue,
          cardStyles[k][index + 2],
          JSON.stringify({ index, property, value, from }),
        );
        if (from.syntax !== "inline") {
          assert.equal(from.location.file, bundle);
          assert.equal(
            readFileSync(join(builtProject, bundle))
              .subarray(from.location.start, from.location.end)
              .toString(),
            from.text,
          );
        }
        return cssValue;
      }),
    );
    if (process.argv.includes("--verify-render")) {
      const chrome = process.env.DUI_CHROMIUM;
      assert.ok(chrome, "Set DUI_CHROMIUM to the browser executable for render verification");
      const captureDir = process.env.DUI_RENDER_CAPTURE_DIR;
      const capture = join(builtProject, `card-${width}.png`);
      const dom = execFileSync(
        chrome,
        [
          "--headless",
          `--window-size=${width},900`,
          "--disable-gpu",
          "--disable-background-networking",
          "--allow-file-access-from-files",
          "--no-first-run",
          `--user-data-dir=${join(builtProject, "browser-profile")}`,
          ...(captureDir ? [`--screenshot=${capture}`] : []),
          "--dump-dom",
          pathToFileURL(join(builtProject, "dist/index.html")).href,
        ],
        { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
      );
      assert.ok(dom.includes(`<output id="responsive">${width},${expected}px</output>`));
      const measured = dom.match(/<output>([^<]+)<\/output>/)?.[1];
      assert.ok(measured);
      assert.deepEqual(JSON.parse(measured), modelValues);
      if (captureDir) {
        mkdirSync(captureDir, { recursive: true });
        copyFileSync(capture, join(captureDir, `card-${width}.png`));
      }
      console.log(
        JSON.stringify({
          viewportWidth: width,
          renderedCards: cards.length,
          propertiesPerCard: cardStyles.length,
          matched: cards.length * cardStyles.length,
        }),
      );
    }
  }
  writeFileSync(
    join(builtProject, bundle),
    `
    .card { gap:7px; color:#123456; width:140px; border-radius:5px; }
    @media (width>=500px) { .card { gap:9px; color:#abcdef; } }
    .card { transform:var(--rotate, ) translateY(1px) }
    .card { border-top-left-radius:2px }
    .sidebar .badge:hover, .notice > strong { width:320px }
  `,
  );
  const staticCard = readBuilt().components[0].root.children[0];
  for (const prop of ["color", "gap", "borderRadius"]) {
    assert.equal(
      staticCard.style.find((d: { property: string }) => d.property === prop).value.kind,
      "unresolved",
    );
  }
  assert.deepEqual(
    staticCard.style.find((d: { property: string }) => d.property === "width").value,
    { kind: "length", value: 140, unit: "px" },
  );
  writeFileSync(
    join(builtProject, bundle),
    `
    .external div { font-size: 30px; }
    main > div { line-height: 20px; width: fit-content; }
    main div { font-size: 14px; }
    div > div { line-height: 90px; }
    .external :is(a, code), [data-missing=yes], #missing, div::before { width: 3px; }
  `,
  );
  const nested = readBuilt().components[0].root.children[0].style;
  const valueOf = (key: string) =>
    nested.find((d: { property: string }) => d.property === key)?.value;
  assert.equal(valueOf("fontSize").kind, "unresolved"); // External class has higher specificity.
  assert.deepEqual(valueOf("lineHeight"), { kind: "length", value: 20, unit: "px" });
  assert.deepEqual(valueOf("width"), { kind: "keyword", value: "fit" });
  writeFileSync(
    join(builtProject, bundle),
    `
    :root[data-mode=night] { --ink: #aabbcc; }
    :root { --ink: #112233; }
    :root[data-mode=day] { --ink: #445566; }
    .card { color: var(--ink); border: 1px solid var(--ink); }
  `,
  );
  for (const [mode, ink] of [
    ["night", "#aabbcc"],
    ["day", "#445566"],
    ["other", "#112233"],
  ]) {
    const declarations = readBuilt("--root-attribute", "data-mode=" + mode).components[0].root
      .children[0].style;
    for (const property of ["color", "borderColor"]) {
      assert.deepEqual(
        declarations.find((d: { property: string }) => d.property === property).value,
        { kind: "color", value: ink },
      );
    }
  }
  assert.equal(
    readBuilt().components[0].root.children[0].style.find(
      (d: { property: string }) => d.property === "color",
    ).value.kind,
    "unresolved",
  );
  writeFileSync(
    join(builtProject, bundle),
    `
    main {display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr))}
    @media(max-width:680px){ main {grid-template-columns:minmax(0,1fr)} }
    main {border:2px dashed #aabbcc;border-bottom:4px solid #112233;backdrop-filter:blur(14px);
      background:color-mix(in srgb,#112233 82%,transparent);border-collapse:collapse;text-transform:uppercase;
      vertical-align:top;text-align:left;white-space:pre-wrap}
    main {gap:8px 10px;column-gap:12px;padding:4px min(10px,1vw);padding-block:12px 18px;font-family:system-ui,sans-serif;text-decoration:none}
    div.card { flex:2 0 40%; order:-2; overflow-x:auto; font-size:clamp(30px,5vw,48px) }
  `,
  );
  for (const width of [390, 1440]) {
    const root = readBuilt("--viewport-width", String(width)).components[0].root;
    const prop = (styles: Array<{ property: string; value: unknown }>, name: string) =>
      styles.find((d) => d.property === name)?.value;
    assert.deepEqual(
      prop(root.style, "gridColumnMinimum"),
      width === 390
        ? { kind: "keyword", value: "single" }
        : { kind: "length", value: 300, unit: "px" },
    );
    for (const [name, value] of Object.entries({
      borderTopStyle: "dashed",
      borderBottomStyle: "solid",
      borderCollapse: "collapse",
      textTransform: "uppercase",
      verticalAlign: "top",
      textAlign: "left",
      whiteSpace: "pre-wrap",
    })) {
      assert.deepEqual(prop(root.style, name), { kind: "keyword", value });
    }
    assert.deepEqual(prop(root.style, "background"), { kind: "color", value: "#112233d1" });
    assert.deepEqual(prop(root.style, "backdropBlur"), { kind: "length", value: 14, unit: "px" });
    assert.deepEqual(prop(root.style, "borderBottomWidth"), {
      kind: "length",
      value: 4,
      unit: "px",
    });
    assert.deepEqual(prop(root.style, "borderTopWidth"), { kind: "length", value: 2, unit: "px" });
    assert.deepEqual(prop(root.style, "paddingStart"), {
      kind: "length",
      value: width === 390 ? 3.9 : 10,
      unit: "px",
    });
    assert.deepEqual(prop(root.style, "paddingTop"), { kind: "length", value: 12, unit: "px" });
    assert.deepEqual(prop(root.style, "paddingBottom"), { kind: "length", value: 18, unit: "px" });
    assert.deepEqual(prop(root.style, "fontFamily"), {
      kind: "keyword",
      value: "system-ui,sans-serif",
    });
    assert.deepEqual(prop(root.style, "textDecorationLine"), { kind: "keyword", value: "none" });
    assert.deepEqual(prop(root.style, "rowGap"), { kind: "length", value: 8, unit: "px" });
    assert.deepEqual(prop(root.style, "columnGap"), { kind: "length", value: 12, unit: "px" });
    assert.deepEqual(prop(root.children[0].style, "shrink"), { kind: "number", value: 0 });
    assert.deepEqual(prop(root.children[0].style, "grow"), { kind: "number", value: 2 });
    assert.deepEqual(prop(root.children[0].style, "flexBasis"), {
      kind: "length",
      value: 40,
      unit: "percent",
    });
    assert.deepEqual(prop(root.children[0].style, "order"), { kind: "number", value: -2 });
    assert.deepEqual(prop(root.children[0].style, "overflowX"), { kind: "keyword", value: "auto" });
    assert.deepEqual(prop(root.children[0].style, "fontSize"), {
      kind: "length",
      value: width === 390 ? 30 : 48,
      unit: "px",
    });
  }
  writeFileSync(
    join(builtProject, "App.tsx"),
    "export function App(){return <body><table><tbody><tr><td>First</td></tr><tr><td>Last</td></tr></tbody></table></body>}",
  );
  writeFileSync(
    join(builtProject, bundle),
    "td {border-bottom:1px solid #123456} tr:last-child td {border-bottom:0}",
  );
  const rows = readBuilt().components[0].root.children[0].children[0].children;
  for (const [index, expected] of [1, 0].entries()) {
    assert.equal(
      rows[index].children[0].style.find(
        (d: { property: string }) => d.property === "borderBottomWidth",
      ).value.value,
      expected,
    );
  }
  writeFileSync(
    join(builtProject, "App.tsx"),
    'export function App(){return <><section id="catalog"><aside>Notes</aside></section><footer>End</footer></>}',
  );
  writeFileSync(
    join(builtProject, bundle),
    `
  #catalog{position:relative;inset:2px 4px 6px;z-index:-3;background-image:radial-gradient(#123456 2px,transparent 2px);background-size:12px 16px}
  #catalog aside{position:sticky;box-shadow:-1px 2px 3px -4px #aabbcc80;border-radius:2px 3px 4px 5px;border-bottom-left-radius:9px}
  `,
  );
  const grouped = readBuilt().components[0].root;
  assert.equal(grouped.kind, "fragment");
  assert.equal(grouped.children.length, 2);
  const styles = (n: { style: { property: string; value: { value: unknown } }[] }) =>
    Object.fromEntries(n.style.map((d) => [d.property, d.value.value]));
  const section = styles(grouped.children[0]);
  for (const [name, value] of Object.entries({
    position: "relative",
    zIndex: -3,
    top: 2,
    right: 4,
    bottom: 6,
    left: 4,
    dotColor: "#123456",
    dotRadius: 2,
    tileWidth: 12,
    tileHeight: 16,
  }))
    assert.equal(section[name], value, name);
  const aside = styles(grouped.children[0].children[0]);
  for (const [name, value] of Object.entries({
    position: "sticky",
    shadowX: -1,
    shadowY: 2,
    shadowBlur: 3,
    shadowSpread: -4,
    shadowColor: "#aabbcc80",
    borderBottomLeftRadius: 9,
    borderTopRightRadius: 3,
  }))
    assert.equal(aside[name], value, name);
} finally {
  rmSync(builtProject, { recursive: true, force: true });
}
console.log(
  JSON.stringify({
    screens: model.screens.length,
    components: model.components.length,
    edges: model.edges.length,
    tokens: model.tokens.length,
  }),
);
