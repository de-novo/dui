export type Cell = {
  path: string;
  width: number;
  status: "verified" | "unverified" | "unrenderable";
  reason?: string;
  image?: string;
};
export type Screen = { id: string; path: string; cells: Cell[] };
export type ScreenMap = {
  screens: Screen[];
  links: { source: string; target: string }[];
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
  for (const cell of raw.overview) {
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
    screen.cells.push({
      path: cell.path,
      width: cell.width,
      status: cell.status,
      image: typeof cell.image === "string" ? cell.image : undefined,
      reason: typeof cell.reason === "string" ? cell.reason : undefined,
    });
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
    links.push({ source: edge.source, target: edge.target });
  }
  return { screens, links };
}
