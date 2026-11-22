import { ArrowDownLeft, ArrowUpRight, Play, X } from "lucide-react";
import type { KeyboardEvent, RefObject } from "react";
import { CaptureStatus } from "../../components/ui/CaptureStatus";
import { Button } from "../../components/ui/button";
import type { Cell, Screen, ScreenMap } from "../../data/overview";
import type { Run } from "../../data/run";
import { useI18n } from "../../i18n/LocaleContext";
import { canOpenLiveCell } from "../../model/live";
import type { DesignFinding } from "../../model/audit";
import type { ScreenCanvasDiff } from "../../model/canvasDiff";
import { screenConnections, stateKeyFor } from "../../model/selection";
import { routeName } from "../map/CaptureCard";
import { CommentPanel } from "./CommentPanel";

function cellLabel(
  cell: Cell,
  multipleStates: boolean,
  index: number,
  stateLabel: (n: number) => string,
): string | null {
  const parts: string[] = [];
  if (cell.locale) parts.push(cell.locale);
  if (multipleStates) {
    if (cell.scenario && cell.scenario !== "default" && !cell.scenario.startsWith("link-")) {
      parts.push(cell.scenario);
    } else if (!cell.locale) {
      parts.push(stateLabel(index + 1));
    }
  }
  return parts.length > 0 ? parts.join(" · ") : null;
}

