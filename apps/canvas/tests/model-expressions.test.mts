import assert from "node:assert/strict";
import { test } from "node:test";
import { resolveModelExpression, type Expr } from "../src/features/model/modelDocument.ts";

const rowsSlot = "src/App.tsx#Rows.rows";
const idSlot = "src/App.tsx#Row.id";
const lookup: Expr = {
  kind: "index",
  object: { kind: "slot", slot: rowsSlot },
  key: { kind: "slot", slot: idSlot },
};
const rows = {
  one: { name: "First benchmark issue" },
  two: { name: "Second benchmark issue" },
};

test("keyed lookup resolves the exact nonempty id and a swapped id", () => {
  for (const [id, name] of [
    ["one", "First benchmark issue"],
    ["two", "Second benchmark issue"],
  ] as const) {
    const resolved = resolveModelExpression(lookup, {
      slots: { [rowsSlot]: rows, [idSlot]: id },
    });
    assert.equal(resolved.known, true);
    assert.equal((resolved.value as { name: string }).name, name);
  }
});

test("a removed key stays unresolved instead of falling back to another entry", () => {
  const resolved = resolveModelExpression(lookup, {
    slots: { [rowsSlot]: rows, [idSlot]: "removed" },
  });
  assert.equal(resolved.known, false);
  assert.equal(resolved.value, undefined);
});

test("numeric keys use object key semantics and missing numeric keys stay unresolved", () => {
  const byNumber = { 1: "one" };
  const numericLookup: Expr = {
    kind: "index",
    object: { kind: "slot", slot: rowsSlot },
    key: { kind: "literal", value: 1 },
  };
  assert.equal(
    resolveModelExpression(numericLookup, { slots: { [rowsSlot]: byNumber } }).value,
    "one",
  );
  const missingNumericLookup: Expr = {
    kind: "index",
    object: { kind: "slot", slot: rowsSlot },
    key: { kind: "literal", value: 2 },
  };
  assert.equal(
    resolveModelExpression(missingNumericLookup, { slots: { [rowsSlot]: byNumber } }).known,
    false,
  );
});
