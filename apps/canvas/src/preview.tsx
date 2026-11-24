import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { Button } from "./components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "./components/ui/dialog";
import {
  archiveFileUrl,
  auditApiUrl,
  diffApiUrl,
  isStaticArchiveMode,
  runsIndexUrl,
} from "./data/archiveUrls";
import { decodeOverview, type ScreenMap as Archive } from "./data/overview";
import type { Run } from "./data/run";
import { CaptureDiffView } from "./features/diff/CaptureDiffView";
import { ScreenInspector } from "./features/inspector/ScreenInspector";
import { routeName } from "./features/map/CaptureCard";
import { MobileScreenDeck, type MobileViewMode } from "./features/map/MobileScreenDeck";
import { ScreenMap } from "./features/map/ScreenMap";
import { WorkspaceRail } from "./features/workspace/WorkspaceRail";
import { WorkPicker } from "./features/workspace/WorkPicker";
import { LocaleProvider, useI18n } from "./i18n/LocaleContext";
import { type AuditReport, decodeAuditReport, openCommentsByPath } from "./model/audit";
import {
  type CanvasDiffVisualMode,
  filterArchiveForDiff,
  indexScreenDiffs,
} from "./model/canvasDiff";
import { type CaptureDiff, decodeCaptureDiff } from "./model/diff";
import { parseSelection, resolveCell, withSearchChanges } from "./model/selection";
import "./preview.css";

function archiveUrl(name: string, run: Run) {
  return archiveFileUrl(name, run.key);
}

