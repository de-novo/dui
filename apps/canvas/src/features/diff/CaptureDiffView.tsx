import { ArrowLeft, GitCompareArrows } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "../../components/ui/button";
import {
  archiveFileUrl,
  diffApiUrl,
  diffOverlayUrl,
  isStaticArchiveMode,
} from "../../data/archiveUrls";
import type { Run } from "../../data/run";
import { useI18n } from "../../i18n/LocaleContext";
import { decodeCaptureDiff, type CaptureDiff, type CellDiffEntry } from "../../model/diff";
import { availableDiffPairs, formatShowcaseRunLabel } from "../../model/runLabels";

function archiveImageUrl(name: string, runKey: string) {
  return archiveFileUrl(name, runKey);
}

function overlayImageUrl(beforeKey: string, afterKey: string, file: string) {
  if (isStaticArchiveMode()) return diffOverlayUrl(file);
  return (
    "/api/diff/overlay?before=" +
    encodeURIComponent(beforeKey) +
    "&after=" +
    encodeURIComponent(afterKey) +
    "&file=" +
    encodeURIComponent(file)
  );
}

function formatRunLabel(run: Run, runs: readonly Run[]) {
  return formatShowcaseRunLabel(run, runs) || run.key.slice(0, 12);
}

function DiffCellCard({
  cell,
  beforeKey,
  afterKey,
}: {
  cell: CellDiffEntry;
  beforeKey: string;
  afterKey: string;
}) {
  const { t } = useI18n();
  const kindLabel =
    cell.kind === "added" ? t.diffAdded : cell.kind === "removed" ? t.diffRemoved : t.diffChanged;

  const percent = cell.pixelDiff ? (cell.pixelDiff.diffRatio * 100).toFixed(2) : "0.00";

  return (
    <article
      className="diff-cell glass-panel"
      data-diff-kind={cell.kind}
      data-diff-screen={cell.path}
      data-diff-state={cell.state}
      data-diff-width={cell.width}
    >
      <header className="diff-cell-header">
        <span className={`diff-kind-badge diff-kind-${cell.kind}`}>{kindLabel}</span>
        <code className="diff-cell-meta">
          {cell.state.slice(0, 10)} · {cell.width} px
          {cell.locale ? ` · ${cell.locale}` : ""}
        </code>
        {cell.pixelDiff && (
          <span className="diff-pixel-summary">
            {t.pixelsChangedLabel(cell.pixelDiff.diffPixels, percent)}
          </span>
        )}
      </header>

      {cell.statusChange && !cell.pixelDiff && (
        <output className="diff-status-change">
          <strong>{t.statusChangeLabel}:</strong>{" "}
          <code>
            {cell.statusChange.from}
            {cell.statusChange.fromReason ? ` (${cell.statusChange.fromReason})` : ""}
          </code>{" "}
          →{" "}
          <code>
            {cell.statusChange.to}
            {cell.statusChange.toReason ? ` (${cell.statusChange.toReason})` : ""}
          </code>
        </output>
      )}

      {cell.kind === "changed" && cell.pixelDiff && cell.before?.image && cell.after?.image && (
        <div className="diff-visual-grid">
          <figure className="diff-figure">
            <figcaption>{t.beforeLabel}</figcaption>
            <img
              src={archiveImageUrl(cell.before.image, beforeKey)}
              alt={`${cell.path} ${t.beforeLabel} ${cell.width}px`}
              loading="lazy"
            />
          </figure>
          <figure className="diff-figure">
            <figcaption>{t.afterLabel}</figcaption>
            <img
              src={archiveImageUrl(cell.after.image, afterKey)}
              alt={`${cell.path} ${t.afterLabel} ${cell.width}px`}
              loading="lazy"
            />
          </figure>
          <figure className="diff-figure diff-figure-overlay">
            <figcaption>{t.pixelDiffOverlay}</figcaption>
            {cell.pixelDiff.overlayImage && (
              <img
                src={overlayImageUrl(beforeKey, afterKey, cell.pixelDiff.overlayImage)}
                alt={`${cell.path} ${t.pixelDiffOverlay} ${cell.width}px`}
                loading="lazy"
              />
            )}
            {cell.pixelDiff.changedBounds && (
              <small className="diff-bounds">
                {cell.pixelDiff.changedBounds.x},{cell.pixelDiff.changedBounds.y}{" "}
                {cell.pixelDiff.changedBounds.width}×{cell.pixelDiff.changedBounds.height}
              </small>
            )}
          </figure>
        </div>
      )}

      {cell.kind === "added" && (
        <div className="diff-single-side">
          {cell.after?.image ? (
            <figure className="diff-figure">
              <figcaption>{t.afterLabel}</figcaption>
              <img
                src={archiveImageUrl(cell.after.image, afterKey)}
                alt={`${cell.path} ${t.diffAdded} ${cell.width}px`}
                loading="lazy"
              />
            </figure>
          ) : (
            <div className="diff-status-change">
              <code>
                {cell.after?.status}
                {cell.after?.reason ? ` (${cell.after.reason})` : ""}
              </code>
            </div>
          )}
        </div>
      )}

      {cell.kind === "removed" && (
        <div className="diff-single-side">
          {cell.before?.image ? (
            <figure className="diff-figure">
              <figcaption>{t.beforeLabel}</figcaption>
              <img
                src={archiveImageUrl(cell.before.image, beforeKey)}
                alt={`${cell.path} ${t.diffRemoved} ${cell.width}px`}
                loading="lazy"
              />
            </figure>
          ) : (
            <div className="diff-status-change">
              <code>
                {cell.before?.status}
                {cell.before?.reason ? ` (${cell.before.reason})` : ""}
              </code>
            </div>
          )}
        </div>
      )}
    </article>
  );
}

