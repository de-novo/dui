import assert from "node:assert/strict";
import test from "node:test";
import { decodeCaptureDiff } from "../src/model/diff.ts";

test("decodes capture diff and suppresses pixelDiff on unrenderable or unverified cells", () => {
  const decoded = decodeCaptureDiff({
    beforeRunKey: "run-before",
    afterRunKey: "run-after",
    summary: {
      totalChanged: 2,
      added: 0,
      removed: 0,
      changed: 2,
      unchanged: 3,
      screensAffected: 2,
    },
    screens: [
      {
        path: "/catalog",
        cells: [
          {
            kind: "changed",
            path: "/catalog",
            scenario: "default",
            state: "state-a",
            width: 375,
            before: { status: "verified", image: "aaa-375.png" },
            after: { status: "verified", image: "bbb-375.png" },
            changedFiles: ["Catalog.tsx"],
            pixelDiff: {
              diffPixels: 42,
              totalPixels: 1000,
              diffRatio: 0.042,
              width: 375,
              height: 200,
              changedBounds: { x: 10, y: 20, width: 50, height: 12 },
              overlayImage: "0123456789abcdef-diff.png",
            },
          },
        ],
      },
      {
        path: "/checkout",
        cells: [
          {
            kind: "changed",
            path: "/checkout",
            scenario: "default",
            state: "state-b",
            width: 375,
            before: { status: "verified", image: "ccc-375.png" },
            after: { status: "unrenderable", reason: "no-render-env" },
            changedFiles: ["Checkout.tsx"],
            statusChange: {
              from: "verified",
              to: "unrenderable",
              toReason: "no-render-env",
            },
            pixelDiff: {
              diffPixels: 1,
              totalPixels: 1,
              diffRatio: 1,
              width: 1,
              height: 1,
            },
          },
        ],
      },
    ],
  });

  assert.equal(decoded.summary.totalChanged, 2);
  assert.equal(decoded.summary.unchanged, 3);
  assert.equal(decoded.screens[0]?.cells[0]?.pixelDiff?.diffPixels, 42);
  assert.deepEqual(decoded.screens[0]?.cells[0]?.changedFiles, ["Catalog.tsx"]);
  assert.equal(decoded.screens[1]?.cells[0]?.pixelDiff, undefined);
  assert.equal(decoded.screens[1]?.cells[0]?.statusChange?.to, "unrenderable");
});
