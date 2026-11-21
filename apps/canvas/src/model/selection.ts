import type { Cell, Screen, ScreenLink, ScreenMap } from "../data/overview";

export type ViewMode = "overview" | "details" | "live";

export type ViewerSelection = {
  screen: string | null;
  state: string | null;
  width: number;
  widths: number[];
  view: ViewMode;
  links: string;
};

export function archiveWidths(archive: ScreenMap): number[] {
  return [...new Set(archive.screens.flatMap((s) => s.cells.map((c) => c.width)))].sort(
    (a, b) => b - a,
  );
}

export function parseSelection(search: string, archive: ScreenMap): ViewerSelection {
  const params = new URLSearchParams(search);
  const widths = archiveWidths(archive);
  const rawWidth = params.get("width");
  const parsedWidth = rawWidth !== null ? Number(rawWidth) : NaN;
  const width = Number.isInteger(parsedWidth) && parsedWidth > 0 ? parsedWidth : (widths[0] ?? 0);
  const screen = params.get("screen");
  const state = params.get("state");
  const links = params.get("links") ?? "overview";
  const rawView = params.get("view");
  let view: ViewMode;
  if (rawView === "overview") view = "overview";
  else if (rawView === "live" && screen !== null) view = "live";
  else if (screen !== null) view = "details";
  else view = "overview";
  return { screen, state, width, widths, view, links };
}

export function stateKeyFor(cells: Cell[], index: number): string {
  const item = cells[index];
  if (!item) return "cell:" + index;
  return item.state && cells.filter((c) => c.state === item.state).length === 1
    ? item.state
    : "cell:" + index;
}

export function resolveCell(
  screen: Screen | undefined,
  width: number,
  state: string | null,
): { cells: Cell[]; cell: Cell | undefined } {
  const cells = screen?.cells.filter((c) => c.width === width) ?? [];
  const cell =
    state === null ? cells[0] : cells.find((_, index) => stateKeyFor(cells, index) === state);
  return { cells, cell };
}

export type ScreenConnection = {
  direction: "outgoing" | "incoming";
  screen: Screen;
  link: ScreenLink;
};

export function screenConnections(archive: ScreenMap, screenId: string): ScreenConnection[] {
  const byId = new Map(archive.screens.map((s) => [s.id, s]));
  const result: ScreenConnection[] = [];
  for (const link of archive.links) {
    if (link.source === screenId) {
      const target = byId.get(link.target);
      if (target) result.push({ direction: "outgoing", screen: target, link });
    } else if (link.target === screenId) {
      const source = byId.get(link.source);
      if (source) result.push({ direction: "incoming", screen: source, link });
    }
  }
  return result.sort(
    (a, b) =>
      Number(b.direction === "outgoing") - Number(a.direction === "outgoing") ||
      a.screen.path.localeCompare(b.screen.path),
  );
}

export function withSearchChanges(href: string, changes: Record<string, string | null>): URL {
  const next = new URL(href);
  for (const [key, value] of Object.entries(changes)) {
    if (value === null) next.searchParams.delete(key);
    else next.searchParams.set(key, value);
  }
  return next;
}
