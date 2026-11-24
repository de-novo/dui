import { createContext, useContext, useMemo } from "react";
import { useI18n } from "../../i18n/LocaleContext";
import { LayoutOverlay } from "../../components/LayoutOverlay";
import type { Cell, Screen } from "../../data/overview";
import {
  inspectAlignmentSeries,
  mapEvidenceCell,
  sharedControlGuides,
} from "../../model/layoutGuides";

export const MapGuideOptions = createContext({ frames: false, guides: false });

export function MapCardGuides({
  screen,
  entry,
  image,
  aspectRatio,
}: {
  screen: Screen;
  entry: Cell;
  image: string;
  aspectRatio: number;
}) {
  const { t } = useI18n();
  const { frames, guides } = useContext(MapGuideOptions);
  const evidence = useMemo(
    () => (frames || guides ? mapEvidenceCell(screen, entry, image) : undefined),
    [frames, guides, screen, entry, image],
  );
  const compare = guides && Boolean(evidence);
  const reports = useMemo(
    () => (compare ? inspectAlignmentSeries(screen.cells) : undefined),
    [compare, screen.cells],
  );
  const change = evidence && reports?.get(evidence)?.changes[0];
  const lines = useMemo(
    () =>
      guides && evidence?.renderTree && !change ? sharedControlGuides(evidence.renderTree) : [],
    [guides, evidence, change],
  );
  if (!evidence?.renderTree || (!frames && !change && !lines.length)) return null;
  return (
    <>
      <LayoutOverlay
        width={evidence.width}
        height={evidence.width / aspectRatio}
        nodes={evidence.renderTree}
        frames={frames}
        guides={guides}
        referenceLines={lines}
        change={change}
      />
      {change && (
        <span className="map-guide-delta">
          {t.guideEdges[change.edge]} · {change.offset} px
        </span>
      )}
    </>
  );
}
