import assert from "node:assert/strict";
import test from "node:test";
import { decodeAuditReport, openCommentsByPath } from "../src/model/audit.ts";

test("decodeAuditReport parses 99% coverage summary, gaps, and per-screen open comments", () => {
  const report = decodeAuditReport({
    goalPercent: 99,
    meetsGoal: false,
    summary: {
      totalCells: 4,
      verifiedCells: 3,
      renderableCells: 4,
      unverifiedCells: 1,
      unrenderableCells: 0,
      verifiedPercent: 75,
      renderablePercent: 100,
      branchOutcomes: 10,
      observedBranchOutcomes: 8,
      missingBranchOutcomes: 2,
      branchPercent: 80,
      openComments: 2,
      resolvedComments: 1,
    },
    gaps: [{ path: "/order", width: 375, status: "unverified", reason: "missing-assertion" }],
    screens: [
      {
        path: "/",
        totalCells: 2,
        verifiedCells: 2,
        renderableCells: 2,
        unverifiedCells: 0,
        unrenderableCells: 0,
        verifiedPercent: 100,
        openComments: 0,
      },
      {
        path: "/order",
        totalCells: 2,
        verifiedCells: 1,
        renderableCells: 2,
        unverifiedCells: 1,
        unrenderableCells: 0,
        verifiedPercent: 50,
        openComments: 2,
      },
    ],
  });
  assert(report);
  assert.equal(report.meetsGoal, false);
  assert.equal(report.summary.verifiedPercent, 75);
  assert.equal(report.gaps.length, 1);
  assert.deepEqual(openCommentsByPath(report), { "/order": 2 });
});
