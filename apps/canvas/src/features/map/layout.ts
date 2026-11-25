import type { Screen, ScreenMap } from "../../data/overview";
import { cardCell } from "../../model/selection.ts";

type Link = ScreenMap["links"][number];
type Point = { x: number; y: number };
type Placed = { screen: Screen; position: Point; frameHeight: number };
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
  const column = cardWidth + Math.max(220, Math.round(cardWidth * 0.22));
  const row = frameHeight + Math.max(200, Math.round(frameHeight * 0.26));
  return { cardWidth, frameHeight, column, row };
}

export function screenFrameHeight(screen: Screen, width: number, metrics: NodeMetrics): number {
  const entry = cardCell(screen, width);
  // Pages archives may store PNG dimensions only on the detailed capture.
  const cell =
    entry?.image && !(entry.imageWidth && entry.imageHeight)
      ? screen.cells.find((c) => c.image === entry.image && c.imageWidth && c.imageHeight)
      : entry;
  if (cell?.imageWidth && cell?.imageHeight && cell.imageWidth > 0 && cell.imageHeight > 0) {
    return Math.max(240, Math.round(metrics.cardWidth * (cell.imageHeight / cell.imageWidth)));
  }
  return metrics.frameHeight;
}

function neighborhood(
  screens: Screen[],
  links: Link[],
  focus: Screen,
  width: number,
  metrics: NodeMetrics,
  measured: ReadonlyMap<string, number>,
): Layout {
  const { column } = metrics;
  const rowGap = metrics.row - metrics.frameHeight;
  const heightOf = (s: Screen) => screenFrameHeight(s, width, metrics);
  const incident = links.filter((e) => e.source === focus.id || e.target === focus.id);
  const incoming = new Set(incident.filter((e) => e.target === focus.id).map((e) => e.source));
  const outgoing = new Set(incident.filter((e) => e.source === focus.id).map((e) => e.target));
  const focusH = heightOf(focus);
  const nodes: Placed[] = [{ screen: focus, position: { x: 0, y: 0 }, frameHeight: focusH }];
  // A bidirectional neighbor appears once, with two independently directed arrows.
  const otherScreens = screens.filter((screen) => screen.id !== focus.id);
  const left = otherScreens.filter((s) => incoming.has(s.id) && !outgoing.has(s.id));
  const right = otherScreens.filter((s) => outgoing.has(s.id));
  for (const [group, x] of [
    [left, -column],
    [right, column],
  ] as const) {
    const boxes = group.map((s) => {
      const fh = heightOf(s);
      return { screen: s, frameHeight: fh, boxHeight: (measured.get(s.id) ?? fh) + rowGap };
    });
    const totalH = boxes.reduce((sum, b) => sum + b.boxHeight, 0);
    let cursorY = ((measured.get(focus.id) ?? focusH) + rowGap - totalH) / 2;
    for (const item of boxes) {
      nodes.push({
        screen: item.screen,
        position: { x, y: Math.round(cursorY) },
        frameHeight: item.frameHeight,
      });
      cursorY += item.boxHeight;
    }
  }
  return { nodes, links: incident };
}

export function layoutMap(
  archive: ScreenMap,
  width: number,
  mode: string,
  measured: ReadonlyMap<string, number> = new Map(),
): Layout & { total: number; metrics: NodeMetrics } {
  const metrics = nodeMetrics(width);
  const { column, row } = metrics;
  const rowGap = row - metrics.frameHeight;
  const screens = [...archive.screens].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  const links = archive.links.filter((e) => !e.observedWidths || e.observedWidths.includes(width));
  const focus = screens.find((s) => mode === "focus:" + s.id);
  if (focus)
    return {
      ...neighborhood(screens, links, focus, width, metrics, measured),
      total: links.length,
      metrics,
    };

  const frameHeights = new Map(screens.map((s) => [s.id, screenFrameHeight(s, width, metrics)]));
  const boxHeights = new Map(
    screens.map((s) => [s.id, (measured.get(s.id) ?? frameHeights.get(s.id)!) + rowGap]),
  );
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
    // Pack wide sibling groups into short columns using each node's real vertical footprint.
    const frames = new Map<string, { cols: number; heightPx: number; selfOffsetY: number }>();
    const offsets = new Map<string, { col: number; yPx: number }>();
    for (const { id } of [...queue].reverse()) {
      const selfBoxH = boxHeights.get(id)!;
      const branches = children.get(id)!;
      if (branches.length === 0) {
        frames.set(id, { cols: 1, heightPx: selfBoxH, selfOffsetY: 0 });
        continue;
      }
      const totalBranchH = branches.reduce((sum, child) => sum + frames.get(child)!.heightPx, 0);
      const bandHeightPx =
        branches.length <= 4 ? Infinity : Math.max(selfBoxH, Math.sqrt(totalBranchH * column));
      let col = 1,
        yPx = 0,
        bandCols = 0,
        heightPx = selfBoxH;
      for (const child of branches) {
        const frame = frames.get(child)!;
        if (yPx > 0 && yPx + frame.heightPx > bandHeightPx) {
          col += bandCols;
          bandCols = 0;
          yPx = 0;
        }
        offsets.set(child, { col, yPx });
        bandCols = Math.max(bandCols, frame.cols);
        yPx += frame.heightPx;
        heightPx = Math.max(heightPx, yPx);
      }
      frames.set(id, {
        cols: col + bandCols,
        heightPx,
        selfOffsetY: Math.round((heightPx - selfBoxH) / 2),
      });
    }
    offsets.set(root.id, { col: 0, yPx: 0 });
    const nodes: Placed[] = queue.map(({ id }) => {
      const offset = offsets.get(id)!;
      for (const child of children.get(id)!) {
        const local = offsets.get(child)!;
        offsets.set(child, { col: offset.col + local.col, yPx: offset.yPx + local.yPx });
      }
      return {
        screen: byId.get(id)!,
        position: {
          x: offset.col * column,
          y: offset.yPx + frames.get(id)!.selfOffsetY,
        },
        frameHeight: frameHeights.get(id)!,
      };
    });
    const frame = frames.get(root.id)!;
    blocks.push({
      nodes,
      width: frame.cols * column,
      height: frame.heightPx,
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
        frameHeight: n.frameHeight,
      });
    x += block.width;
    shelfHeight = Math.max(shelfHeight, block.height);
  }
  return { nodes, links: mode === "all" ? links : forest, total: links.length, metrics };
}

// Find the nearest horizontal lane outside the union of the intervening card bounds.
export function clearLane(preferred: number, intervals: { top: number; bottom: number }[]) {
  const sorted = intervals.toSorted((a, b) => a.top - b.top);
  let bottom = -Infinity;
  let top = -Infinity;
  for (const interval of sorted) {
    if (interval.top > bottom) {
      if (preferred < interval.top) break;
      top = interval.top;
    }
    bottom = Math.max(bottom, interval.bottom);
  }
  if (preferred <= top || preferred >= bottom) return preferred;
  return preferred - top <= bottom - preferred ? top : bottom;
}
