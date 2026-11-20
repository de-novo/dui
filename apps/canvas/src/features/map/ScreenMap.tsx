import { Fragment, useMemo } from "react";

import { CaptureCard } from "./CaptureCard";
import { layoutMap } from "./layout";
import {
  Background,
  Controls,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import type { Screen, ScreenMap as Archive } from "../../data/overview";

type CardNode = Node<
  { screen: Screen; width: number; imageUrl: (name: string) => string },
  "screen"
>;

function Card({ data }: NodeProps<CardNode>) {
  const cell = (data.screen.entries ?? data.screen.cells).find(
    (c) => c.width === data.width,
  );
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
      <CaptureCard screen={data.screen} cell={cell} imageUrl={data.imageUrl} />
    </>
  );
}

const nodeTypes = { screen: Card };

export function ScreenMap({
  railInset,
  archive,
  width,
  mode,
  imageUrl,
  select,
}: {
  railInset: number;
  archive: Archive;
  width: number;
  mode: string;
  imageUrl: (name: string) => string;
  select: (id: string) => void;
}) {
  const layout = useMemo(
    () => layoutMap(archive, width, mode),
    [archive, width, mode],
  );
  const nodes: CardNode[] = layout.nodes.map(({ screen, position }) => ({
    id: screen.id,
    type: "screen",
    position,
    data: { screen, width, imageUrl },
    ariaLabel: screen.path,
    className: mode === "focus:" + screen.id ? "focus-screen" : undefined,
  }));
  const positions = new Map(layout.nodes.map((n) => [n.screen.id, n.position]));
  const edges = layout.links.map(({ source, target }) => {
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
    return {
      id: JSON.stringify([source, target]),
      source,
      target,
      type: "smoothstep",
      sourceHandle: out,
      targetHandle: into,
      label: layout.total === 1 ? "Discovered link" : undefined,
      ariaLabel: `Discovered link from ${source} to ${target}`,
      style: { stroke: "var(--map-link)" },
      markerEnd: { type: MarkerType.ArrowClosed, color: "var(--map-link)" },
    };
  });
  return (
    <ReactFlow
      key={JSON.stringify([mode, width])}
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      fitView
      minZoom={0.04}
      fitViewOptions={{
        padding: {
          left: `${railInset + 18}px`,
          right: "18px",
          top: "48px",
          bottom: "18px",
        },
        minZoom: 0.04,
      }}
      nodesDraggable={false}
      nodesConnectable={false}
      onNodeClick={(_, node) => select(node.id)}
    >
      <Background color="var(--map-grid)" gap={20} size={1} />
      <Controls showInteractive={false} />
    </ReactFlow>
  );
}
