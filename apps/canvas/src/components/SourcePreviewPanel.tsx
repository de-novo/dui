import { useState } from "react";
import type { PreviewItem, SourcePreview } from "../data/sourcePreview";
import { useI18n } from "../i18n/LocaleContext";

function Contents({
  items,
  activate,
}: {
  items: PreviewItem[];
  activate: (control: number) => void;
}) {
  const { t } = useI18n();
  return items.map((item, i) => {
    if (item.kind === "text") return <span key={i}>{item.text}</span>;
    if (item.kind === "switch")
      return (
        <label key={i}>
          <input
            type="checkbox"
            role="switch"
            aria-checked={item.checked}
            aria-label={item.label || t.previewSwitch(item.control + 1)}
            checked={item.checked}
            onChange={() => activate(item.control)}
          />
          {item.label}
        </label>
      );
    // The Compose adapter emits a text caption beside its switch in a Row.
    const caption = item.children.find((child) => child.kind === "text");
    const toggle = item.children.find((child) => child.kind === "switch");
    if (item.kind === "row" && item.children.length === 2 && toggle && !toggle.label && caption)
      return (
        <label key={i}>
          {caption.text}
          <input
            type="checkbox"
            role="switch"
            aria-checked={toggle.checked}
            checked={toggle.checked}
            onChange={() => activate(toggle.control)}
          />
        </label>
      );
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
      <h3>{t.sourcePreview}</h3>
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
