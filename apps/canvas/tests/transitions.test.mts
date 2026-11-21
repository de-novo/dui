import assert from "node:assert/strict";
import test from "node:test";
import { decodeOverview } from "../src/data/overview.ts";

test("decodes extracted, inferred, and replay-verified transition edge provenance", () => {
  const map = decodeOverview({
    nodes: [
      { id: "route:/order", kind: "screen", label: "/order" },
      { id: "route:/confirm", kind: "screen", label: "/confirm" },
      { id: "route:/help", kind: "screen", label: "/help" },
    ],
    edges: [
      {
        source: "route:/order",
        target: "route:/confirm",
        kind: "linksTo",
        origin: "extracted",
        provenance: "verified",
        replayVerified: true,
        action: { kind: "click", selector: "[data-testid=cancel]", label: "Cancel order" },
        confirmedWidths: [375, 1280],
        observedWidths: [375, 1280],
      },
      {
        source: "route:/order",
        target: "route:/help",
        kind: "linksTo",
        origin: "inferred",
        provenance: "inferred",
        replayVerified: false,
        failedWidths: { "375": "selector-not-found" },
      },
    ],
    overview: [
      { path: "/order", width: 375, status: "verified", image: "a.png" },
      { path: "/confirm", width: 375, status: "verified", image: "b.png" },
      { path: "/help", width: 375, status: "verified", image: "c.png" },
    ],
  });

  const verified = map.links.find((l) => l.target === "route:/confirm")!;
  const inferred = map.links.find((l) => l.target === "route:/help")!;
  assert.equal(verified.provenance, "verified");
  assert.equal(verified.replayVerified, true);
  assert.equal(verified.action?.label, "Cancel order");
  assert.deepEqual(verified.confirmedWidths, [375, 1280]);

  assert.equal(inferred.provenance, "inferred");
  assert.equal(inferred.replayVerified, false);
  assert.equal(inferred.failedWidths?.["375"], "selector-not-found");
});
