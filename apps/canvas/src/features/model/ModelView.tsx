import { useRef, useState } from "react";
import { Button } from "../../components/ui/button";
import { useI18n } from "../../i18n/LocaleContext";
import { readModel, type Model, type ModelRenderResult, type SourceModelRenderer } from "./modelDocument";
import { createEngineModelRenderer } from "./engineModelRenderer";

const unavailableRenderer: SourceModelRenderer = {
  render: () => {
    const markup = '<div class="dui-engine-error" style="padding:8px;color:#b00020">dui layout engine unavailable</div>';
    return { html: markup, markup, unresolved: ["engine-unavailable"] };
  },
};
const renderer = await createEngineModelRenderer().catch(() => unavailableRenderer);

export function ModelView() {
  const { t } = useI18n();
  const [model, setModel] = useState<Model>();
  const [id, setId] = useState("");
  const [error, setError] = useState(false);
  const [preview, setPreview] = useState<ModelRenderResult>();
  const input = useRef<HTMLInputElement>(null);
  const revision = useRef(0);
  function show(m: Model, key: string) {
    try {
      const view = renderer.render(m, key);
      setPreview(view);
      setId(key);
      setError(false);
    } catch {
      setPreview(undefined);
      setError(true);
    }
  }
  async function open(file: File) {
    const version = ++revision.current;
    try {
      const m = readModel(await file.text());
      if (version !== revision.current) return;
      setModel(m);
      show(m, m.screens?.[0]?.component ?? m.components![0].id);
    } catch {
      if (version === revision.current) {
        setModel(undefined);
        setPreview(undefined);
        setError(true);
      }
    }
  }
  return (
    <main className="model-workbench">
      <header>
        <a href="?">{t.screenMap}</a>
        <h1>{t.approximateModel}</h1>
      </header>
      <div className="model-controls">
        <Button onClick={() => input.current?.click()}>{t.openModel}</Button>
        <input
          type="file"
          ref={input}
          hidden
          accept=".json,application/json"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void open(f);
            e.target.value = "";
          }}
        />
        {model && (
          <select
            aria-label={t.selectedScreenLabel}
            value={id}
            onChange={(e) => show(model, e.target.value)}
          >
            {model.components?.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        )}
      </div>
      {error && <output role="alert">{t.invalidModel}</output>}
      {preview && (
        <>
          <iframe title={t.approximateModel} sandbox="" srcDoc={preview.html} />
          {preview.unresolved.length > 0 && (
            <details>
              <summary>
                {t.discoveryLimitations} ({preview.unresolved.length})
              </summary>
              <ul>
                {preview.unresolved.map((u) => (
                  <li key={u}>
                    <code>{u}</code>
                  </li>
                ))}
              </ul>
            </details>
          )}
        </>
      )}
    </main>
  );
}
