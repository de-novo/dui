import { useMemo, useState, type MouseEvent } from "react";
import type { Cell } from "../../data/overview";
import { useI18n } from "../../i18n/LocaleContext";
import {
  boxModelOverlayRects,
  computeSpacingGuides,
  pickNodeAtPoint,
} from "../../model/elementInspect";

export function ElementInspector({
  cell,
  imageUrl,
  externalSelectedId,
  onHighlightNode,
}: {
  cell: Cell;
  imageUrl: string;
  externalSelectedId?: string | null;
  onHighlightNode?: (nodeId: string | null) => void;
}) {
  const { t } = useI18n();
  const nodes = useMemo(() => cell.renderTree ?? [], [cell.renderTree]);
  const [localSelectedId, setLocalSelectedId] = useState<string | null>(() => nodes[0]?.id ?? null);
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const [naturalSize, setNaturalSize] = useState<{ w: number; h: number }>({
    w: Math.max(1, cell.width),
    h: Math.max(1, Math.round(cell.width * 0.75)),
  });

  const activeSelectedId = externalSelectedId ?? localSelectedId;
  const selected = useMemo(
    () => nodes.find((n) => n.id === activeSelectedId) ?? nodes[0] ?? null,
    [nodes, activeSelectedId],
  );
  const hovered = useMemo(
    () => (hoveredId ? (nodes.find((n) => n.id === hoveredId) ?? null) : null),
    [nodes, hoveredId],
  );

  if (nodes.length === 0) return null;

  const stageW = Math.max(1, cell.width);
  const stageH = Math.max(1, Math.round((naturalSize.h / Math.max(1, naturalSize.w)) * stageW));

  function pointToCapture(event: MouseEvent<HTMLDivElement>): { x: number; y: number } {
    const box = event.currentTarget.getBoundingClientRect();
    const rx = (event.clientX - box.left) / Math.max(1, box.width);
    const ry = (event.clientY - box.top) / Math.max(1, box.height);
    return { x: rx * stageW, y: ry * stageH };
  }

  function handleMove(event: MouseEvent<HTMLDivElement>) {
    const pt = pointToCapture(event);
    const hit = pickNodeAtPoint(nodes, pt.x, pt.y);
    const nextHover = hit?.id ?? null;
    setHoveredId(nextHover);
    onHighlightNode?.(nextHover ?? selected?.id ?? null);
  }

  function handleClick(event: MouseEvent<HTMLDivElement>) {
    const pt = pointToCapture(event);
    const hit = pickNodeAtPoint(nodes, pt.x, pt.y);
    if (hit) {
      setLocalSelectedId(hit.id);
      onHighlightNode?.(hit.id);
    }
  }

  const boxes = selected ? boxModelOverlayRects(selected) : null;
  const guides = selected && hovered ? computeSpacingGuides(selected, hovered) : [];
  const pct = (val: number, total: number) => `${Math.max(0, Math.min(100, (val / total) * 100))}%`;
  const m = selected?.boxModel?.margin ?? { top: 0, right: 0, bottom: 0, left: 0 };
  const b = selected?.boxModel?.border ?? { top: 0, right: 0, bottom: 0, left: 0 };
  const p = selected?.boxModel?.padding ?? { top: 0, right: 0, bottom: 0, left: 0 };

  return (
    <section className="inspector-section element-inspector" aria-label={t.elementInspectorTitle}>
      <div className="element-inspector-header">
        <h3>{t.elementInspectorTitle}</h3>
        <span className="element-count-pill">{nodes.length}</span>
      </div>

      <div
        className="element-stage"
        onMouseMove={handleMove}
        onMouseLeave={() => {
          setHoveredId(null);
          onHighlightNode?.(selected?.id ?? null);
        }}
        onClick={handleClick}
        role="presentation"
      >
        <img
          src={imageUrl}
          alt={cell.path}
          onLoad={(e) => {
            const img = e.currentTarget;
            if (img.naturalWidth > 0 && img.naturalHeight > 0) {
              setNaturalSize({ w: img.naturalWidth, h: img.naturalHeight });
            }
          }}
        />
        {boxes && selected && (
          <div className="element-overlay-layer" aria-hidden="true">
            <span
              className="box-overlay-margin"
              style={{
                left: pct(boxes.margin.x, stageW),
                top: pct(boxes.margin.y, stageH),
                width: pct(boxes.margin.width, stageW),
                height: pct(boxes.margin.height, stageH),
              }}
            />
            <span
              className="box-overlay-padding"
              style={{
                left: pct(boxes.padding.x, stageW),
                top: pct(boxes.padding.y, stageH),
                width: pct(boxes.padding.width, stageW),
                height: pct(boxes.padding.height, stageH),
              }}
            />
            <span
              className="box-overlay-content"
              style={{
                left: pct(boxes.content.x, stageW),
                top: pct(boxes.content.y, stageH),
                width: pct(boxes.content.width, stageW),
                height: pct(boxes.content.height, stageH),
              }}
            />
            <span
              className="box-size-pill"
              style={{
                left: pct(selected.rect.x, stageW),
                top: pct(Math.max(0, selected.rect.y - 22), stageH),
              }}
            >
              {selected.tag} · {selected.rect.width} × {selected.rect.height}
            </span>
          </div>
        )}
        {hovered && hovered.id !== selected?.id && (
          <div className="element-overlay-layer" aria-hidden="true">
            <span
              className="box-overlay-hover"
              style={{
                left: pct(hovered.rect.x, stageW),
                top: pct(hovered.rect.y, stageH),
                width: pct(hovered.rect.width, stageW),
                height: pct(hovered.rect.height, stageH),
              }}
            />
            {guides.map((g, idx) => {
              const left = Math.min(g.x1, g.x2);
              const top = Math.min(g.y1, g.y2);
              const len = g.axis === "x" ? Math.abs(g.x2 - g.x1) : Math.abs(g.y2 - g.y1);
              return (
                <span
                  key={`${g.axis}:${idx}`}
                  className={`redline-guide redline-${g.axis}`}
                  style={{
                    left: pct(left, stageW),
                    top: pct(top, stageH),
                    ...(g.axis === "x"
                      ? { width: pct(len, stageW), height: "1px" }
                      : { width: "1px", height: pct(len, stageH) }),
                  }}
                >
                  <em className="redline-label">{g.distance}px</em>
                </span>
              );
            })}
          </div>
        )}
      </div>

      {selected && (
        <div className="element-spec-card">
          <div className="element-spec-title">
            <code>{selected.id}</code>
            <span>
              {selected.rect.width} × {selected.rect.height} px · ({selected.rect.x},{" "}
              {selected.rect.y})
            </span>
          </div>

          <div className="box-model-diagram" aria-label={t.boxModelLabel}>
            <div className="bm-layer bm-margin">
              <span className="bm-tag">margin</span>
              <span className="bm-top">{m.top}</span>
              <span className="bm-right">{m.right}</span>
              <span className="bm-bottom">{m.bottom}</span>
              <span className="bm-left">{m.left}</span>
              <div className="bm-layer bm-border">
                <span className="bm-tag">border</span>
                <span className="bm-top">{b.top}</span>
                <span className="bm-right">{b.right}</span>
                <span className="bm-bottom">{b.bottom}</span>
                <span className="bm-left">{b.left}</span>
                <div className="bm-layer bm-padding">
                  <span className="bm-tag">padding</span>
                  <span className="bm-top">{p.top}</span>
                  <span className="bm-right">{p.right}</span>
                  <span className="bm-bottom">{p.bottom}</span>
                  <span className="bm-left">{p.left}</span>
                  <div className="bm-content">
                    {Math.max(0, selected.rect.width - p.left - p.right - b.left - b.right)} ×{" "}
                    {Math.max(0, selected.rect.height - p.top - p.bottom - b.top - b.bottom)}
                  </div>
                </div>
              </div>
            </div>
          </div>

          <dl className="element-props-grid">
            {selected.boxModel?.gap && (
              <>
                <dt>gap</dt>
                <dd>{selected.boxModel.gap}</dd>
              </>
            )}
            {selected.boxModel?.borderRadius && (
              <>
                <dt>radius</dt>
                <dd>{selected.boxModel.borderRadius}</dd>
              </>
            )}
            {Object.entries(selected.style).map(([k, v]) => (
              <div key={k} className="element-prop-row">
                <dt>{k}</dt>
                <dd>
                  {(k === "color" || k === "backgroundColor") && (
                    <span className="color-swatch" style={{ backgroundColor: v }} />
                  )}
                  {v}
                </dd>
              </div>
            ))}
          </dl>

          {selected.attributes && Object.keys(selected.attributes).length > 0 && (
            <details className="readonly-disclosure">
              <summary>{t.elementAttributesLabel}</summary>
              <dl className="element-props-grid">
                {Object.entries(selected.attributes).map(([k, v]) => (
                  <div key={k} className="element-prop-row">
                    <dt>{k}</dt>
                    <dd>{v}</dd>
                  </div>
                ))}
              </dl>
            </details>
          )}
        </div>
      )}

      <details className="readonly-disclosure">
        <summary>
          {t.elementTreeLabel} ({nodes.length})
        </summary>
        <ul className="element-node-list">
          {nodes.slice(0, 60).map((node) => (
            <li key={node.id}>
              <button
                type="button"
                className={`element-node-btn${selected?.id === node.id ? " active" : ""}`}
                onMouseEnter={() => setHoveredId(node.id)}
                onMouseLeave={() => setHoveredId(null)}
                onClick={() => setSelectedId(node.id)}
              >
                <code>{node.id}</code>
                <span>
                  {node.rect.width}×{node.rect.height}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </details>
    </section>
  );
}
