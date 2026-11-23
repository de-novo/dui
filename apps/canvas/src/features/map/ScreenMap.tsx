import { Fragment, useEffect, useMemo, useState, type KeyboardEvent } from "react";

import { CaptureCard } from "./CaptureCard";
import { layoutMap } from "./layout";
import {
  Background,
  BaseEdge,
  ControlButton,
  Controls,
  EdgeLabelRenderer,
  Handle,
  MarkerType,
  PanOnScrollMode,
  Position,
  ReactFlow,
  getSmoothStepPath,
  useReactFlow,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import type { Screen, ScreenMap as Archive } from "../../data/overview";
import type { CanvasDiffVisualMode, ScreenCanvasDiff } from "../../model/canvasDiff";
import type { LiveSurfaceMode } from "../../model/liveSurface";

type CardNode = Node<
  {
    screen: Screen;
    width: number;
    cardWidth: number;
    frameHeight: number;
    selected: boolean;
    related: boolean;
    dimmed: boolean;
    openComments: number;
    imageUrl: (name: string) => string;
    screenDiff?: ScreenCanvasDiff;
    diffVisualMode?: CanvasDiffVisualMode;
    beforeRunKey?: string;
    afterRunKey?: string;
    liveBaseUrl?: string;
    liveBridgePath?: string;
    liveSurfaceMode?: LiveSurfaceMode;
  },
  "screen"
>;

function ZoomOneToOneButton({
  target,
  cardWidth,
  frameHeight,
}: {
  target?: { position: { x: number; y: number }; frameHeight?: number };
  cardWidth: number;
  frameHeight: number;
}) {
  const { setCenter, zoomTo, zoomIn, zoomOut, fitView } = useReactFlow();

  const jumpOneToOne = () => {
    if (target) {
      const focusH = Math.min(target.frameHeight ?? frameHeight, 800);
      void setCenter(target.position.x + cardWidth / 2, target.position.y + focusH / 2, {
        zoom: 1,
        duration: 200,
      });
    } else {
      void zoomTo(1, { duration: 200 });
    }
  };

  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      const el = event.target as HTMLElement | null;
      if (
        el &&
        (el.tagName === "INPUT" ||
          el.tagName === "TEXTAREA" ||
          el.tagName === "SELECT" ||
          el.isContentEditable)
      ) {
        return;
      }
      if (event.key === "+" || event.key === "=") {
        event.preventDefault();
        void zoomIn({ duration: 150 });
      } else if (event.key === "-") {
        event.preventDefault();
        void zoomOut({ duration: 150 });
      } else if (event.key === "0") {
        event.preventDefault();
        jumpOneToOne();
      } else if (event.key === "!" || (event.shiftKey && event.key === "1")) {
        event.preventDefault();
        void fitView({ duration: 200 });
      }
    };
    addEventListener("keydown", onKey);
    return () => removeEventListener("keydown", onKey);
  });

  return (
    <ControlButton
      onClick={jumpOneToOne}
      title="Zoom to 1:1 actual capture size (0)"
      aria-label="Zoom to 1:1 actual capture size"
    >
      <span className="zoom-1x-label">1:1</span>
    </ControlButton>
  );
}

function Card({ data }: NodeProps<CardNode>) {
  const cell = (data.screen.entries ?? data.screen.cells).find((c) => c.width === data.width);
  return (
    <>
      {Object.values(Position).map((position) => (
        <Fragment key={position}>
          <Handle
            type="source"
            id={position}
            position={position}
            style={
              position === Position.Left || position === Position.Right
                ? { top: "46%" }
                : { left: "46%" }
            }
          />
          <Handle
            type="target"
            id={position}
            position={position}
            style={
              position === Position.Left || position === Position.Right
                ? { top: "54%" }
                : { left: "54%" }
            }
          />
        </Fragment>
      ))}
      <CaptureCard
        screen={data.screen}
        cell={cell}
        imageUrl={data.imageUrl}
        selected={data.selected}
        related={data.related}
        dimmed={data.dimmed}
        cardWidth={data.cardWidth}
        frameHeight={data.frameHeight}
        openComments={data.openComments}
        screenDiff={data.screenDiff}
        diffVisualMode={data.diffVisualMode}
        beforeRunKey={data.beforeRunKey}
        afterRunKey={data.afterRunKey}
        liveBaseUrl={data.liveBaseUrl}
        liveBridgePath={data.liveBridgePath}
        liveSurfaceMode={data.liveSurfaceMode}
      />
    </>
  );
}

