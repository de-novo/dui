import { useState } from "react";
import type { PreviewItem, SourcePreview } from "../data/sourcePreview";
import { useI18n } from "../i18n/LocaleContext";

type Activate = (control: number) => void;

function Toggle({
  label,
  checked,
  control,
  activate,
}: {
  label: string;
  checked: boolean;
  control: number;
  activate?: Activate;
}) {
  const { t } = useI18n();
  // Map cards are read-only snapshots inside a clickable card; only the inspector operates them.
  if (!activate)
    return (
      <span className="preview-toggle">
        <span>{label}</span>
        <span className="preview-switch" data-checked={checked ? "true" : "false"} />
      </span>
    );
  return (
    <label className="preview-toggle">
      <span>{label}</span>
      <input
        type="checkbox"
        role="switch"
        className="preview-switch"
        aria-checked={checked}
        aria-label={label || t.previewSwitch(control + 1)}
        checked={checked}
        onChange={() => activate(control)}
      />
    </label>
  );
}

function Contents({ items, activate }: { items: PreviewItem[]; activate?: Activate }) {
  return items.map((item, i) => {
    if (item.kind === "text") return <span key={i}>{item.text}</span>;
    if (item.kind === "switch") return <Toggle key={i} {...item} activate={activate} />;
    // The Compose adapter emits a text caption beside its switch in a Row.
    const caption = item.children.find((child) => child.kind === "text");
    const toggle = item.children.find((child) => child.kind === "switch");
    if (item.kind === "row" && item.children.length === 2 && toggle && !toggle.label && caption)
      return <Toggle key={i} {...toggle} label={caption.text} activate={activate} />;
    return (
      <div key={i} className={"preview-" + item.kind}>
        <Contents items={item.children} activate={activate} />
      </div>
    );
  });
}

/** The first observed frame, drawn without controls for map cards. */
export function SourcePreviewSnapshot({ items }: { items: PreviewItem[] }) {
  return (
    <div className="source-preview-snapshot preview-column" aria-hidden="true">
      <Contents items={items} />
    </div>
  );
}

export function SourcePreviewPanel({ preview }: { preview: SourcePreview }) {
  const { t } = useI18n();
  const [index, setIndex] = useState(0);
  const frame = preview.status === "ready" ? preview.frames[index] : undefined;
  return (
    <section className="source-preview-panel inspector-section" aria-label={t.sourcePreview}>
      <h3>
        {t.sourcePreview} ·{" "}
        <span className="capture-status" data-status="unverified">
          {t.unverified}
        </span>
      </h3>
      {preview.status === "ready" && frame ? (
        <>
          <div className="preview-column">
            <Contents items={frame.items} activate={(id) => setIndex(frame.next[id]!)} />
          </div>
          <small>
            <code title={preview.digest}>SHA-256 {preview.digest.slice(0, 12)}</code>
          </small>
        </>
      ) : (
        preview.status !== "ready" && (
          <p>
            <strong>{t.previewStatus[preview.status]}</strong> · <code>{preview.reason}</code>
          </p>
        )
      )}
    </section>
  );
}
