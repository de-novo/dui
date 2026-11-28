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
  height?: number,
): ModelRenderResult | undefined {
  const colorScheme = useViewerColorScheme();
  return useMemo(() => {
    if (!renderer || !model || !componentId) return undefined;
    try {
      return renderer.render(model, componentId, { width, height, colorScheme });
    } catch {
      return undefined;
    }
  }, [renderer, model, componentId, width, height, colorScheme]);
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
  const frame = useRef<HTMLDivElement>(null);
  const host = useRef<HTMLDivElement>(null);
  const [fitWidth, setFitWidth] = useState(0);
  useLayoutEffect(() => {
    const element = frame.current;
    if (!element) return;
    const measure = ([entry]: ResizeObserverEntry[]) => setFitWidth(entry?.contentRect.width ?? 0);
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  useLayoutEffect(() => {
    const element = host.current;
    if (!element) return;
    (element as HTMLElement & { inert: boolean }).inert = true;
    const shadow = element.shadowRoot ?? element.attachShadow({ mode: "open" });
    shadow.innerHTML = render.markup;
  }, [render.markup]);
  const unresolvedTitle = render.unresolved.join("\n");
  const logicalWidth = render.viewport.width;
  const logicalHeight = render.viewport.height;
  const scale = logicalWidth > 0 && fitWidth > 0 ? Math.min(1, fitWidth / logicalWidth) : 1;
  return (
    <div
      ref={frame}
      className="approximate-model-frame"
      data-viewport={logicalHeight === undefined ? undefined : "true"}
      style={
        logicalHeight === undefined
          ? undefined
          : { aspectRatio: `${logicalWidth} / ${logicalHeight}` }
      }
    >
      <div
        ref={host}
        className="approximate-model-content"
        aria-hidden="true"
        data-logical-width={logicalWidth}
        data-logical-height={logicalHeight}
        style={{
          width: `${logicalWidth}px`,
          height: logicalHeight === undefined ? undefined : `${logicalHeight}px`,
          transform: scale === 1 ? undefined : `scale(${scale})`,
        }}
      />
      <span className="approximate-model-badge">{label}</span>
      {showUnresolved && render.unresolved.length > 0 && unresolvedLabel && (
        <span className="approximate-model-unresolved" title={unresolvedTitle}>
          {unresolvedLabel}
        </span>
      )}
    </div>
  );
}
