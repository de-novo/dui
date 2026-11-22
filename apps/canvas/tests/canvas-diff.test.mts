import assert from "node:assert/strict";
import test from "node:test";
import type { ScreenMap } from "../src/data/overview.ts";
import {
  filterArchiveForDiff,
  indexScreenDiffs,
  resolveCardDiffImageUrl,
} from "../src/model/canvasDiff.ts";
import type { CaptureDiff } from "../src/model/diff.ts";

const sampleArchive: ScreenMap = {
  screens: [
    {
      id: "route:/",
      path: "/",
      origin: "entry",
      cells: [
        {
          path: "/",
          scenario: "default",
          state: "s0",
          width: 1280,
          status: "verified",
          image: "home-after.png",
        },
      ],
    },
    {
      id: "route:/about",
      path: "/about",
      origin: "entry",
      cells: [
        {
          path: "/about",
          scenario: "default",
          state: "s0",
          width: 1280,
          status: "verified",
          image: "about.png",
        },
      ],
    },
  ],
  links: [{ source: "route:/", target: "route:/about", origin: "anchor", widths: [1280] }],
  widths: [1280],
};

const sampleDiff: CaptureDiff = {
  beforeRunKey: "proj/run-main",
  afterRunKey: "proj/run-feat",
  summary: {
    totalChanged: 2,
    added: 0,
    removed: 1,
    changed: 1,
    unchanged: 1,
    screensAffected: 2,
  },
  screens: [
    {
      path: "/",
      cells: [
        {
          kind: "changed",
          path: "/",
          scenario: "default",
          state: "s0",
          width: 1280,
          before: { status: "verified", image: "home-before.png" },
          after: { status: "verified", image: "home-after.png" },
          changedFiles: ["src/Home.tsx"],
          pixelDiff: {
            diffPixels: 420,
            totalPixels: 10000,
            diffRatio: 0.042,
            width: 1280,
            height: 800,
            overlayImage: "abc-diff.png",
          },
        },
      ],
    },
    {
      path: "/legacy",
      cells: [
        {
          kind: "removed",
          path: "/legacy",
          scenario: "default",
          state: "s0",
          width: 1280,
          before: { status: "verified", image: "legacy-before.png" },
          changedFiles: ["src/Legacy.tsx"],
        },
      ],
    },
  ],
};

test("indexScreenDiffs and filterArchiveForDiff map changed/removed screens onto the 2D canvas", () => {
  const indexed = indexScreenDiffs(sampleDiff, 1280);
  assert.equal(indexed["/"]?.kind, "changed");
  assert.equal(indexed["/"]?.maxDiffRatio, 0.042);
  assert.equal(indexed["/"]?.overlayImage, "abc-diff.png");
  assert.equal(indexed["/legacy"]?.kind, "removed");

  // With changedOnly=false, all existing screens plus synthetic removed screens appear.
  const allWithRemoved = filterArchiveForDiff(sampleArchive, indexed, true, false);
  assert.deepEqual(
    allWithRemoved.screens.map((s) => s.path),
    ["/", "/about", "/legacy"],
  );

  // With changedOnly=true, unchanged /about is hidden so only changed/removed screens remain.
  const changedOnly = filterArchiveForDiff(sampleArchive, indexed, true, true);
  assert.deepEqual(
    changedOnly.screens.map((s) => s.path),
    ["/", "/legacy"],
  );
  assert.equal(changedOnly.links.length, 0);

  // Visual mode resolves after, before, and pixel-diff overlay URLs cleanly.
  assert.equal(
    resolveCardDiffImageUrl("overlay", indexed["/"], "proj/run-main", "proj/run-feat"),
    "/api/diff-overlay?file=abc-diff.png",
  );
  assert.equal(
    resolveCardDiffImageUrl("before", indexed["/"], "proj/run-main", "proj/run-feat"),
    "/archive/home-before.png?run=proj%2Frun-main",
  );
  assert.equal(
    resolveCardDiffImageUrl("after", indexed["/"], "proj/run-main", "proj/run-feat"),
    "/archive/home-after.png?run=proj%2Frun-feat",
  );
});
