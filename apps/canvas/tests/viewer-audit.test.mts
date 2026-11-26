import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const plugin = new URL("../audit/viewer-check.mjs", import.meta.url).pathname;
const designSpec = {
  content: JSON.parse(readFileSync(new URL("../audit/rules.json", import.meta.url), "utf8")),
};

function audit(input: Record<string, unknown>) {
  const out = execFileSync(process.execPath, [plugin], {
    input: JSON.stringify({
      version: 1,
      width: 1280,
      designSpec,
      keyboard: null,
      dismissal: null,
      ...input,
    }),
    encoding: "utf8",
  });
  return JSON.parse(out) as { findings: { rule: string; nodeId: string }[]; unverified: string[] };
}

const button = {
  id: "button#0",
  tag: "button",
  text: "Open",
  rect: { x: 10, y: 20, width: 80, height: 32 },
};
const link = { id: "a#0", tag: "a", text: "Docs", rect: { x: 10, y: 80, width: 60, height: 24 } };
const record = (node: typeof button, extra = {}) => ({
  tag: node.tag,
  label: node.text,
  rect: node.rect,
  ...extra,
});

test("a Tab stop without a focus indicator and an unreached control are findings", () => {
  const result = audit({
    renderTree: [button, link],
    keyboard: {
      complete: true,
      targets: [record(button), record(link)],
      stops: [record(button, { focusVisible: false })],
    },
  });
  assert.deepEqual(result.findings.map((f) => `${f.rule} ${f.nodeId}`).sort(), [
    "focus-visible button#0",
    "keyboard-reachable a#0",
  ]);
});

test("an incomplete Tab walk is unverified, not a pass", () => {
  const result = audit({
    renderTree: [button],
    keyboard: { complete: false, targets: [record(button)], stops: [] },
  });
  assert.equal(result.findings.length, 0);
  assert.match(result.unverified[0]!, /did not return to its start/);
});

test("an overlay that ignores Escape or drops focus is a finding", () => {
  const dialog = {
    id: "dialog#0",
    tag: "dialog",
    text: "",
    rect: { x: 0, y: 0, width: 400, height: 300 },
  };
  const stays = audit({
    renderTree: [dialog],
    dismissal: { closedByEscape: false, focusReturned: null },
  });
  assert.deepEqual(
    stays.findings.map((f) => f.rule),
    ["escape-dismiss"],
  );
  const drops = audit({
    renderTree: [dialog],
    dismissal: { closedByEscape: true, focusReturned: false },
  });
  assert.deepEqual(
    drops.findings.map((f) => f.rule),
    ["focus-return"],
  );
  const clean = audit({
    renderTree: [dialog],
    dismissal: { closedByEscape: true, focusReturned: true },
  });
  assert.equal(clean.findings.length, 0);
});

test("a screen without interaction evidence is unverified", () => {
  assert.match(audit({ renderTree: [button] }).unverified[0]!, /No interaction evidence/);
});