export function ScreenInspector({
  archive,
  run,
  screen,
  width,
  widths,
  locale,
  locales,
  cells,
  cell,
  stateParam,
  dismissRef,
  onKeyDown,
  onClose,
  onSelectWidth,
  onSelectLocale,
  onSelectState,
  onSelectScreen,
  onOpenLive,
  screenDiff,
  beforeRunKey,
  designFindings = [],
}: {
  archive: ScreenMap;
  run: Run;
  screen?: Screen;
  width: number;
  widths: number[];
  locale: string | null;
  locales: string[];
  cells: Cell[];
  cell?: Cell;
  stateParam: string | null;
  dismissRef: RefObject<HTMLButtonElement | null>;
  onKeyDown: (event: KeyboardEvent) => void;
  onClose: () => void;
  onSelectWidth: (width: number) => void;
  onSelectLocale: (locale: string | null) => void;
  onSelectState: (stateKey: string, width?: number) => void;
  onSelectScreen: (screenId: string) => void;
  onOpenLive: (targetCell: { state: string; width: number }) => void;
  screenDiff?: ScreenCanvasDiff;
  beforeRunKey?: string;
  designFindings?: DesignFinding[];
}) {
  const { t } = useI18n();
  const liveGate = canOpenLiveCell(cell, run);
  const connections = screen ? screenConnections(archive, screen.id) : [];
  const rawScreenCells = screen?.cells ?? [];
  const screenCells =
    locale === null ? rawScreenCells : rawScreenCells.filter((c) => c.locale === locale);
  const title = screen ? routeName(screen.path) : t.screenUnavailable;
  const showFullPath = Boolean(screen && title !== screen.path);
  const widthsOnScreen = new Set(screenCells.map((c) => c.width)).size;
  const multipleStatesPerWidth = screenCells.length > widthsOnScreen;
  const audit = cell?.localeAudit;
  const hasLocaleIssues = Boolean(
    audit && (audit.missingKeys.length > 0 || audit.overflows.length > 0 || audit.rtlMismatch),
  );

  return (
    <aside className="inspector glass-panel" aria-label={t.screenDetails}>
      <div className="panel-title">
        <div className="panel-heading">
          <h2>{title}</h2>
          {showFullPath && screen && <code className="route-path">{screen.path}</code>}
          <p className="origin-meta">{t.archive}</p>
        </div>
        <Button
          aria-label={t.backToOverview}
          title={t.backToOverview}
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
            {t.openLive}
          </Button>
          {!liveGate.ok && <p className="live-unavailable-note">{liveGate.reason}</p>}
        </div>
      )}

      {screenDiff && (
        <section className="inspector-diff-section" aria-label={t.diffTitle}>
          <div className="inspector-diff-header">
            <span className={`diff-badge diff-badge-${screenDiff.kind}`}>
              {screenDiff.kind === "added"
                ? t.diffAdded
                : screenDiff.kind === "removed"
                  ? t.diffRemoved
                  : t.diffChanged}
            </span>
            {screenDiff.maxDiffRatio !== null && screenDiff.maxDiffRatio > 0 && (
              <span className="inspector-diff-ratio">
                {(screenDiff.maxDiffRatio * 100).toFixed(1)}% px
              </span>
            )}
          </div>
          <div className="inspector-diff-previews">
            {screenDiff.beforeImage && beforeRunKey && (
              <figure>
                <figcaption>{t.beforeLabel}</figcaption>
                <img
                  alt={t.beforeLabel}
                  src={`/archive/${encodeURIComponent(screenDiff.beforeImage)}?run=${encodeURIComponent(beforeRunKey)}`}
                />
              </figure>
            )}
            {screenDiff.afterImage && (
              <figure>
                <figcaption>{t.afterLabel}</figcaption>
                <img
                  alt={t.afterLabel}
                  src={`/archive/${encodeURIComponent(screenDiff.afterImage)}?run=${encodeURIComponent(run.key)}`}
                />
              </figure>
            )}
            {screenDiff.overlayImage && (
              <figure>
                <figcaption>{t.pixelDiffOverlay}</figcaption>
                <img
                  alt={t.pixelDiffOverlay}
                  src={`/api/diff-overlay?file=${encodeURIComponent(screenDiff.overlayImage)}`}
                />
              </figure>
            )}
          </div>
          {screenDiff.renderDiff && screenDiff.renderDiff.changes.length > 0 && (
            <ul className="inspector-render-changes">
              {screenDiff.renderDiff.changes.slice(0, 8).map((chg) => (
                <li key={`${chg.kind}:${chg.id}`} className={`render-change-${chg.kind}`}>
                  <span>{chg.summary}</span>
                </li>
              ))}
            </ul>
          )}
          {screenDiff.renderDiff?.visualOnly && (
            <p className="inspector-visual-only">{t.canvasDiffVisualOnly}</p>
          )}
        </section>
      )}

      {locales.length > 1 && (
        <label>
          {t.localeFilter}
          <select
            aria-label={t.localeFilter}
            onKeyDown={onKeyDown}
            value={locale ?? ""}
            onChange={(e) => onSelectLocale(e.target.value || null)}
          >
            <option value="">{t.allLocales}</option>
            {locales.map((loc) => (
              <option key={loc} value={loc}>
                {loc}
              </option>
            ))}
          </select>
        </label>
      )}

      {(!screen || screenCells.length === 0) && (
        <label>
          {t.width}
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
        <section className="inspector-section" aria-label={t.statesAndWidths}>
          <h3>{multipleStatesPerWidth ? t.statesAndWidths : t.widths}</h3>
          <ul className="cell-matrix">
            {screenCells.map((entry, index) => {
              const widthCells = screenCells.filter((c) => c.width === entry.width);
              const localIndex = widthCells.indexOf(entry);
              const entryKey = stateKeyFor(widthCells, localIndex);
              const active = entry === cell;
              const subtitle = cellLabel(entry, widthCells.length > 1, localIndex, t.stateLabel);
              return (
                <li key={`${entry.width}-${entry.locale ?? ""}-${entry.state ?? index}`}>
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
                      {!entry.image && <span className="missing-image-tag">{t.noImage}</span>}
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      )}

      {designFindings.length > 0 && (
        <details className="readonly-disclosure link-details-disclosure" open>
          <summary>
            {t.designIssuesTitle} ({designFindings.length})
          </summary>
          <ul className="readonly-list">
            {designFindings.slice(0, 12).map((finding, idx) => (
              <li key={`${finding.kind}:${finding.nodeId}:${idx}`} className="link-evidence-item">
                <span className={`design-finding-sev design-finding-${finding.severity}`}>
                  {finding.summary}
                </span>
                <code className="selector-evidence">
                  {finding.measured} · {finding.expected}
                </code>
              </li>
            ))}
          </ul>
        </details>
      )}

      {hasLocaleIssues && audit && (
        <details className="readonly-disclosure link-details-disclosure" open>
          <summary>{t.localeIssues}</summary>
          <ul className="readonly-list">
            {audit.missingKeys.length > 0 && (
              <li className="link-evidence-item">
                <span>{t.missingKeysLabel(audit.missingKeys)}</span>
              </li>
            )}
            {audit.overflows.map((ov) => (
              <li key={ov.selector} className="link-evidence-item">
                <span>{t.overflowLabel(ov.selector)}</span>
                <code className="selector-evidence">
                  {ov.scrollWidth} / {ov.clientWidth} px
                </code>
              </li>
            ))}
            {audit.rtlMismatch && (
              <li className="link-evidence-item">
                <span>{t.rtlMismatchLabel(audit.rtlMismatch.observed)}</span>
              </li>
            )}
          </ul>
        </details>
      )}

      {cell && !cell.image && (
        <output className="missing-capture-box">
          <strong>{t.imageUnavailable}</strong>
          <p className="capture-meta">
            {stateParam !== null && !cell ? t.stateUnavailable : <CaptureStatus cell={cell} />}
          </p>
        </output>
      )}

      {stateParam !== null && !cell && cells.length > 0 && (
        <output className="missing-capture-box">
          <strong>{t.stateUnavailable}</strong>
        </output>
      )}

      {connections.length > 0 && (
        <section className="inspector-section" aria-label={t.links}>
          <h3>{t.links}</h3>
          <ul className="link-evidence-list">
            {connections.map(({ direction, screen: partner, link }) => {
              const badge =
                link.provenance === "verified"
                  ? t.edgeVerified
                  : link.provenance === "inferred"
                    ? t.edgeInferred
                    : t.edgeExtracted;
              return (
                <li key={`${direction}-${partner.id}`}>
                  <button
                    type="button"
                    className="cell-row link-row"
                    data-edge-provenance={link.provenance}
                    data-edge-verified={String(link.replayVerified)}
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
                    <span className="link-row-state">{badge}</span>
                  </button>
                </li>
              );
            })}
          </ul>
          <details className="readonly-disclosure link-details-disclosure">
            <summary>{t.linkEvidence}</summary>
            <ul className="readonly-list">
              {connections.map(({ direction, screen: partner, link }) => {
                const widths = link.confirmedWidths?.length
                  ? link.confirmedWidths
                  : link.observedWidths;
                return (
                  <li key={`${direction}-${partner.id}`} className="link-evidence-item">
                    <strong>{partner.path}</strong>
                    <span>
                      {link.action?.label ? `${link.action.label} · ` : ""}
                      {link.origin === "anchor" ? t.domAnchor : (link.origin ?? t.observedAnchor)}
                      {widths?.length ? ` · ${widths.join(", ")} px` : ""} ·{" "}
                      {link.replayVerified ? t.replayVerifiedLabel : t.notReplayVerified}
                    </span>
                    {link.selector && <code className="selector-evidence">{link.selector}</code>}
                    {link.failedWidths &&
                      Object.entries(link.failedWidths).map(([w, reason]) => (
                        <code key={w} className="selector-evidence">
                          {w} px: {reason}
                        </code>
                      ))}
                  </li>
                );
              })}
            </ul>
          </details>
        </section>
      )}
      {screen && (
        <CommentPanel
          run={run}
          screen={screen}
          cell={cell}
          outgoingTargets={connections
            .filter((c) => c.direction === "outgoing")
            .map((c) => c.screen.path)}
          onKeyDown={onKeyDown}
        />
      )}
    </aside>
  );
}
