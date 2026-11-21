import { Fragment, useMemo, type KeyboardEvent } from "react";

import { CaptureCard } from "./CaptureCard";
import { layoutMap } from "./layout";
import {
  Background,
  ControlButton,
  Controls,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  useReactFlow,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import type { Screen, ScreenMap as Archive } from "../../data/overview";

type CardNode = Node<
  {
    screen: Screen;
    width: number;
    cardWidth: number;
    frameHeight: number;
    selected: boolean;
    imageUrl: (name: string) => string;
  },
  "screen"
>;

function ZoomOneToOneButton({
  target,
  cardWidth,
  frameHeight,
}: {
  target?: { position: { x: number; y: number } };
  cardWidth: number;
  frameHeight: number;
}) {
  const { setCenter, zoomTo } = useReactFlow();
  return (
    <ControlButton
      onClick={() => {
        if (target) {
          void setCenter(target.position.x + cardWidth / 2, target.position.y + frameHeight / 2, {
            zoom: 1,
            duration: 200,
          });
        } else {
          void zoomTo(1, { duration: 200 });
        }
      }}
      title="Zoom to 1:1 actual capture size"
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
        cardWidth={data.cardWidth}
        frameHeight={data.frameHeight}
      />
    </>
  );
}

const nodeTypes = { screen: Card };

export function ScreenMap({
  railInset,
  archive,
  width,
  mode,
  selectedId,
  imageUrl,
  select,
}: {
  railInset: number;
  archive: Archive;
  width: number;
  mode: string;
  selectedId?: string | null;
  imageUrl: (name: string) => string;
  select: (id: string, active?: Element | null) => void;
}) {
  const layout = useMemo(() => layoutMap(archive, width, mode), [archive, width, mode]);
  const { cardWidth, frameHeight } = layout.metrics;
  const strokeWidth = Math.max(2, Math.round(cardWidth / 320));
  const labelFontSize = Math.max(12, Math.round(cardWidth / 48));
  const nodes: CardNode[] = layout.nodes.map(({ screen, position }) => {
    const isSelected = screen.id === selectedId;
    const classes = [
      mode === "focus:" + screen.id ? "focus-screen" : "",
      isSelected ? "selected-screen" : "",
    ]
      .filter(Boolean)
      .join(" ");
    return {
      id: screen.id,
      type: "screen",
      position,
      selected: isSelected,
      data: { screen, width, cardWidth, frameHeight, selected: isSelected, imageUrl },
      ariaLabel: isSelected ? `${screen.path} (selected)` : screen.path,
      className: classes || undefined,
    };
  });
  const positions = new Map(layout.nodes.map((n) => [n.screen.id, n.position]));
  const edges = layout.links.map(
    ({ source, target, origin, provenance, replayVerified, action }) => {
      const a = positions.get(source)!,
        b = positions.get(target)!;
      const [out, into] =
        a.x === b.x
          ? a.y < b.y
            ? [Position.Bottom, Position.Top]
            : [Position.Top, Position.Bottom]
          : a.x < b.x
            ? [Position.Right, Position.Left]
            : [Position.Left, Position.Right];
      const originText = origin === "anchor" ? "Discovered anchor" : "Discovered link";
      const strokeColor = replayVerified ? "var(--primary)" : "var(--map-link)";
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
      return {
        id: JSON.stringify([source, target]),
        source,
        target,
        type: "smoothstep",
        sourceHandle: out,
        targetHandle: into,
        label: edgeLabel,
        labelStyle: { fill: "var(--foreground)", fontSize: labelFontSize, fontWeight: 500 },
        labelBgStyle: { fill: "var(--card)", fillOpacity: 0.94, stroke: "var(--border)" },
        labelBgPadding: [8, 4] as [number, number],
        labelBgBorderRadius: 6,
        ariaLabel: replayVerified
          ? `Replay-verified transition from ${source} to ${target}`
          : `${originText} (${provenance}) from ${source} to ${target}, not replay-verified`,
        style: { stroke: strokeColor, strokeWidth, strokeDasharray },
        markerEnd: { type: MarkerType.ArrowClosed, color: strokeColor, width: 18, height: 18 },
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
      fitView
      minZoom={0.02}
      maxZoom={3}
      panOnDrag
      zoomOnScroll
      zoomOnPinch
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
      onNodeClick={(_, node) => select(node.id)}
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
