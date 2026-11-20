import { useState } from "react";
import type { Cell, Screen, ScreenMap } from "../../data/overview";
import { Button } from "../../components/ui/button";
import { CaptureCard } from "./CaptureCard";

export function ScreenNeighborhood({
  archive,
  screen,
  cell,
  width,
  imageUrl,
  select,
}: {
  archive: ScreenMap;
  screen: Screen;
  cell?: Cell;
  width: number;
  imageUrl: (name: string) => string;
  select: (id: string) => void;
}) {
  const [page, setPage] = useState(0);
  const links = archive.links.filter(
    (e) =>
      (e.target === screen.id || e.source === screen.id) &&
      (!e.observedWidths || e.observedWidths.includes(width)),
  );
  const adjacent = new Set(links.flatMap((e) => [e.source, e.target]));
  const outgoing = new Set(
    links
      .filter((link) => link.source === screen.id)
      .map((link) => link.target),
  );
  const neighbors = archive.screens
    .filter((s) => s.id !== screen.id && adjacent.has(s.id))
    .sort(
      (a, b) =>
        Number(outgoing.has(b.id)) - Number(outgoing.has(a.id)) ||
        a.path.localeCompare(b.path),
    );
  const shown = neighbors.slice(page * 2, page * 2 + 2);
  return (
    <section
      className="neighborhood"
      aria-label="Selected screen and its connections"
    >
      <div className={"relation-stage" + (!shown.length ? " isolated" : "")}>
        <div className="relation-focus">
          <CaptureCard screen={screen} cell={cell} imageUrl={imageUrl} />
        </div>
        {!!shown.length && (
          <>
            <svg
              className="relation-lines"
              viewBox="0 0 100 100"
              preserveAspectRatio="none"
              aria-hidden="true"
            >
              <defs>
                <marker
                  id="neighbor-arrow"
                  viewBox="0 0 10 10"
                  refX="9"
                  refY="5"
                  markerWidth="4"
                  markerHeight="4"
                  orient="auto-start-reverse"
                >
                  <path d="M0 0L10 5L0 10Z" />
                </marker>
              </defs>
              {shown.map((n, i) => (
                <path
                  key={n.id}
                  className="connector"
                  d={`M0 50 H50 V${shown.length === 1 ? 50 : i === 0 ? 25 : 75} H100`}
                  markerStart={
                    links.some((e) => e.source === n.id)
                      ? "url(#neighbor-arrow)"
                      : undefined
                  }
                  markerEnd={
                    links.some((e) => e.target === n.id)
                      ? "url(#neighbor-arrow)"
                      : undefined
                  }
                />
              ))}
            </svg>
            <div className="relation-neighbors">
              {shown.map((n) => (
                <button
                  key={n.id}
                  aria-label={"Inspect " + n.path}
                  onClick={() => select(n.id)}
                >
                  <CaptureCard
                    screen={n}
                    cell={(n.entries ?? n.cells).find((c) => c.width === width)}
                    imageUrl={imageUrl}
                  />
                </button>
              ))}
            </div>
          </>
        )}
      </div>
      <div className="relation-pager">
        {neighbors.length
          ? `Connected screens · ${page * 2 + 1}–${page * 2 + shown.length} of ${neighbors.length}`
          : "No connected screens at this width"}
        {neighbors.length > 2 && (
          <>
            <Button
              variant="outline"
              size="sm"
              aria-label="Previous connected screens"
              disabled={!page}
              onClick={() => setPage(page - 1)}
            >
              ‹
            </Button>
            <Button
              variant="outline"
              size="sm"
              aria-label="Next connected screens"
              disabled={(page + 1) * 2 >= neighbors.length}
              onClick={() => setPage(page + 1)}
            >
              ›
            </Button>
          </>
        )}
      </div>
    </section>
  );
}
