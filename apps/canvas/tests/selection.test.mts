import assert from "node:assert/strict";
import { test } from "node:test";
import type { ScreenMap } from "../src/data/overview.ts";
import {
  cardCell,
  parseSelection,
  resolveCell,
  screenConnections,
  withSearchChanges,
} from "../src/model/selection.ts";

const archive: ScreenMap = {
  screens: [
    {
      id: "route:/?q=demo",
      path: "/?q=demo",
      cells: [
        { path: "/?q=demo", width: 375, status: "verified", state: "s-graph", scenario: "default" },
        {
          path: "/?q=demo",
          width: 1280,
          status: "verified",
          state: "s-graph",
          scenario: "default",
        },
      ],
    },
    {
      id: "route:/",
      path: "/",
      cells: [
        { path: "/", width: 375, status: "verified", state: "s-home", scenario: "link-0" },
        { path: "/", width: 1280, status: "verified", state: "s-home", scenario: "link-0" },
      ],
    },
  ],
  links: [
    {
      source: "route:/?q=demo",
      target: "route:/",
      observedWidths: [375, 1280],
      origin: "anchor",
      selector: "header a[href]",
      replayVerified: false,
    },
  ],
};

test("selection preserves selected screen when returning to overview from live view", () => {
  const live = parseSelection(
    "?screen=route%3A%2F%3Fq%3Ddemo&state=s-graph&width=375&view=live",
    archive,
  );
  assert.equal(live.view, "live");
  assert.equal(live.screen, "route:/?q=demo");
  assert.equal(live.width, 375);
  assert.equal(live.edges, "element");
  assert.equal(parseSelection("?edges=card", archive).edges, "card");
  assert.equal(parseSelection("?edges=nonsense", archive).edges, "element");

  const overviewUrl = withSearchChanges(
    "http://127.0.0.1/?screen=route%3A%2F%3Fq%3Ddemo&state=s-graph&width=375&view=live",
    { view: "overview" },
  );
  const overview = parseSelection(overviewUrl.search, archive);
  assert.equal(overview.view, "overview");
  assert.equal(overview.screen, "route:/?q=demo");
  assert.equal(overview.state, "s-graph");
  assert.equal(overview.width, 375);
});

test("resolves screen connections and selected cell deterministically", () => {
  const { cell } = resolveCell(archive.screens[0], 1280, "s-graph");
  assert.equal(cell?.width, 1280);
  assert.equal(cell?.state, "s-graph");

  const outgoing = screenConnections(archive, "route:/?q=demo");
  assert.equal(outgoing.length, 1);
  assert.equal(outgoing[0].direction, "outgoing");
  assert.equal(outgoing[0].screen.id, "route:/");
  assert.equal(outgoing[0].link.replayVerified, false);

  const incoming = screenConnections(archive, "route:/");
  assert.equal(incoming.length, 1);
  assert.equal(incoming[0].direction, "incoming");
  assert.equal(incoming[0].screen.id, "route:/?q=demo");
});

test("a native declaration without a capture at the selected width keeps its recorded width", () => {
  const nativeSource = { source: "ios/Settings.swift", name: "Settings" };
  const ios = {
    id: "ios",
    path: "ios/Settings.swift#Settings",
    nativeSource,
    cells: [{ path: "ios", width: 402, status: "unverified", scenario: "ios", image: "ios.png" }],
  } as unknown as ScreenMap["screens"][number];
  assert.equal(cardCell(ios, 411)?.image, "ios.png");
  assert.equal(resolveCell(ios, 411, null).cell?.width, 402);
  // Web screens keep the strict width match.
  assert.equal(cardCell({ ...ios, nativeSource: undefined }, 411), undefined);
});
