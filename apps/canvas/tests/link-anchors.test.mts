import assert from "node:assert/strict";
import test from "node:test";
import type { Cell, RenderNode, Screen } from "../src/data/overview.ts";
import { anchorsTo, linkPath, treeCell } from "../src/model/linkAnchors.ts";

const node = (id: string, tag: string, rect: RenderNode["rect"], href?: string): RenderNode => ({
  id,
  tag,
  text: id,
  rect,
  style: {},
  ...(href ? { attributes: { href } } : {}),
});

const cell = (renderTree: RenderNode[], extra: Partial<Cell> = {}): Cell => ({
  path: "/docs/",
  width: 1000,
  status: "verified",
  image: "a.png",
  imageWidth: 2000,
  imageHeight: 4000,
  renderTree,
  ...extra,
});

test("linkPath drops hashes and index.html and keeps queries", () => {
  assert.equal(linkPath("/"), "/");
  assert.equal(linkPath("/index.html"), "/");
  assert.equal(linkPath("/docs/index.html#top"), "/docs/");
  assert.equal(linkPath("/cli.html?tab=mcp#x"), "/cli.html?tab=mcp");
});

test("anchorsTo resolves relative hrefs in document order as capture fractions", () => {
  // A 2x PNG: 2000 × 4000 image pixels are 1000 × 2000 CSS pixels.
  const tree = [
    node("nav", "a", { x: 500, y: 10, width: 100, height: 20 }, "../cli.html"),
    node("brand", "a", { x: 0, y: 10, width: 80, height: 20 }, "./"),
    node("btn", "button", { x: 0, y: 0, width: 10, height: 10 }),
    node("cta", "a", { x: 200, y: 400, width: 200, height: 40 }, "/cli.html#flags"),
    node("hidden", "a", { x: 0, y: 0, width: 0, height: 0 }, "/cli.html"),
    node("external", "a", { x: 0, y: 50, width: 50, height: 10 }, "https://x.test/cli.html"),
    node("below", "a", { x: 0, y: 2500, width: 50, height: 10 }, "/cli.html"),
  ];
  assert.deepEqual(anchorsTo(cell(tree), "/docs/", "/cli.html"), [
    { x: 0.5, y: 0.005, width: 0.1, height: 0.01 },
    { x: 0.2, y: 0.2, width: 0.2, height: 0.02 },
  ]);
  assert.deepEqual(anchorsTo(cell(tree), "/docs/", "/docs/index.html"), [
    { x: 0, y: 0.005, width: 0.08, height: 0.01 },
  ]);
  assert.deepEqual(anchorsTo(cell(tree), "/docs/", "/missing"), []);
  assert.deepEqual(anchorsTo(cell([]), "/docs/", "/cli.html"), []);
  assert.deepEqual(anchorsTo(cell(tree, { imageHeight: undefined }), "/docs/", "/cli.html"), []);
});

test("anchorsTo clips anchors that leave the capture on the left or right", () => {
  const tree = [node("wide", "a", { x: -100, y: 0, width: 1200, height: 10 }, "/a")];
  assert.deepEqual(anchorsTo(cell(tree), "/", "/a"), [{ x: 0, y: 0, width: 1, height: 0.005 }]);
});

test("treeCell finds the capture behind an overview entry", () => {
  const tree = [node("a", "a", { x: 0, y: 0, width: 10, height: 10 }, "/")];
  const full = cell(tree, { image: "x.png" });
  const stated = cell(tree, { image: "y.png", state: "open", width: 375 });
  const narrow = cell(tree, { image: "z.png", width: 375 });
  const screen: Screen = { id: "s", path: "/docs/", cells: [stated, full, narrow] };
  const entry = cell([], { image: "x.png", renderTree: undefined });
  assert.equal(treeCell(screen, entry), full);
  assert.equal(treeCell(screen, full), full);
  assert.equal(
    treeCell(screen, cell([], { image: "gone.png", width: 375, renderTree: undefined })),
    narrow,
  );
  assert.equal(treeCell(screen, undefined), undefined);
});
