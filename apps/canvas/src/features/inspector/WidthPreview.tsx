// The selected screen at any width (docs/en/web-viewer.md#width-preview). When the project can
// run, the screen is rendered from its source at that width through the live session, scripts
// included. Otherwise the capture's HTML snapshot is laid out at that width: it keeps its style
// sheets, so media queries apply, but layout chosen by scripts is not in it.
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Button } from "../../components/ui/button";
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "../../components/ui/dialog";
import { archiveFileUrl } from "../../data/archiveUrls";
import type { Cell } from "../../data/overview";
import { useI18n } from "../../i18n/LocaleContext";
import { SNAPSHOT_MAX_MISMATCH } from "../map/snapshotSlots";
import { nearestSource } from "../../model/widthPreview";
import { request } from "./ResponsivePreview";

const MIN_WIDTH = 320;
const percent = (value: number) => `${(value * 100).toFixed(2)} %`;

/** The capture a live session mounts: its state at its captured width. */
export type LiveCell = { state: string; width: number };
type Frame = { src: string; width: number };

/** Renders the screen from source at `width`; reports a session it cannot open. */
function LiveFrame({
  run,
  cell,
  width,
  scale,
  onFrame,
  onError,
}: {
  run: string;
  cell: LiveCell;
  width: number;
  scale: number;
  onFrame: (width: number | null) => void;
  onError: (message: string) => void;
}) {
  const session = useRef("");
  const revision = useRef(0);
  const shown = useRef(0);
  const queue = useRef<Promise<void>>(Promise.resolve());
  const [ready, setReady] = useState(false);
  const [frame, setFrame] = useState<Frame>();
  const { state, width: capturedWidth } = cell;
  useEffect(() => {
    let closed = false;
    let id = "";
    const mounted = { state, width: capturedWidth };
    const leave = () => {
      if (id)
        navigator.sendBeacon(
          "/api/responsive",
          new Blob([JSON.stringify({ op: "close", session: id })], { type: "application/json" }),
        );
    };
    window.addEventListener("pagehide", leave);
    (async () => {
      const opened = await request<{ session: string; revision: number }>({
        op: "open",
        cell: mounted,
        run,
        requestId: crypto.randomUUID(),
      });
      id = opened.session;
      if (closed) {
        void request({ op: "close", session: id }).catch(() => {});
        return;
      }
      session.current = id;
      revision.current = opened.revision;
      const layout = await request<{ width: number }>({ op: "layout", session: id, cell: mounted });
      shown.current = Math.round(layout.width);
      if (!closed) setReady(true);
    })().catch((e) => {
      if (!closed) onError(e instanceof Error ? e.message : String(e));
    });
    return () => {
      closed = true;
      window.removeEventListener("pagehide", leave);
      if (id) void request({ op: "close", session: id }).catch(() => {});
    };
  }, [run, state, capturedWidth, onError]);
  useEffect(() => {
    if (!ready) return;
    onFrame(null);
    // Resizes run one after another; a slider drag settles before the next one starts.
    const timer = setTimeout(() => {
      queue.current = queue.current.then(async () => {
        const address = { session: session.current, cell: { state, width: capturedWidth } };
        try {
          if (shown.current !== width) {
            const resized = await request<{ revision: number; layout: { width: number } }>({
              ...address,
              op: "resize",
              width,
              expectedRevision: revision.current,
              requestId: crypto.randomUUID(),
            });
            revision.current = resized.revision;
            shown.current = Math.round(resized.layout.width);
          }
          const next = await request<{ imageBase64: string }>({ ...address, op: "frame" });
          const src = "data:image/png;base64," + next.imageBase64;
          const image = new Image();
          image.src = src;
          await image.decode();
          setFrame({ src, width: image.naturalWidth });
          onFrame(shown.current);
        } catch (e) {
          onError(e instanceof Error ? e.message : String(e));
        }
      });
    }, 250);
    return () => clearTimeout(timer);
  }, [ready, width, state, capturedWidth, onFrame, onError]);
  return frame ? (
    <img
      className="width-preview-live"
      alt=""
      src={frame.src}
      style={{ width: `${frame.width * scale}px` }}
    />
  ) : null;
}

function Stage({
  runKey,
  sources,
  initial,
  live,
}: {
  runKey: string;
  sources: Cell[];
  initial: number;
  live?: LiveCell;
}) {
  const { t } = useI18n();
  const [width, setWidth] = useState(initial);
  const [liveError, setLiveError] = useState("");
  const [liveWidth, setLiveWidth] = useState<number | null>(null);
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
  const source = nearestSource(sources, width);
  const snapshot = source?.snapshot;
  const captured = sources.some((c) => c.width === width);
  const scale = stage.w > 0 ? Math.min(1, stage.w / width) : 1;
  const fromSource = Boolean(live && !liveError);
  const scriptDriven = sources.some((c) =>
    (c.snapshot?.reflow ?? []).some((r) => r.mismatch > SNAPSHOT_MAX_MISMATCH),
  );
  const setClamped = (value: number) =>
    Number.isFinite(value) && setWidth(Math.round(Math.min(maxWidth, Math.max(MIN_WIDTH, value))));
  const status = fromSource ? "live" : captured ? "captured" : "reflow";
  const widths = [...new Set([...sources.map((c) => c.width), ...(live ? [live.width] : [])])];
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
          {widths
            .sort((a, b) => a - b)
            .map((w) => (
              <Button
                key={w}
                size="sm"
                variant="outline"
                aria-pressed={w === width}
                onClick={() => setWidth(w)}
              >
                {w} px
              </Button>
            ))}
        </fieldset>
      </div>
      {liveError && (
        <p role="alert" className="width-preview-status">
          {t.widthLiveFailed(liveError)}
        </p>
      )}
      <p
        className="width-preview-status"
        data-width-preview-status={status}
        aria-busy={fromSource && liveWidth !== width}
      >
        {fromSource
          ? t.widthLive(width)
          : captured && snapshot
            ? t.widthCaptured(width, percent(snapshot.fidelity ?? 0))
            : source
              ? t.widthReflowed(source.width, width)
              : t.reflowUnmeasured}
      </p>
      {!fromSource && snapshot && (
        <p
          className="width-preview-agreement"
          data-script-driven={scriptDriven ? "true" : undefined}
        >
          {snapshot.reflow?.length
            ? t.reflowAgreement(
                snapshot.reflow.map((r) => `${r.width} px ${percent(r.mismatch)}`).join(" · "),
              )
            : t.reflowUnmeasured}
          {scriptDriven && <strong> {t.reflowWarning}</strong>}
        </p>
      )}
      <div ref={stageRef} className="width-preview-stage" data-live-width={liveWidth ?? undefined}>
        {stage.w > 0 &&
          (fromSource && live ? (
            <LiveFrame
              run={runKey}
              cell={live}
              width={width}
              scale={scale}
              onFrame={setLiveWidth}
              onError={setLiveError}
            />
          ) : (
            snapshot && (
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
            )
          ))}
      </div>
    </>
  );
}

export function WidthPreview({
  runKey,
  sources,
  width,
  live,
}: {
  runKey: string;
  sources: Cell[];
  width: number;
  /** Set when the project can run: the screen is then rendered from its source. */
  live?: LiveCell;
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
        {open && <Stage runKey={runKey} sources={sources} initial={width} live={live} />}
      </DialogContent>
    </Dialog>
  );
}
