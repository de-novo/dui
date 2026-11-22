import assert from "node:assert/strict";
import test from "node:test";
import type { RenderNode } from "../src/data/overview.ts";
import {
  boxModelOverlayRects,
  computeSpacingGuides,
  pickNodeAtPoint,
} from "../src/model/elementInspect.ts";

const card: RenderNode = {
  id: "section#0",
  tag: "section",
  text: "",
  rect: { x: 16, y: 16, width: 320, height: 200 },
  style: { display: "flex", flexDirection: "column" },
  boxModel: {
    margin: { top: 8, right: 8, bottom: 16, left: 8 },
    padding: { top: 16, right: 16, bottom: 16, left: 16 },
    border: { top: 1, right: 1, bottom: 1, left: 1 },
    borderRadius: "12px",
    gap: "12px",
  },
};

const heading: RenderNode = {
  id: "h1#0",
  tag: "h1",
  text: "Billing",
  rect: { x: 32, y: 32, width: 200, height: 32 },
  style: { fontSize: "24px", fontWeight: "700" },
  parentId: "section#0",
};

const button: RenderNode = {
  id: "button#0",
  tag: "button",
  text: "Upgrade",
  rect: { x: 32, y: 88, width: 120, height: 40 },
  style: { backgroundColor: "rgb(24, 24, 27)" },
  parentId: "section#0",
};

test("pickNodeAtPoint selects deepest smallest-area element containing coordinates", () => {
  const hit = pickNodeAtPoint([card, heading, button], 48, 96);
  assert.equal(hit?.id, "button#0");
  const cardHit = pickNodeAtPoint([card, heading, button], 280, 180);
  assert.equal(cardHit?.id, "section#0");
  assert.equal(pickNodeAtPoint([card, heading, button], 500, 500), null);
});

test("boxModelOverlayRects expands margin outward and insets border and padding", () => {
  const boxes = boxModelOverlayRects(card);
  assert.deepEqual(boxes.margin, { x: 8, y: 8, width: 336, height: 224 });
  assert.deepEqual(boxes.border, { x: 16, y: 16, width: 320, height: 200 });
  assert.deepEqual(boxes.padding, { x: 17, y: 17, width: 318, height: 198 });
  assert.deepEqual(boxes.content, { x: 33, y: 33, width: 286, height: 166 });
});

test("computeSpacingGuides measures peer gap and container insets", () => {
  const peerGuides = computeSpacingGuides(heading, button);
  assert.equal(peerGuides.length, 1);
  assert.equal(peerGuides[0]?.axis, "y");
  assert.equal(peerGuides[0]?.distance, 24);

  const insetGuides = computeSpacingGuides(heading, card);
  assert.equal(insetGuides.length, 4);
  const topGuide = insetGuides.find((g) => g.axis === "y" && g.y1 === 16);
  const leftGuide = insetGuides.find((g) => g.axis === "x" && g.x1 === 16);
  assert.equal(topGuide?.distance, 16);
  assert.equal(leftGuide?.distance, 16);
});
