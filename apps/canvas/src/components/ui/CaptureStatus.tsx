import type { Cell } from "../../data/overview";

const labels = {
  verified: "Verified",
  unverified: "Unverified",
  unrenderable: "Unrenderable",
} satisfies Record<Cell["status"], string>;

export function CaptureStatus({ cell }: { cell?: Cell }) {
  if (!cell) return <span className="capture-reason">No capture at this width</span>;
  return (
    <>
      <span className="capture-status" data-status={cell.status}>
        {labels[cell.status]}
      </span>
      {cell.reason && <span className="capture-reason">{cell.reason}</span>}
    </>
  );
}
