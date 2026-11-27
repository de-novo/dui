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
assert.ok(row.children[1].unresolvedStyle[0].text.includes("styles.status"));
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
