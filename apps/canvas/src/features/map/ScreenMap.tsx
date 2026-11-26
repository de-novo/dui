import {
  Fragment,
  useCallback,
  useContext,
  useEffect,
  useEffectEvent,
  useLayoutEffect,
  useRef,
  useMemo,
  useState,
  type KeyboardEvent,
  type ReactNode,
  type RefObject,
} from "react";

import { Search, Map as MapIcon } from "lucide-react";
import { useI18n } from "../../i18n/LocaleContext";
import { MapGuideOptions } from "./MapCardGuides";
import { CaptureCard } from "./CaptureCard";
import { clearLane, layoutMap } from "./layout";
import { cardCell } from "../../model/selection";
import { SNAPSHOT_MIN_SCREEN_WIDTH, snapshotUsable, useSnapshotFrame } from "./snapshotSlots";
import {
  Background,
  BaseEdge,
  ControlButton,
  Controls,
  EdgeLabelRenderer,
  Handle,
  MarkerType,
  MiniMap,
  PanOnScrollMode,
  Position,
  ReactFlow,
  getSmoothStepPath,
  getViewportForBounds,
  useViewportStore,
  type Viewport,
  type FitViewOptions,
  useReactFlow,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeProps,
  type OnNodesChange,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import type { Screen, ScreenMap as Archive } from "../../data/overview";
import type { CanvasDiffVisualMode, ScreenCanvasDiff } from "../../model/canvasDiff";

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
  },
  "screen"
>;

const minMapZoom = 0.02;
const maxMapZoom = 3;

function ResponsiveOverview({
  padding,
  measured,
}: {
  padding: FitViewOptions["padding"];
  measured: boolean;
}) {
  const width = useViewportStore((state) => state.width);
  const height = useViewportStore((state) => state.height);
  const flow = useReactFlow();
  const ready = measured && flow.viewportInitialized;
  const lastFit = useRef<Viewport | null>(null);
  const refit = useEffectEvent(() => {
    const bounds = flow.getNodesBounds(flow.getNodes());
    const next = getViewportForBounds(bounds, width, height, minMapZoom, maxMapZoom, padding);
    const current = flow.getViewport();
    const previous = lastFit.current;
    // Leave a user-selected camera in place until the next explicit Fit view.
    if (
      !previous ||
      (Math.abs(current.x - previous.x) < 0.5 &&
        Math.abs(current.y - previous.y) < 0.5 &&
        Math.abs(current.zoom - previous.zoom) < 0.00001)
    ) {
      void flow.setViewport(next);
    }
    lastFit.current = next;
  });
  useLayoutEffect(() => {
    if (ready && width && height) refit();
  }, [ready, width, height, padding]);
  return null;
}

// React Flow scales the entire card, including SVG strokes. Keep guides legible
// at overview zoom without rerendering every card when the camera moves.
function MapGuideScale({ root }: { root: RefObject<HTMLDivElement | null> }) {
  const { frames, guides } = useContext(MapGuideOptions);
  const zoom = useViewportStore((state) => (frames || guides ? state.transform[2] : 1));
  useLayoutEffect(() => {
    root.current?.style.setProperty("--map-guide-unit", 1 / zoom + "px");
  }, [root, zoom]);
  return null;
}

function ZoomOneToOneButton({
  fitOptions,
  target,
  cardWidth,
  frameHeight,
}: {
  fitOptions: FitViewOptions;
  target?: { position: { x: number; y: number }; frameHeight?: number };
  cardWidth: number;
  frameHeight: number;
}) {
  const { t } = useI18n();
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
        void fitView({ ...fitOptions, duration: 200 });
      }
    };
    addEventListener("keydown", onKey);
    return () => removeEventListener("keydown", onKey);
  });

  return (
    <ControlButton
      onClick={jumpOneToOne}
      title={`${t.zoomOneToOne} (0)`}
      aria-label={t.zoomOneToOne}
    >
      <span className="zoom-1x-label">1:1</span>
    </ControlButton>
  );
}

// The last node object built for each screen. A re-measured card changes the layout, which
// rebuilds every node; a card whose node is equal to its previous one keeps that object, so
// React Flow re-renders only the cards that changed.
const previousCards = new WeakMap<object, CardNode>();
function sameCardAsBefore(node: CardNode): CardNode {
  const prior = previousCards.get(node.data.screen);
  if (
    prior &&
    prior.position.x === node.position.x &&
    prior.position.y === node.position.y &&
    prior.selected === node.selected &&
    prior.className === node.className &&
    prior.ariaLabel === node.ariaLabel &&
    shallowEqual(prior.data, node.data)
  )
    return prior;
  previousCards.set(node.data.screen, node);
  return node;
}
function shallowEqual<T extends object>(a: T, b: T): boolean {
  const keys = Object.keys(a) as (keyof T)[];
  return keys.length === Object.keys(b).length && keys.every((key) => Object.is(a[key], b[key]));
}

