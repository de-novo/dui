import { useEffect, useSyncExternalStore } from "react";
import type { CellSnapshot } from "../../data/overview";
import { SNAPSHOT_MAX_MISMATCH } from "../../model/snapshotFidelity";

export { SNAPSHOT_MAX_MISMATCH };

/** Cards at least this wide on screen draw their HTML snapshot (docs/en/web-viewer.md#html-cards). */
export const SNAPSHOT_MIN_SCREEN_WIDTH = 320;
/** At most this many snapshot frames are mounted at once. */
export const SNAPSHOT_MAX_FRAMES = 12;

export type SnapshotChoice =
  | { show: "snapshot" }
  | { show: "png"; why: "none" | "fidelity" | "unmeasured" | "small" | "busy" };

/** Whether a snapshot may replace the PNG at all, regardless of zoom and frame budget. */
export function snapshotUsable(snapshot: CellSnapshot | undefined): SnapshotChoice {
  if (!snapshot) return { show: "png", why: "none" };
  if (snapshot.fidelity === undefined) return { show: "png", why: "unmeasured" };
  if (snapshot.fidelity > SNAPSHOT_MAX_MISMATCH) return { show: "png", why: "fidelity" };
  return { show: "snapshot" };
}

// Frames are granted in request order; a released frame goes to the oldest waiting card.
const granted = new Set<string>();
const waiting: string[] = [];
const listeners = new Set<() => void>();
const notify = () => listeners.forEach((listener) => listener());
function request(id: string) {
  if (granted.has(id) || waiting.includes(id)) return;
  if (granted.size < SNAPSHOT_MAX_FRAMES) granted.add(id);
  else waiting.push(id);
  notify();
}
function release(id: string) {
  const at = waiting.indexOf(id);
  if (at >= 0) waiting.splice(at, 1);
  if (granted.delete(id)) {
    const next = waiting.shift();
    if (next) granted.add(next);
  }
  notify();
}
function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Holds a snapshot frame for `id` while `wanted`; true once the frame is granted. */
export function useSnapshotFrame(id: string, wanted: boolean): boolean {
  useEffect(() => {
    if (!wanted) return;
    request(id);
    return () => release(id);
  }, [id, wanted]);
  return useSyncExternalStore(subscribe, () => wanted && granted.has(id));
}

/** Whether the map currently shows the HTML snapshot of card `id`. */
export function useSnapshotShown(id: string | undefined): boolean {
  return useSyncExternalStore(subscribe, () => Boolean(id && granted.has(id)));
}
