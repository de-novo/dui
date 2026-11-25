import type { RenderNode } from "../data/overview";

export type Rect = { x: number; y: number; width: number; height: number };

export type SpacingGuide = {
  axis: "x" | "y";
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  distance: number;
};

export type BoxModelRects = {
  margin: Rect;
  border: Rect;
  padding: Rect;
  content: Rect;
};

/** Pick the first visible element inside the capture viewport, falling back to `nodes[0]`. */
export function pickInitialNode(nodes: RenderNode[], viewportWidth: number): RenderNode | null {
  if (nodes.length === 0) return null;
  const visible = nodes.find((node) => {
    const r = node.rect;
    return (
      r.width > 0 &&
      r.height > 0 &&
      r.y >= 0 &&
      r.x + r.width > 0 &&
      r.x < Math.max(1, viewportWidth)
    );
  });
  return visible ?? nodes[0] ?? null;
}

/** Pick the deepest (smallest area) rendered element containing `(x, y)` in capture coordinates. */
export function pickNodeAtPoint(nodes: RenderNode[], x: number, y: number): RenderNode | null {
  let best: RenderNode | null = null;
  let bestArea = Number.POSITIVE_INFINITY;
  for (const node of nodes) {
    const r = node.rect;
    if (r.width <= 0 || r.height <= 0) continue;
    if (x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height) {
      const area = r.width * r.height;
      if (area <= bestArea) {
        bestArea = area;
        best = node;
      }
    }
  }
  return best;
}

/** Compute margin, border, padding, and content rectangles for a node's DevTools box-model overlay. */
export function boxModelOverlayRects(node: RenderNode): BoxModelRects {
  const r = node.rect;
  const m = node.boxModel?.margin ?? { top: 0, right: 0, bottom: 0, left: 0 };
  const b = node.boxModel?.border ?? { top: 0, right: 0, bottom: 0, left: 0 };
  const p = node.boxModel?.padding ?? { top: 0, right: 0, bottom: 0, left: 0 };
  const margin: Rect = {
    x: r.x - m.left,
    y: r.y - m.top,
    width: Math.max(0, r.width + m.left + m.right),
    height: Math.max(0, r.height + m.top + m.bottom),
  };
  const border: Rect = { ...r };
  const padding: Rect = {
    x: r.x + b.left,
    y: r.y + b.top,
    width: Math.max(0, r.width - b.left - b.right),
    height: Math.max(0, r.height - b.top - b.bottom),
  };
  const content: Rect = {
    x: padding.x + p.left,
    y: padding.y + p.top,
    width: Math.max(0, padding.width - p.left - p.right),
    height: Math.max(0, padding.height - p.top - p.bottom),
  };
  return { margin, border, padding, content };
}

/** Compute redline distance measurement guides between a selected element and a target element. */
export function computeSpacingGuides(selected: RenderNode, target: RenderNode): SpacingGuide[] {
  if (selected.id === target.id) return [];
  const a = selected.rect;
  const b = target.rect;
  const guides: SpacingGuide[] = [];

  const aRight = a.x + a.width;
  const aBottom = a.y + a.height;
  const bRight = b.x + b.width;
  const bBottom = b.y + b.height;

  const midX = Math.round(a.x + a.width / 2);
  const midY = Math.round(a.y + a.height / 2);

  // Check if target contains selected -> measure inset distances to all 4 inner edges
  if (b.x <= a.x && b.y <= a.y && bRight >= aRight && bBottom >= aBottom) {
    if (a.y - b.y > 0) {
      guides.push({ axis: "y", x1: midX, y1: b.y, x2: midX, y2: a.y, distance: a.y - b.y });
    }
    if (bBottom - aBottom > 0) {
      guides.push({
        axis: "y",
        x1: midX,
        y1: aBottom,
        x2: midX,
        y2: bBottom,
        distance: bBottom - aBottom,
      });
    }
    if (a.x - b.x > 0) {
      guides.push({ axis: "x", x1: b.x, y1: midY, x2: a.x, y2: midY, distance: a.x - b.x });
    }
    if (bRight - aRight > 0) {
      guides.push({
        axis: "x",
        x1: aRight,
        y1: midY,
        x2: bRight,
        y2: midY,
        distance: bRight - aRight,
      });
    }
    return guides;
  }

  // Horizontal separation
  if (aRight < b.x) {
    guides.push({ axis: "x", x1: aRight, y1: midY, x2: b.x, y2: midY, distance: b.x - aRight });
  } else if (bRight < a.x) {
    guides.push({ axis: "x", x1: bRight, y1: midY, x2: a.x, y2: midY, distance: a.x - bRight });
  }

  // Vertical separation
  if (aBottom < b.y) {
    guides.push({ axis: "y", x1: midX, y1: aBottom, x2: midX, y2: b.y, distance: b.y - aBottom });
  } else if (bBottom < a.y) {
    guides.push({ axis: "y", x1: midX, y1: bBottom, x2: midX, y2: a.y, distance: a.y - bBottom });
  }

  return guides;
}
