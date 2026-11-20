import assert from "node:assert/strict";
import { test } from "node:test";
import { layoutMap } from "../src/features/map/layout.ts";
import type { ScreenMap } from "../src/data/overview.ts";

const archive: ScreenMap = {
  screens: ["a", "b", "c", "d", "e"].map((id) => ({
    id,
    path: "/" + id,
    cells: [],
  })),
  links: [
    { source: "a", target: "b" },
    { source: "b", target: "a" },
    { source: "b", target: "c" },
    { source: "c", target: "a", observedWidths: [375] },
    { source: "d", target: "e" },
  ],
};

test("overview spans each connected group using recorded directions and stable positions", () => {
  const overview = layoutMap(archive, 375, "overview");
  assert.equal(overview.nodes.length, 5);
  assert.equal(overview.links.length, 3);
  assert.equal(overview.total, 5);
  assert.ok(overview.links.includes(archive.links[4]));
  assert.equal(
    new Set(overview.nodes.map((n) => JSON.stringify(n.position))).size,
    5,
  );
  for (const edge of overview.links) assert.ok(archive.links.includes(edge));
  assert.deepEqual(
    overview,
    layoutMap(
      {
        screens: [...archive.screens].reverse(),
        links: [...archive.links].reverse(),
      },
      375,
      "overview",
    ),
  );
  assert.deepEqual(layoutMap(archive, 375, "all").links, archive.links);
});

test("focus preserves reverse links and filters evidence by capture width", () => {
  const focused = layoutMap(archive, 1280, "focus:a");
  assert.deepEqual(focused.links, archive.links.slice(0, 2));
  assert.deepEqual(
    focused.nodes.map((n) => n.screen.id),
    ["a", "b"],
  );
  assert.equal(layoutMap(archive, 375, "focus:a").links.length, 3);
  const isolated = layoutMap({ ...archive, links: [] }, 375, "focus:c");
  assert.deepEqual(
    isolated.nodes.map((n) => n.screen.id),
    ["c"],
  );
  assert.equal(isolated.links.length, 0);
});
