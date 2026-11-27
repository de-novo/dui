import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { Model, ModelRenderResult, SourceModelRenderer } from "./modelDocument";

function useViewerColorScheme(): "light" | "dark" {
  const read = () =>
    document.documentElement.dataset.theme === "dark" ? ("dark" as const) : ("light" as const);
  const [scheme, setScheme] = useState(read);
  useEffect(() => {
    const update = () => setScheme(read());
    const observer = new MutationObserver(update);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    const media = matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", update);
    return () => {
      observer.disconnect();
      media.removeEventListener("change", update);
    };
  }, []);
  return scheme;
}

/** Render one component through the injected model renderer, rebuilding only when its inputs change. */
export function useApproximateModelRender(
  renderer: SourceModelRenderer | undefined,
  model: Model | undefined,
  componentId: string | undefined,
  width: number,
): ModelRenderResult | undefined {
  const colorScheme = useViewerColorScheme();
  return useMemo(() => {
    if (!renderer || !model || !componentId) return undefined;
    try {
      return renderer.render(model, componentId, { width, colorScheme });
    } catch {
      return undefined;
    }
  }, [renderer, model, componentId, width, colorScheme]);
}

/** Mounts the renderer's inert markup in a shadow root. It cannot receive clicks or run code. */
export function ApproximateModelFrame({
  render,
  label,
  unresolvedLabel,
  showUnresolved = true,
}: {
  render: ModelRenderResult;
  label: string;
  unresolvedLabel?: string;
  /** Mobile cards have no caption, so they keep the compact unresolved pill in the frame. */
  showUnresolved?: boolean;
}) {
  const host = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const element = host.current;
    if (!element) return;
    (element as HTMLElement & { inert: boolean }).inert = true;
    const shadow = element.shadowRoot ?? element.attachShadow({ mode: "open" });
    shadow.innerHTML = render.markup;
  }, [render.markup]);
  const unresolvedTitle = render.unresolved.join("\n");
  return (
    <div className="approximate-model-frame">
      <div ref={host} className="approximate-model-content" aria-hidden="true" />
      <span className="approximate-model-badge">{label}</span>
      {showUnresolved && render.unresolved.length > 0 && unresolvedLabel && (
        <span className="approximate-model-unresolved" title={unresolvedTitle}>
          {unresolvedLabel}
        </span>
      )}
    </div>
  );
}
