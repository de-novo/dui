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

type CardNode = Node<{ screen: Screen; width: number }, "screen">;

function Card({ data }: NodeProps<CardNode>) {
  const cell = data.screen.cells.find((c) => c.width === data.width);
  return (
    <article className="screen-card">
      <Handle type="target" position={Position.Left} />
      <strong title={data.screen.path}>{data.screen.path}</strong>
      {cell?.image && (
        <img
          loading="lazy"
          alt=""
          src={"/archive/" + encodeURIComponent(cell.image)}
        />
      )}
      <small>
        {cell
          ? [cell.width + " px", cell.status, cell.reason]
              .filter(Boolean)
              .join(" · ")
          : "No capture at this width"}
      </small>
      <Handle type="source" position={Position.Right} />
    </article>
  );
}

const nodeTypes = { screen: Card };

export function ScreenMap({
  archive,
  width,
  select,
}: {
  archive: Archive;
  width: number;
  select: (id: string) => void;
}) {
  const columns = Math.ceil(Math.sqrt(archive.screens.length));
  const nodes: CardNode[] = archive.screens.map((screen, i) => ({
    id: screen.id,
    type: "screen",
    position: { x: (i % columns) * 440, y: Math.floor(i / columns) * 260 },
    data: { screen, width },
    ariaLabel: screen.path,
  }));
  const visibleLinks = archive.links.filter(
    (link) => !link.observedWidths || link.observedWidths.includes(width),
  );
  const edges = visibleLinks.map(({ source, target }) => ({
    id: JSON.stringify([source, target]),
    source,
    target,
    label: visibleLinks.length === 1 ? "Discovered link" : undefined,
    ariaLabel: `Discovered link from ${source} to ${target}`,
    style: { stroke: "#39735e", strokeWidth: 2 },
    markerEnd: { type: MarkerType.ArrowClosed, color: "#39735e" },
  }));
  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      fitView
      minZoom={0.08}
      fitViewOptions={{ padding: 0.12, minZoom: 0.08 }}
      nodesDraggable={false}
      nodesConnectable={false}
      onNodeClick={(_, node) => select(node.id)}
    >
      <div className="map-summary" role="status">
        {nodes.length} screens · {edges.length} discovered links
      </div>
      <Background />
      <Controls showInteractive={false} />
    </ReactFlow>
  );
}
