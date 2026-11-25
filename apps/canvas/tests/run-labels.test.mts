import assert from "node:assert/strict";
import test from "node:test";
import type { Run } from "../src/data/run.ts";
import {
  availableDiffPairs,
  comparableRunsFor,
  formatDiffCompareLabel,
  formatShowcaseRunLabel,
} from "../src/model/runLabels.ts";

const demoRuns: Run[] = [
  {
    key: "shadcn-admin",
    projectId: "oss-shadcn-admin",
    project: "satnaing/shadcn-admin",
    branch: "MIT · React 19 + Vite 8",
    version: "★ 16.9k · TanStack Router",
    diffWith: [],
  },
  {
    key: "dui-site",
    projectId: "oss-dui-site",
    project: "de-novo/dui (dui.sh)",
    branch: "Apache-2.0 · Zero-dep HTML",
    version: "Current capture",
    diffWith: ["dui-site-baseline"],
  },
  {
    key: "dui-site-baseline",
    projectId: "oss-dui-site",
    project: "de-novo/dui (dui.sh)",
    branch: "Apache-2.0 · Zero-dep HTML",
    version: "Baseline capture",
    diffWith: [],
  },
];

test("formatShowcaseRunLabel distinguishes multiple captures of the same project", () => {
  const labels = demoRuns.map((run) => formatShowcaseRunLabel(run, demoRuns));
  assert.equal(new Set(labels).size, demoRuns.length);
  assert.equal(labels[0], "satnaing/shadcn-admin · MIT · React 19 + Vite 8");
  assert.equal(labels[1], "de-novo/dui (dui.sh) · Current capture");
  assert.equal(labels[2], "de-novo/dui (dui.sh) · Baseline capture");
});

test("formatDiffCompareLabel preserves human-readable versions instead of slicing words", () => {
  const label = formatDiffCompareLabel(demoRuns[2]!, [demoRuns[2]!]);
  assert.equal(label, "Baseline capture");
  assert.equal(label.includes("apture") && !label.includes("capture"), false);

  const hashRun: Run = {
    key: "r2",
    projectId: "p1",
    project: "app",
    branch: "main",
    version: "20261125-060000-abcdef12",
  };
  assert.equal(formatDiffCompareLabel(hashRun, [hashRun]), "abcdef12");
});

test("static archive mode restricts comparable runs and diff pairs to declared diffWith entries", () => {
  assert.deepEqual(
    comparableRunsFor(demoRuns[0]!, demoRuns, true).map((r) => r.key),
    [],
  );
  assert.deepEqual(
    comparableRunsFor(demoRuns[1]!, demoRuns, true).map((r) => r.key),
    ["dui-site-baseline"],
  );
  assert.deepEqual(
    comparableRunsFor(demoRuns[2]!, demoRuns, true).map((r) => r.key),
    [],
  );

  const pairs = availableDiffPairs(demoRuns, true);
  assert.deepEqual(
    pairs.map((p) => [p.before.key, p.after.key]),
    [["dui-site-baseline", "dui-site"]],
  );
});

test("live loopback mode compares same-project runs even without diffWith metadata", () => {
  const localRuns: Run[] = [
    { key: "r1", projectId: "p1", project: "app", branch: "main", version: "v1" },
    { key: "r2", projectId: "p1", project: "app", branch: "feat", version: "v2" },
    { key: "r3", projectId: "p2", project: "other", branch: "main", version: "v1" },
  ];
  assert.deepEqual(
    comparableRunsFor(localRuns[1]!, localRuns, false).map((r) => r.key),
    ["r1"],
  );
});
