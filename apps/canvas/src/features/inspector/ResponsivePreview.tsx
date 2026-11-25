import { useEffect, useRef, useState } from "react";
import { Button } from "../../components/ui/button";
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "../../components/ui/dialog";
import { useI18n } from "../../i18n/LocaleContext";

type CellKey = { state: string; width: number };
type Bounds = { width: number; height: number };
type Layout = Bounds & {
  maxSize: number;
  containers: (Bounds & { id: number; label: string })[];
  fields: {
    id: number;
    label: string;
    value: string;
    options?: { value: string; label: string; disabled: boolean }[];
  }[];
};

async function request<T>(payload: object): Promise<T> {
  const response = await fetch("/api/responsive", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  const result = await response.json();
  if (!response.ok) throw Error(result.reason || response.statusText);
  return result;
}

function MountedScreen({ run, cell }: { run: string; cell: CellKey }) {
  const { t } = useI18n();
  const session = useRef("");
  const revision = useRef(0);
  const mounted = useRef(false);
  const [layout, setLayout] = useState<Layout>();
  const [image, setImage] = useState("");
  const [target, setTarget] = useState(0);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const { state, width } = cell;
  useEffect(() => {
    let closed = false;
    let id = "";
    mounted.current = true;
    const cleanup = () => {
      if (id) void request({ op: "close", session: id }).catch(() => {});
    };
    function leave() {
      if (id)
        navigator.sendBeacon(
          "/api/responsive",
          new Blob([JSON.stringify({ op: "close", session: id })], { type: "application/json" }),
        );
    }
    window.addEventListener("pagehide", leave);
    async function open() {
      const selection = { state, width };
      const opened = await request<{ session: string; revision: number }>({
        op: "open",
        cell: selection,
        run,
        requestId: crypto.randomUUID(),
      });
      id = opened.session;
      if (closed) {
        cleanup();
        return;
      }
      session.current = id;
      revision.current = opened.revision;
      const next = await request<Layout>({ op: "layout", session: id, cell: selection });
      const frame = await request<{ imageBase64: string }>({
        op: "frame",
        session: id,
        cell: selection,
      });
      if (!closed) {
        setLayout(next);
        setImage(frame.imageBase64);
      }
    }
    void open()
      .catch((e) => {
        if (!closed) setError(String(e.message));
      })
      .finally(() => {
        if (!closed) setBusy(false);
      });
    return () => {
      closed = true;
      mounted.current = false;
      window.removeEventListener("pagehide", leave);
      cleanup();
    };
  }, [run, state, width]);

  async function change(payload: object) {
    setBusy(true);
    setError("");
    const address = { session: session.current, cell };
    try {
      const result = await request<{ revision: number; layout: Layout }>({
        ...address,
        ...payload,
        expectedRevision: revision.current,
        requestId: crypto.randomUUID(),
      });
      revision.current = result.revision;
      const frame = await request<{ imageBase64: string }>({ ...address, op: "frame" });
      if (mounted.current) {
        setLayout(result.layout);
        setImage(frame.imageBase64);
      }
    } catch (e) {
      if (mounted.current) setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }
  const container = layout?.containers.find((c) => c.id === target);
  const size = container ?? layout;
  return (
    <>
      {busy && !layout && <output>{t.loadingLivePreview}</output>}
      {error && <p role="alert">{error}</p>}
      {size && layout && (
        <>
          {layout.containers.length > 0 && (
            <label>
              {t.constraint}
              <select
                disabled={busy}
                value={container?.id ?? 0}
                onChange={(e) => setTarget(Number(e.target.value))}
              >
                <option value={0}>{t.viewport}</option>
                {layout.containers.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.label} · {c.id}
                  </option>
                ))}
              </select>
            </label>
          )}
          <form
            className="responsive-dimensions"
            key={JSON.stringify([container?.id, size.width, size.height])}
            onSubmit={(e) => {
              e.preventDefault();
              const data = new FormData(e.currentTarget);
              const width = Number(data.get("width")),
                height = Number(data.get("height"));
              const dims = {
                ...(width !== Math.round(size.width) && { width }),
                ...(height !== Math.round(size.height) && { height }),
              };
              if (Object.keys(dims).length)
                void change({ op: "resize", container: container?.id, ...dims });
            }}
          >
            <label>
              {t.width} (px)
              <input
                name="width"
                type="number"
                min="1"
                max={layout.maxSize}
                required
                defaultValue={Math.round(size.width)}
                disabled={busy}
              />
            </label>
            <label>
              {t.height} (px)
              <input
                name="height"
                type="number"
                min="1"
                max={layout.maxSize}
                required
                defaultValue={Math.round(size.height)}
                disabled={busy}
              />
            </label>
            <Button type="submit" disabled={busy}>
              {t.applyLayout}
            </Button>
          </form>
          {layout.fields.map((f) => (
            <form
              className="responsive-dimensions"
              key={JSON.stringify([f.id, f.value])}
              onSubmit={(e) => {
                e.preventDefault();
                void change({
                  op: "fill",
                  field: f.id,
                  text: new FormData(e.currentTarget).get("value"),
                });
              }}
            >
              <label>
                {f.label}
                {f.options ? (
                  <select name="value" defaultValue={f.value} disabled={busy}>
                    {f.options.map((opt, i) => (
                      <option key={i} value={opt.value} disabled={opt.disabled}>
                        {opt.label}
                      </option>
                    ))}
                  </select>
                ) : (
                  <input name="value" autoComplete="off" defaultValue={f.value} disabled={busy} />
                )}
              </label>
              <Button type="submit" disabled={busy}>
                {t.applyLayout}
              </Button>
            </form>
          ))}
        </>
      )}
      {image && (
        <img
          className="responsive-frame"
          aria-busy={busy}
          width={layout?.width}
          height={layout?.height}
          alt={t.livePreview}
          src={"data:image/png;base64," + image}
        />
      )}
    </>
  );
}
export function ResponsivePreview({ run, cell }: { run: string; cell: CellKey }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger render={<Button variant="outline" />}>{t.responsivePreview}</DialogTrigger>
      <DialogContent className="responsive-dialog">
        <DialogTitle>{t.responsivePreview}</DialogTitle>
        {open && <MountedScreen run={run} cell={cell} />}
      </DialogContent>
    </Dialog>
  );
}
