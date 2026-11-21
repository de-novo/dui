import { ArrowLeft, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "../../components/ui/button";
import { liveFrameUrl } from "../../data/live";
import type { Screen } from "../../data/overview";
import { provenanceDiffers } from "../../model/live";
import type { LiveSessionState } from "./useLiveSession";

function shortCommit(commit: string | null): string {
  return commit ? commit.slice(0, 7) : "uncommitted";
}

export function LiveStage({
  screen,
  width,
  session,
  onBackToOverview,
  onBackToDetails,
  onReopen,
}: {
  screen: Screen;
  width: number;
  session: LiveSessionState;
  onBackToOverview: () => void;
  onBackToDetails: () => void;
  onReopen: () => void;
}) {
  const [frameState, setFrameState] = useState<{
    sessionId: string;
    loadedUrl: string | null;
    failedUrl: string | null;
  }>({ sessionId: "", loadedUrl: null, failedUrl: null });

  const sessionId = session.phase === "ready" ? session.session : "";
  const targetUrl =
    session.phase === "ready"
      ? liveFrameUrl(session.session, session.cell, session.revision)
      : null;

  useEffect(() => {
    if (!targetUrl || !sessionId) return;
    let cancelled = false;
    const probe = new Image();
    probe.onload = () => {
      if (cancelled) return;
      setFrameState({ sessionId, loadedUrl: targetUrl, failedUrl: null });
    };
    probe.onerror = () => {
      if (cancelled) return;
      setFrameState((prev) => ({
        sessionId,
        loadedUrl: prev.sessionId === sessionId ? prev.loadedUrl : null,
        failedUrl: targetUrl,
      }));
    };
    probe.src = targetUrl;
    return () => {
      cancelled = true;
    };
  }, [sessionId, targetUrl]);

  const displayedUrl =
    targetUrl && frameState.sessionId === sessionId ? frameState.loadedUrl : null;
  const frameFailed =
    targetUrl !== null && frameState.sessionId === sessionId && frameState.failedUrl === targetUrl;
  const loadingFrame = targetUrl !== null && displayedUrl !== targetUrl && !frameFailed;

  const showLiveProvenance = session.phase === "ready" && provenanceDiffers(session.provenance);

  return (
    <section className="live-stage" aria-label="Live screen preview">
      <header className="live-pill glass-panel">
        <Button variant="ghost" size="sm" onClick={onBackToOverview}>
          <ArrowLeft aria-hidden="true" />
          Overview
        </Button>
        <span className="pill-divider" aria-hidden="true" />
        <Button variant="ghost" size="sm" onClick={onBackToDetails}>
          Archive
        </Button>
        <span className="pill-divider" aria-hidden="true" />
        <span className="live-badge" data-phase={session.phase}>
          <span className="live-dot" aria-hidden="true" />
          {session.phase === "opening"
            ? "Starting…"
            : session.phase === "ready"
              ? `Live · ${width} px`
              : "Disconnected"}
        </span>
      </header>

      {showLiveProvenance && (
        <output className="provenance-banner glass-panel">
          Live worktree ({shortCommit(session.provenance.live.commit)}
          {session.provenance.live.dirty ? " dirty" : ""}) differs from archive (
          {shortCommit(session.provenance.archive.commit)})
        </output>
      )}

      <div className="live-viewport">
        {session.phase === "opening" && (
          <output className="live-state-card glass-panel" aria-live="polite">
            <h3>Starting live session…</h3>
            <p>
              Launching <code>{screen.path}</code> at {width} px.
            </p>
            <div className="live-state-actions">
              <Button variant="outline" size="sm" onClick={onBackToDetails}>
                Cancel
              </Button>
            </div>
          </output>
        )}

        {session.phase === "error" && (
          <div className="live-state-card glass-panel" role="alert">
            <h3>Live session unavailable</h3>
            <p className="capture-meta">{session.reason}</p>
            <div className="live-state-actions">
              <Button variant="default" size="sm" onClick={onReopen}>
                <RefreshCw aria-hidden="true" />
                Reopen
              </Button>
              <Button variant="outline" size="sm" onClick={onBackToDetails}>
                Back to archive
              </Button>
            </div>
          </div>
        )}

        {session.phase === "ready" && (
          <div className="live-frame-shell" style={{ maxWidth: `${Math.max(width, 320)}px` }}>
            {displayedUrl ? (
              <img
                className={loadingFrame ? "frame-refreshing" : undefined}
                alt={`Live view of ${screen.path} at ${width} px`}
                src={displayedUrl}
              />
            ) : frameFailed ? (
              <div className="missing-capture-box" role="alert">
                <strong>Could not capture the live frame.</strong>
                <Button variant="outline" size="sm" onClick={onReopen}>
                  Reopen
                </Button>
              </div>
            ) : (
              <output className="missing-capture-box">
                <strong>Loading live preview…</strong>
              </output>
            )}
          </div>
        )}
      </div>
    </section>
  );
}
