import type { RenderNode } from "../../data/overview";
import { edgePosition, type AlignmentChange, type AlignmentEdge } from "../../model/layoutGuides";

export function LayoutOverlay({
  width,
  height,
  nodes,
  frames,
  guides,
  selected,
  change,
}: {
  width: number;
  height: number;
  nodes: RenderNode[];
  frames: boolean;
  guides: boolean;
  selected: RenderNode | null;
  change?: AlignmentChange;
}) {
  const lines: { node: RenderNode; edge: AlignmentEdge }[] = [];
  if (guides && change) {
    lines.push(
      { node: change.first, edge: change.edge },
      { node: change.second, edge: change.edge },
    );
  } else if (guides && selected) {
    for (const edge of ["top", "bottom", "left", "right"] as const)
      lines.push({ node: selected, edge });
  }
  return (
    <svg
      className="layout-overlay"
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      {frames && (
        <g className="layout-frames">
          {nodes.map((node, index) => (
            <rect key={index} {...node.rect} />
          ))}
        </g>
      )}
      {guides && change && (
        <g className="layout-focus-frames">
          <rect {...change.first.rect} />
          <rect {...change.second.rect} />
        </g>
      )}
      <g className="layout-guide-lines">
        {lines.map(({ node, edge }, index) => {
          const value = edgePosition(node, edge);
          const vertical = edge === "left" || edge === "right" || edge === "center";
          return (
            <line
              key={index}
              x1={vertical ? value : 0}
              x2={vertical ? value : width}
              y1={vertical ? 0 : value}
              y2={vertical ? height : value}
            />
          );
        })}
      </g>
    </svg>
  );
}
