import { useState } from "react";
import type { Cell, Screen } from "../../data/overview";
import { useI18n } from "../../i18n/LocaleContext";
import {
  resolveCardDiffImageUrl,
  type CanvasDiffVisualMode,
  type ScreenCanvasDiff,
} from "../../model/canvasDiff";

export function isOverlayPath(path: string): boolean {
  return path.includes("#overlay:");
}

export function routeName(path: string) {
  const overlayIdx = path.indexOf("#overlay:");
  if (overlayIdx >= 0) {
    const slug = path.slice(overlayIdx + "#overlay:".length);
    return slug.replace(/-/g, " ") || "Overlay";
  }
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
  screenDiff,
  diffVisualMode = "after",
  beforeRunKey,
  afterRunKey,
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
  screenDiff?: ScreenCanvasDiff;
  diffVisualMode?: CanvasDiffVisualMode;
  beforeRunKey?: string;
  afterRunKey?: string;
}) {
  const { t } = useI18n();
  const [loadedSize, setLoadedSize] = useState<{ src: string; w: number; h: number } | null>(null);
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
  const capH = Math.max(1, screenDiff?.captureHeight ?? naturalH ?? frameHeight ?? 800);
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
          cardWidth && frameHeight
            ? { aspectRatio: `${cardWidth} / ${frameHeight}` }
            : naturalW && naturalH
              ? { aspectRatio: `${naturalW} / ${naturalH}` }
              : undefined
        }
      >
        {resolvedSrc ? (
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
        {isOverlayPath(screen.path) && <span className="card-overlay-pill">MODAL</span>}
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
        {diffBadgeText && screenDiff && (
          <span className={`card-diff-pill card-diff-pill-${screenDiff.kind}`}>
            {diffBadgeText}
          </span>
        )}
      </div>
      <div className="card-caption">
        <strong>{routeName(screen.path)}</strong>
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
