import assert from "node:assert/strict";
import test from "node:test";
import { decodeAuditReport, openCommentsByPath } from "../src/model/audit.ts";

test("decodeAuditReport parses 99% coverage summary, gaps, and per-screen open comments", () => {
  const report = decodeAuditReport({
    goalPercent: 99,
    meetsGoal: false,
    summary: {
      totalCells: 4,
      plannedCells: 4,
      capturedCells: 3,
      capturedRenderableCells: 3,
      notCapturedCells: 1,
      verifiedCells: 3,
      renderableCells: 4,
      unverifiedCells: 1,
      unrenderableCells: 0,
      verifiedPercent: 100,
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
        capturedCells: 2,
        capturedRenderableCells: 2,
        notCapturedCells: 0,
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
        capturedCells: 1,
        capturedRenderableCells: 1,
        notCapturedCells: 1,
        verifiedCells: 1,
        renderableCells: 2,
        unverifiedCells: 1,
        unrenderableCells: 0,
        verifiedPercent: 100,
        openComments: 2,
      },
    ],
  });
  assert(report);
  assert.equal(report.meetsGoal, false);
  assert.equal(report.summary.verifiedPercent, 100);
  assert.equal(report.summary.plannedCells, 4);
  assert.equal(report.summary.capturedCells, 3);
  assert.equal(report.summary.capturedRenderableCells, 3);
  assert.equal(report.summary.notCapturedCells, 1);
  assert.equal(report.gaps.length, 1);
  assert.deepEqual(openCommentsByPath(report), { "/order": 2 });
});
