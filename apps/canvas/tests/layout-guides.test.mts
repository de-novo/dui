import assert from "node:assert/strict";
import test from "node:test";
import { decodeOverview, type Cell, type RenderNode } from "../src/data/overview.ts";
import { inspectAlignmentSeries } from "../src/model/layoutGuides.ts";

function button(text: string, x: number, y: number): RenderNode {
  return {
    id: text,
    tag: "button",
    text,
    parentId: "toolbar",
    style: {},
    rect: { x, y, width: 32, height: 32 },
  };
}

function capture(width: number, ...nodes: RenderNode[]): Cell {
  return { path: "/records", width, status: "verified", renderTree: nodes };
}

const save = button("Save", 10, 20);
const close = button("Close", 70, 20);
const small = capture(401, save, close);
const large = capture(911, save, close);

test("measures an isolated shift against the two adjacent captures in width order", () => {
  const moved = { ...close, rect: { ...close.rect, y: 41, height: 33 } };
  const medium = capture(678, save, moved);
  const result = inspectAlignmentSeries([large, small, medium]);
  assert.equal(result.size, 1);
  assert.deepEqual(result.get(medium), {
    widths: [401, 911],
    compared: 1,
    changes: [{ first: save, second: moved, edge: "middle", offset: 21.5 }],
  });
});

test("recognizes column alignment using captured DOM parents and semantic identities", () => {
  const series = [402, 805, 990].map((w, i) =>
    capture(
      w,
      ...[0, 1].map((j) => ({
        ...button("", j * (i === 1 ? 10 : 0), j * 50),
        id: "node-" + (i + j),
        parentId: undefined,
        attributes: { "aria-label": j ? "Next" : "Previous", "data-dui-dom-parent": "section" + w },
      })),
    ),
  );
  const result = inspectAlignmentSeries(series).get(series[1]);
  assert.equal(result?.compared, 1);
  assert.equal(result?.changes[0].edge, "center");
  assert.equal(result?.changes[0].offset, 10);
});

test("preserved edges, rounding and a persistent breakpoint do not produce changes", () => {
  const taller = capture(650, save, { ...close, rect: { ...close.rect, height: 60 } });
  const rounded = capture(650, save, button("Close", 70, 21));
  const wrapped = capture(650, save, button("Close", 10, 80));
  for (const middle of [taller, rounded]) {
    assert.deepEqual(inspectAlignmentSeries([small, middle, large]).get(middle)?.changes, []);
  }
  assert.deepEqual(
    inspectAlignmentSeries([small, wrapped, { ...wrapped, width: 900 }]).get(wrapped)?.changes,
    [],
  );
});

test("unmatched, non-peer and ambiguous controls provide no alignment baseline", () => {
  for (const smallNodes of [
    [save, close, close],
    [save, { ...close, parentId: "other" }],
    [save, { ...close, text: "Dismiss" }],
    [save, { ...close, attributes: { "data-testid": "action" } }],
  ]) {
    const medium = capture(678, save, {
      ...close,
      rect: { ...close.rect, y: 55 },
      ...(smallNodes[1].attributes ? { attributes: { id: "action" } } : {}),
    });
    assert.equal(
      inspectAlignmentSeries([capture(400, ...smallNodes), medium, large]).get(medium)?.compared,
      0,
    );
  }
});

test("requires usable neighbors from the same screen, scenario, state and locale", () => {
  const medium = capture(678, save, close);
  for (const unsuitable of [
    { ...large, path: "/other" },
    { ...large, locale: "fr" },
    { ...large, state: "open" },
    { ...large, scenario: "auth" },
    { ...large, status: "unrenderable" as const },
    { ...large, renderTree: undefined },
  ])
    assert.equal(inspectAlignmentSeries([small, medium, unsuitable]).size, 0);
  assert.equal(inspectAlignmentSeries([small, medium, medium, large]).size, 0);
  const missing = { ...medium, width: 700, renderTree: undefined };
  assert.equal(inspectAlignmentSeries([small, medium, missing, large]).size, 0);
});

test("keeps a redirect to a different screen out of the alignment comparison", () => {
  const medium = capture(678, save, close);
  const destination = { ...large, redirectedTo: "/login" };
  assert.equal(inspectAlignmentSeries([small, medium, destination]).get(medium)?.compared, 0);
});

test("archive decoding ignores missing coordinates and retains recorded zero-size nodes", () => {
  const sample = capture(
    678,
    save,
    ...[
      { ...close.rect, x: undefined },
      { ...close.rect, y: NaN },
      { ...close.rect, width: -1 },
      { ...close.rect, height: 0 },
    ].map((rect) => ({ ...close, rect }) as RenderNode),
  );
  const result = decodeOverview({
    nodes: [{ kind: "screen", id: "screen", label: sample.path }],
    edges: [],
    overview: [sample],
  });
  assert.deepEqual(
    result.screens[0].cells[0].renderTree?.map((node) => node.id),
    [save.id, close.id],
  );
});