function MapView({
  archive,
  run,
  runs,
  onOpenDiff,
}: {
  archive: Archive;
  run: Run;
  runs: Run[];
  onOpenDiff: () => void;
}) {
  const mapArea = useRef<HTMLDivElement>(null);
  const overviewLayer = useRef<HTMLDivElement>(null);
  const [railInset, setRailInset] = useState<number>();
  useLayoutEffect(() => {
    const area = mapArea.current;
    const layer = overviewLayer.current;
    if (!area || !layer) return;
    const measure = () => setRailInset(layer.clientWidth - area.clientWidth);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(area);
    observer.observe(layer);
    return () => observer.disconnect();
  }, []);

  const { t } = useI18n();
  const [search, setSearch] = useState(location.search);
  const selection = parseSelection(search, archive);
  const {
    screen: selectedId,
    state,
    width,
    widths,
    locale,
    locales,
    view,
    links: mode,
  } = selection;
  const screen = archive.screens.find((s) => s.id === selectedId);
  const { cells, cell } = resolveCell(screen, width, state, locale);

  const [optionsOpen, setOptionsOpen] = useState(false);
  const [isMobile, setIsMobile] = useState(() => matchMedia("(max-width: 768px)").matches);
  const [mobileView, setMobileView] = useState<MobileViewMode>("deck");
  useEffect(() => {
    const mq = matchMedia("(max-width: 768px)");
    const onChange = () => setIsMobile(mq.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);
  const [audit, setAudit] = useState<AuditReport | null>(null);
  const openCommentCount = audit?.summary.openComments ?? 0;
  const showAuditPill = openCommentCount > 0;
  const [compareBeforeKey, setCompareBeforeKey] = useState<string>(() => {
    const fromUrl = new URLSearchParams(location.search).get("compare");
    return fromUrl ?? "";
  });
  const [changedOnly, setChangedOnly] = useState(false);
  const [diffVisualMode, setDiffVisualMode] = useState<CanvasDiffVisualMode>("render");
  const [canvasDiffState, setCanvasDiffState] = useState<{
    pairKey: string;
    diff: CaptureDiff | null;
  }>({ pairKey: "", diff: null });
  const optionsButton = useRef<HTMLButtonElement>(null);
  const dismiss = useRef<HTMLButtonElement>(null);
  const origin = useRef<HTMLElement | SVGElement | null>(null);
  const previousView = useRef(view);

  const staticDemo = isStaticArchiveMode();
  const otherRuns = useMemo(
    () => runs.filter((r) => r.key !== run.key && r.projectId === run.projectId),
    [runs, run.key, run.projectId],
  );
  const wantPixelOverlay = diffVisualMode === "overlay";
  const activePairKey = compareBeforeKey ? `${compareBeforeKey}::${run.key}` : "";
  const activeDiff =
    activePairKey && canvasDiffState.pairKey.startsWith(activePairKey)
      ? canvasDiffState.diff
      : null;
  const diffActive = Boolean(compareBeforeKey && activeDiff);

  useEffect(() => {
    if (!compareBeforeKey) return;
    const reqKey = `${compareBeforeKey}::${run.key}:${wantPixelOverlay ? "px" : "rt"}`;
    const controller = new AbortController();
    fetch(diffApiUrl(compareBeforeKey, run.key, wantPixelOverlay), {
      cache: "no-store",
      signal: controller.signal,
    })
      .then((res) => (res.ok ? res.json() : null))
      .then((raw) => {
        if (!controller.signal.aborted) {
          setCanvasDiffState({ pairKey: reqKey, diff: raw ? decodeCaptureDiff(raw) : null });
        }
      })
      .catch(() => {
        if (!controller.signal.aborted) setCanvasDiffState({ pairKey: reqKey, diff: null });
      });
    return () => controller.abort();
  }, [compareBeforeKey, run.key, wantPixelOverlay]);

  const diffsByPath = indexScreenDiffs(activeDiff, width);
  const visibleArchive = filterArchiveForDiff(archive, diffsByPath, diffActive, changedOnly);
  const selectedScreen = visibleArchive.screens.find((s) => s.id === selectedId) ?? screen;
  const selectedDiff = selectedScreen ? diffsByPath[selectedScreen.path] : undefined;

  useEffect(() => {
    const controller = new AbortController();
    fetch(auditApiUrl(run.key), {
      cache: "no-store",
      signal: controller.signal,
    })
      .then((res) => (res.ok ? res.json() : null))
      .then((raw) => {
        if (!controller.signal.aborted) setAudit(decodeAuditReport(raw));
      })
      .catch(() => {
        if (!controller.signal.aborted) setAudit(null);
      });
    return () => controller.abort();
  }, [run.key]);

  useEffect(() => {
    const pop = () => setSearch(location.search);
    addEventListener("popstate", pop);
    return () => removeEventListener("popstate", pop);
  }, []);

  useEffect(() => {
    const prev = previousView.current;
    previousView.current = view;
    if (view === "details") {
      if (prev !== view) dismiss.current?.focus();
    } else if (prev !== "overview") {
      const target = selectedId
        ? Array.from(
            overviewLayer.current?.querySelectorAll<HTMLElement>(".react-flow__node") ?? [],
          ).find((node) => node.dataset.id === selectedId)
        : null;
      (target ?? origin.current)?.focus();
    }
  }, [view, selectedId]);

  function update(changes: Record<string, string | null>) {
    const next = withSearchChanges(location.href, changes);
    history.pushState(null, "", next);
    setSearch(next.search);
  }

  function returnToOverview() {
    if (selectedId) {
      update({ view: "overview" });
    } else {
      update({ screen: null, state: null, view: null });
    }
  }

  function inspectKey(event: React.KeyboardEvent) {
    if (event.key === "Escape") returnToOverview();
  }

  function select(id: string, active: Element | null = document.activeElement) {
    if (view === "overview" && (active instanceof HTMLElement || active instanceof SVGElement)) {
      origin.current = active;
    }
    update({ screen: id, state: null, view: null });
  }

  const showDetails = view === "details" && selectedId !== null;
  const showOverview = !showDetails;
  const showMobileDeck = isMobile && mobileView === "deck";

  return (
    <section
      className={"workspace" + (showMobileDeck ? " mobile-deck-active" : "")}
      aria-label={t.screenMap}
    >
      <div className="map" ref={mapArea}>
        {showMobileDeck ? (
          <MobileScreenDeck
            archive={visibleArchive}
            width={width}
            widths={widths}
            selectedId={selectedId}
            diffsByPath={diffsByPath}
            diffActive={diffActive}
            diffVisualMode={diffVisualMode}
            beforeRunKey={compareBeforeKey || undefined}
            afterRunKey={run.key}
            imageUrl={(name) => archiveUrl(name, run)}
            mobileView={mobileView}
            onSelectMobileView={setMobileView}
            onSelectWidth={(nextWidth) => update({ width: String(nextWidth), state: null })}
            onSelectScreen={(id) => select(id)}
          />
        ) : (
          <div ref={overviewLayer} className="overview-layer">
            {railInset !== undefined && (
              <ScreenMap
                railInset={railInset}
                archive={visibleArchive}
                width={width}
                mode={mode}
                selectedId={selectedId}
                focusActive={showDetails}
                openCommentsByPath={openCommentsByPath(audit)}
                diffsByPath={diffsByPath}
                diffActive={diffActive}
                diffVisualMode={diffVisualMode}
                beforeRunKey={compareBeforeKey || undefined}
                afterRunKey={run.key}
                imageUrl={(name) => archiveUrl(name, run)}
                select={select}
                onClearSelection={returnToOverview}
              />
            )}
          </div>
        )}

        {(showAuditPill ||
          otherRuns.length > 0 ||
          (staticDemo && runs.length > 1) ||
          (isMobile && mobileView === "map")) && (
          <div className="map-top-bar">
            {isMobile && mobileView === "map" && (
              <div className="mobile-map-return glass-panel">
                <div className="mobile-view-switch" role="tablist" aria-label="Mobile view mode">
                  <button
                    type="button"
                    role="tab"
                    aria-selected={false}
                    className="mobile-view-tab"
                    onClick={() => setMobileView("deck")}
                  >
                    <span>{t.mobileViewCards}</span>
                  </button>
                  <button
                    type="button"
                    role="tab"
                    aria-selected={true}
                    className="mobile-view-tab active"
                    onClick={() => setMobileView("map")}
                  >
                    <span>{t.mobileViewMap}</span>
                  </button>
                </div>
              </div>
            )}
            {staticDemo && runs.length > 1 && (
              <section
                className="canvas-diff-bar glass-panel showcase-project-bar"
                aria-label="Showcase projects"
              >
                <label className="canvas-diff-select-wrap">
                  <span>Project</span>
                  <select
                    aria-label="Showcase project"
                    value={run.key}
                    onChange={(e) => {
                      location.href = `./?run=${encodeURIComponent(e.target.value)}`;
                    }}
                  >
                    {runs.map((candidate) => (
                      <option key={candidate.key} value={candidate.key}>
                        {candidate.project} · {candidate.branch ?? candidate.version}
                      </option>
                    ))}
                  </select>
                </label>
                <a className="canvas-diff-chip" href="../" style={{ textDecoration: "none" }}>
                  ← dui.sh
                </a>
              </section>
            )}

            {showAuditPill && (
              <output className="audit-summary-pill glass-panel">
                <span className="audit-open-comments">
                  {t.auditOpenCommentsLabel(openCommentCount)}
                </span>
              </output>
            )}

            {otherRuns.length > 0 && (
              <section className="canvas-diff-bar glass-panel" aria-label={t.diffTitle}>
                <label className="canvas-diff-select-wrap">
                  <span>{t.canvasDiffCompare}</span>
                  <select
                    aria-label={t.canvasDiffCompare}
                    value={compareBeforeKey}
                    onChange={(e) => {
                      const nextKey = e.target.value;
                      setCompareBeforeKey(nextKey);
                      update({ compare: nextKey || null });
                    }}
                  >
                    <option value="">{t.canvasDiffOff}</option>
                    {otherRuns.map((candidate) => (
                      <option key={candidate.key} value={candidate.key}>
                        {candidate.project} · {candidate.branch ?? "—"} ·{" "}
                        {candidate.version.slice(-6)}
                      </option>
                    ))}
                  </select>
                </label>

                {compareBeforeKey && (
                  <>
                    <button
                      type="button"
                      className={"canvas-diff-chip" + (changedOnly ? " active" : "")}
                      aria-pressed={changedOnly}
                      onClick={() => setChangedOnly((v) => !v)}
                    >
                      {t.canvasDiffChangedOnly}
                    </button>

                    <div className="canvas-diff-mode-group">
                      {(
                        [
                          ["render", t.canvasDiffModeRender],
                          ["after", t.canvasDiffModeAfter],
                          ["before", t.canvasDiffModeBefore],
                          ["overlay", t.canvasDiffModeOverlay],
                        ] as const
                      ).map(([modeKey, label]) => (
                        <button
                          key={modeKey}
                          type="button"
                          className={
                            "canvas-diff-mode-btn" + (diffVisualMode === modeKey ? " active" : "")
                          }
                          aria-pressed={diffVisualMode === modeKey}
                          onClick={() => setDiffVisualMode(modeKey)}
                        >
                          {label}
                        </button>
                      ))}
                    </div>

                    {activeDiff && (
                      <span className="canvas-diff-counts">
                        {activeDiff.summary.totalChanged === 0
                          ? t.canvasDiffNoChanges
                          : t.canvasDiffSummary(
                              activeDiff.summary.changed,
                              activeDiff.summary.added,
                              activeDiff.summary.removed,
                            )}
                      </span>
                    )}
                  </>
                )}
              </section>
            )}
          </div>
        )}

        {showOverview && screen && (
          <section className="preserved-selection glass-panel" aria-label="Selected screen">
            <span className="preserved-label">
              Selected: <code>{routeName(screen.path)}</code>
            </span>
            <Button variant="outline" size="sm" onClick={() => update({ view: null })}>
              Inspect screen
            </Button>
          </section>
        )}

        <Button
          className="map-options"
          variant="outline"
          ref={optionsButton}
          aria-haspopup="dialog"
          aria-expanded={optionsOpen}
          onClick={() => setOptionsOpen(true)}
        >
          <svg
            aria-hidden="true"
            viewBox="0 0 20 20"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
          >
            <path d="M3 6h5m4 0h5M3 14h9m4 0h1" />
            <circle cx="10" cy="6" r="2" />
            <circle cx="14" cy="14" r="2" />
          </svg>{" "}
          {t.mapOptions}
        </Button>
      </div>

      {showDetails && (
        <ScreenInspector
          archive={visibleArchive}
          run={run}
          screen={selectedScreen}
          width={width}
          widths={widths}
          locale={locale}
          locales={locales}
          cells={cells}
          cell={cell}
          stateParam={state}
          dismissRef={dismiss}
          onKeyDown={inspectKey}
          onClose={returnToOverview}
          onSelectWidth={(nextWidth) => update({ width: String(nextWidth), state: null })}
          onSelectLocale={(nextLocale) => update({ locale: nextLocale, state: null })}
          onSelectState={(nextState, nextWidth) =>
            update({
              state: nextState,
              ...(nextWidth !== undefined ? { width: String(nextWidth) } : {}),
            })
          }
          onSelectScreen={(id) => select(id)}
          screenDiff={selectedDiff}
          beforeRunKey={compareBeforeKey || undefined}
        />
      )}

      <Dialog open={optionsOpen} onOpenChange={setOptionsOpen}>
        <DialogContent finalFocus={() => (showOverview ? optionsButton.current : dismiss.current)}>
          <DialogTitle>{t.mapOptions}</DialogTitle>
          <label>
            Find screen
            <select
              value=""
              onChange={(e) => {
                if (e.target.value) {
                  setOptionsOpen(false);
                  select(e.target.value, optionsButton.current);
                }
              }}
            >
              <option value="">Select a screen</option>
              {archive.screens.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.path}
                </option>
              ))}
            </select>
          </label>
          <label>
            Connections
            <select
              value={mode}
              onChange={(e) =>
                update({
                  links: e.target.value === "overview" ? null : e.target.value,
                })
              }
            >
              <option value="overview">Overview</option>
              <option value="all">All links</option>
              <optgroup label="Links to and from a screen">
                {archive.screens.map((s) => (
                  <option key={s.id} value={"focus:" + s.id}>
                    {s.path}
                  </option>
                ))}
              </optgroup>
            </select>
          </label>
          <label>
            Capture width
            <select value={width} onChange={(e) => update({ width: e.target.value, state: null })}>
              {widths.map((w) => (
                <option key={w} value={w}>
                  {w} px
                </option>
              ))}
            </select>
          </label>
          {runs.length >= 2 && (
            <Button
              variant="outline"
              onClick={() => {
                setOptionsOpen(false);
                onOpenDiff();
              }}
            >
              {t.compareCaptures}
            </Button>
          )}
        </DialogContent>
      </Dialog>
    </section>
  );
}

