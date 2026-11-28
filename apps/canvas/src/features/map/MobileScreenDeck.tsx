import { ArrowRight, Search, Layers, Map as MapIcon } from "lucide-react";
import { DiscoveryLimitations, SourceCardBody } from "../../components/NativeSourceEvidence";
import { Button } from "../../components/ui/button";
import type { Screen, ScreenMap as Archive } from "../../data/overview";
import { useI18n } from "../../i18n/LocaleContext";
import {
  type CanvasDiffVisualMode,
  resolveCardDiffImageUrl,
  type ScreenCanvasDiff,
} from "../../model/canvasDiff";
import { captureBadgeCounts, routeName } from "./CaptureCard";
import { ApproximateModelFrame, useApproximateModelRender } from "../model/ApproximateModelFrame";
import type { Model, SourceModelRenderer } from "../model/modelDocument";
import type { SourceInputRenderContext } from "../model/sourceInputs";
import { liveCell, useLiveFrame } from "./liveFrames";
import { memo, useState } from "react";

/** A deck card without a capture, rendered from source when asked: rendering holds a browser. */
function LiveDeckFrame({
  run,
  cell,
  label,
}: {
  run: string;
  cell: { state: string; width: number };
  label: string;
}) {
  const { t } = useI18n();
  const [requested, setRequested] = useState(false);
  const live = useLiveFrame(run, cell, requested);
  return (
    <div className="mobile-deck-frame" data-showing={live.frame ? "live" : undefined}>
      {live.frame ? (
        <img src={live.frame.src} alt={label} />
      ) : live.pending || live.error ? (
        <div className="missing-frame-copy" aria-busy={live.pending || undefined}>
          <span>{live.error ? t.liveRenderFailed : t.liveRendering}</span>
          {live.error && <small>{live.error}</small>}
        </div>
      ) : (
        <div className="missing-frame-copy">
          <Button
            size="sm"
            variant="outline"
            data-live-render=""
            onClick={() => setRequested(true)}
          >
            {t.liveRender}
          </Button>
        </div>
      )}
    </div>
  );
}

const MobileApproximateCard = memo(
  function MobileApproximateCard({
    renderer,
    model,
    screen,
    width,
    height,
    imageUrl,
    input,
  }: {
    renderer?: SourceModelRenderer;
    model?: Model;
    screen: Screen;
    width: number;
    height?: number;
    imageUrl: (name: string) => string;
    input?: SourceInputRenderContext;
  }) {
    const { t } = useI18n();
    const render = useApproximateModelRender(
      renderer,
      model,
      screen.component,
      width,
      height,
      imageUrl,
      input,
    );
    if (!render) return null;
    return (
      <>
        <div className="mobile-deck-frame mobile-deck-frame-model" data-showing="model">
          <ApproximateModelFrame
            render={render}
            label={t.approximateModel}
            showUnresolved={false}
          />
        </div>
        {render.unresolved.length > 0 && (
          <div className="mobile-model-unresolved" title={render.unresolved.join("\n")}>
            {t.unresolvedModelParts(render.unresolved.length)}
          </div>
        )}
      </>
    );
  },
  (before, after) =>
    before.renderer === after.renderer &&
    before.model === after.model &&
    before.screen === after.screen &&
    before.width === after.width &&
    before.height === after.height &&
    before.imageUrl === after.imageUrl &&
    before.input === after.input,
);

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
  openSearch,
  onSelectMobileView,
  onSelectWidth,
  onSelectScreen,
  liveRun,
  sourceModel,
  modelRenderer,
  modelInput,
  viewportHeight,
}: {
  /** Render cards without a capture from source (the run's project can run). */
  liveRun?: string;
  sourceModel?: Model;
  modelRenderer?: SourceModelRenderer;
  /** Explicit archived fixture values for the selected source-input state. */
  modelInput?: SourceInputRenderContext;
  /** Explicit logical viewport height for source-model cards. */
  viewportHeight?: number;
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
  openSearch: () => void;
  onSelectMobileView: (next: MobileViewMode) => void;
  onSelectWidth: (nextWidth: number) => void;
  onSelectScreen: (id: string) => void;
}) {
  const { t } = useI18n();
  const screenById = new Map(archive.screens.map((s) => [s.id, s]));

  return (
    <div className="mobile-deck" aria-label={t.screenMap}>
      <div className="mobile-deck-toolbar glass-panel">
        <div className="mobile-view-switch" role="tablist" aria-label={t.mobileViewMode}>
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

        <Button variant="ghost" size="icon" aria-label={t.findScreen} onClick={openSearch}>
          <Search aria-hidden="true" />
        </Button>
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
        <DiscoveryLimitations limitations={archive.screenDiscovery?.limitations} />
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
          const { notCaptured, unverifiedAfterCapture, unavailable } = captureBadgeCounts(
            screen.cells,
          );
          const outgoing = archive.links
            .filter((l) => l.source === screen.id && l.target !== screen.id)
            .map((l) => screenById.get(l.target))
            .filter((target): target is Screen => Boolean(target))
            .filter((target, idx, arr) => arr.findIndex((x) => x.id === target.id) === idx)
            .slice(0, 4);

          return (
            <article
              key={screen.id}
              data-screen-id={screen.id}
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
                    <strong>
                      {screen.nativeSource?.name ?? screen.name ?? routeName(screen.path)}
                    </strong>
                  </span>
                  <span className="mobile-deck-meta">
                    {screenDiff && (
                      <span className={`mobile-diff-tag kind-${screenDiff.kind}`}>
                        {screenDiff.kind === "added"
                          ? t.diffAdded
                          : screenDiff.kind === "removed"
                            ? t.diffRemoved
                            : t.diffChanged}
                      </span>
                    )}
                    {cell?.width
                      ? `${cell.width}px`
                      : screen.nativeSource
                        ? null
                        : screen.cells.length}
                  </span>
                </div>

                {(notCaptured > 0 || unverifiedAfterCapture > 0 || unavailable > 0) && (
                  <div className="mobile-deck-status-row">
                    {notCaptured > 0 && (
                      <span>
                        {t.notCaptured} {notCaptured}
                      </span>
                    )}
                    {unverifiedAfterCapture > 0 && (
                      <span>
                        {t.unverifiedAfterCapture} {unverifiedAfterCapture}
                      </span>
                    )}
                    {unavailable > 0 && (
                      <span>
                        {t.unrenderable} {unavailable}
                      </span>
                    )}
                  </div>
                )}

                {!cell && !src && sourceModel && screen.component && (
                  <MobileApproximateCard
                    renderer={modelRenderer}
                    model={sourceModel}
                    screen={screen}
                    width={width && width > 0 ? width : 375}
                    height={viewportHeight ?? Math.round((width && width > 0 ? width : 375) * 1.6)}
                    imageUrl={imageUrl}
                    input={modelInput}
                  />
                )}
                {screen.nativeSource && !cell && !src && !screen.component && (
                  <div className="source-frame">
                    <SourceCardBody source={screen.nativeSource} preview={screen.sourcePreview} />
                  </div>
                )}
                {!src &&
                  !screenDiff &&
                  liveRun &&
                  (() => {
                    const live = liveCell(screen, cell, width ?? 0);
                    return live ? (
                      <LiveDeckFrame run={liveRun} cell={live} label={routeName(screen.path)} />
                    ) : null;
                  })()}
                {src && (
                  <div
                    className={
                      "mobile-deck-frame" + (screen.nativeSource ? " mobile-deck-frame-whole" : "")
                    }
                  >
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
