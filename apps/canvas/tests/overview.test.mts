import assert from "node:assert/strict";
import { test } from "node:test";
import { decodeOverview } from "../src/data/overview.ts";

const entry = { path: "/", width: 375, status: "verified", image: "entry.png" };
const graph = {
  nodes: [{ kind: "screen", id: "home", label: "/" }],
  edges: [],
  overview: [entry],
};

test("screen counts include every reported cell once and retain the entry thumbnail", () => {
  const {
    screens: [screen],
  } = decodeOverview({
    ...graph,
    cells: [
      entry,
      {
        ...entry,
        state: "empty",
        scenario: "Empty",
        status: "unverified",
        reason: "assert-failed",
      },
    ],
  });
  assert.equal(screen.cells.length, 2);
  assert.equal(screen.entries?.length, 1);
  assert.equal(screen.entries?.[0].image, "entry.png");
  assert.equal(screen.cells[1].scenario, "Empty");
});

test("legacy maps without a full report retain their captured cells", () => {
  const {
    screens: [screen],
  } = decodeOverview(graph);
  assert.equal(screen.cells.length, 1);
  assert.equal(screen.cells[0].status, "verified");
  assert.throws(() => decodeOverview({ ...graph, cells: null }));
});
