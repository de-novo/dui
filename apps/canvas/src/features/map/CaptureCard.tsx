import type { Cell, Screen } from "../../data/overview";

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
}) {
  const missing = screen.cells.filter((c) => c.status === "unverified").length;
  const unavailable = screen.cells.filter((c) => c.status === "unrenderable").length;
  const large = Boolean(cardWidth && cardWidth >= 800);
  return (
    <article
      className={
        "screen-card" +
        (selected ? " selected" : "") +
        (related ? " related" : "") +
        (dimmed ? " dimmed" : "") +
        (large ? " screen-card-large" : "")
      }
      style={cardWidth ? { width: `${cardWidth}px` } : undefined}
      data-selected={selected ? "true" : undefined}
      data-related={related ? "true" : undefined}
      data-dimmed={dimmed ? "true" : undefined}
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
        {cell?.image ? (
          <img loading="lazy" alt="" src={imageUrl(cell.image)} />
        ) : (
          <div className="missing-frame-copy">
            <span>{cell ? "Missing image" : "No capture at this width"}</span>
            {cell?.reason && <small>{cell.reason}</small>}
          </div>
        )}
      </div>
      <div className="card-caption">
        <strong>{routeName(screen.path)}</strong>
        {openComments > 0 && (
          <small className="card-comment-badge" title={openComments + " unresolved comments"}>
            Comments {openComments}
          </small>
        )}
        {missing > 0 && <small title={missing + " unverified cells"}>Unverified {missing}</small>}
        {unavailable > 0 && <small>Unrenderable {unavailable}</small>}
      </div>
    </article>
  );
}
