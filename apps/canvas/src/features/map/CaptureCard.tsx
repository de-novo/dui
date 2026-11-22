import type { Cell, Screen } from "../../data/overview";
import { useI18n } from "../../i18n/LocaleContext";
import {
  resolveCardDiffImageUrl,
  type CanvasDiffVisualMode,
  type ScreenCanvasDiff,
} from "../../model/canvasDiff";

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
  const diffClass = screenDiff ? ` diff-${screenDiff.kind}` : "";
  const diffBadgeText = screenDiff
    ? screenDiff.kind === "added"
      ? t.diffAdded
      : screenDiff.kind === "removed"
        ? t.diffRemoved
        : screenDiff.maxDiffRatio !== null && screenDiff.maxDiffRatio > 0
          ? `${t.diffChanged} · ${(screenDiff.maxDiffRatio * 100).toFixed(1)}%`
          : t.diffChanged
    : null;
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
          cardWidth && frameHeight ? { aspectRatio: `${cardWidth} / ${frameHeight}` } : undefined
        }
      >
        {resolvedSrc ? (
          <img loading="lazy" alt="" src={resolvedSrc} />
        ) : (
          <div className="missing-frame-copy">
            <span>{cell ? "Missing image" : "No capture at this width"}</span>
            {cell?.reason && <small>{cell.reason}</small>}
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
