import type { Cell, Screen } from "../../data/overview";

export function routeName(path: string) {
  return path.split("?")[0].split("/").filter(Boolean).slice(-2).join("/") || path;
}

export function CaptureCard({
  screen,
  cell,
  imageUrl,
}: {
  screen: Screen;
  cell?: Cell;
  imageUrl: (name: string) => string;
}) {
  const missing = screen.cells.filter((c) => c.status === "unverified").length;
  const unavailable = screen.cells.filter((c) => c.status === "unrenderable").length;
  return (
    <article className="screen-card" title={screen.path}>
      <div className="capture-frame">
        {cell?.image ? (
          <img loading="lazy" alt="" src={imageUrl(cell.image)} />
        ) : (
          <span>No capture at this width</span>
        )}
      </div>
      <div className="card-caption">
        <strong>{routeName(screen.path)}</strong>
        {missing > 0 && <small title={missing + " unverified cells"}>Unverified {missing}</small>}
        {unavailable > 0 && <small>Unrenderable {unavailable}</small>}
      </div>
    </article>
  );
}
