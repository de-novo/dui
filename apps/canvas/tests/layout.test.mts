import assert from "node:assert/strict";
import { test } from "node:test";
import { clearLane, layoutMap, nodeMetrics } from "../src/features/map/layout.ts";
import type { ScreenMap } from "../src/data/overview.ts";

const archive: ScreenMap = {
  screens: ["a", "b", "c", "d", "e"].map((id) => ({
    id,
    path: "/" + id,
    cells: [],
  })),
  links: [
    { source: "a", target: "b" },
    { source: "b", target: "a" },
    { source: "b", target: "c" },
    { source: "c", target: "a", observedWidths: [375] },
    { source: "d", target: "e" },
  ],
};

test("overview spans each connected group using recorded directions and stable positions", () => {
  const overview = layoutMap(archive, 375, "overview");
  assert.equal(overview.nodes.length, 5);
  assert.equal(overview.links.length, 3);
  assert.equal(overview.total, 5);
  assert.ok(overview.links.includes(archive.links[4]));
  assert.equal(new Set(overview.nodes.map((n) => JSON.stringify(n.position))).size, 5);
  for (const edge of overview.links) assert.ok(archive.links.includes(edge));
  assert.deepEqual(
    overview,
    layoutMap(
      {
        screens: [...archive.screens].reverse(),
        links: [...archive.links].reverse(),
      },
      375,
      "overview",
    ),
  );
  assert.deepEqual(layoutMap(archive, 375, "all").links, archive.links);
});

test("focus preserves reverse links and filters evidence by capture width", () => {
  const focused = layoutMap(archive, 1280, "focus:a");
  assert.deepEqual(focused.links, archive.links.slice(0, 2));
  assert.deepEqual(
    focused.nodes.map((n) => n.screen.id),
    ["a", "b"],
  );
  assert.equal(layoutMap(archive, 375, "focus:a").links.length, 3);
  const isolated = layoutMap({ ...archive, links: [] }, 375, "focus:c");
  assert.deepEqual(
    isolated.nodes.map((n) => n.screen.id),
    ["c"],
  );
  assert.equal(isolated.links.length, 0);
});

test("wide groups share columns without overlapping or inventing links", () => {
  const screens = Array.from({ length: 26 }, (_, i) => ({
    id: String(i),
    path: "/" + i,
    cells: [],
  }));
  const links = screens.slice(1).map((s) => ({ source: "0", target: s.id }));
  const plan = layoutMap({ screens, links }, 1280, "overview");
  const positions = plan.nodes.map((n) => n.position);
  assert.equal(new Set(positions.map((p) => JSON.stringify(p))).size, screens.length);
  assert.ok(new Set(positions.map((p) => p.x)).size > 2);
  assert.ok(new Set(positions.map((p) => p.y)).size < screens.length / 2);
  assert.deepEqual(new Set(plan.links), new Set(links));
});

test("nodeMetrics sizes map cards 1:1 with capture width and spaces columns without overlap", () => {
  const mobile = nodeMetrics(375);
  assert.equal(mobile.cardWidth, 375);
  assert.ok(mobile.column >= mobile.cardWidth + 120);
  assert.ok(mobile.frameHeight >= 500);

  const desktop = nodeMetrics(1280);
  assert.equal(desktop.cardWidth, 1280);
  assert.equal(desktop.frameHeight, 800);
  assert.ok(desktop.column >= desktop.cardWidth + 120);

  const plan = layoutMap(archive, 1280, "overview");
  assert.equal(plan.metrics.cardWidth, 1280);
  const xs = [...new Set(plan.nodes.map((n) => n.position.x))].sort((a, b) => a - b);
  for (let i = 1; i < xs.length; i += 1) {
    assert.ok(xs[i]! - xs[i - 1]! >= desktop.cardWidth, "columns must not overlap at 1:1 width");
  }
});

test("layoutMap sizes tall full-page captures to their real aspect ratio without vertical overlap", () => {
  const tallArchive: ScreenMap = {
    screens: [
      {
        id: "home",
        path: "/",
        cells: [
          {
            path: "/",
            width: 1280,
            status: "verified",
            image: "home.png",
            imageWidth: 1280,
            imageHeight: 800,
          },
        ],
      },
      {
        id: "orders",
        path: "/orders",
        cells: [
          {
            path: "/orders",
            width: 1280,
            status: "verified",
            image: "orders.png",
            imageWidth: 1280,
            imageHeight: 3200,
          },
        ],
      },
      {
        id: "detail",
        path: "/orders/1",
        cells: [
          {
            path: "/orders/1",
            width: 1280,
            status: "verified",
            image: "detail.png",
            imageWidth: 1280,
            imageHeight: 1600,
          },
        ],
      },
    ],
    links: [
      { source: "home", target: "orders", provenance: "verified", replayVerified: true },
      { source: "home", target: "detail", provenance: "verified", replayVerified: true },
    ],
  };

  // The overview names the capture; only the detailed cell supplies its PNG dimensions.
  for (const screen of tallArchive.screens) {
    screen.entries = screen.cells.map(({ width, path, status, image }) => ({
      width,
      path,
      status,
      image,
    }));
  }
  const plan = layoutMap(tallArchive, 1280, "overview");
  const ordersNode = plan.nodes.find((n) => n.screen.id === "orders")!;
  const detailNode = plan.nodes.find((n) => n.screen.id === "detail")!;
  assert.equal(ordersNode.frameHeight, 3200);
  assert.equal(detailNode.frameHeight, 1600);
  // Same column siblings must never overlap vertically when the upper screen is 3200px tall.
  const [top, bottom] =
    ordersNode.position.y < detailNode.position.y
      ? [ordersNode, detailNode]
      : [detailNode, ordersNode];
  assert.ok(
    bottom.position.y >= top.position.y + top.frameHeight + 120,
    `expected bottom node (${bottom.position.y}) below top node (${top.position.y} + ${top.frameHeight})`,
  );
});

test("measured card boxes reserve space at every width in overview, all links and focus", () => {
  const screens = Array.from({ length: 20 }, (_, i) => ({
    id: String(i),
    path: "/page/" + i,
    cells: [],
  }));
  const map = { screens, links: screens.slice(1).map((s) => ({ source: "0", target: s.id })) };
  for (const width of [401, 937, 1493]) {
    const heights = new Map(screens.map((s, i) => [s.id, 179 + ((i * 1307 + width) % 4901)]));
    for (const mode of ["overview", "all", "focus:0"]) {
      const { nodes, metrics } = layoutMap(map, width, mode, heights);
      for (const [i, a] of nodes.entries()) {
        for (const b of nodes.slice(i + 1)) {
          if (Math.abs(a.position.x - b.position.x) >= metrics.cardWidth) continue;
          const [upper, lower] = a.position.y <= b.position.y ? [a, b] : [b, a];
          assert.ok(lower.position.y > upper.position.y + heights.get(upper.screen.id)!);
        }
      }
    }
  }
});

test("the nearest clear lane accounts for nested and overlapping obstacle intervals", () => {
  const obstacles = [
    { top: 80, bottom: 140 },
    { top: 10, bottom: 90 },
    { top: 40, bottom: 50 },
    { top: 180, bottom: 215 },
  ];
  assert.equal(clearLane(60, obstacles), 10);
  assert.equal(clearLane(100, obstacles), 140);
  assert.equal(clearLane(160, obstacles), 160);
  assert.equal(clearLane(-40, obstacles), -40);
  assert.equal(clearLane(205, obstacles), 215);
});
