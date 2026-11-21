import assert from "node:assert/strict";
import { test } from "node:test";
import {
  canOpenLiveCell,
  decodeLiveEdit,
  decodeLiveInspect,
  decodeLiveOpen,
  decodeLiveSave,
  provenanceDiffers,
} from "../src/model/live.ts";

test("decodes live session responses and detects provenance differences", () => {
  const opened = decodeLiveOpen({
    session: "a".repeat(64),
    revision: 0,
    cell: { state: "s1", width: 1280 },
    provenance: {
      archive: { commit: "1111111", dirty: false },
      live: { commit: "2222222", dirty: true, branch: "main" },
    },
  });
  assert.equal(opened.revision, 0);
  assert.equal(provenanceDiffers(opened.provenance), true);

  const inspected = decodeLiveInspect({
    session: opened.session,
    revision: 1,
    cell: { state: "s1", width: 1280 },
    controls: [
      {
        slot: "main.tsx#1:2:open",
        owner: "Menu",
        instance: "inst-1",
        label: "open",
        type: "boolean",
        source: { file: "main.tsx" },
        editable: true,
        reason: null,
        value: true,
        initialValue: false,
        options: [false, true],
      },
    ],
  });
  assert.equal(inspected.controls[0].initialValue, false);
  assert.deepEqual(inspected.controls[0].options, [false, true]);

  const edited = decodeLiveEdit({
    session: opened.session,
    cell: { state: "s1", width: 1280 },
    slot: "main.tsx#1:2:open",
    instance: "inst-1",
    revision: 1,
    status: "unverified",
    reason: "missing-assertion",
    observedValue: true,
    evidence: { mode: "semantic-diff", changed: true },
  });
  assert.equal(edited.status, "unverified");
  assert.equal(edited.evidence.changed, true);

  const saved = decodeLiveSave({
    project: "p1",
    run: "r2",
    runKey: "k2",
    cell: { state: "live-1", width: 1280 },
    revision: 1,
    status: "verified",
    reason: null,
    image: "img.png",
    imageSha256: "abc",
  });
  assert.equal(saved.runKey, "k2");
});

test("gates live opening for missing state IDs, saved live revisions, and unrenderable cells", () => {
  const run = {
    key: "k1",
    projectId: "p1",
    project: "demo",
    branch: "main",
    version: "v1",
    liveAvailable: true,
  };
  assert.equal(
    canOpenLiveCell({ path: "/", width: 1280, status: "verified", state: "s1" }, run).ok,
    true,
  );
  assert.equal(
    canOpenLiveCell(
      { path: "/", width: 1280, status: "verified", state: "s1", scenario: "live" },
      run,
    ).ok,
    false,
  );
  assert.equal(
    canOpenLiveCell(
      { path: "/", width: 1280, status: "unrenderable", reason: "no-render-env", state: "s1" },
      run,
    ).ok,
    false,
  );
  assert.equal(
    canOpenLiveCell(
      { path: "/", width: 1280, status: "verified", state: "s1" },
      { ...run, liveAvailable: false },
    ).ok,
    false,
  );
});
