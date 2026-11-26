// Map cards without a capture, rendered from source in the running app through the live session
// (docs/en/web-viewer.md#shared-screen-maps). A card asks for its frame while it is in view; at
// most two renders run at once, and each finished frame is kept for the rest of the visit.
import { useEffect, useState } from "react";
import type { Cell, Screen } from "../../data/overview";
import { request } from "../inspector/ResponsivePreview";

/** Renders running at once: each opens a page in the project's shared browser. */
const CONCURRENT = 2;
/** Frames are at most this tall, the live session's largest size. */
const MAX_HEIGHT = 4096;

export type LiveFrame = { src: string; width: number; height: number };
type Entry = { promise: Promise<LiveFrame>; frame?: LiveFrame; error?: string };

const frames = new Map<string, Entry>();
const queue: (() => void)[] = [];
let running = 0;

function next() {
  while (running < CONCURRENT && queue.length) {
    running += 1;
    queue.shift()!();
  }
}

async function render(run: string, cell: { state: string; width: number }): Promise<LiveFrame> {
  const opened = await request<{ session: string }>({
    op: "open",
    run,
    cell,
    requestId: crypto.randomUUID(),
  });
  const address = { session: opened.session, cell };
  try {
    // The whole page, as a capture would show it, not just the first screen.
    const layout = await request<{ height: number; scrollHeight: number }>({
      ...address,
      op: "layout",
    });
    const height = Math.min(MAX_HEIGHT, Math.max(layout.height, layout.scrollHeight));
    if (height > layout.height)
      await request({
        ...address,
        op: "resize",
        height,
        expectedRevision: 0,
        requestId: crypto.randomUUID(),
      });
    const frame = await request<{ imageBase64: string }>({ ...address, op: "frame" });
    const src = "data:image/png;base64," + frame.imageBase64;
    const image = new Image();
    image.src = src;
    await image.decode();
    return { src, width: image.naturalWidth, height: image.naturalHeight };
  } finally {
    void request({ op: "close", session: opened.session }).catch(() => {});
  }
}

/** The frame for `cell` of `run`, rendered once and then reused. */
function liveFrame(run: string, cell: { state: string; width: number }): Entry {
  const key = JSON.stringify([run, cell.state, cell.width]);
  let entry = frames.get(key);
  if (!entry) {
    const created: Entry = {
      promise: new Promise<LiveFrame>((resolve, reject) => {
        queue.push(() => {
          render(run, cell)
            .then((frame) => {
              created.frame = frame;
              resolve(frame);
            })
            .catch((e: unknown) => {
              created.error = e instanceof Error ? e.message : String(e);
              reject(e);
            })
            .finally(() => {
              running -= 1;
              next();
            });
        });
        next();
      }),
    };
    created.promise.catch(() => {});
    frames.set(key, created);
    entry = created;
  }
  return entry;
}

/** The capture a card would show when it has none: the screen's first state at that width. */
export function liveCell(screen: Screen, shown: Cell | undefined, width: number) {
  if (shown?.image || screen.nativeSource) return undefined;
  const cell = screen.cells.find(
    (c) => c.width === (shown?.width ?? width) && c.state && c.scenario !== "live",
  );
  return cell?.state ? { state: cell.state, width: cell.width } : undefined;
}

/** A card's live frame once `wanted`: rendering, rendered, or why it could not be. */
export function useLiveFrame(
  run: string | undefined,
  cell: { state: string; width: number } | undefined,
  wanted: boolean,
): { frame?: LiveFrame; error?: string; pending: boolean } {
  const [, settle] = useState(0);
  const state = cell?.state;
  const width = cell?.width;
  useEffect(() => {
    if (!wanted || !run || !state || !width) return;
    const entry = liveFrame(run, { state, width });
    if (entry.frame || entry.error) return;
    let current = true;
    const done = () => {
      if (current) settle((n) => n + 1);
    };
    entry.promise.then(done, done);
    return () => {
      current = false;
    };
  }, [wanted, run, state, width]);
  const entry = run && state && width ? frames.get(JSON.stringify([run, state, width])) : undefined;
  return {
    frame: entry?.frame,
    error: entry?.error,
    pending: Boolean(wanted && run && state && width && !entry?.frame && !entry?.error),
  };
}
