// The versioned source model (docs/en/source-model.md, #353): the reference model of
// fixtures/source-model validates against schemas/source-model.v1.schema.json, bad shapes
// are rejected, and the Rust reader's own rules (references, order, style values) pass
// their tests. Needs no browser.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { Ajv2020 } from "ajv/dist/2020.js";

const root = resolve(import.meta.dirname, "..");
const readJson = (path: string): unknown => JSON.parse(readFileSync(join(root, path), "utf8"));

// `strictRequired` is off because the schema lists `kind` as required next to `oneOf`
// branches that define it; every other strict check stays on.
const ajv = new Ajv2020({ allErrors: true, strict: true, strictRequired: false });
const validate = ajv.compile(readJson("schemas/source-model.v1.schema.json") as object);

type Json = Record<string, any>;
const reference = (): Json => readJson("fixtures/source-model/model.json") as Json;

const ok = validate(reference());
assert.ok(ok, `reference model: ${ajv.errorsText(validate.errors, { separator: "\n" })}`);

// Shapes the schema must reject. The Rust reader rejects the same ones (reader_rejects_bad_shapes).
const rejected: [string, (model: Json) => void][] = [
  ["schema version", (m) => (m.schemaVersion = 2)],
  ["unknown field", (m) => (m.generatedAt = 0)],
  ["missing files", (m) => delete m.files],
  ["uppercase color", (m) => (m.tokens[0].value.value = "#1D4ED8")],
  ["unknown style property", (m) => (m.components[0].root.style[0].property = "float")],
  ["unknown node kind", (m) => (m.components[0].root.kind = "view")],
  ["absolute path", (m) => (m.files[0].path = "/src/App.tsx")],
  ["parent path", (m) => (m.files[0].path = "../App.tsx")],
  ["bad hash", (m) => (m.files[0].sha256 = "abc")],
  ["zero line", (m) => (m.screens[0].location.line = 0)],
  ["empty enum", (m) => (m.components[0].slots[1].type.values = [])],
  ["route without slash", (m) => (m.screens[1].route = "settings")],
  ["token without location or package", (m) => delete m.tokens[4].package],
];
for (const [name, mutate] of rejected) {
  const model = reference();
  mutate(model);
  assert.equal(validate(model), false, `schema accepted ${name}`);
}

// Rules only the reader checks: the schema accepts these, the Rust tests reject them.
const readerOnly: [string, (model: Json) => void][] = [
  ["unsorted components", (m) => m.components.reverse()],
  ["dangling slot", (m) => (m.components[0].root.children[2].on.slot = "src/Missing.tsx#X.y")],
  ["wrong keyword", (m) => (m.components[0].root.style[0].value.value = "diagonal")],
];
for (const [name, mutate] of readerOnly) {
  const model = reference();
  mutate(model);
  assert.equal(validate(model), true, `schema should leave ${name} to the reader`);
}

// Additive v1 shapes: old readers still see the unresolved fallback while new readers
// can use document typography and typed viewport/clamp refinements.
const evolved = reference();
evolved.documentStyle = {
  root: {
    style: [
      {
        property: "fontSize",
        value: { kind: "length", value: 62.5, unit: "percent" },
        from: {
          syntax: "class",
          text: "font-size: 62.5%;",
          location: evolved.components[0].root.location,
        },
      },
    ],
  },
};
evolved.components[0].root.style = [
  {
    property: "width",
    value: {
      kind: "unresolved",
      text: "width: clamp(60px, 12vw, 150px);",
      reason: "Viewport or clamp length is only available as a typed refinement",
      refinement: {
        kind: "clamp",
        min: { value: 60, unit: "px" },
        preferred: { value: 12, unit: "vw" },
        max: { value: 150, unit: "px" },
      },
    },
    from: {
      syntax: "class",
      text: "width: clamp(60px, 12vw, 150px);",
      location: evolved.components[0].root.location,
    },
  },
];
assert.equal(validate(evolved), true, JSON.stringify(validate.errors));

execFileSync("cargo", ["test", "--locked", "--quiet", "-p", "dui-core", "source_model"], {
  cwd: root,
  stdio: ["ignore", "inherit", "inherit"],
});

console.log(
  JSON.stringify({
    ok: true,
    schemaRejects: rejected.length,
    readerOnly: readerOnly.length,
  }),
);
