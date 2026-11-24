import { ArrowRight, Layers, Map as MapIcon } from "lucide-react";
import type { Screen, ScreenMap as Archive } from "../../data/overview";
import { useI18n } from "../../i18n/LocaleContext";
import {
  type CanvasDiffVisualMode,
  resolveCardDiffImageUrl,
  type ScreenCanvasDiff,
} from "../../model/canvasDiff";
import { routeName } from "./CaptureCard";

export type MobileViewMode = "deck" | "map";

function preferredCellForWidth(screen: Screen, width?: number) {
  if (width !== undefined) {
    const exact = screen.cells.find((c) => c.width === width && c.image);
    if (exact) return exact;
  }
  return screen.cells.find((c) => c.image) ?? screen.cells[0];
}

export function MobileScreenDeck({
  archive,
  width,
  widths,
  selectedId,
  diffsByPath,
  diffActive,
  diffVisualMode,
  beforeRunKey,
  afterRunKey,
  imageUrl,
  mobileView,
  onSelectMobileView,
  onSelectWidth,
  onSelectScreen,
}: {
  archive: Archive;
  width?: number;
  widths: number[];
  selectedId: string | null;
  diffsByPath: Record<string, ScreenCanvasDiff>;
  diffActive: boolean;
  diffVisualMode: CanvasDiffVisualMode;
  beforeRunKey?: string;
  afterRunKey: string;
  imageUrl: (name: string) => string;
  mobileView: MobileViewMode;
  onSelectMobileView: (next: MobileViewMode) => void;
  onSelectWidth: (nextWidth: number) => void;
  onSelectScreen: (id: string) => void;
}) {
  const { t } = useI18n();
  const screenById = new Map(archive.screens.map((s) => [s.id, s]));

  return (
    <div className="mobile-deck" aria-label={t.screenMap}>
      <div className="mobile-deck-toolbar glass-panel">
        <div className="mobile-view-switch" role="tablist" aria-label="Mobile view mode">
          <button
            type="button"
            role="tab"
            aria-selected={mobileView === "deck"}
            className={"mobile-view-tab" + (mobileView === "deck" ? " active" : "")}
            onClick={() => onSelectMobileView("deck")}
          >
            <Layers aria-hidden="true" />
            <span>{t.mobileViewCards}</span>
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={mobileView === "map"}
            className={"mobile-view-tab" + (mobileView === "map" ? " active" : "")}
            onClick={() => onSelectMobileView("map")}
          >
            <MapIcon aria-hidden="true" />
            <span>{t.mobileViewMap}</span>
          </button>
        </div>

        {widths.length > 0 && (
          <div className="mobile-width-pills" aria-label={t.width}>
            {widths.map((w) => (
              <button
                key={w}
                type="button"
                className={"mobile-width-pill" + (w === width ? " active" : "")}
                aria-pressed={w === width}
                onClick={() => onSelectWidth(w)}
              >
                {w}px
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="mobile-deck-list">
        {archive.screens.map((screen) => {
          const cell = preferredCellForWidth(screen, width);
          const isOverlay = screen.path.includes("#overlay:");
          const screenDiff = diffActive ? diffsByPath[screen.path] : undefined;
          const fallbackSrc = cell?.image ? imageUrl(cell.image) : undefined;
          const src = screenDiff
            ? resolveCardDiffImageUrl(
                diffVisualMode,
                screenDiff,
                beforeRunKey,
                afterRunKey,
                fallbackSrc,
              )
            : fallbackSrc;
          const outgoing = archive.links
            .filter((l) => l.source === screen.id && l.target !== screen.id)
            .map((l) => screenById.get(l.target))
            .filter((target): target is Screen => Boolean(target))
            .filter((target, idx, arr) => arr.findIndex((x) => x.id === target.id) === idx)
            .slice(0, 4);

          return (
            <article
              key={screen.id}
              className={
                "mobile-deck-card" +
                (selectedId === screen.id ? " selected" : "") +
                (screenDiff ? ` diff-${screenDiff.kind}` : "")
              }
            >
              <button
                type="button"
                className="mobile-deck-card-hit"
                onClick={() => onSelectScreen(screen.id)}
              >
                <div className="mobile-deck-card-head">
                  <span className="mobile-deck-route">
                    {isOverlay && <span className="mobile-modal-tag">MODAL</span>}
                    <strong>{routeName(screen.path)}</strong>
                  </span>
                  <span className="mobile-deck-meta">
                    {screenDiff && (
                      <span className={`mobile-diff-tag kind-${screenDiff.kind}`}>
                        {screenDiff.kind}
                      </span>
                    )}
                    {cell?.width ? `${cell.width}px` : `${screen.cells.length}`}
                  </span>
                </div>

                {src && (
                  <div className="mobile-deck-frame">
                    <img src={src} alt={routeName(screen.path)} loading="lazy" />
                  </div>
                )}
              </button>

              {outgoing.length > 0 && (
                <div className="mobile-deck-links">
                  {outgoing.map((target) => (
                    <button
                      key={target.id}
                      type="button"
                      className="mobile-deck-link-chip"
                      onClick={() => onSelectScreen(target.id)}
                    >
                      <ArrowRight aria-hidden="true" />
                      <span>{routeName(target.path)}</span>
                    </button>
                  ))}
                </div>
              )}
            </article>
          );
        })}
      </div>
    </div>
  );
}
