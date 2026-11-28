import type { Cell, Screen, ScreenLink, ScreenMap } from "../data/overview";

export type ViewMode = "overview" | "details" | "live";
/** Where map link edges start: the anchor element on the capture, or the card border. */
export type EdgeStart = "element" | "card";

export type ViewerSelection = {
  screen: string | null;
  state: string | null;
  width: number;
  /** Optional explicit logical viewport height for source-model cards. */
  height: number | null;
  widths: number[];
  locale: string | null;
  locales: string[];
  view: ViewMode;
  links: string;
  edges: EdgeStart;
};

export function archiveWidths(archive: ScreenMap): number[] {
  return [...new Set(archive.screens.flatMap((s) => s.cells.map((c) => c.width)))].sort(
    (a, b) => b - a,
  );
}

export function archiveLocales(archive: ScreenMap): string[] {
  return [
    ...new Set(
      archive.screens.flatMap((s) =>
        s.cells.map((c) => c.locale).filter((l): l is string => typeof l === "string"),
      ),
    ),
  ].sort();
}

export function parseSelection(search: string, archive: ScreenMap): ViewerSelection {
  const params = new URLSearchParams(search);
  const widths = archiveWidths(archive);
  const locales = archiveLocales(archive);
  const rawWidth = params.get("width");
  const parsedWidth = rawWidth !== null ? Number(rawWidth) : NaN;
  const width = Number.isInteger(parsedWidth) && parsedWidth > 0 ? parsedWidth : (widths[0] ?? 0);
  const rawHeight = params.get("height");
  const parsedHeight = rawHeight !== null ? Number(rawHeight) : NaN;
  const height = Number.isInteger(parsedHeight) && parsedHeight > 0 ? parsedHeight : null;
  const rawLocale = params.get("locale");
  const locale = rawLocale && locales.includes(rawLocale) ? rawLocale : null;
  const screen = params.get("screen");
  const state = params.get("state");
  const links = params.get("links") ?? "overview";
  const edges: EdgeStart = params.get("edges") === "card" ? "card" : "element";
  const rawView = params.get("view");
  let view: ViewMode;
  if (rawView === "overview") view = "overview";
  else if (rawView === "live" && screen !== null) view = "live";
  else if (screen !== null) view = "details";
  else view = "overview";
  return { screen, state, width, height, widths, locale, locales, view, links, edges };
}

export function stateKeyFor(cells: Cell[], index: number): string {
  const item = cells[index];
  if (!item) return "cell:" + index;
  return item.state && cells.filter((c) => c.state === item.state).length === 1
    ? item.state
    : "cell:" + index;
}

/**
 * The width a screen is shown at. A native source declaration is recorded once per runtime at
 * its device's own width, so with no capture at the selected width it keeps its recorded width.
 */
export function screenWidth(screen: Screen | undefined, width: number): number {
  const cells = screen?.entries ?? screen?.cells ?? [];
  if (!screen?.nativeSource || cells.some((c) => c.width === width)) return width;
  return cells.find((c) => c.image)?.width ?? width;
}

/** The capture a map card shows for a screen at the selected width. */
export function cardCell(screen: Screen, width: number): Cell | undefined {
  const shown = screenWidth(screen, width);
  return (screen.entries ?? screen.cells).find((c) => c.width === shown);
}

export function resolveCell(
  screen: Screen | undefined,
  width: number,
  state: string | null,
  locale: string | null = null,
): { cells: Cell[]; cell: Cell | undefined } {
  const shown = screenWidth(screen, width);
  const cells =
    screen?.cells.filter((c) => c.width === shown && (locale === null || c.locale === locale)) ??
    [];
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
