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
  activate: Activate;
}) {
  const { t } = useI18n();
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

function Contents({ items, activate }: { items: PreviewItem[]; activate: Activate }) {
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
          <p className="source-preview-note">{t.sourcePreviewNotNative}</p>
          <div className="preview-column source-preview-schematic">
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