// Shared style objects: Handle is memoized, and a new object per render would re-render all
// eight handles of every card that renders.
const handleStyles = {
  source: { side: { top: "46%" }, axis: { left: "46%" } },
  target: { side: { top: "54%" }, axis: { left: "54%" } },
} as const;
function handleStyle(position: Position, kind: "source" | "target") {
  const sides = position === Position.Left || position === Position.Right;
  return sides ? handleStyles[kind].side : handleStyles[kind].axis;
}

function Card({
  id,
  data,
  positionAbsoluteX,
  positionAbsoluteY,
  width,
  height,
}: NodeProps<CardNode>) {
  const cell = cardCell(data.screen, data.width);
  // A faithful snapshot replaces the PNG while the card is large enough on screen and in view.
  // The selector returns a boolean, so panning and zooming re-render a card only when that
  // answer changes.
  const usable = snapshotUsable(cell?.snapshot).show === "snapshot" && !data.screenDiff;
  const wanted = useViewportStore((state) => {
    if (!usable) return false;
    const [tx, ty, zoom] = state.transform;
    const left = positionAbsoluteX * zoom + tx;
    const top = positionAbsoluteY * zoom + ty;
    const w = (width ?? data.cardWidth) * zoom;
    const h = (height ?? data.frameHeight) * zoom;
    return (
      data.cardWidth * zoom >= SNAPSHOT_MIN_SCREEN_WIDTH &&
      left < state.width &&
      left + w > 0 &&
      top < state.height &&
      top + h > 0
    );
  });
  const granted = useSnapshotFrame(id, wanted);
  return (
    <>
      {Object.values(Position).map((position) => (
        <Fragment key={position}>
          <Handle
            type="source"
            id={position}
            position={position}
            style={handleStyle(position, "source")}
          />
          <Handle
            type="target"
            id={position}
            position={position}
            style={handleStyle(position, "target")}
          />
        </Fragment>
      ))}
      <CaptureCard
        screen={data.screen}
        cell={cell}
        imageUrl={data.imageUrl}
        snapshotUrl={granted && cell?.snapshot ? data.imageUrl(cell.snapshot.file) : undefined}
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
      />
    </>
  );
}

