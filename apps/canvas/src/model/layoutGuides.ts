import type { Cell, RenderNode, Screen } from "../data/overview";

export type AlignmentEdge = "middle" | "top" | "bottom" | "center" | "left" | "right";
export type AlignmentChange = {
  first: RenderNode;
  second: RenderNode;
  edge: AlignmentEdge;
  offset: number;
};

export function edgePosition({ rect }: RenderNode, edge: AlignmentEdge) {
  switch (edge) {
    case "middle":
      return rect.y + rect.height / 2;
    case "top":
      return rect.y;
    case "bottom":
      return rect.y + rect.height;
    case "center":
      return rect.x + rect.width / 2;
    case "left":
      return rect.x;
    case "right":
      return rect.x + rect.width;
  }
}

const controlTags = new Set(["button", "a", "input", "select", "textarea", "summary"]);
const controlRoles = new Set([
  "button",
  "link",
  "tab",
  "menuitem",
  "checkbox",
  "radio",
  "switch",
  "combobox",
  "textbox",
]);

export function isMeasuredControl(node: RenderNode) {
  return (
    node.rect.width > 0 &&
    node.rect.height > 0 &&
    (controlTags.has(node.tag) || controlRoles.has(node.role ?? ""))
  );
}

function indexedControls(cell: Cell) {
  const controls = new Map<string, RenderNode | null>();
  for (const node of cell.renderTree ?? []) {
    if (!isMeasuredControl(node)) continue;
    const a = node.attributes;
    const testId = a?.["data-testid"];
    const id = a?.id || (node.id.startsWith("#") ? node.id.slice(1) : undefined);
    const label = a?.["aria-label"] || a?.name || node.text;
    if (!testId && !id && !label && !a?.href) continue;
    const identity = testId ? ["testId", testId] : id ? ["id", id] : ["control", label, a?.href];
    const key = JSON.stringify([node.tag, node.role, identity]);
    controls.set(key, controls.has(key) ? null : node);
  }
  return controls;
}

function peers(a: RenderNode, b: RenderNode) {
  if (a.parentId && a.parentId === b.parentId) return true;
  const parent = a.attributes?.["data-dui-dom-parent"];
  return Boolean(parent && b.attributes?.["data-dui-dom-parent"] === parent);
}

const axes: { start: "x" | "y"; size: "width" | "height"; edges: AlignmentEdge[] }[] = [
  { start: "x", size: "width", edges: ["middle", "bottom", "top"] },
  { start: "y", size: "height", edges: ["center", "left", "right"] },
];

export type AlignmentReport = { widths: number[]; compared: number; changes: AlignmentChange[] };
type Evidence = { cell: Cell; controls: ReturnType<typeof indexedControls> };

function order(a: RenderNode, b: RenderNode, start: "x" | "y", size: "width" | "height") {
  if (a.rect[start] + a.rect[size] <= b.rect[start]) return -1;
  if (b.rect[start] + b.rect[size] <= a.rect[start]) return 1;
  return 0;
}

function offset(a: RenderNode, b: RenderNode, edge: AlignmentEdge) {
  return Math.abs(edgePosition(a, edge) - edgePosition(b, edge));
}

// A change needs independent evidence on both sides of the inspected width.
// Capture rectangles are rounded to whole CSS pixels.
function compareNeighbors(before: Evidence, current: Evidence, after: Evidence): AlignmentReport {
  const destination = ({ cell }: Evidence) =>
    cell.screenEvidence?.observedPath ?? cell.redirectedTo ?? cell.path;
  const widths = [before.cell.width, after.cell.width];
  if (destination(before) !== destination(current) || destination(after) !== destination(current)) {
    return { widths, compared: 0, changes: [] };
  }
  const matches = [...current.controls].flatMap(([key, node]) => {
    const a = before.controls.get(key),
      b = after.controls.get(key);
    return node && a && b ? [{ node, before: a, after: b }] : [];
  });
  const changes: AlignmentChange[] = [];
  let compared = 0;
  for (const [i, a] of matches.entries()) {
    for (const b of matches.slice(i + 1)) {
      if (!peers(a.node, b.node) || !peers(a.before, b.before) || !peers(a.after, b.after))
        continue;
      for (const { start, size, edges } of axes) {
        const firstOrder = order(a.before, b.before, start, size);
        if (!firstOrder || order(a.after, b.after, start, size) !== firstOrder) continue;
        const shared = edges.filter(
          (edge) => offset(a.before, b.before, edge) <= 1 && offset(a.after, b.after, edge) <= 1,
        );
        if (!shared.length) continue;
        compared++;
        if (shared.some((edge) => offset(a.node, b.node, edge) <= 1)) continue;
        const edge = shared[0];
        changes.push({ first: a.node, second: b.node, edge, offset: offset(a.node, b.node, edge) });
      }
    }
  }
  return {
    widths,
    compared,
    changes: changes.sort((a, b) => b.offset - a.offset),
  };
}

