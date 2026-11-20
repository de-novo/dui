import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Button } from "./components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "./components/ui/dialog";
import { createRoot } from "react-dom/client";
import { decodeOverview, type ScreenMap as Archive } from "./data/overview";
import type { Run } from "./data/run";
import { CaptureStatus } from "./components/ui/CaptureStatus";
import { ScreenNeighborhood } from "./features/map/ScreenNeighborhood";
import { routeName } from "./features/map/CaptureCard";
import { ScreenMap } from "./features/map/ScreenMap";
import { WorkspaceRail } from "./features/workspace/WorkspaceRail";
import { WorkPicker } from "./features/workspace/WorkPicker";
import "./preview.css";

function archiveUrl(name: string, run: Run) {
  return "/archive/" + encodeURIComponent(name) + "?run=" + encodeURIComponent(run.key);
}

function MapView({ archive, run }: { archive: Archive; run: Run }) {
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
  const [search, setSearch] = useState(location.search);
  const params = new URLSearchParams(search);
  const widths = [...new Set(archive.screens.flatMap((s) => s.cells.map((c) => c.width)))].sort(
    (a, b) => b - a,
  );
  const width = Number(params.get("width") ?? widths[0] ?? 0);
  const selectedId = params.get("screen");
  const screen = archive.screens.find((s) => s.id === selectedId);
  const mode = params.get("links") ?? "overview";
  const [optionsOpen, setOptionsOpen] = useState(false);
  const optionsButton = useRef<HTMLButtonElement>(null);
  const dismiss = useRef<HTMLButtonElement>(null);
  const origin = useRef<HTMLElement | SVGElement | null>(null);
  useEffect(() => {
    const pop = () => setSearch(location.search);
    addEventListener("popstate", pop);
    return () => removeEventListener("popstate", pop);
  }, []);
  useEffect(() => {
    if (selectedId !== null) dismiss.current?.focus();
    else origin.current?.focus();
  }, [selectedId]);
  function update(changes: Record<string, string | null>) {
    const next = new URL(location.href);
    for (const [key, value] of Object.entries(changes)) {
      if (value === null) next.searchParams.delete(key);
      else next.searchParams.set(key, value);
    }
    history.pushState(null, "", next);
    setSearch(next.search);
  }
  function inspectKey(event: React.KeyboardEvent) {
    if (event.key === "Escape") update({ screen: null, state: null });
  }
  function select(id: string, active: Element | null = document.activeElement) {
    if (selectedId === null && (active instanceof HTMLElement || active instanceof SVGElement))
      origin.current = active;
    update({ screen: id, state: null });
  }
  const cells = screen?.cells.filter((c) => c.width === width) ?? [];
  const state = params.get("state");
  const stateKey = (i: number) =>
    cells[i].state && cells.filter((c) => c.state === cells[i].state).length === 1
      ? cells[i].state!
      : "cell:" + i;
  const cell = state === null ? cells[0] : cells.find((_, i) => stateKey(i) === state);
  return (
    <section className="workspace" aria-label="Screen map">
      <div className="map" ref={mapArea}>
        <div ref={overviewLayer} className="overview-layer" aria-hidden={!!screen} inert={!!screen}>
          {railInset !== undefined && (
            <ScreenMap
              railInset={railInset}
              archive={archive}
              width={width}
              mode={mode}
              imageUrl={(name) => archiveUrl(name, run)}
              select={select}
            />
          )}
        </div>
        {screen && (
          <ScreenNeighborhood
            key={JSON.stringify([screen.id, width])}
            archive={archive}
            screen={screen}
            cell={cell}
            width={width}
            imageUrl={(name) => archiveUrl(name, run)}
            select={select}
          />
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
          Map options
        </Button>
      </div>
      <Dialog open={optionsOpen} onOpenChange={setOptionsOpen}>
        <DialogContent
          finalFocus={() => (selectedId === null ? optionsButton.current : dismiss.current)}
        >
          <DialogTitle>Map options</DialogTitle>
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
        </DialogContent>
      </Dialog>
      {selectedId !== null && (
        <aside className="inspector" aria-label="Screen details">
          <div className="panel-title">
            <div>
              <h2>{screen ? routeName(screen.path) : "Screen unavailable"}</h2>
              {screen && <code className="route-path">{screen.path}</code>}
            </div>
            <Button
              aria-label="Close"
              variant="ghost"
              size="icon"
              ref={dismiss}
              onKeyDown={inspectKey}
              onClick={() => update({ screen: null, state: null })}
            >
              ×
            </Button>
          </div>
          <label>
            Width
            <select
              onKeyDown={inspectKey}
              value={width}
              onChange={(e) => update({ width: e.target.value, state: null })}
            >
              {widths.map((w) => (
                <option key={w} value={w}>
                  {w} px
                </option>
              ))}
            </select>
          </label>
          {cells.length > 1 && (
            <label>
              State
              <select
                onKeyDown={inspectKey}
                value={cell ? cells.indexOf(cell) : -1}
                onChange={(e) => update({ state: stateKey(Number(e.target.value)) })}
              >
                {!cell && <option value={-1}>State unavailable</option>}
                {cells.map((c, i) => (
                  <option key={i} value={i}>
                    {c.scenario ?? "State"} · {c.state?.slice(0, 8) ?? i + 1}
                  </option>
                ))}
              </select>
            </label>
          )}
          <div className="inspection-result">
            <h3>Verification</h3>
            <p className="capture-meta">
              {state !== null && !cell ? "State unavailable" : <CaptureStatus cell={cell} />}
            </p>
          </div>
          {cell?.image && (
            <img alt={"Archived screen at " + width + " px"} src={archiveUrl(cell.image, run)} />
          )}
        </aside>
      )}
    </section>
  );
}

async function json(url: string) {
  const response = await fetch(url);
  if (!response.ok) throw Error("Could not load the archive");
  return response.json();
}
function Viewer({
  runs,
  run,
  archive,
  error,
}: {
  runs: Run[];
  run?: Run;
  archive?: Archive;
  error?: string;
}) {
  const [pickerOpen, setPickerOpen] = useState(false);
  return (
    <main className="viewer">
      <WorkspaceRail run={run} open={() => setPickerOpen(true)} />
      {archive && run ? (
        <MapView archive={archive} run={run} />
      ) : (
        <output>{error ?? "No saved screen maps."}</output>
      )}
      <Dialog open={pickerOpen} onOpenChange={setPickerOpen}>
        <DialogContent>
          <DialogTitle>Select workspace</DialogTitle>
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
  runs = await json("/archive/runs.json");
  const key = new URLSearchParams(location.search).get("run");
  run = key === null ? runs[0] : runs.find((r) => r.key === key);
  if (key !== null && !run) throw Error("This workspace is unavailable.");
  const archive = run ? decodeOverview(await json(archiveUrl("graph.json", run))) : undefined;
  root.render(<Viewer runs={runs} run={run} archive={archive} />);
} catch (error) {
  root.render(<Viewer runs={runs} run={run} error={String(error)} />);
}
