import { useState } from "react";
import type { Cell, Screen } from "../../data/overview";
import { useI18n } from "../../i18n/LocaleContext";
import type { DesignFinding } from "../../model/audit";
import {
  resolveCardDiffImageUrl,
  type CanvasDiffVisualMode,
  type ScreenCanvasDiff,
} from "../../model/canvasDiff";
import {
  computeLiveFrameTransform,
  resolveLiveBridgeUrl,
  resolveLiveSurfaceUrl,
  type LiveSurfaceMode,
} from "../../model/liveSurface";

export function routeName(path: string) {
  return path.split("?")[0].split("/").filter(Boolean).slice(-2).join("/") || path;
}

export function CaptureCard({
  screen,
  cell,
  imageUrl,
  selected = false,
  related = false,
  dimmed = false,
  cardWidth,
  frameHeight,
  openComments = 0,
  designFindings = [],
  showDesignOverlay = false,
  screenDiff,
  diffVisualMode = "after",
  beforeRunKey,
  afterRunKey,
  liveBaseUrl,
  liveBridgePath,
  liveSurfaceMode = "capture",
}: {
  screen: Screen;
  cell?: Cell;
  imageUrl: (name: string) => string;
  selected?: boolean;
  related?: boolean;
  dimmed?: boolean;
  cardWidth?: number;
  frameHeight?: number;
  openComments?: number;
  designFindings?: DesignFinding[];
  showDesignOverlay?: boolean;
  screenDiff?: ScreenCanvasDiff;
  diffVisualMode?: CanvasDiffVisualMode;
  beforeRunKey?: string;
  afterRunKey?: string;
  liveBaseUrl?: string;
  liveBridgePath?: string;
  liveSurfaceMode?: LiveSurfaceMode;
}) {
  const { t } = useI18n();
  const [liveFallback, setLiveFallback] = useState(false);
  const [loadedSize, setLoadedSize] = useState<{ src: string; w: number; h: number } | null>(null);
  const liveUrl =
    liveSurfaceMode === "live" && !liveFallback
      ? (resolveLiveBridgeUrl(liveBridgePath, screen.path) ??
        resolveLiveSurfaceUrl(liveBaseUrl, screen.path))
      : null;
  const missing = screen.cells.filter((c) => c.status === "unverified").length;
  const unavailable = screen.cells.filter((c) => c.status === "unrenderable").length;
  const large = Boolean(cardWidth && cardWidth >= 800);
  const fallbackSrc = cell?.image ? imageUrl(cell.image) : undefined;
  const resolvedSrc = resolveCardDiffImageUrl(
    diffVisualMode,
    screenDiff,
    beforeRunKey,
    afterRunKey,
    fallbackSrc,
  );
  const activeLoaded = resolvedSrc && loadedSize?.src === resolvedSrc ? loadedSize : null;
  const naturalW = cell?.imageWidth ?? activeLoaded?.w;
  const naturalH = cell?.imageHeight ?? activeLoaded?.h;
  const effectiveFrameHeight =
    !liveUrl && cardWidth && naturalW && naturalH
      ? Math.max(240, Math.round(cardWidth * (naturalH / naturalW)))
      : frameHeight;
  const liveTransform =
    liveUrl && cardWidth && effectiveFrameHeight
      ? computeLiveFrameTransform(cell?.width ?? 1280, cardWidth, effectiveFrameHeight)
      : null;
  const diffClass = screenDiff ? ` diff-${screenDiff.kind}` : "";
  const renderChangeCount = screenDiff?.renderDiff?.changes.length ?? 0;
  const diffBadgeText = screenDiff
    ? screenDiff.kind === "added"
      ? t.diffAdded
      : screenDiff.kind === "removed"
        ? t.diffRemoved
        : renderChangeCount > 0
          ? `${t.diffChanged} · ${t.canvasDiffElementsCount(renderChangeCount)}`
          : screenDiff.maxDiffRatio !== null && screenDiff.maxDiffRatio > 0
            ? `${t.diffChanged} · ${(screenDiff.maxDiffRatio * 100).toFixed(1)}%`
            : t.diffChanged
    : null;
  const showRenderBoxes =
    diffVisualMode === "render" && Boolean(screenDiff && renderChangeCount > 0);
  const capW = Math.max(1, screenDiff?.captureWidth ?? naturalW ?? cell?.width ?? 1280);
  const capH = Math.max(1, screenDiff?.captureHeight ?? naturalH ?? effectiveFrameHeight ?? 800);
  return (
    <article
      className={
        "screen-card" +
        (selected ? " selected" : "") +
        (related ? " related" : "") +
        (dimmed ? " dimmed" : "") +
        (large ? " screen-card-large" : "") +
        diffClass
      }
      style={cardWidth ? { width: `${cardWidth}px` } : undefined}
      data-selected={selected ? "true" : undefined}
      data-related={related ? "true" : undefined}
      data-dimmed={dimmed ? "true" : undefined}
      data-diff-kind={screenDiff?.kind}
      data-card-width={cardWidth}
      data-open-comments={openComments > 0 ? openComments : undefined}
      title={screen.path}
    >
      <div
        className="capture-frame"
        style={
          cardWidth && effectiveFrameHeight
            ? { aspectRatio: `${cardWidth} / ${effectiveFrameHeight}` }
            : naturalW && naturalH
              ? { aspectRatio: `${naturalW} / ${naturalH}` }
              : undefined
        }
      >
        {liveUrl && liveTransform ? (
          <iframe
            className="live-surface-iframe"
            src={liveUrl}
            title={`Live ${screen.path}`}
            sandbox="allow-scripts allow-same-origin allow-forms"
            onError={() => setLiveFallback(true)}
            style={{
              width: `${liveTransform.viewportWidth}px`,
              height: `${liveTransform.viewportHeight}px`,
              transform: `scale(${liveTransform.scale})`,
            }}
          />
        ) : resolvedSrc ? (
          <img
            alt=""
            src={resolvedSrc}
            onLoad={(event) => {
              const img = event.currentTarget;
              if (img.naturalWidth > 0 && img.naturalHeight > 0) {
                setLoadedSize({
                  src: resolvedSrc,
                  w: img.naturalWidth,
                  h: img.naturalHeight,
                });
              }
            }}
          />
        ) : (
          <div className="missing-frame-copy">
            <span>{cell ? "Missing image" : "No capture at this width"}</span>
            {cell?.reason && <small>{cell.reason}</small>}
          </div>
        )}
        {liveUrl && <span className="card-live-pill">LIVE</span>}
        {showRenderBoxes && screenDiff?.renderDiff && (
          <div className="render-diff-box-layer" aria-hidden="true">
            {screenDiff.renderDiff.changes.slice(0, 24).map((chg) => {
              const rect = chg.afterRect ?? chg.beforeRect;
              if (!rect || rect.width <= 0 || rect.height <= 0) return null;
              const leftPct = Math.max(0, Math.min(100, (rect.x / capW) * 100));
              const topPct = Math.max(0, Math.min(100, (rect.y / capH) * 100));
              const wPct = Math.max(1, Math.min(100 - leftPct, (rect.width / capW) * 100));
              const hPct = Math.max(1, Math.min(100 - topPct, (rect.height / capH) * 100));
              const cls =
                chg.kind === "added"
                  ? "render-diff-box-added"
                  : chg.kind === "removed"
                    ? "render-diff-box-removed"
                    : "render-diff-box-changed";
              return (
                <span
                  key={`${chg.kind}:${chg.id}`}
                  className={`render-diff-box ${cls}`}
                  title={chg.summary}
                  style={{
                    left: `${leftPct}%`,
                    top: `${topPct}%`,
                    width: `${wPct}%`,
                    height: `${hPct}%`,
                  }}
                />
              );
            })}
          </div>
        )}
        {showDesignOverlay && designFindings.length > 0 && (
          <div className="render-diff-box-layer" aria-hidden="true">
            {designFindings.slice(0, 24).map((finding, idx) => {
              const rect = finding.rect;
              if (rect.width <= 0 || rect.height <= 0) return null;
              const baseW = Math.max(1, naturalW ?? cell?.width ?? capW);
              const baseH = Math.max(1, naturalH ?? effectiveFrameHeight ?? capH);
              const leftPct = Math.max(0, Math.min(100, (rect.x / baseW) * 100));
              const topPct = Math.max(0, Math.min(100, (rect.y / baseH) * 100));
              const wPct = Math.max(1, Math.min(100 - leftPct, (rect.width / baseW) * 100));
              const hPct = Math.max(1, Math.min(100 - topPct, (rect.height / baseH) * 100));
              return (
                <span
                  key={`${finding.kind}:${finding.nodeId}:${idx}`}
                  className={`render-diff-box design-lint-box-${finding.severity}`}
                  title={finding.summary}
                  style={{
                    left: `${leftPct}%`,
                    top: `${topPct}%`,
                    width: `${wPct}%`,
                    height: `${hPct}%`,
                  }}
                />
              );
            })}
          </div>
        )}
        {diffBadgeText && screenDiff && (
          <span className={`card-diff-pill card-diff-pill-${screenDiff.kind}`}>
            {diffBadgeText}
          </span>
        )}
      </div>
      <div className="card-caption">
        <strong>{routeName(screen.path)}</strong>
        {designFindings.length > 0 && (
          <small
            className="card-design-badge"
            title={t.auditDesignIssuesLabel(designFindings.length)}
          >
            {t.auditDesignIssuesLabel(designFindings.length)}
          </small>
        )}
        {openComments > 0 && (
          <small className="card-comment-badge" title={openComments + " unresolved comments"}>
            {t.comments} {openComments}
          </small>
        )}
        {missing > 0 && (
          <small title={missing + " unverified cells"}>
            {t.unverified} {missing}
          </small>
        )}
        {unavailable > 0 && (
          <small>
            {t.unrenderable} {unavailable}
          </small>
        )}
      </div>
    </article>
  );
}