export function inspectAlignmentSeries(cells: readonly Cell[]) {
  const groups = new Map<string, Map<number, Evidence | null>>();
  for (const cell of cells) {
    const key = JSON.stringify([cell.path, cell.scenario, cell.state, cell.locale]);
    const widths = groups.get(key) ?? new Map<number, Evidence | null>();
    const usable =
      !widths.has(cell.width) &&
      cell.status !== "unrenderable" &&
      cell.screenEvidence?.routeMatches !== false &&
      !(cell.screenEvidence?.documentStatus && cell.screenEvidence.documentStatus >= 400) &&
      cell.renderTree?.length;
    widths.set(cell.width, usable ? { cell, controls: indexedControls(cell) } : null);
    groups.set(key, widths);
  }
  const reports = new Map<Cell, AlignmentReport>();
  for (const group of groups.values()) {
    const ordered = [...group].sort(([a], [b]) => a - b).map(([, evidence]) => evidence);
    for (let i = 1; i < ordered.length - 1; i++) {
      const before = ordered[i - 1],
        current = ordered[i],
        after = ordered[i + 1];
      if (before && current && after)
        reports.set(current.cell, compareNeighbors(before, current, after));
    }
  }
  return reports;
}

// Overview entries may only carry the image and width. Match the image being painted
// to a detailed cell before using its geometry or comparing neighboring widths.
export function mapEvidenceCell(screen: Screen, entry: Cell, image: string) {
  const original = image === entry.image;
  if (original && screen.cells.includes(entry))
    return entry.status !== "unrenderable" && entry.renderTree?.length ? entry : undefined;
  const matches = screen.cells.filter(
    (cell) =>
      cell.path === entry.path &&
      cell.width === entry.width &&
      cell.image === image &&
      (!original ||
        (["state", "scenario", "locale"] as const).every(
          (field) => entry[field] === undefined || entry[field] === cell[field],
        )),
  );
  const fallback = original ? entry : undefined;
  const [match] = matches;
  const cell = matches.length === 1 && match.renderTree?.length ? match : fallback;
  return cell?.renderTree?.length && cell.status !== "unrenderable" ? cell : undefined;
}

export type GuideLine = { vertical: boolean; position: number };

// Pick the most widely shared row and column of non-overlapping sibling controls.
// Rectangles come from one capture, so only exactly coincident edges count here.
export function sharedControlGuides(nodes: readonly RenderNode[]): GuideLine[] {
  const controls = nodes.filter(isMeasuredControl);
  const lines: GuideLine[] = [];
  for (const vertical of [false, true]) {
    const edges: AlignmentEdge[] = vertical
      ? ["left", "center", "right"]
      : ["middle", "top", "bottom"];
    const groups = new Map<string, { position: number; spans: { start: number; end: number }[] }>();
    for (const node of controls) {
      const parent = node.attributes?.["data-dui-dom-parent"] ?? node.parentId;
      if (!parent) continue;
      const start = vertical ? node.rect.y : node.rect.x;
      const end = start + (vertical ? node.rect.height : node.rect.width);
      for (const edge of edges) {
        const position = edgePosition(node, edge);
        const key = JSON.stringify([parent, edge, position]);
        const group = groups.get(key) ?? { position, spans: [] };
        group.spans.push({ start, end });
        groups.set(key, group);
      }
    }
    let most = 1;
    let winner: GuideLine | undefined;
    for (const { position, spans } of groups.values()) {
      if (spans.length <= most) continue;
      let count = 0;
      let lastEnd = -Infinity;
      for (const span of spans.sort((a, b) => a.end - b.end)) {
        if (span.start < lastEnd) continue;
        lastEnd = span.end;
        count++;
      }
      if (count > most) {
        most = count;
        winner = { vertical, position };
      }
    }
    if (winner) lines.push(winner);
  }
  return lines;
}