async function json(url: string) {
  const response = await fetch(url);
  if (!response.ok) throw Error("archive-load-failed");
  return response.json();
}

function Viewer({
  initialRuns,
  run,
  archive,
  error,
}: {
  initialRuns: Run[];
  run?: Run;
  archive?: Archive;
  error?: string;
}) {
  const { t } = useI18n();
  const runs = initialRuns;
  const [pickerOpen, setPickerOpen] = useState(false);
  const [search, setSearch] = useState(location.search);

  useEffect(() => {
    const pop = () => setSearch(location.search);
    addEventListener("popstate", pop);
    return () => removeEventListener("popstate", pop);
  }, []);

  const params = new URLSearchParams(search);
  const isDiffView = params.get("view") === "diff";
  const diffBefore = params.get("before") ?? (runs.length >= 2 ? runs[1]?.key : runs[0]?.key) ?? "";
  const diffAfter = params.get("after") ?? run?.key ?? runs[0]?.key ?? "";

  function openDiff(beforeKey = diffBefore, afterKey = diffAfter) {
    const next = withSearchChanges(location.href, {
      view: "diff",
      before: beforeKey,
      after: afterKey,
    });
    history.pushState(null, "", next);
    setSearch(next.search);
  }

  function closeDiff() {
    const next = withSearchChanges(location.href, {
      view: null,
      before: null,
      after: null,
    });
    history.pushState(null, "", next);
    setSearch(next.search);
  }

  return (
    <main className="viewer">
      <WorkspaceRail run={run} open={() => setPickerOpen(true)} />
      {isDiffView && diffBefore && diffAfter ? (
        <CaptureDiffView
          runs={runs}
          beforeKey={diffBefore}
          afterKey={diffAfter}
          onSelectPair={(nextBefore, nextAfter) => openDiff(nextBefore, nextAfter)}
          onClose={closeDiff}
        />
      ) : archive && run ? (
        <MapView archive={archive} run={run} runs={runs} onOpenDiff={() => openDiff()} />
      ) : (
        <output>
          {error === "archive-load-failed"
            ? t.archiveLoadFailed
            : error === "workspace-unavailable"
              ? t.workspaceUnavailable
              : (error ?? t.noSavedMaps)}
        </output>
      )}
      <Dialog open={pickerOpen} onOpenChange={setPickerOpen}>
        <DialogContent>
          <DialogTitle>{t.selectWorkspace}</DialogTitle>
          <WorkPicker current={run} runs={runs} />
        </DialogContent>
      </Dialog>
    </main>
  );
}

const root = createRoot(document.getElementById("root")!);
let runs: Run[] = [],
  run: Run | undefined;
try {
  runs = await json(runsIndexUrl());
  const key = new URLSearchParams(location.search).get("run");
  run = key === null ? runs[0] : runs.find((r) => r.key === key);
  if (key !== null && !run) throw Error("workspace-unavailable");
  const archive = run ? decodeOverview(await json(archiveUrl("graph.json", run))) : undefined;
  root.render(
    <LocaleProvider>
      <Viewer initialRuns={runs} run={run} archive={archive} />
    </LocaleProvider>,
  );
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  root.render(
    <LocaleProvider>
      <Viewer initialRuns={runs} run={run} error={message} />
    </LocaleProvider>,
  );
}