export function CaptureDiffView({
  runs,
  beforeKey,
  afterKey,
  onSelectPair,
  onClose,
}: {
  runs: Run[];
  beforeKey: string;
  afterKey: string;
  onSelectPair: (nextBefore: string, nextAfter: string) => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const pairKey = `${beforeKey}:${afterKey}`;
  const [result, setResult] = useState<{
    pairKey: string;
    diff: CaptureDiff | null;
    error: string | null;
  }>({ pairKey: "", diff: null, error: null });

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    addEventListener("keydown", onKey);
    return () => removeEventListener("keydown", onKey);
  }, [onClose]);

  useEffect(() => {
    let cancelled = false;
    const url = diffApiUrl(beforeKey, afterKey, true);
    void fetch(url)
      .then(async (res) => {
        if (!res.ok) throw Error(t.diffFailed);
        return decodeCaptureDiff(await res.json());
      })
      .then((decoded) => {
        if (!cancelled) {
          setResult({ pairKey, diff: decoded, error: null });
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setResult({
            pairKey,
            diff: null,
            error: err instanceof Error ? err.message : t.diffFailed,
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [beforeKey, afterKey, pairKey, t.diffFailed]);

  const loading = result.pairKey !== pairKey;
  const diff = loading ? null : result.diff;
  const error = loading ? null : result.error;
  const staticMode = isStaticArchiveMode();
  const staticPairs = staticMode ? availableDiffPairs(runs, true) : [];
  const beforeOptions =
    staticPairs.length > 0
      ? Array.from(new Map(staticPairs.map((p) => [p.before.key, p.before])).values())
      : runs;
  const afterOptions =
    staticPairs.length > 0
      ? Array.from(
          new Map(
            staticPairs
              .filter((p) => p.before.key === beforeKey)
              .map((p) => [p.after.key, p.after]),
          ).values(),
        )
      : runs;
  const visibleAfterOptions =
    afterOptions.length > 0
      ? afterOptions
      : Array.from(new Map(staticPairs.map((p) => [p.after.key, p.after])).values());

  return (
    <section className="diff-workspace" aria-label={t.diffTitle}>
      <header className="diff-toolbar glass-panel">
        <div className="diff-toolbar-title">
          <Button variant="outline" size="sm" onClick={onClose}>
            <ArrowLeft aria-hidden="true" />
            {t.backToOverview}
          </Button>
          <h2>
            <GitCompareArrows className="diff-title-icon" aria-hidden="true" />
            {t.diffTitle}
          </h2>
        </div>

        <div className="diff-pickers">
          <label className="diff-picker">
            <span>{t.beforeCapture}</span>
            <select
              aria-label={t.beforeCapture}
              value={beforeKey}
              onChange={(e) => {
                const nextBefore = e.target.value;
                if (staticPairs.length > 0) {
                  const match = staticPairs.find(
                    (p) => p.before.key === nextBefore && p.after.key === afterKey,
                  );
                  const fallbackAfter = staticPairs.find((p) => p.before.key === nextBefore)?.after
                    .key;
                  onSelectPair(nextBefore, match ? afterKey : (fallbackAfter ?? afterKey));
                } else {
                  onSelectPair(nextBefore, afterKey);
                }
              }}
            >
              {beforeOptions.map((r) => (
                <option key={r.key} value={r.key}>
                  {formatRunLabel(r, runs)}
                </option>
              ))}
            </select>
          </label>
          <label className="diff-picker">
            <span>{t.afterCapture}</span>
            <select
              aria-label={t.afterCapture}
              value={afterKey}
              onChange={(e) => {
                const nextAfter = e.target.value;
                if (staticPairs.length > 0) {
                  const match = staticPairs.find(
                    (p) => p.before.key === beforeKey && p.after.key === nextAfter,
                  );
                  const fallbackBefore = staticPairs.find((p) => p.after.key === nextAfter)?.before
                    .key;
                  onSelectPair(match ? beforeKey : (fallbackBefore ?? beforeKey), nextAfter);
                } else {
                  onSelectPair(beforeKey, nextAfter);
                }
              }}
            >
              {visibleAfterOptions.map((r) => (
                <option key={r.key} value={r.key}>
                  {formatRunLabel(r, runs)}
                </option>
              ))}
            </select>
          </label>
        </div>

        {diff && (
          <output className="diff-summary-pill" data-diff-total-changed={diff.summary.totalChanged}>
            {t.diffSummaryLabel(diff.summary.totalChanged, diff.summary.unchanged)}
          </output>
        )}
      </header>

      <div className="diff-content">
        {loading && <output className="diff-state-box glass-panel">{t.loadingDiff}</output>}
        {!loading && error && (
          <output className="diff-state-box glass-panel diff-error">{error}</output>
        )}
        {!loading && !error && diff && diff.summary.totalChanged === 0 && (
          <section className="diff-empty glass-panel" data-diff-empty="true">
            <h3>{t.emptyDiffTitle}</h3>
            <p>{t.emptyDiffDetail(diff.summary.unchanged)}</p>
          </section>
        )}
        {!loading &&
          !error &&
          diff &&
          diff.screens.map((screen) => (
            <section
              key={screen.path}
              className="diff-screen-group"
              data-diff-screen-group={screen.path}
            >
              <header className="diff-screen-heading">
                <h3>
                  <code>{screen.path}</code>
                </h3>
                <span className="diff-screen-count">{screen.cells.length}</span>
              </header>
              <div className="diff-cell-list">
                {screen.cells.map((cell) => (
                  <DiffCellCard
                    key={`${cell.path}:${cell.state}:${cell.width}:${cell.locale ?? ""}`}
                    cell={cell}
                    beforeKey={diff.beforeRunKey || beforeKey}
                    afterKey={diff.afterRunKey || afterKey}
                  />
                ))}
              </div>
            </section>
          ))}
      </div>
    </section>
  );
}
