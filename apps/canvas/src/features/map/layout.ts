import type { Screen, ScreenMap } from "../../data/overview";

type Link = ScreenMap["links"][number];
type Point = { x: number; y: number };
type Placed = { screen: Screen; position: Point };
type Layout = { nodes: Placed[]; links: Link[] };

const column = 440;
const row = 265;

function neighborhood(screens: Screen[], links: Link[], focus: Screen): Layout {
  const incident = links.filter(
    (e) => e.source === focus.id || e.target === focus.id,
  );
  const incoming = new Set(
    incident.filter((e) => e.target === focus.id).map((e) => e.source),
  );
  const outgoing = new Set(
    incident.filter((e) => e.source === focus.id).map((e) => e.target),
  );
  const nodes = [{ screen: focus, position: { x: 0, y: 0 } }];
  // A bidirectional neighbor appears once, with two independently directed arrows.
  const otherScreens = screens.filter((screen) => screen.id !== focus.id);
  const left = otherScreens.filter(
    (s) => incoming.has(s.id) && !outgoing.has(s.id),
  );
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
): Layout & { total: number } {
  const screens = [...archive.screens].sort((a, b) =>
    a.id < b.id ? -1 : a.id > b.id ? 1 : 0,
  );
  const links = archive.links.filter(
    (e) => !e.observedWidths || e.observedWidths.includes(width),
  );
  const focus = screens.find((s) => mode === "focus:" + s.id);
  if (focus)
    return { ...neighborhood(screens, links, focus), total: links.length };

  const neighbors = new Map(
    screens.map((s) => [s.id, new Map<string, Link>()]),
  );
  // Prefer the observed forward edge when either direction can join the same pair.
  for (const e of links) neighbors.get(e.source)!.set(e.target, e);
  for (const e of links) {
    const opposite = neighbors.get(e.target)!;
    if (!opposite.has(e.source)) opposite.set(e.source, e);
  }
  const roots = [...screens].sort(
    (a, b) => neighbors.get(b.id)!.size - neighbors.get(a.id)!.size,
  );
  const byId = new Map(screens.map((s) => [s.id, s]));
  const seen = new Set<string>();
  const forest: Link[] = [];
  const blocks: { nodes: Placed[]; width: number; height: number }[] = [];
  for (const root of roots) {
    if (seen.has(root.id)) continue;
    seen.add(root.id);
    const queue = [{ id: root.id, depth: 0 }];
    const children = new Map<string, string[]>();
    let depth = 0;
    for (const item of queue) {
      depth = Math.max(depth, item.depth);
      const branches: string[] = [];
      children.set(item.id, branches);
      const adjacent = neighbors.get(item.id)!;
      for (const id of [...adjacent.keys()].sort()) {
        if (seen.has(id)) continue;
        seen.add(id);
        forest.push(adjacent.get(id)!);
        branches.push(id);
        queue.push({ id, depth: item.depth + 1 });
      }
    }
    // Allocate a contiguous band of rows to each subtree; center the parent in its band.
    const span = new Map<string, number>();
    for (const { id } of [...queue].reverse()) {
      span.set(
        id,
        children.get(id)!.reduce((sum, child) => sum + span.get(child)!, 0) ||
          1,
      );
    }
    const starts = new Map([[root.id, 0]]);
    const nodes = queue.map(({ id, depth }) => {
      const start = starts.get(id)!;
      let next = start;
      for (const child of children.get(id)!) {
        starts.set(child, next);
        next += span.get(child)!;
      }
      return {
        screen: byId.get(id)!,
        position: {
          x: depth * column,
          y: (start + (span.get(id)! - 1) / 2) * row,
        },
      };
    });
    blocks.push({
      nodes,
      width: (depth + 1) * column,
      height: span.get(root.id)! * row,
    });
  }
  const targetWidth = Math.sqrt(
    blocks.reduce((a, b) => a + b.width * b.height, 0) * 1.6,
  );
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
  return { nodes, links: mode === "all" ? links : forest, total: links.length };
}
