import { ArrowDownLeft, ArrowUpRight, Play, X } from "lucide-react";
import type { KeyboardEvent, RefObject } from "react";
import { CaptureStatus } from "../../components/ui/CaptureStatus";
import { Button } from "../../components/ui/button";
import type { Cell, Screen, ScreenMap } from "../../data/overview";
import type { Run } from "../../data/run";
import { canOpenLiveCell } from "../../model/live";
import { screenConnections, stateKeyFor } from "../../model/selection";
import { routeName } from "../map/CaptureCard";

function cellLabel(cell: Cell, multipleStates: boolean, index: number): string | null {
  if (!multipleStates) return null;
  if (cell.scenario && cell.scenario !== "default" && !cell.scenario.startsWith("link-")) {
    return cell.scenario;
  }
  return `State ${index + 1}`;
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
  const widthsOnScreen = new Set(screenCells.map((c) => c.width)).size;
  const multipleStatesPerWidth = screenCells.length > widthsOnScreen;

  return (
    <aside className="inspector glass-panel" aria-label="Screen details">
      <div className="panel-title">
        <div className="panel-heading">
          <h2>{title}</h2>
          {showFullPath && screen && <code className="route-path">{screen.path}</code>}
          <p className="origin-meta">Archive</p>
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
            Open live
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

      {screen && screenCells.length > 0 && (
        <section className="inspector-section" aria-label="Captured states and widths">
          <h3>{multipleStatesPerWidth ? "States & widths" : "Widths"}</h3>
          <ul className="cell-matrix">
            {screenCells.map((entry, index) => {
              const widthCells = screenCells.filter((c) => c.width === entry.width);
              const localIndex = widthCells.indexOf(entry);
              const entryKey = stateKeyFor(widthCells, localIndex);
              const active = entry === cell;
              const subtitle = cellLabel(entry, widthCells.length > 1, localIndex);
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
                      {subtitle && <span>{subtitle}</span>}
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

      {cell && !cell.image && (
        <output className="missing-capture-box">
          <strong>Image unavailable</strong>
          <p className="capture-meta">
            {stateParam !== null && !cell ? "State unavailable" : <CaptureStatus cell={cell} />}
          </p>
        </output>
      )}

      {stateParam !== null && !cell && cells.length > 0 && (
        <output className="missing-capture-box">
          <strong>State unavailable</strong>
        </output>
      )}

      {connections.length > 0 && (
        <section className="inspector-section" aria-label="Links">
          <h3>Links</h3>
          <ul className="link-evidence-list">
            {connections.map(({ direction, screen: partner }) => (
              <li key={`${direction}-${partner.id}`}>
                <button
                  type="button"
                  className="cell-row link-row"
                  onKeyDown={onKeyDown}
                  onClick={() => onSelectScreen(partner.id)}
                >
                  <span className="link-row-target">
                    {direction === "outgoing" ? (
                      <ArrowUpRight aria-hidden="true" />
                    ) : (
                      <ArrowDownLeft aria-hidden="true" />
                    )}
                    <span className="link-route">{partner.path}</span>
                  </span>
                  <span className="link-row-state">Discovered</span>
                </button>
              </li>
            ))}
          </ul>
          <details className="readonly-disclosure link-details-disclosure">
            <summary>Link evidence</summary>
            <ul className="readonly-list">
              {connections.map(({ direction, screen: partner, link }) => (
                <li key={`${direction}-${partner.id}`} className="link-evidence-item">
                  <strong>{partner.path}</strong>
                  <span>
                    {link.origin === "anchor" ? "DOM anchor" : (link.origin ?? "Observed anchor")}
                    {link.observedWidths?.length ? ` · ${link.observedWidths.join(", ")} px` : ""} ·
                    Not replay-verified
                  </span>
                  {link.selector && <code className="selector-evidence">{link.selector}</code>}
                </li>
              ))}
            </ul>
          </details>
        </section>
      )}
    </aside>
  );
}
