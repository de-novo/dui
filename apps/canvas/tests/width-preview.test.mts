import assert from "node:assert/strict";
import test from "node:test";
import type { Cell } from "../src/data/overview.ts";
import { nearestSource, widthPreviewSources } from "../src/model/widthPreview.ts";

const cell = (width: number, extra: Partial<Cell> = {}): Cell => ({
  path: "/",
  width,
  status: "verified",
  state: "s",
  scenario: "default",
  snapshot: { file: `${"a".repeat(64)}-${width}.html`, fidelity: 0, omitted: 0 },
  ...extra,
});

test("width preview sources are faithful snapshots of the same state, by width", () => {
  const selected = cell(1280);
  const cells = [
    cell(1280),
    cell(375),
    cell(768, { state: "other" }),
    cell(1024, { snapshot: { file: "x", fidelity: 0.02, omitted: 0 } }),
    cell(900, { snapshot: { file: "x", omitted: 0 } }),
    cell(600, { snapshot: undefined }),
    cell(414, { locale: "ko" }),
  ];
  assert.deepEqual(
    widthPreviewSources(cells, selected).map((c) => c.width),
    [375, 1280],
  );
  assert.deepEqual(widthPreviewSources(cells, undefined), []);
});

test("the nearest captured width lays the preview out, ties to the wider", () => {
  const sources = [cell(375), cell(1280)];
  assert.equal(nearestSource(sources, 320)?.width, 375);
  assert.equal(nearestSource(sources, 827)?.width, 375);
  assert.equal(nearestSource(sources, 828)?.width, 1280);
  assert.equal(nearestSource(sources, 827.5)?.width, 1280);
  assert.equal(nearestSource([], 900), undefined);
});
