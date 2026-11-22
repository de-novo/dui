import type { Screen, ScreenMap } from "../data/overview";
import type { CaptureDiff, CellDiffEntry } from "./diff";

export type CanvasDiffVisualMode = "after" | "before" | "overlay";

export type ScreenCanvasDiff = {
  path: string;
  kind: "changed" | "added" | "removed";
  changedCount: number;
  maxDiffRatio: number | null;
  beforeImage?: string;
  afterImage?: string;
  overlayImage?: string;
  changedFiles: string[];
  entries: CellDiffEntry[];
};

export function indexScreenDiffs(
  diff: CaptureDiff | null,
  width: number,
): Record<string, ScreenCanvasDiff> {
  if (!diff) return {};
  const map: Record<string, ScreenCanvasDiff> = {};
  for (const group of diff.screens) {
    if (group.cells.length === 0) continue;
    const allAdded = group.cells.every((c) => c.kind === "added");
    const allRemoved = group.cells.every((c) => c.kind === "removed");
    const kind: "changed" | "added" | "removed" = allAdded
      ? "added"
      : allRemoved
        ? "removed"
        : "changed";

    const widthMatches = group.cells.filter((c) => c.width === width);
    const preferred =
      widthMatches.find((c) => c.pixelDiff?.overlayImage) ??
      widthMatches.find((c) => c.after?.image || c.before?.image) ??
      group.cells.find((c) => c.pixelDiff?.overlayImage) ??
      group.cells[0];

    let maxDiffRatio: number | null = null;
    const files = new Set<string>();
    for (const cell of group.cells) {
      if (cell.pixelDiff) {
        maxDiffRatio =
          maxDiffRatio === null
            ? cell.pixelDiff.diffRatio
            : Math.max(maxDiffRatio, cell.pixelDiff.diffRatio);
      }
      for (const f of cell.changedFiles) files.add(f);
    }

    map[group.path] = {
      path: group.path,
      kind,
      changedCount: group.cells.length,
      maxDiffRatio,
      beforeImage: preferred?.before?.image,
      afterImage: preferred?.after?.image,
      overlayImage: preferred?.pixelDiff?.overlayImage,
      changedFiles: [...files].sort(),
      entries: group.cells,
    };
  }
  return map;
}

export function filterArchiveForDiff(
  archive: ScreenMap,
  diffsByPath: Record<string, ScreenCanvasDiff>,
  diffActive: boolean,
  changedOnly: boolean,
): ScreenMap {
  if (!diffActive) return archive;
  const existingPaths = new Set(archive.screens.map((s) => s.path));
  const syntheticRemoved: Screen[] = [];
  for (const entry of Object.values(diffsByPath)) {
    if (entry.kind !== "removed" || existingPaths.has(entry.path)) continue;
    syntheticRemoved.push({
      id: `route:${entry.path}`,
      path: entry.path,
      discoveredFrom: { kind: "entry" },
      cells: entry.entries.map((c) => ({
        path: entry.path,
        scenario: c.scenario,
        state: c.state,
        width: c.width,
        locale: c.locale,
        status: c.before?.status === "verified" ? "verified" : "unverified",
        reason: c.before?.reason,
        image: c.before?.image,
      })),
    });
  }

  const combinedScreens = [...archive.screens, ...syntheticRemoved];
  if (!changedOnly) {
    return syntheticRemoved.length > 0 ? { ...archive, screens: combinedScreens } : archive;
  }

  const filteredScreens = combinedScreens.filter((s) => Boolean(diffsByPath[s.path]));
  const keptIds = new Set(filteredScreens.map((s) => s.id));
  const filteredLinks = archive.links.filter((l) => keptIds.has(l.source) && keptIds.has(l.target));
  return {
    ...archive,
    screens: filteredScreens,
    links: filteredLinks,
  };
}

export function resolveCardDiffImageUrl(
  mode: CanvasDiffVisualMode,
  screenDiff: ScreenCanvasDiff | undefined,
  beforeRunKey: string | undefined,
  afterRunKey: string | undefined,
  fallbackImageUrl?: string,
): string | undefined {
  if (!screenDiff) return fallbackImageUrl;
  const archiveUrl = (name: string, runKey: string) =>
    `/archive/${encodeURIComponent(name)}?run=${encodeURIComponent(runKey)}`;

  if (mode === "overlay" && screenDiff.overlayImage) {
    return `/api/diff-overlay?file=${encodeURIComponent(screenDiff.overlayImage)}`;
  }
  if (mode === "before" && screenDiff.beforeImage && beforeRunKey) {
    return archiveUrl(screenDiff.beforeImage, beforeRunKey);
  }
  if (screenDiff.kind === "removed" && screenDiff.beforeImage && beforeRunKey) {
    return archiveUrl(screenDiff.beforeImage, beforeRunKey);
  }
  if (screenDiff.afterImage && afterRunKey) {
    return archiveUrl(screenDiff.afterImage, afterRunKey);
  }
  return fallbackImageUrl;
}
