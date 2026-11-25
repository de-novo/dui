import assert from "node:assert/strict";
import { createHash, randomUUID } from "node:crypto";
import { test } from "node:test";
import { decodeSourcePreview } from "../src/data/sourcePreview.ts";

test("a ready archive scene is bound to its card, entry and source digest", () => {
  const entry = "Settings";
  const screenId = randomUUID();
  const sourceSha256 = createHash("sha256").update(screenId).digest("hex");
  const scene = {
    version: 1,
    entry,
    sourceSha256,
    frames: [{ tokens: [["text", entry]], next: [] }],
  };
  const raw = { status: "ready", screenId, sourceSha256, scene };
  assert.equal(decodeSourcePreview(raw, entry, screenId).status, "ready");
  assert.equal(decodeSourcePreview(raw, entry, randomUUID()).status, "stale");
  assert.equal(decodeSourcePreview(raw, "Other", screenId).status, "stale");
  assert.equal(
    decodeSourcePreview({ ...raw, scene: { ...scene, sourceSha256: "" } }, entry, screenId).status,
    "stale",
  );
  for (const frame of [
    { tokens: [["unknown", entry]], next: [] },
    { tokens: [["switch", false, entry]], next: [1] },
    { tokens: [["end"]], next: [] },
  ]) {
    const preview = { ...raw, scene: { ...scene, frames: [frame] } };
    assert.equal(decodeSourcePreview(preview, entry, screenId).status, "unsupported");
  }
});
