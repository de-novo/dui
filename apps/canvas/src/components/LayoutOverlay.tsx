import type { RenderNode } from "../data/overview";
import {
  edgePosition,
  type AlignmentChange,
  type AlignmentEdge,
  type GuideLine,
} from "../model/layoutGuides";

export function LayoutOverlay({
  width,
  height,
  nodes,
  frames,
  guides,
  selected,
  referenceLines = [],
  change,
}: {
  width: number;
  height: number;
  nodes: RenderNode[];
  frames: boolean;
  guides: boolean;
  selected?: RenderNode | null;
  referenceLines?: GuideLine[];
  change?: AlignmentChange;
}) {
  const lines: GuideLine[] = guides ? [...referenceLines] : [];
  function addLine(node: RenderNode, edge: AlignmentEdge) {
    lines.push({
      vertical: edge === "left" || edge === "right" || edge === "center",
      position: edgePosition(node, edge),
    });
  }
  if (guides && change) {
    addLine(change.first, change.edge);
    addLine(change.second, change.edge);
  } else if (guides && selected) {
    for (const edge of ["top", "bottom", "left", "right"] as const) addLine(selected, edge);
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
        {lines.map(({ vertical, position: value }, index) => {
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