type GutterEdgeData = {
  colGap: number;
  gutterY?: number;
  multiColumn: boolean;
  highlighted: boolean;
  labelFontSize: number;
  /** Overlay or replay-verified edges keep the primary color when focused. */
  emphasized: boolean;
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
  desktop,
  mapOptions,
  openSearch,
  railInset,
  topInset = 0,
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
  imageUrl,
  select,
  onClearSelection,
}: {
  desktop: boolean;
  mapOptions: ReactNode;
  openSearch: () => void;
  railInset: number;
  topInset?: number;
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
  imageUrl: (name: string) => string;
  select: (id: string, active?: Element | null) => void;
  onClearSelection?: () => void;
}) {
  const { t } = useI18n();
  const flowElement = useRef<HTMLDivElement>(null);
  const [miniMap, setMiniMap] = useState(false);
  const [heights, setHeights] = useState<ReadonlyMap<string, number>>(() => new Map());
  const measureNodes = useCallback<OnNodesChange<CardNode>>((changes) => {
    const sizes = changes.toArray().filter((change) => change.type === "dimensions");
    if (!sizes.length) return;
    setHeights((previous) => {
      let updated: Map<string, number> | undefined;
      for (const { id, dimensions } of sizes) {
        const height = dimensions?.height;
        if (height && previous.get(id) !== height) {
          (updated ??= new Map(previous)).set(id, height);
        }
      }
      return updated ?? previous;
    });
  }, []);
  const layout = useMemo(
    () => layoutMap(archive, width, mode, heights),
    [archive, width, mode, heights],
  );
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
  // React Flow remeasures and re-renders a node whenever its object changes. Node objects are
  // built without selection; selecting then replaces only the selected and related cards.
  // Fading the other cards during selection focus is done in CSS (`data-node-focus`).
  const baseNodes = useMemo<CardNode[]>(
    () =>
      layout.nodes.map(({ screen, position, frameHeight: nodeFrameHeight }) => {
        const screenDiff = diffsByPath?.[screen.path];
        const diffDimmed = diffActive && !screenDiff;
        const classes = [
          mode === "focus:" + screen.id ? "focus-screen" : "",
          diffDimmed ? "dimmed-screen" : "",
          screenDiff ? `diff-node-${screenDiff.kind}` : "",
        ]
          .filter(Boolean)
          .join(" ");
        return {
          id: screen.id,
          type: "screen",
          position,
          selected: false,
          data: {
            screen,
            width,
            cardWidth,
            frameHeight: nodeFrameHeight,
            selected: false,
            related: false,
            dimmed: diffDimmed,
            openComments: openCommentsByPath?.[screen.path] ?? 0,
            imageUrl,
            screenDiff,
            diffVisualMode,
            beforeRunKey,
            afterRunKey,
          },
          ariaLabel: screen.path,
          className: classes || undefined,
        };
      }),
    [
      layout.nodes,
      diffsByPath,
      diffActive,
      mode,
      width,
      cardWidth,
      openCommentsByPath,
      imageUrl,
      diffVisualMode,
      beforeRunKey,
      afterRunKey,
    ],
  );
  const nodes = useMemo<CardNode[]>(
    () =>
      baseNodes.map((node) => {
        const isSelected = node.id === selectedId;
        const isRelated = dimming && !isSelected && relatedIds.has(node.id);
        if (!isSelected && !isRelated) return sameCardAsBefore(node);
        const className = [node.className, isSelected ? "selected-screen" : "related-screen"]
          .filter(Boolean)
          .join(" ");
        return sameCardAsBefore({
          ...node,
          selected: isSelected,
          data: { ...node.data, selected: isSelected, related: isRelated },
          ariaLabel: isSelected ? `${node.data.screen.path} (selected)` : node.ariaLabel,
          className,
        });
      }),
    [baseNodes, selectedId, dimming, relatedIds],
  );
  // Edge geometry depends only on the layout. Hover and selection emphasis below reuse these
  // objects for every edge they do not touch, so React Flow re-renders only the changed edges.
  const baseEdges = useMemo<MapEdge[]>(() => {
    const positions = new Map(layout.nodes.map((n) => [n.screen.id, n.position]));
    const cardRects = layout.nodes.map((n) => ({
      x: n.position.x,
      y: n.position.y,
      w: cardWidth,
      h: heights.get(n.screen.id) ?? n.frameHeight,
    }));
    const findClearGutterY = (x1: number, x2: number, preferredY: number): number => {
      const left = Math.min(x1, x2);
      const right = Math.max(x1, x2);
      // Leave room for the curve at either end of the horizontal segment.
      const radius = 28;
      const obstacles = cardRects.filter((r) => r.x + r.w > left && r.x < right);
      return clearLane(
        preferredY,
        obstacles.map((r) => ({ top: r.y - radius, bottom: r.y + r.h + radius })),
      );
    };
    return layout.links.map(
      ({ source, target, origin, provenance, replayVerified, action }, edgeIdx) => {
        const a = positions.get(source)!,
          b = positions.get(target)!;
        const dx = Math.abs(b.x - a.x);
        const multiColumn = dx > column * 1.1;
        const interveningSibling =
          a.x === b.x &&
          cardRects.some(
            (r) => r.x === a.x && r.y > Math.min(a.y, b.y) && r.y < Math.max(a.y, b.y),
          );
        const [out, into] = interveningSibling
          ? [Position.Right, Position.Right]
          : a.x === b.x
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
        const isOverlayEdge = origin === "overlay";
        const emphasized = isOverlayEdge || Boolean(replayVerified);
        const strokeColor = emphasized ? "var(--primary)" : "var(--map-link)";
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
            )
          : preferredGutterY;
        return {
          id: JSON.stringify([source, target]),
          source,
          target,
          type: "gutter",
          zIndex: 0,
          sourceHandle: out,
          targetHandle: into,
          label: edgeLabel,
          data: {
            colGap,
            gutterY,
            multiColumn,
            highlighted: layout.total <= 6,
            labelFontSize,
            emphasized,
          },
          ariaLabel: replayVerified
            ? `Replay-verified transition from ${source} to ${target}`
            : `${originText} (${provenance}) from ${source} to ${target}, not replay-verified`,
          style: {
            stroke: strokeColor,
            strokeWidth,
            strokeDasharray,
            opacity: isOverlayEdge ? 0.72 : 0.28,
          },
          markerEnd: { type: MarkerType.ArrowClosed, color: strokeColor, width: 16, height: 16 },
        } satisfies MapEdge;
      },
    );
  }, [layout, heights, cardWidth, column, rowGap, colGap, strokeWidth, labelFontSize]);
  // The focused card's edges are drawn on top in full strength. Every other edge keeps its
  // object and is dimmed by the `data-edge-focus` rule in preview.css.
  const edges = useMemo<MapEdge[]>(() => {
    if (!activeFocusId) return baseEdges;
    return baseEdges.map((edge) => {
      if (edge.source !== activeFocusId && edge.target !== activeFocusId) return edge;
      const stroke = edge.data?.emphasized ? "var(--primary)" : "var(--foreground)";
      return {
        ...edge,
        zIndex: 10,
        className: "edge-focus",
        data: { ...edge.data!, highlighted: true },
        style: { ...edge.style, stroke, strokeWidth: Math.round(strokeWidth * 1.7), opacity: 1 },
        markerEnd: { type: MarkerType.ArrowClosed, color: stroke, width: 16, height: 16 },
      };
    });
  }, [baseEdges, activeFocusId, strokeWidth]);

  // Stable handlers: React Flow re-renders every node when a handler prop changes identity.
  const onNodeMouseEnter = useCallback((_: unknown, node: CardNode) => setHoveredId(node.id), []);
  const onNodeMouseLeave = useCallback(() => setHoveredId(null), []);
  // `select` is a new function on every parent render; read the latest one at click time.
  const selectRef = useRef(select);
  useLayoutEffect(() => {
    selectRef.current = select;
  });
  const onNodeClick = useCallback((_: unknown, node: CardNode) => selectRef.current(node.id), []);
  const onPaneClick = useCallback(() => {
    if (focusActive) onClearSelection?.();
  }, [focusActive, onClearSelection]);

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key !== "Enter" && event.key !== " ") return;
    const node = (event.target as Element | null)?.closest?.(".react-flow__node");
    const id = node?.getAttribute("data-id");
    if (id && archive.screens.some((s) => s.id === id)) {
      event.preventDefault();
      select(id, node);
    }
  }

  const fitOptions = useMemo<FitViewOptions>(
    () => ({
      padding: {
        left: `${railInset + 18}px`,
        right: "18px",
        top: `${Math.max(48, Math.ceil(topInset) + 18)}px`,
        bottom: "18px",
      },
      minZoom: minMapZoom,
    }),
    [railInset, topInset],
  );

  return (
    <ReactFlow
      ref={flowElement}
      key={JSON.stringify([mode, width])}
      nodes={nodes}
      onNodesChange={measureNodes}
      edges={edges}
      nodeTypes={nodeTypes}
      edgeTypes={edgeTypes}
      // Edges carry no action; as Tab stops they only add focus targets without an indicator.
      edgesFocusable={false}
      fitView
      minZoom={minMapZoom}
      maxZoom={maxMapZoom}
      panOnDrag
      panOnScroll
      panOnScrollMode={PanOnScrollMode.Free}
      panOnScrollSpeed={1.2}
      zoomOnScroll={false}
      zoomOnPinch
      zoomOnDoubleClick={false}
      zoomActivationKeyCode={["Meta", "Control"]}
      fitViewOptions={fitOptions}
      nodesDraggable={false}
      nodesConnectable={false}
      onKeyDown={handleKeyDown}
      data-edge-focus={edgeFocusActive ? "" : undefined}
      data-node-focus={dimming ? "" : undefined}
      onNodeMouseEnter={onNodeMouseEnter}
      onNodeMouseLeave={onNodeMouseLeave}
      onNodeClick={onNodeClick}
      onPaneClick={onPaneClick}
    >
      <MapGuideScale root={flowElement} />
      <ResponsiveOverview
        padding={fitOptions.padding}
        measured={nodes.length > 0 && nodes.every((node) => heights.has(node.id))}
      />
      <Background color="var(--map-grid)" gap={20} size={1} />
      {desktop && miniMap && (
        <MiniMap
          position="top-right"
          pannable
          zoomable
          ariaLabel={t.miniMap}
          bgColor="var(--card)"
          nodeColor="var(--muted-foreground)"
          maskColor="var(--glass-surface)"
        />
      )}
      <div className="map-toolbar">
        <Controls
          fitViewOptions={fitOptions}
          showInteractive={false}
          orientation={desktop ? "vertical" : "horizontal"}
        >
          {desktop && (
            <>
              <ControlButton
                onClick={openSearch}
                aria-label={t.findScreen}
                title={t.findScreen + " (⌘/Ctrl K)"}
                aria-keyshortcuts="Meta+K Control+K"
              >
                <Search aria-hidden="true" />
              </ControlButton>
              <ControlButton
                onClick={() => setMiniMap((open) => !open)}
                aria-label={t.miniMap}
                aria-pressed={miniMap}
                title={t.miniMap}
              >
                <MapIcon aria-hidden="true" />
              </ControlButton>
            </>
          )}
          <ZoomOneToOneButton
            fitOptions={fitOptions}
            target={layout.nodes.find((n) => n.screen.id === selectedId) ?? layout.nodes[0]}
            cardWidth={cardWidth}
            frameHeight={frameHeight}
          />
        </Controls>
        {mapOptions}
      </div>
    </ReactFlow>
  );
}
