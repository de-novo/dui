import { ArrowDownLeft, ArrowUpRight, Play, X } from "lucide-react";
import type { KeyboardEvent, RefObject } from "react";
import { CaptureStatus } from "../../components/ui/CaptureStatus";
import { Button } from "../../components/ui/button";
import type { Cell, Screen, ScreenMap, ScreenOrigin } from "../../data/overview";
import type { Run } from "../../data/run";
import { canOpenLiveCell } from "../../model/live";
import { screenConnections, stateKeyFor } from "../../model/selection";
import { routeName } from "../map/CaptureCard";

function originLabel(origin: ScreenOrigin): string {
  if (origin.kind === "entry") return "Entry screen";
  if (origin.kind === "router") return `Router · ${origin.file}`;
  return `Discovered anchor from ${origin.from}`;
}

export function ScreenInspector({
  archive,
  run,
  screen,
  width,
  widths,
  cells,
  cell,
  stateParam,
  dismissRef,
  imageUrl,
  onKeyDown,
  onClose,
  onSelectWidth,
  onSelectState,
  onSelectScreen,
  onOpenLive,
}: {
  archive: ScreenMap;
  run: Run;
  screen?: Screen;
  width: number;
  widths: number[];
  cells: Cell[];
  cell?: Cell;
  stateParam: string | null;
  dismissRef: RefObject<HTMLButtonElement | null>;
  imageUrl: (name: string) => string;
  onKeyDown: (event: KeyboardEvent) => void;
  onClose: () => void;
  onSelectWidth: (width: number) => void;
  onSelectState: (stateKey: string, width?: number) => void;
  onSelectScreen: (screenId: string) => void;
  onOpenLive: (targetCell: { state: string; width: number }) => void;
}) {
  const liveGate = canOpenLiveCell(cell, run);
  const connections = screen ? screenConnections(archive, screen.id) : [];
  const screenCells = screen?.cells ?? [];
  const title = screen ? routeName(screen.path) : "Screen unavailable";
  const showFullPath = Boolean(screen && title !== screen.path);

  return (
    <aside className="inspector" aria-label="Screen details">
      <div className="panel-title">
        <div className="panel-heading">
          <h2>{title}</h2>
          {showFullPath && screen && <code className="route-path">{screen.path}</code>}
          {screen?.discoveredFrom && (
            <p
              className="origin-meta"
              title={
                screen.discoveredFrom.kind === "anchor" ? screen.discoveredFrom.selector : undefined
              }
            >
              {originLabel(screen.discoveredFrom)}
            </p>
          )}
        </div>
        <Button
          aria-label="Back to overview"
          title="Back to overview"
          variant="ghost"
          size="icon"
          ref={dismissRef}
          onKeyDown={onKeyDown}
          onClick={onClose}
        >
          <X aria-hidden="true" />
        </Button>
      </div>

      {screen && (
        <div className="live-entry">
          <Button
            className="live-open-button"
            variant="default"
            disabled={!liveGate.ok}
            onKeyDown={onKeyDown}
            onClick={() => {
              if (liveGate.ok) onOpenLive(liveGate.cell);
            }}
          >
            <Play aria-hidden="true" />
            Open live session
          </Button>
          {!liveGate.ok && <p className="live-unavailable-note">{liveGate.reason}</p>}
        </div>
      )}

      {(!screen || screenCells.length === 0) && (
        <label>
          Width
          <select
            onKeyDown={onKeyDown}
            value={width}
            onChange={(e) => onSelectWidth(Number(e.target.value))}
          >
            {widths.map((w) => (
              <option key={w} value={w}>
                {w} px
              </option>
            ))}
          </select>
        </label>
      )}

      {cells.length > 4 && (
        <label>
          State
          <select
            onKeyDown={onKeyDown}
            value={cell ? cells.indexOf(cell) : -1}
            onChange={(e) => onSelectState(stateKeyFor(cells, Number(e.target.value)))}
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

      {screen && screenCells.length > 0 && (
        <section className="inspector-section" aria-label="Captured states and widths">
          <h3>States and widths</h3>
          <ul className="cell-matrix">
            {screenCells.map((entry, index) => {
              const widthCells = screenCells.filter((c) => c.width === entry.width);
              const entryKey = stateKeyFor(widthCells, widthCells.indexOf(entry));
              const active = entry === cell;
              return (
                <li key={`${entry.width}-${entry.state ?? index}`}>
                  <button
                    type="button"
                    className={"cell-row" + (active ? " active" : "")}
                    aria-current={active ? "true" : undefined}
                    onKeyDown={onKeyDown}
                    onClick={() => onSelectState(entryKey, entry.width)}
                  >
                    <span className="cell-row-title">
                      <strong>{entry.width} px</strong>
                      <span>
                        {entry.scenario ?? "default"}
                        {entry.state ? ` · ${entry.state.slice(0, 8)}` : ""}
                      </span>
                    </span>
                    <span className="cell-row-status">
                      <CaptureStatus cell={entry} />
                      {!entry.image && <span className="missing-image-tag">No image</span>}
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      )}

      <div className="inspection-result">
        <div className="section-header-row">
          <h3>Verification</h3>
          <span className="provenance-badge">Archive</span>
        </div>
        <p className="capture-meta">
          {stateParam !== null && !cell ? "State unavailable" : <CaptureStatus cell={cell} />}
        </p>
      </div>

      <figure className="archive-preview">
        <figcaption className="archive-preview-caption">
          <span>Archived capture</span>
          <span>{width} px</span>
        </figcaption>
        {cell?.image ? (
          <img alt={`Archived screen at ${width} px`} src={imageUrl(cell.image)} />
        ) : (
          <output className="missing-capture-box">
            <strong>Image unavailable</strong>
            <p className="capture-meta">
              {stateParam !== null && !cell ? "State unavailable" : <CaptureStatus cell={cell} />}
            </p>
          </output>
        )}
      </figure>

      <section className="inspector-section" aria-label="Discovered links">
        <h3>Discovered links</h3>
        {connections.length === 0 ? (
          <p className="empty-meta">No discovered links recorded for this screen.</p>
        ) : (
          <ul className="link-evidence-list">
            {connections.map(({ direction, screen: partner, link }) => (
              <li key={`${direction}-${partner.id}`} className="link-evidence-item">
                <button
                  type="button"
                  className="link-target-button"
                  onKeyDown={onKeyDown}
                  onClick={() => onSelectScreen(partner.id)}
                >
                  {direction === "outgoing" ? (
                    <ArrowUpRight aria-hidden="true" />
                  ) : (
                    <ArrowDownLeft aria-hidden="true" />
                  )}
                  <span className="link-route">{partner.path}</span>
                </button>
                <div className="link-evidence-meta">
                  <span className="evidence-tag">Discovered · not replay-verified</span>
                  <span>
                    {direction === "outgoing" ? "To" : "From"} ·{" "}
                    {link.origin === "anchor" ? "DOM anchor" : (link.origin ?? "Observed anchor")}
                  </span>
                  {link.observedWidths && link.observedWidths.length > 0 && (
                    <span>Observed at {link.observedWidths.join(", ")} px</span>
                  )}
                  {link.selector && <code className="selector-evidence">{link.selector}</code>}
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>
    </aside>
  );
}
