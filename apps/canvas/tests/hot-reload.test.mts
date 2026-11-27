import assert from "node:assert/strict";
import { test } from "node:test";
import { applyModelPatch, applySourceGraphPatch } from "../src/features/model/hotReload.ts";
import type { Model } from "../src/features/model/modelDocument.ts";

test("model patches preserve unchanged entity object identity", () => {
  const orders = { id: "src/Orders.tsx#Orders", root: { type: "element" } };
  const settings = { id: "src/Settings.tsx#Settings", root: { type: "element" } };
  const model = {
    schemaVersion: 1,
    files: [{ path: "src/Orders.tsx" }, { path: "src/Settings.tsx" }],
    screens: [],
    components: [orders, settings],
  } as unknown as Model;
  const nextOrders = { ...orders, root: { type: "element", text: "changed" } };
  const patched = applyModelPatch(model, {
    components: { upsert: [nextOrders], remove: [] },
    files: { upsert: [], remove: ["src/Settings.tsx"] },
    affectedScreens: ["route:/"],
  });
  assert.equal((patched.components as unknown[])[0], nextOrders);
  assert.equal((patched.components as unknown[])[1], settings);
  assert.deepEqual(patched.files, [{ path: "src/Orders.tsx" }]);
});

test("graph patches clone only invalidated screens and preserve capture cells", () => {
  const orders = {
    id: "route:/",
    path: "/",
    cells: [{ path: "/", width: 390, status: "unverified" as const }],
  };
  const settings = { id: "route:/settings", path: "/settings", cells: [] };
  const archive = {
    screens: [orders, settings],
    links: [
      {
        source: "route:/settings",
        target: "route:/",
        provenance: "extracted" as const,
        replayVerified: false,
        modelEdge: "edge-1",
      },
    ],
  };
  const patched = applySourceGraphPatch(
    archive,
    {
      links: {
        upsert: [
          {
            key: "edge-1",
            source: "route:/",
            target: "route:/settings",
            modelEdge: "edge-1",
          },
        ],
        remove: [],
      },
    },
    ["route:/"],
  );
  assert.notEqual(
    patched.screens.find((s) => s.id === "route:/"),
    orders,
  );
  assert.equal(
    patched.screens.find((s) => s.id === "route:/settings"),
    settings,
  );
  assert.equal(patched.screens.find((s) => s.id === "route:/")?.cells, orders.cells);
  assert.equal(patched.links[0].source, "route:/");
  assert.equal(patched.links[0].modelEdge, "edge-1");
});
