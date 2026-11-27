import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  symlinkSync,
  writeFileSync,
  readFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { Ajv2020 } from "ajv/dist/2020.js";

const root = resolve(import.meta.dirname, "..");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const fixture = join(root, "fixtures/ui-sdk-primitives");
const stage = mkdtempSync(join(tmpdir(), "dui-ui-sdk-primitives-"));
const schema = JSON.parse(readFileSync(join(root, "rules/schema.json"), "utf8")) as object;
const pack = JSON.parse(readFileSync(join(root, "rules/ui-sdk.json"), "utf8"));
const ajv = new Ajv2020({ allErrors: true, strict: true });
const validatePack = ajv.compile(schema);
assert.ok(
  validatePack(pack),
  `ui-sdk rule pack schema: ${ajv.errorsText(validatePack.errors, { separator: "\n" })}`,
);

type ScanFact = { file: string; start: number; end: number; kind: string } & Record<string, any>;

function key(fact: ScanFact) {
  return `${fact.file}#${fact.start}:${fact.end}:${fact.kind}`;
}

try {
  const app = join(stage, "app");
  cpSync(fixture, app, {
    recursive: true,
    filter: (path) => !["node_modules", "dist"].includes(basename(path)),
  });
  symlinkSync(join(fixture, "node_modules"), join(app, "node_modules"), "dir");
  writeFileSync(
    join(app, "unresolved.tsx"),
    `import { definePrimitive } from "@ui-sdk/core";
type ProbeOptions = { value?: string };
export const unresolvedProbe = definePrimitive({
  name: "unresolved-probe",
  options: { value: "string" },
  state: (options: ProbeOptions) => ({ value: options.value ?? "" }),
  api: () => ({}),
  slots: (slot) => ({
    result: slot({ present: (state) => state.value.includes("inventory") }),
  }),
});
`,
  );
  const out = join(stage, "out");
  mkdirSync(out, { recursive: true });
  const raw = execFileSync(binary, ["run", app, join(app, "dui.json"), out], {
    encoding: "utf8",
    env: { ...process.env, HOME: join(stage, "home") },
  });
  const report = JSON.parse(raw) as {
    cells: {
      width: number;
      status: string;
      image?: string;
      input: Record<string, unknown>;
      checks?: { selector?: string; present?: boolean }[];
      observation?: { assertions?: boolean[] };
    }[];
    unobservedSlots: unknown[];
    primitiveLimitations?: { primitive: string; limitations: { reason?: string }[] }[];
  };
  const facts = JSON.parse(readFileSync(join(out, "facts.json"), "utf8")) as {
    compiler: string;
    facts: ScanFact[];
  };
  assert.equal(facts.compiler, "7.0.2");
  const primitive = facts.facts.find(
    (fact) => fact.kind === "primitive" && fact.primitive === "lookup-control",
  );
  assert.ok(primitive, "lookup-control primitive fact");
  assert.deepEqual(primitive.state.controls, { open: "open", value: "value" });
  const stateFields = Object.fromEntries(
    primitive.state.shape.fields.map((field: any) => [field.name, field.shape.kind]),
  );
  assert.equal(stateFields.value, "string");
  assert.equal(stateFields.open, "union");
  const clear = primitive.slots.find((slot: any) => slot.name === "clear");
  const popup = primitive.slots.find((slot: any) => slot.name === "popup");
  assert.deepEqual(clear.present, {
    status: "resolved",
    predicate: { operator: "neq", key: "value", value: "" },
  });
  assert.deepEqual(popup.present, {
    status: "resolved",
    predicate: { operator: "truthy", key: "open" },
  });

  const slots = facts.facts.filter((fact) => fact.kind === "slot" && fact.primitive);
  assert.equal(slots.length, 2);
  const valueSlot = slots.find((slot) => slot.primitive.state === "value");
  const openSlot = slots.find((slot) => slot.primitive.state === "open");
  assert.ok(valueSlot && openSlot, "controlled React state slots are compiler-linked");
  const valueId = key(valueSlot);
  const openId = key(openSlot);
  assert.deepEqual(report.unobservedSlots, []);

  for (const width of [390, 1440]) {
    const cells = report.cells.filter((cell) => cell.width === width);
    assert.ok(cells.length >= 8, `planned cells at ${width}px`);
    for (const [value, open] of [
      ["", false],
      ["", true],
      ["sample", false],
      ["sample", true],
    ] as const) {
      assert.ok(
        cells.some((cell) => cell.input[valueId] === value && cell.input[openId] === open),
        `missing value=${JSON.stringify(value)} open=${open} at ${width}px`,
      );
    }
    for (const cell of cells) {
      assert.equal(cell.status, "verified", `cell at ${width}px verified`);
      assert.equal(typeof cell.image, "string", `capture image at ${width}px`);
      assert.ok(
        cell.observation?.assertions?.every(Boolean),
        `all presence assertions pass at ${width}px`,
      );
    }
    const closedEmpty = cells.find(
      (cell) => cell.input[valueId] === "" && cell.input[openId] === false,
    );
    const openFilled = cells.find(
      (cell) => cell.input[valueId] === "sample" && cell.input[openId] === true,
    );
    assert.ok(
      closedEmpty?.checks?.some(
        (check) =>
          check.selector === '[data-slot="lookup-control-clear"]' && check.present === false,
      ),
    );
    assert.ok(
      closedEmpty?.checks?.some(
        (check) =>
          check.selector === '[data-slot="lookup-control-popup"]' && check.present === false,
      ),
    );
    assert.ok(
      openFilled?.checks?.some(
        (check) =>
          check.selector === '[data-slot="lookup-control-clear"]' && check.present === true,
      ),
    );
    assert.ok(
      openFilled?.checks?.some(
        (check) =>
          check.selector === '[data-slot="lookup-control-popup"]' && check.present === true,
      ),
    );
  }

  const limitations = report.primitiveLimitations ?? [];
  assert.ok(
    limitations.some(
      (entry) =>
        entry.primitive === "unresolved-probe" &&
        entry.limitations.some((limit) =>
          limit.reason?.includes("not a state key, its negation, or a strict literal equality"),
        ),
    ),
    "unsupported predicate form is reported as a limitation",
  );
  console.log(
    JSON.stringify({
      ok: true,
      cells: report.cells.length,
      widths: [390, 1440],
      primitive: primitive.primitive,
      limitations: limitations.length,
    }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
