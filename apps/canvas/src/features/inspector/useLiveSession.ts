import { useCallback, useEffect, useRef, useState } from "react";
import {
  LiveError,
  closeLiveSession,
  createRequestId,
  inspectLiveSession,
  openLiveSession,
  resetLiveControl,
  saveLiveRevision,
  setLiveControl,
} from "../../data/live";
import type {
  LiveCellRef,
  LiveControl,
  LiveEditResult,
  LiveProvenance,
  LiveSaveResult,
} from "../../model/live";

export type LiveSessionState =
  | { phase: "opening" }
  | { phase: "error"; reason: string }
  | {
      phase: "ready";
      session: string;
      cell: LiveCellRef;
      revision: number;
      provenance: LiveProvenance;
      controls: LiveControl[];
      busy: boolean;
      lastEdit: LiveEditResult | null;
      lastError: string | null;
      lastSave: LiveSaveResult | null;
    };

export function useLiveSession(
  runKey: string,
  cell: LiveCellRef,
  onSaved?: (saved: LiveSaveResult) => void,
) {
  const cellState = cell.state;
  const cellWidth = cell.width;
  const [state, setState] = useState<LiveSessionState>({ phase: "opening" });
  const [attempt, setAttempt] = useState(0);
  const generation = useRef(0);
  const activeSession = useRef<string | null>(null);
  const revisionRef = useRef(0);
  const busyRef = useRef(false);

  const release = useCallback((sessionId: string | null) => {
    if (!sessionId) return;
    if (activeSession.current === sessionId) activeSession.current = null;
    void closeLiveSession(sessionId).catch(() => {});
  }, []);

  useEffect(() => {
    const token = ++generation.current;
    const controller = new AbortController();
    const targetCell: LiveCellRef = { state: cellState, width: cellWidth };
    busyRef.current = false;

    const requestId = createRequestId("open");
    let openedId: string | null = null;

    void (async () => {
      try {
        const opened = await openLiveSession(runKey, targetCell, requestId, controller.signal);
        openedId = opened.session;
        if (generation.current !== token) {
          release(opened.session);
          return;
        }
        activeSession.current = opened.session;
        revisionRef.current = opened.revision;
        const inspected = await inspectLiveSession(opened.session, targetCell, controller.signal);
        if (generation.current !== token) {
          release(opened.session);
          return;
        }
        revisionRef.current = inspected.revision;
        setState({
          phase: "ready",
          session: opened.session,
          cell: targetCell,
          revision: inspected.revision,
          provenance: opened.provenance,
          controls: inspected.controls,
          busy: false,
          lastEdit: null,
          lastError: null,
          lastSave: null,
        });
      } catch (error) {
        if (controller.signal.aborted || generation.current !== token) return;
        const reason = error instanceof LiveError ? error.reason : String(error);
        setState({ phase: "error", reason });
      }
    })();

    const onUnload = () => {
      const id = activeSession.current;
      if (!id) return;
      activeSession.current = null;
      void fetch("/api/live", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ op: "close", session: id }),
        keepalive: true,
      }).catch(() => {});
    };
    addEventListener("pagehide", onUnload);

    return () => {
      removeEventListener("pagehide", onUnload);
      generation.current += 1;
      controller.abort();
      release(openedId ?? activeSession.current);
    };
  }, [runKey, cellState, cellWidth, attempt, release]);

  const readySessionId = state.phase === "ready" ? state.session : null;

  useEffect(() => {
    if (!readySessionId) return;
    const targetCell: LiveCellRef = { state: cellState, width: cellWidth };
    const token = generation.current;
    const timer = setInterval(() => {
      if (busyRef.current || generation.current !== token) return;
      void inspectLiveSession(readySessionId, targetCell)
        .then((inspected) => {
          if (generation.current !== token || busyRef.current) return;
          if (inspected.revision !== revisionRef.current) {
            revisionRef.current = inspected.revision;
            setState((prev) =>
              prev.phase === "ready" && prev.session === readySessionId
                ? {
                    ...prev,
                    revision: inspected.revision,
                    controls: inspected.controls,
                    lastError: null,
                  }
                : prev,
            );
          }
        })
        .catch((error) => {
          if (generation.current !== token) return;
          const reason = error instanceof LiveError ? error.reason : "session-expired";
          if (reason === "session-expired" || reason === "live-page-expired") {
            activeSession.current = null;
            setState({ phase: "error", reason });
          }
        });
    }, 2500);
    return () => clearInterval(timer);
  }, [readySessionId, cellState, cellWidth]);

  async function mutate(
    op: "set" | "reset",
    slot: string,
    instance: string,
    value: unknown,
    expect?: { selector: string; text: string },
  ) {
    if (state.phase !== "ready" || busyRef.current) return;
    const sessionId = state.session;
    const targetCell: LiveCellRef = { state: cellState, width: cellWidth };
    const token = generation.current;
    const expectedRevision = revisionRef.current;
    const requestId = createRequestId(op);
    busyRef.current = true;
    setState((prev) => (prev.phase === "ready" ? { ...prev, busy: true, lastError: null } : prev));
    try {
      const result =
        op === "set"
          ? await setLiveControl({
              session: sessionId,
              cell: targetCell,
              slot,
              instance,
              value,
              expectedRevision,
              requestId,
              expect,
            })
          : await resetLiveControl({
              session: sessionId,
              cell: targetCell,
              slot,
              instance,
              expectedRevision,
              requestId,
              expect,
            });
      if (generation.current !== token) return;
      const inspected = await inspectLiveSession(sessionId, targetCell);
      if (generation.current !== token) return;
      revisionRef.current = inspected.revision;
      busyRef.current = false;
      setState((prev) =>
        prev.phase === "ready" && prev.session === sessionId
          ? {
              ...prev,
              revision: inspected.revision,
              controls: inspected.controls,
              busy: false,
              lastEdit: result,
              lastError: null,
            }
          : prev,
      );
    } catch (error) {
      if (generation.current !== token) return;
      busyRef.current = false;
      const reason = error instanceof LiveError ? error.reason : String(error);
      if (reason === "session-expired" || reason === "live-page-expired") {
        activeSession.current = null;
        setState({ phase: "error", reason });
        return;
      }
      setState((prev) =>
        prev.phase === "ready" ? { ...prev, busy: false, lastError: reason } : prev,
      );
    }
  }

  async function save() {
    if (state.phase !== "ready" || busyRef.current) return;
    const sessionId = state.session;
    const targetCell: LiveCellRef = { state: cellState, width: cellWidth };
    const token = generation.current;
    const expectedRevision = revisionRef.current;
    const requestId = createRequestId("save");
    busyRef.current = true;
    setState((prev) => (prev.phase === "ready" ? { ...prev, busy: true, lastError: null } : prev));
    try {
      const saved = await saveLiveRevision({
        session: sessionId,
        cell: targetCell,
        expectedRevision,
        requestId,
      });
      if (generation.current !== token) return;
      busyRef.current = false;
      setState((prev) =>
        prev.phase === "ready" && prev.session === sessionId
          ? { ...prev, busy: false, lastSave: saved, lastError: null }
          : prev,
      );
      onSaved?.(saved);
    } catch (error) {
      if (generation.current !== token) return;
      busyRef.current = false;
      const reason = error instanceof LiveError ? error.reason : String(error);
      if (reason === "session-expired" || reason === "live-page-expired") {
        activeSession.current = null;
        setState({ phase: "error", reason });
        return;
      }
      setState((prev) =>
        prev.phase === "ready" ? { ...prev, busy: false, lastError: reason } : prev,
      );
    }
  }

  function reopen() {
    setState({ phase: "opening" });
    setAttempt((n) => n + 1);
  }

  function close() {
    release(activeSession.current);
  }

  return { state, mutate, save, reopen, close };
}
