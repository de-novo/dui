// The selected screen's HTML snapshot laid out at any width (docs/en/web-viewer.md#width-preview).
// A snapshot keeps its style sheets, so the browser reflows it with its media queries; layout
// that scripts choose is not in it. Only captured widths are evidence; the rest is a preview.
import { useLayoutEffect, useRef, useState } from "react";
import { Button } from "../../components/ui/button";
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "../../components/ui/dialog";
import { archiveFileUrl } from "../../data/archiveUrls";
import type { Cell } from "../../data/overview";
import { useI18n } from "../../i18n/LocaleContext";
import { SNAPSHOT_MAX_MISMATCH } from "../map/snapshotSlots";
import { nearestSource } from "../../model/widthPreview";

const MIN_WIDTH = 320;
const percent = (value: number) => `${(value * 100).toFixed(2)} %`;

function Stage({ runKey, sources, initial }: { runKey: string; sources: Cell[]; initial: number }) {
  const { t } = useI18n();
  const [width, setWidth] = useState(initial);
  const stageRef = useRef<HTMLDivElement>(null);
  const [stage, setStage] = useState({ w: 0, h: 0 });
  useLayoutEffect(() => {
    const node = stageRef.current;
    if (!node) return;
    const observer = new ResizeObserver(([entry]) =>
      setStage({ w: entry!.contentRect.width, h: entry!.contentRect.height }),
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, []);
  const maxWidth = Math.max(1920, ...sources.map((c) => c.width));
  const source = nearestSource(sources, width)!;
  const snapshot = source.snapshot!;
  const captured = sources.some((c) => c.width === width);
  const scale = stage.w > 0 ? Math.min(1, stage.w / width) : 1;
  const scriptDriven = sources.some((c) =>
    (c.snapshot?.reflow ?? []).some((r) => r.mismatch > SNAPSHOT_MAX_MISMATCH),
  );
  const setClamped = (value: number) =>
    Number.isFinite(value) && setWidth(Math.round(Math.min(maxWidth, Math.max(MIN_WIDTH, value))));
  return (
    <>
      <div className="width-preview-controls">
        <label>
          {t.widthPreviewWidth}
          <input
            type="number"
            min={MIN_WIDTH}
            max={maxWidth}
            value={width}
            data-width-preview-input=""
            onChange={(e) => setClamped(e.currentTarget.valueAsNumber)}
          />
        </label>
        <input
          type="range"
          aria-label={t.widthPreviewWidth}
          min={MIN_WIDTH}
          max={maxWidth}
          value={width}
          onChange={(e) => setClamped(e.currentTarget.valueAsNumber)}
        />
        <fieldset className="width-preview-captured">
          <legend className="sr-only">{t.capturedWidths}</legend>
          {sources.map((c) => (
            <Button
              key={c.width}
              size="sm"
              variant="outline"
              aria-pressed={c.width === width}
              onClick={() => setWidth(c.width)}
            >
              {c.width} px
            </Button>
          ))}
        </fieldset>
      </div>
      <p
        className="width-preview-status"
        data-width-preview-status={captured ? "captured" : "reflow"}
      >
        {captured
          ? t.widthCaptured(width, percent(snapshot.fidelity ?? 0))
          : t.widthReflowed(source.width, width)}
      </p>
      <p className="width-preview-agreement" data-script-driven={scriptDriven ? "true" : undefined}>
        {snapshot.reflow?.length
          ? t.reflowAgreement(
              snapshot.reflow.map((r) => `${r.width} px ${percent(r.mismatch)}`).join(" · "),
            )
          : t.reflowUnmeasured}
        {scriptDriven && <strong> {t.reflowWarning}</strong>}
      </p>
      <div ref={stageRef} className="width-preview-stage">
        {stage.w > 0 && (
          <iframe
            className="width-preview-frame"
            src={archiveFileUrl(snapshot.file, runKey)}
            sandbox=""
            title={t.widthPreview}
            style={{
              width: `${width}px`,
              height: `${stage.h / scale}px`,
              left: `${Math.max(0, (stage.w - width * scale) / 2)}px`,
              transform: `scale(${scale})`,
            }}
          />
        )}
      </div>
    </>
  );
}

export function WidthPreview({
  runKey,
  sources,
  width,
}: {
  runKey: string;
  sources: Cell[];
  width: number;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger render={<Button variant="outline" data-width-preview-open="" />}>
        {t.widthPreview}
      </DialogTrigger>
      <DialogContent className="width-preview-dialog">
        <DialogTitle>{t.widthPreview}</DialogTitle>
        {open && <Stage runKey={runKey} sources={sources} initial={width} />}
      </DialogContent>
    </Dialog>
  );
}
