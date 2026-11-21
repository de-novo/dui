import assert from "node:assert/strict";
import test from "node:test";
import { decodeCommentList, formatCommentTarget } from "../src/model/comments.ts";

test("decodeCommentList parses comments and counts open vs resolved", () => {
  const decoded = decodeCommentList({
    summary: { total: 2, open: 1, resolved: 1 },
    comments: [
      {
        id: "cmt_1",
        projectId: "prj_1",
        runId: "run_1",
        author: "reviewer",
        body: "Tighten mobile CTA padding",
        status: "open",
        createdAtMs: 100,
        updatedAtMs: 100,
        location: { kind: "cell", path: "/order", width: 375 },
      },
      {
        id: "cmt_2",
        projectId: "prj_1",
        runId: "run_1",
        author: "reviewer",
        body: "Confirm checkout arrow",
        status: "resolved",
        createdAtMs: 200,
        updatedAtMs: 250,
        location: { kind: "transition", path: "/order", to: "/confirm" },
      },
    ],
  });
  assert.equal(decoded.summary.total, 2);
  assert.equal(decoded.summary.open, 1);
  assert.equal(decoded.summary.resolved, 1);
  assert.equal(formatCommentTarget(decoded.comments[0].location), "375 px");
  assert.equal(formatCommentTarget(decoded.comments[1].location), "/order → /confirm");
});
