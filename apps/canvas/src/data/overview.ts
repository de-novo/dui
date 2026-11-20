export type Cell = {
  path: string;
  width: number;
  status: "verified" | "unverified" | "unrenderable";
  reason?: string;
  image?: string;
  state?: string;
  scenario?: string;
};
export type Screen = {
  id: string;
  path: string;
  cells: Cell[];
  entries?: Cell[];
};
export type ScreenMap = {
  screens: Screen[];
  links: { source: string; target: string; observedWidths?: number[] }[];
};

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function decodeOverview(raw: unknown): ScreenMap {
  if (
    !record(raw) ||
    !Array.isArray(raw.nodes) ||
    !Array.isArray(raw.overview) ||
    !Array.isArray(raw.edges)
  )
    throw Error("Invalid archive graph");
  const screens: Screen[] = [];
  const byPath = new Map<string, Screen>();
  for (const node of raw.nodes) {
    if (!record(node) || node.kind !== "screen") continue;
    if (typeof node.id !== "string" || typeof node.label !== "string")
      throw Error("Invalid screen");
    const screen: Screen = { id: node.id, path: node.label, cells: [] };
    byPath.set(screen.path, screen);
    screens.push(screen);
  }
  const detailed = raw.cells !== undefined;
  if (detailed && !Array.isArray(raw.cells)) throw Error("Invalid archive cells");
  const sources = detailed ? [raw.overview, raw.cells as unknown[]] : [raw.overview];
  for (const [index, source] of sources.entries()) {
    for (const cell of source) {
      if (
        !record(cell) ||
        typeof cell.path !== "string" ||
        typeof cell.width !== "number" ||
        !Number.isInteger(cell.width) ||
        cell.width <= 0 ||
        (cell.status !== "verified" &&
          cell.status !== "unverified" &&
          cell.status !== "unrenderable")
      )
        throw Error("Invalid capture in archive overview");
      const screen = byPath.get(cell.path);
      if (!screen) throw Error("Capture references a missing screen");
      const values = detailed && index === 0 ? (screen.entries ??= []) : screen.cells;
      values.push({
        state: typeof cell.state === "string" ? cell.state : undefined,
        scenario: typeof cell.scenario === "string" ? cell.scenario : undefined,
        path: cell.path,
        width: cell.width,
        status: cell.status,
        image: typeof cell.image === "string" ? cell.image : undefined,
        reason: typeof cell.reason === "string" ? cell.reason : undefined,
      });
    }
  }
  const ids = new Set(screens.map((s) => s.id));
  const links: ScreenMap["links"] = [];
  for (const edge of raw.edges) {
    if (!record(edge) || edge.kind !== "linksTo") continue;
    if (
      typeof edge.source !== "string" ||
      typeof edge.target !== "string" ||
      !ids.has(edge.source) ||
      !ids.has(edge.target)
    )
      throw Error("Invalid discovery link");
    const widths = edge.observedWidths;
    if (
      widths !== undefined &&
      (!Array.isArray(widths) ||
        !widths.every((w) => typeof w === "number" && Number.isInteger(w) && w > 0))
    )
      throw Error("Invalid discovery link widths");
    links.push({
      source: edge.source,
      target: edge.target,
      observedWidths: widths,
    });
  }
  return { screens, links };
}
