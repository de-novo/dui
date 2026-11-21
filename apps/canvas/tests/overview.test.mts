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

test("screen discovery origins and link evidence remain non-replay-verified", () => {
  const decoded = decodeOverview({
    nodes: [
      {
        kind: "screen",
        id: "route:/?q=demo",
        label: "/?q=demo",
        discoveredFrom: { kind: "entry" },
      },
      {
        kind: "screen",
        id: "route:/",
        label: "/",
        discoveredFrom: {
          kind: "anchor",
          from: "/?q=demo",
          selector: "header a[href]",
        },
      },
    ],
    edges: [
      {
        kind: "linksTo",
        source: "route:/?q=demo",
        target: "route:/",
        observedWidths: [375, 1280],
        origin: "anchor",
        selector: "header a[href]",
      },
    ],
    overview: [
      { path: "/?q=demo", width: 1280, status: "verified", image: "graph.png" },
      { path: "/", width: 1280, status: "unverified", reason: "not-rendered" },
    ],
  });
  assert.deepEqual(decoded.screens[0].discoveredFrom, { kind: "entry" });
  assert.deepEqual(decoded.screens[1].discoveredFrom, {
    kind: "anchor",
    from: "/?q=demo",
    selector: "header a[href]",
  });
  assert.equal(decoded.screens[1].cells[0].image, undefined);
  assert.equal(decoded.screens[1].cells[0].reason, "not-rendered");
  assert.deepEqual(decoded.links[0], {
    source: "route:/?q=demo",
    target: "route:/",
    observedWidths: [375, 1280],
    origin: "anchor",
    selector: "header a[href]",
    replayVerified: false,
  });
});
