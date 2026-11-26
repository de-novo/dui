// Which captures a width preview lays out (docs/en/web-viewer.md#width-preview).
import type { Cell } from "../data/overview";
import { SNAPSHOT_MAX_MISMATCH } from "./snapshotFidelity.ts";

/** Captures of the same state, scenario and locale whose snapshot can be shown. */
export function widthPreviewSources(cells: Cell[], cell: Cell | undefined): Cell[] {
  if (!cell) return [];
  return cells
    .filter(
      (c) =>
        c.state === cell.state &&
        c.scenario === cell.scenario &&
        c.locale === cell.locale &&
        c.snapshot?.fidelity !== undefined &&
        c.snapshot.fidelity <= SNAPSHOT_MAX_MISMATCH,
    )
    .sort((a, b) => a.width - b.width);
}

/** The captured width nearest `width` lays the preview out; ties go to the wider capture. */
export function nearestSource(sources: Cell[], width: number): Cell | undefined {
  let best: Cell | undefined;
  for (const c of sources) {
    if (!best || Math.abs(c.width - width) <= Math.abs(best.width - width)) best = c;
  }
  return best;
}
