import type { Screen, ScreenMap } from "../../data/overview";

type Link = ScreenMap["links"][number];
type Point = { x: number; y: number };
type Placed = { screen: Screen; position: Point };
type Layout = { nodes: Placed[]; links: Link[] };

export type NodeMetrics = {
  cardWidth: number;
  frameHeight: number;
  column: number;
  row: number;
};

export function nodeMetrics(captureWidth: number): NodeMetrics {
  const cardWidth = Math.max(
    360,
    Number.isFinite(captureWidth) && captureWidth > 0 ? captureWidth : 375,
  );
  const frameHeight = Math.round(cardWidth * (cardWidth <= 500 ? 1.6 : 0.625));
  const column = cardWidth + Math.max(120, Math.round(cardWidth * 0.14));
  const row = frameHeight + Math.max(160, Math.round(frameHeight * 0.2));
  return { cardWidth, frameHeight, column, row };
}

function neighborhood(
  screens: Screen[],
  links: Link[],
  focus: Screen,
  metrics: NodeMetrics,
): Layout {
  const { column, row } = metrics;
  const incident = links.filter((e) => e.source === focus.id || e.target === focus.id);
  const incoming = new Set(incident.filter((e) => e.target === focus.id).map((e) => e.source));
  const outgoing = new Set(incident.filter((e) => e.source === focus.id).map((e) => e.target));
  const nodes = [{ screen: focus, position: { x: 0, y: 0 } }];
  // A bidirectional neighbor appears once, with two independently directed arrows.
  const otherScreens = screens.filter((screen) => screen.id !== focus.id);
  const left = otherScreens.filter((s) => incoming.has(s.id) && !outgoing.has(s.id));
  const right = otherScreens.filter((s) => outgoing.has(s.id));
  for (const [group, x] of [
    [left, -column],
    [right, column],
  ] as const) {
    group.forEach((screen, i) =>
      nodes.push({
        screen,
        position: { x, y: (i - (group.length - 1) / 2) * row },
      }),
    );
  }
  return { nodes, links: incident };
}

export function layoutMap(
  archive: ScreenMap,
  width: number,
  mode: string,
): Layout & { total: number; metrics: NodeMetrics } {
  const metrics = nodeMetrics(width);
  const { column, row } = metrics;
  const screens = [...archive.screens].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  const links = archive.links.filter((e) => !e.observedWidths || e.observedWidths.includes(width));
  const focus = screens.find((s) => mode === "focus:" + s.id);
  if (focus)
    return { ...neighborhood(screens, links, focus, metrics), total: links.length, metrics };

  const neighbors = new Map(screens.map((s) => [s.id, new Map<string, Link>()]));
  // Prefer the observed forward edge when either direction can join the same pair.
  for (const e of links) neighbors.get(e.source)!.set(e.target, e);
  for (const e of links) {
    const opposite = neighbors.get(e.target)!;
    if (!opposite.has(e.source)) opposite.set(e.source, e);
  }
  const roots = [...screens].sort((a, b) => neighbors.get(b.id)!.size - neighbors.get(a.id)!.size);
  const byId = new Map(screens.map((s) => [s.id, s]));
  const seen = new Set<string>();
  const forest: Link[] = [];
  const blocks: { nodes: Placed[]; width: number; height: number }[] = [];
  for (const root of roots) {
    if (seen.has(root.id)) continue;
    seen.add(root.id);
    const queue = [{ id: root.id }];
    const children = new Map<string, string[]>();
    for (const item of queue) {
      const branches: string[] = [];
      children.set(item.id, branches);
      const adjacent = neighbors.get(item.id)!;
      for (const id of [...adjacent.keys()].sort()) {
        if (seen.has(id)) continue;
        seen.add(id);
        forest.push(adjacent.get(id)!);
        branches.push(id);
        queue.push({ id });
      }
    }
    // Pack wide sibling groups into short columns instead of one long strip.
    const frames = new Map<string, { width: number; height: number; center: number }>();
    const offsets = new Map<string, Point>();
    for (const { id } of [...queue].reverse()) {
      const branches = children.get(id)!;
      const bandHeight =
        branches.length <= 4
          ? Infinity
          : Math.ceil(
              Math.sqrt(
                (branches.reduce((sum, child) => sum + frames.get(child)!.height, 0) * column) /
                  row,
              ),
            );
      let x = 1,
        y = 0,
        bandWidth = 0,
        height = 1;
      for (const child of branches) {
        const frame = frames.get(child)!;
        if (y && y + frame.height > bandHeight) {
          x += bandWidth;
          bandWidth = 0;
          y = 0;
        }
        offsets.set(child, { x, y });
        bandWidth = Math.max(bandWidth, frame.width);
        y += frame.height;
        height = Math.max(height, y);
      }
      frames.set(id, {
        width: x + bandWidth,
        height,
        center: (height - 1) / 2,
      });
    }
    offsets.set(root.id, { x: 0, y: 0 });
    const nodes = queue.map(({ id }) => {
      const offset = offsets.get(id)!;
      for (const child of children.get(id)!) {
        const local = offsets.get(child)!;
        offsets.set(child, { x: offset.x + local.x, y: offset.y + local.y });
      }
      return {
        screen: byId.get(id)!,
        position: {
          x: offset.x * column,
          y: (offset.y + frames.get(id)!.center) * row,
        },
      };
    });
    const frame = frames.get(root.id)!;
    blocks.push({
      nodes,
      width: frame.width * column,
      height: frame.height * row,
    });
  }
  const targetWidth = Math.sqrt(blocks.reduce((a, b) => a + b.width * b.height, 0) * 1.6);
  const nodes: Placed[] = [];
  let x = 0,
    y = 0,
    shelfHeight = 0;
  for (const block of blocks) {
    if (x && x + block.width > targetWidth) {
      y += shelfHeight;
      x = 0;
      shelfHeight = 0;
    }
    for (const n of block.nodes)
      nodes.push({
        screen: n.screen,
        position: { x: x + n.position.x, y: y + n.position.y },
      });
    x += block.width;
    shelfHeight = Math.max(shelfHeight, block.height);
  }
  return { nodes, links: mode === "all" ? links : forest, total: links.length, metrics };
}
