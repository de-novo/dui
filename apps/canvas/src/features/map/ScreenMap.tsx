import { Fragment, useMemo } from "react";

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
  const unverified = data.screen.cells.filter(
    (c) => c.status === "unverified",
  ).length;
  const unrenderable = data.screen.cells.filter(
    (c) => c.status === "unrenderable",
  ).length;
  const cell = (data.screen.entries ?? data.screen.cells).find(
    (c) => c.width === data.width,
  );
  return (
    <article className="screen-card">
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
      <strong title={data.screen.path}>{data.screen.path}</strong>
      {cell?.image && (
        <img loading="lazy" alt="" src={data.imageUrl(cell.image)} />
      )}
      {!!(unverified || unrenderable) && (
        <footer className="capture-meta">
          {unverified > 0 && <span>Unverified {unverified}</span>}
          {unrenderable > 0 && <span>Unrenderable {unrenderable}</span>}
        </footer>
      )}
    </article>
  );
}

const nodeTypes = { screen: Card };

export function ScreenMap({
  archive,
  width,
  mode,
  selected,
  imageUrl,
  select,
}: {
  archive: Archive;
  width: number;
  mode: string;
  selected?: string;
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
    className:
      selected === screen.id || mode === "focus:" + screen.id
        ? "focus-screen"
        : undefined,
  }));
  const positions = new Map(layout.nodes.map((n) => [n.screen.id, n.position]));
  const visibleLinks = selected
    ? archive.links.filter(
        (e) =>
          positions.has(e.source) &&
          positions.has(e.target) &&
          (e.source === selected || e.target === selected) &&
          (!e.observedWidths || e.observedWidths.includes(width)),
      )
    : layout.links;
  const edges = visibleLinks.map(({ source, target }) => {
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
        padding: { x: "18px", top: "48px", bottom: "18px" },
        minZoom: 0.04,
      }}
      nodesDraggable={false}
      nodesConnectable={false}
      onNodeClick={(_, node) => select(node.id)}
    >
      <Background color="var(--map-grid)" />
      <Controls showInteractive={false} />
    </ReactFlow>
  );
}
