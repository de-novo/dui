// Version 1 of the source model changes only by addition (docs/en/source-model.md §9).
// Compares schemas/source-model.v1.schema.json with the same file on a base revision
// (SOURCE_MODEL_BASE, default origin/main) and fails when a property, $defs entry, enum
// value or const disappears or changes, or when a field becomes required. Needs no browser.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";

type Json = Record<string, unknown>;
const schemaPath = "schemas/source-model.v1.schema.json";
const isObject = (value: unknown): value is Json =>
  typeof value === "object" && value !== null && !Array.isArray(value);

// A oneOf/anyOf branch is matched by its `kind` const when it has one, so adding a
// branch anywhere does not shift the others.
const branchKey = (branch: unknown, index: number): string => {
  const kind = isObject(branch) && isObject(branch.properties) ? branch.properties.kind : undefined;
  return isObject(kind) && "const" in kind ? `kind=${JSON.stringify(kind.const)}` : `#${index}`;
};

export const breakingChanges = (base: unknown, head: unknown, path = "$"): string[] => {
  if (!isObject(base)) return [];
  if (!isObject(head)) return [`${path}: removed`];
  const problems: string[] = [];
  for (const key of ["properties", "$defs"]) {
    const from = base[key];
    if (!isObject(from)) continue;
    const to = isObject(head[key]) ? (head[key] as Json) : {};
    for (const [name, child] of Object.entries(from)) {
      if (!(name in to)) problems.push(`${path}.${key}.${name}: removed or renamed`);
      else problems.push(...breakingChanges(child, to[name], `${path}.${key}.${name}`));
    }
  }
  if (Array.isArray(base.enum)) {
    const to = new Set((Array.isArray(head.enum) ? head.enum : []).map((v) => JSON.stringify(v)));
    for (const value of base.enum) {
      if (!to.has(JSON.stringify(value)))
        problems.push(`${path}.enum: ${JSON.stringify(value)} removed or renamed`);
    }
  }
  if ("const" in base && JSON.stringify(base.const) !== JSON.stringify(head.const)) {
    problems.push(`${path}.const: changed from ${JSON.stringify(base.const)}`);
  }
  const required = new Set(Array.isArray(base.required) ? base.required : []);
  for (const name of Array.isArray(head.required) ? head.required : []) {
    if (!required.has(name) && isObject(base.properties) && name in base.properties) {
      problems.push(`${path}.required: existing field ${JSON.stringify(name)} became required`);
    }
  }
  for (const key of ["items", "additionalProperties", "not", "if", "then", "else"]) {
    if (isObject(base[key]))
      problems.push(...breakingChanges(base[key], head[key], `${path}.${key}`));
  }
  for (const key of ["oneOf", "anyOf", "allOf", "prefixItems"]) {
    const from = base[key];
    if (!Array.isArray(from)) continue;
    const to = new Map(
      (Array.isArray(head[key]) ? (head[key] as unknown[]) : []).map((b, i) => [
        branchKey(b, i),
        b,
      ]),
    );
    from.forEach((branch, i) => {
      const id = branchKey(branch, i);
      if (!to.has(id)) problems.push(`${path}.${key}[${id}]: removed or renamed`);
      else problems.push(...breakingChanges(branch, to.get(id), `${path}.${key}[${id}]`));
    });
  }
  return problems;
};

// The check must catch what it claims to catch.
const sample = {
  properties: { kind: { enum: ["a", "b"] }, size: { type: "number" } },
  required: ["kind"],
  oneOf: [{ properties: { kind: { const: "box" } } }],
};
assert.deepEqual(breakingChanges(sample, structuredClone(sample)), []);
const grown = structuredClone(sample) as any;
grown.properties.kind.enum.push("c");
grown.properties.extra = {};
grown.oneOf.unshift({ properties: { kind: { const: "text" } } });
assert.deepEqual(breakingChanges(sample, grown), []);
const cases: [string, (s: any) => void][] = [
  ["removed property", (s) => delete s.properties.size],
  ["removed enum value", (s) => (s.properties.kind.enum = ["a"])],
  ["newly required", (s) => s.required.push("size")],
  ["removed branch", (s) => (s.oneOf = [])],
];
for (const [name, change] of cases) {
  const head = structuredClone(sample);
  change(head);
  assert.ok(breakingChanges(sample, head).length > 0, `not detected: ${name}`);
}

const root = resolve(import.meta.dirname, "..");
const ref = process.env.SOURCE_MODEL_BASE ?? "origin/main";
let baseText: string;
try {
  baseText = execFileSync("git", ["show", `${ref}:${schemaPath}`], {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  });
} catch {
  console.log(JSON.stringify({ ok: true, skipped: `no ${schemaPath} at ${ref}` }));
  process.exit(0);
}
const problems = breakingChanges(
  JSON.parse(baseText),
  JSON.parse(readFileSync(join(root, schemaPath), "utf8")),
);
assert.deepEqual(
  problems,
  [],
  `source model v1 must change only by addition; use schemaVersion 2 for:\n${problems.join("\n")}`,
);
console.log(JSON.stringify({ ok: true, base: ref }));
