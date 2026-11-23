import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Cell } from "../src/data/overview.ts";
import {
  computeLiveFrameTransform,
  isLoopbackOrigin,
  liveSurfaceKey,
  mergeLiveRenderTree,
  parseLiveTelemetryMessage,
  resolveLiveBridgeUrl,
  resolveLiveSurfaceUrl,
} from "../src/model/liveSurface.ts";

describe("liveSurface protocol (v0.2.0)", () => {
  it("accepts only loopback origins and resolves screen paths cleanly", () => {
    assert.equal(isLoopbackOrigin("http://127.0.0.1:4173"), true);
    assert.equal(isLoopbackOrigin("http://localhost:3000"), true);
    assert.equal(isLoopbackOrigin("https://evil.example.com"), false);
    assert.equal(isLoopbackOrigin("javascript:alert(1)"), false);

    assert.equal(
      resolveLiveSurfaceUrl("http://127.0.0.1:4173/", "/editor?tab=markdown"),
      "http://127.0.0.1:4173/editor?tab=markdown",
    );
    assert.equal(resolveLiveSurfaceUrl("https://evil.example.com", "/editor"), null);
    assert.equal(
      resolveLiveBridgeUrl("/live-surface", "/editor?tab=a"),
      "/live-surface/editor?tab=a",
    );
    assert.equal(resolveLiveBridgeUrl("https://evil.example.com", "/editor"), null);
  });

  it("computes exact viewport scaling to fit 2D canvas cards 1:1", () => {
    const tf = computeLiveFrameTransform(1280, 320, 200);
    assert.equal(tf.viewportWidth, 1280);
    assert.equal(tf.scale, 0.25);
    assert.equal(tf.viewportHeight, 800);
  });

  it("parses live postMessage renderTree telemetry and merges into active cell", () => {
    const msg = parseLiveTelemetryMessage({
      type: "dui:live-render-tree",
      path: "/editor",
      width: 1280,
      capturedAtMs: 1700000000000,
      renderTree: [
        {
          id: "h1#0",
          tag: "h1",
          text: "Live Mounted Heading",
          rect: { x: 24, y: 16, width: 320, height: 40 },
          style: { fontSize: "28px" },
          boxModel: {
            margin: { top: 0, right: 0, bottom: 16, left: 0 },
            padding: { top: 4, right: 8, bottom: 4, left: 8 },
            border: { top: 0, right: 0, bottom: 0, left: 0 },
            gap: "12px",
          },
          attributes: { "data-live": "true" },
        },
      ],
    });
    assert.ok(msg);
    assert.equal(msg.renderTree.length, 1);
    assert.equal(msg.renderTree[0].boxModel?.gap, "12px");

    const baseCell: Cell = {
      path: "/editor",
      width: 1280,
      status: "verified",
      renderTree: [
        {
          id: "h1#0",
          tag: "h1",
          text: "Static Capture Heading",
          rect: { x: 24, y: 16, width: 300, height: 40 },
          style: {},
        },
      ],
    };
    const merged = mergeLiveRenderTree(baseCell, {
      [liveSurfaceKey("/editor", 1280)]: msg.renderTree,
    });
    assert.equal(merged.renderTree?.[0]?.text, "Live Mounted Heading");
    assert.equal(merged.renderTree?.[0]?.attributes?.["data-live"], "true");
  });
});
