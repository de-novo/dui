import {
  decodeLiveEdit,
  decodeLiveInspect,
  decodeLiveOpen,
  decodeLiveSave,
  type LiveCellRef,
  type LiveEditResult,
  type LiveInspectResult,
  type LiveOpenResult,
  type LiveSaveResult,
} from "../model/live";

export class LiveError extends Error {
  reason: string;
  constructor(reason: string) {
    super(reason);
    this.name = "LiveError";
    this.reason = reason;
  }
}

async function postLive(payload: Record<string, unknown>, signal?: AbortSignal): Promise<unknown> {
  const response = await fetch("/api/live", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
    signal,
  });
  let body: unknown = null;
  try {
    body = await response.json();
  } catch {
    if (!response.ok) throw new LiveError(`live-http-${response.status}`);
  }
  if (!response.ok) {
    const reason =
      typeof body === "object" &&
      body !== null &&
      "reason" in body &&
      typeof body.reason === "string"
        ? body.reason
        : `live-http-${response.status}`;
    throw new LiveError(reason);
  }
  return body;
}

export function createRequestId(prefix: string): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export async function openLiveSession(
  runKey: string,
  cell: LiveCellRef,
  requestId: string,
  signal?: AbortSignal,
): Promise<LiveOpenResult> {
  const raw = await postLive({ op: "open", run: runKey, cell, requestId }, signal);
  return decodeLiveOpen(raw);
}

export async function inspectLiveSession(
  session: string,
  cell: LiveCellRef,
  signal?: AbortSignal,
): Promise<LiveInspectResult> {
  const raw = await postLive({ op: "inspect", session, cell }, signal);
  return decodeLiveInspect(raw);
}

export async function setLiveControl(
  params: {
    session: string;
    cell: LiveCellRef;
    slot: string;
    instance: string;
    value: unknown;
    expectedRevision: number;
    requestId: string;
    expect?: { selector: string; text: string };
  },
  signal?: AbortSignal,
): Promise<LiveEditResult> {
  const payload: Record<string, unknown> = {
    op: "set",
    session: params.session,
    cell: params.cell,
    slot: params.slot,
    instance: params.instance,
    value: params.value,
    expectedRevision: params.expectedRevision,
    requestId: params.requestId,
  };
  if (params.expect) payload.expect = params.expect;
  return decodeLiveEdit(await postLive(payload, signal));
}

export async function resetLiveControl(
  params: {
    session: string;
    cell: LiveCellRef;
    slot: string;
    instance: string;
    expectedRevision: number;
    requestId: string;
    expect?: { selector: string; text: string };
  },
  signal?: AbortSignal,
): Promise<LiveEditResult> {
  const payload: Record<string, unknown> = {
    op: "reset",
    session: params.session,
    cell: params.cell,
    slot: params.slot,
    instance: params.instance,
    expectedRevision: params.expectedRevision,
    requestId: params.requestId,
  };
  if (params.expect) payload.expect = params.expect;
  return decodeLiveEdit(await postLive(payload, signal));
}

export async function saveLiveRevision(
  params: {
    session: string;
    cell: LiveCellRef;
    expectedRevision: number;
    requestId: string;
  },
  signal?: AbortSignal,
): Promise<LiveSaveResult> {
  return decodeLiveSave(
    await postLive(
      {
        op: "save",
        session: params.session,
        cell: params.cell,
        expectedRevision: params.expectedRevision,
        requestId: params.requestId,
      },
      signal,
    ),
  );
}

export async function closeLiveSession(session: string): Promise<void> {
  await postLive({ op: "close", session });
}

export function liveFrameUrl(session: string, cell: LiveCellRef, revision: number): string {
  const query = new URLSearchParams({
    session,
    state: cell.state,
    width: String(cell.width),
    rev: String(revision),
  });
  return `/api/live/frame?${query.toString()}`;
}