type GutterEdgeData = {
  colGap: number;
  gutterY?: number;
  multiColumn: boolean;
  highlighted: boolean;
  dimmed: boolean;
  labelFontSize: number;
};

type MapEdge = Edge<GutterEdgeData, "gutter">;

function buildRoundedPolyline(pts: { x: number; y: number }[], radius = 24): string {
  if (pts.length < 2) return "";
  let d = `M ${pts[0].x} ${pts[0].y}`;
  for (let i = 1; i < pts.length - 1; i += 1) {
    const prev = pts[i - 1];
    const curr = pts[i];
    const next = pts[i + 1];
    const dx1 = curr.x - prev.x;
    const dy1 = curr.y - prev.y;
    const dx2 = next.x - curr.x;
    const dy2 = next.y - curr.y;
    const len1 = Math.hypot(dx1, dy1);
    const len2 = Math.hypot(dx2, dy2);
    if (len1 < 1 || len2 < 1) {
      d += ` L ${curr.x} ${curr.y}`;
      continue;
    }
    const r = Math.min(radius, len1 / 2, len2 / 2);
    const p1x = curr.x - (dx1 / len1) * r;
    const p1y = curr.y - (dy1 / len1) * r;
    const p2x = curr.x + (dx2 / len2) * r;
    const p2y = curr.y + (dy2 / len2) * r;
    d += ` L ${p1x} ${p1y} Q ${curr.x} ${curr.y} ${p2x} ${p2y}`;
  }
  const last = pts[pts.length - 1];
  d += ` L ${last.x} ${last.y}`;
  return d;
}

function GutterEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  style,
  markerEnd,
  label,
  data,
}: EdgeProps<MapEdge>) {
  const colGap = data?.colGap ?? 220;
  const multiColumn = Boolean(data?.multiColumn);
  const gutterY = data?.gutterY;
  let edgePath: string;
  let labelX: number;
  let labelY: number;

  if (multiColumn && gutterY !== undefined) {
    const dir = targetX >= sourceX ? 1 : -1;
    const stub = Math.min(88, Math.max(44, colGap * 0.34));
    const exitX = sourceX + dir * stub;
    const entryX = targetX - dir * stub;
    edgePath = buildRoundedPolyline(
      [
        { x: sourceX, y: sourceY },
        { x: exitX, y: sourceY },
        { x: exitX, y: gutterY },
        { x: entryX, y: gutterY },
        { x: entryX, y: targetY },
        { x: targetX, y: targetY },
      ],
      28,
    );
    labelX = (exitX + entryX) / 2;
    labelY = gutterY;
  } else {
    [edgePath, labelX, labelY] = getSmoothStepPath({
      sourceX,
      sourceY,
      sourcePosition,
      targetX,
      targetY,
      targetPosition,
      borderRadius: 24,
      offset: Math.min(80, Math.max(36, colGap * 0.28)),
    });
  }

  return (
    <>
      <BaseEdge id={id} path={edgePath} style={style} markerEnd={markerEnd} />
      {label && data?.highlighted && (
        <EdgeLabelRenderer>
          <div
            className="map-edge-label"
            style={{
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
              fontSize: `${data.labelFontSize}px`,
            }}
          >
            {label}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}

const nodeTypes = { screen: Card };
const edgeTypes = { gutter: GutterEdge };

export function ScreenMap({
  railInset,
  archive,
  width,
  mode,
  selectedId,
  focusActive = false,
  openCommentsByPath,
  diffsByPath,
  diffActive = false,
  diffVisualMode = "after",
  beforeRunKey,
  afterRunKey,
  liveBaseUrl,
  liveBridgePath,
  liveSurfaceMode = "capture",
  imageUrl,
  select,
  onClearSelection,
}: {
  railInset: number;
  archive: Archive;
  width: number;
  mode: string;
  selectedId?: string | null;
  focusActive?: boolean;
  openCommentsByPath?: Record<string, number>;
  diffsByPath?: Record<string, ScreenCanvasDiff>;
  diffActive?: boolean;
  diffVisualMode?: CanvasDiffVisualMode;
  beforeRunKey?: string;
  afterRunKey?: string;
  liveBaseUrl?: string;
  liveBridgePath?: string;
  liveSurfaceMode?: LiveSurfaceMode;
  imageUrl: (name: string) => string;
  select: (id: string, active?: Element | null) => void;
  onClearSelection?: () => void;
}) {
  const layout = useMemo(() => layoutMap(archive, width, mode), [archive, width, mode]);
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const { cardWidth, frameHeight, column, row } = layout.metrics;
  const colGap = column - cardWidth;
  const rowGap = row - frameHeight;
  const strokeWidth = Math.max(2, Math.round(cardWidth / 320));
  const labelFontSize = Math.max(12, Math.round(cardWidth / 48));
  const activeFocusId = hoveredId ?? (focusActive ? selectedId : null);
  const dimming = Boolean(focusActive && selectedId);
  const edgeFocusActive = Boolean(activeFocusId);
  const relatedIds = useMemo(() => {
    const set = new Set<string>();
    if (!focusActive || !selectedId) return set;
    set.add(selectedId);
    for (const link of layout.links) {
      if (link.source === selectedId) set.add(link.target);
      if (link.target === selectedId) set.add(link.source);
    }
    return set;
  }, [focusActive, selectedId, layout.links]);
  const nodes: CardNode[] = layout.nodes.map(
    ({ screen, position, frameHeight: nodeFrameHeight }) => {
      const screenDiff = diffsByPath?.[screen.path];
      const isSelected = screen.id === selectedId;
      const isRelated = dimming && !isSelected && relatedIds.has(screen.id);
      const isDimmed = (dimming && !relatedIds.has(screen.id)) || (diffActive && !screenDiff);
      const classes = [
        mode === "focus:" + screen.id ? "focus-screen" : "",
        isSelected ? "selected-screen" : "",
        isRelated ? "related-screen" : "",
        isDimmed ? "dimmed-screen" : "",
        screenDiff ? `diff-node-${screenDiff.kind}` : "",
      ]
        .filter(Boolean)
        .join(" ");
      return {
        id: screen.id,
        type: "screen",
        position,
        selected: isSelected,
        data: {
          screen,
          width,
          cardWidth,
          frameHeight: nodeFrameHeight,
          selected: isSelected,
          related: isRelated,
          dimmed: isDimmed,
          openComments: openCommentsByPath?.[screen.path] ?? 0,
          imageUrl,
          screenDiff,
          diffVisualMode,
          beforeRunKey,
          afterRunKey,
          liveBaseUrl,
          liveBridgePath,
          liveSurfaceMode,
        },
        ariaLabel: isSelected ? `${screen.path} (selected)` : screen.path,
        className: classes || undefined,
      };
    },
  );
  const positions = new Map(layout.nodes.map((n) => [n.screen.id, n.position]));
  const cardRects = layout.nodes.map((n) => ({
    x: n.position.x,
    y: n.position.y,
    w: cardWidth,
    h: n.frameHeight + 96,
  }));
  const findClearGutterY = (
    x1: number,
    x2: number,
    preferredY: number,
    trackIndex: number,
  ): number => {
    const loX = Math.min(x1, x2) + 8;
    const hiX = Math.max(x1, x2) - 8;
    const blocking = cardRects.filter((r) => r.x + r.w > loX && r.x < hiX);
    if (blocking.length === 0) return preferredY;
    const pad = 28;
    const isClear = (y: number) => blocking.every((r) => y < r.y - pad || y > r.y + r.h + pad);
    if (isClear(preferredY)) return preferredY;
    const minTop = Math.min(...blocking.map((r) => r.y));
    const maxBottom = Math.max(...blocking.map((r) => r.y + r.h));
    const trackStep = 28;
    const topBus = minTop - 72 - (trackIndex % 4) * trackStep;
    const bottomBus = maxBottom + 72 + (trackIndex % 4) * trackStep;
    return Math.abs(preferredY - topBus) <= Math.abs(preferredY - bottomBus) ? topBus : bottomBus;
  };
  const edges: MapEdge[] = layout.links.map(
    ({ source, target, origin, provenance, replayVerified, action }, edgeIdx) => {
      const a = positions.get(source)!,
        b = positions.get(target)!;
      const dx = Math.abs(b.x - a.x);
      const multiColumn = dx > column * 1.1;
      const [out, into] =
        a.x === b.x
          ? a.y < b.y
            ? [Position.Bottom, Position.Top]
            : [Position.Top, Position.Bottom]
          : a.x < b.x
            ? [Position.Right, Position.Left]
            : [Position.Left, Position.Right];
      const originText =
        origin === "overlay"
          ? "Discovered overlay"
          : origin === "anchor"
            ? "Discovered anchor"
            : "Discovered link";
      const isConnectedToFocus =
        edgeFocusActive && (source === activeFocusId || target === activeFocusId);
      const isEdgeDimmed = edgeFocusActive && !isConnectedToFocus;
      const isOverlayEdge = origin === "overlay";
      const strokeColor =
        isOverlayEdge || replayVerified
          ? "var(--primary)"
          : isConnectedToFocus
            ? "var(--foreground)"
            : "var(--map-link)";
      const edgeStrokeWidth = isConnectedToFocus ? Math.round(strokeWidth * 1.7) : strokeWidth;
      const strokeDasharray =
        provenance === "verified" ? undefined : provenance === "inferred" ? "2 4" : "5 4";
      const edgeLabel = action?.label
        ? action.label
        : replayVerified
          ? "Verified"
          : provenance === "inferred"
            ? "Inferred"
            : layout.total === 1
              ? "Extracted"
              : undefined;
      const trackOffset = ((edgeIdx % 3) - 1) * 18;
      const preferredGutterY = b.y - Math.max(64, Math.round(rowGap * 0.42)) + trackOffset;
      const gutterY = multiColumn
        ? findClearGutterY(
            a.x + (b.x >= a.x ? cardWidth : 0),
            b.x + (b.x >= a.x ? 0 : cardWidth),
            preferredGutterY,
            edgeIdx,
          )
        : preferredGutterY;
      const baseOpacity = isOverlayEdge ? 0.72 : 0.28;
      return {
        id: JSON.stringify([source, target]),
        source,
        target,
        type: "gutter",
        zIndex: isConnectedToFocus ? 10 : 0,
        sourceHandle: out,
        targetHandle: into,
        label: edgeLabel,
        data: {
          colGap,
          gutterY,
          multiColumn,
          highlighted: isConnectedToFocus || layout.total <= 6,
          dimmed: isEdgeDimmed,
          labelFontSize,
        },
        ariaLabel: replayVerified
          ? `Replay-verified transition from ${source} to ${target}`
          : `${originText} (${provenance}) from ${source} to ${target}, not replay-verified`,
        style: {
          stroke: strokeColor,
          strokeWidth: edgeStrokeWidth,
          strokeDasharray,
          opacity: isEdgeDimmed ? 0.08 : isConnectedToFocus ? 1 : baseOpacity,
        },
        markerEnd: { type: MarkerType.ArrowClosed, color: strokeColor, width: 16, height: 16 },
      };
    },
  );

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key !== "Enter" && event.key !== " ") return;
    const node = (event.target as Element | null)?.closest?.(".react-flow__node");
    const id = node?.getAttribute("data-id");
    if (id && archive.screens.some((s) => s.id === id)) {
      event.preventDefault();
      select(id, node);
    }
  }

  return (
    <ReactFlow
      key={JSON.stringify([mode, width])}
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      edgeTypes={edgeTypes}
      fitView
      minZoom={0.02}
      maxZoom={3}
      panOnDrag
      panOnScroll
      panOnScrollMode={PanOnScrollMode.Free}
      panOnScrollSpeed={1.2}
      zoomOnScroll={false}
      zoomOnPinch
      zoomOnDoubleClick={false}
      zoomActivationKeyCode={["Meta", "Control"]}
      fitViewOptions={{
        padding: {
          left: `${railInset + 18}px`,
          right: "18px",
          top: "48px",
          bottom: "18px",
        },
        minZoom: 0.02,
      }}
      nodesDraggable={false}
      nodesConnectable={false}
      onKeyDown={handleKeyDown}
      onNodeMouseEnter={(_, node) => setHoveredId(node.id)}
      onNodeMouseLeave={() => setHoveredId(null)}
      onNodeClick={(_, node) => select(node.id)}
      onPaneClick={() => {
        if (focusActive) onClearSelection?.();
      }}
    >
      <Background color="var(--map-grid)" gap={20} size={1} />
      <Controls showInteractive={false}>
        <ZoomOneToOneButton
          target={layout.nodes.find((n) => n.screen.id === selectedId) ?? layout.nodes[0]}
          cardWidth={cardWidth}
          frameHeight={frameHeight}
        />
      </Controls>
    </ReactFlow>
  );
}
