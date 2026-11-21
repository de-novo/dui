import type { Cell } from "../../data/overview";
import { useI18n } from "../../i18n/LocaleContext";

export function CaptureStatus({ cell }: { cell?: Cell }) {
  const { t } = useI18n();
  if (!cell) return <span className="capture-reason">{t.noCaptureAtWidth}</span>;
  const label =
    cell.status === "verified"
      ? t.verified
      : cell.status === "unrenderable"
        ? t.unrenderable
        : t.unverified;
  return (
    <>
      <span className="capture-status" data-status={cell.status}>
        {label}
      </span>
      {cell.reason && <span className="capture-reason">{cell.reason}</span>}
    </>
  );
}
