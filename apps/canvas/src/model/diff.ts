export type CellDiffKind = "added" | "removed" | "changed";

export type CellSideSummary = {
  status: string;
  reason?: string;
  image?: string;
  cacheKey?: string;
};

export type StatusChange = {
  from: string;
  to: string;
  fromReason?: string;
  toReason?: string;
};

export type PixelBounds = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type PixelDiffMetrics = {
  diffPixels: number;
  totalPixels: number;
  diffRatio: number;
  width: number;
  height: number;
  changedBounds?: PixelBounds;
  overlayImage?: string;
};

export type CellDiffEntry = {
  kind: CellDiffKind;
  path: string;
  scenario: string;
  state: string;
  width: number;
  locale?: string;
  before?: CellSideSummary;
  after?: CellSideSummary;
  changedFiles: string[];
  statusChange?: StatusChange;
  pixelDiff?: PixelDiffMetrics;
};

export type ScreenDiffGroup = {
  path: string;
  cells: CellDiffEntry[];
};

export type CaptureDiffSummary = {
  totalChanged: number;
  added: number;
  removed: number;
  changed: number;
  unchanged: number;
  screensAffected: number;
};

export type CaptureDiff = {
  beforeRunKey: string;
  afterRunKey: string;
  summary: CaptureDiffSummary;
  screens: ScreenDiffGroup[];
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function asNumber(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function decodeSide(raw: unknown): CellSideSummary | undefined {
  if (!isRecord(raw)) return undefined;
  const status = asString(raw.status);
  if (!status) return undefined;
  return {
    status,
    reason: asString(raw.reason),
    image: asString(raw.image),
    cacheKey: asString(raw.cacheKey),
  };
}

function decodeStatusChange(raw: unknown): StatusChange | undefined {
  if (!isRecord(raw)) return undefined;
  const from = asString(raw.from);
  const to = asString(raw.to);
  if (!from || !to) return undefined;
  return {
    from,
    to,
    fromReason: asString(raw.fromReason),
    toReason: asString(raw.toReason),
  };
}

function decodePixelDiff(raw: unknown): PixelDiffMetrics | undefined {
  if (!isRecord(raw)) return undefined;
  const diffPixels = asNumber(raw.diffPixels);
  const totalPixels = asNumber(raw.totalPixels);
  const diffRatio = asNumber(raw.diffRatio);
  const width = asNumber(raw.width);
  const height = asNumber(raw.height);
  if (
    diffPixels === undefined ||
    totalPixels === undefined ||
    diffRatio === undefined ||
    width === undefined ||
    height === undefined
  ) {
    return undefined;
  }
  let changedBounds: PixelBounds | undefined;
  if (isRecord(raw.changedBounds)) {
    const x = asNumber(raw.changedBounds.x);
    const y = asNumber(raw.changedBounds.y);
    const bw = asNumber(raw.changedBounds.width);
    const bh = asNumber(raw.changedBounds.height);
    if (x !== undefined && y !== undefined && bw !== undefined && bh !== undefined) {
      changedBounds = { x, y, width: bw, height: bh };
    }
  }
  return {
    diffPixels,
    totalPixels,
    diffRatio,
    width,
    height,
    changedBounds,
    overlayImage: asString(raw.overlayImage),
  };
}

export function decodeCaptureDiff(raw: unknown): CaptureDiff {
  if (!isRecord(raw) || !isRecord(raw.summary) || !Array.isArray(raw.screens)) {
    throw Error("Invalid capture diff response");
  }
  const summary: CaptureDiffSummary = {
    totalChanged: asNumber(raw.summary.totalChanged) ?? 0,
    added: asNumber(raw.summary.added) ?? 0,
    removed: asNumber(raw.summary.removed) ?? 0,
    changed: asNumber(raw.summary.changed) ?? 0,
    unchanged: asNumber(raw.summary.unchanged) ?? 0,
    screensAffected: asNumber(raw.summary.screensAffected) ?? 0,
  };
  const screens: ScreenDiffGroup[] = [];
  for (const rawScreen of raw.screens) {
    if (!isRecord(rawScreen) || !Array.isArray(rawScreen.cells)) continue;
    const path = asString(rawScreen.path) ?? "/";
    const cells: CellDiffEntry[] = [];
    for (const rawCell of rawScreen.cells) {
      if (!isRecord(rawCell)) continue;
      const kind = rawCell.kind;
      if (kind !== "added" && kind !== "removed" && kind !== "changed") continue;
      const before = decodeSide(rawCell.before);
      const after = decodeSide(rawCell.after);
      const statusChange = decodeStatusChange(rawCell.statusChange);
      const canPixelDiff =
        kind === "changed" && before?.status === "verified" && after?.status === "verified";
      cells.push({
        kind,
        path: asString(rawCell.path) ?? path,
        scenario: asString(rawCell.scenario) ?? "default",
        state: asString(rawCell.state) ?? "",
        width: asNumber(rawCell.width) ?? 0,
        locale: asString(rawCell.locale),
        before,
        after,
        changedFiles: Array.isArray(rawCell.changedFiles)
          ? rawCell.changedFiles.filter((f): f is string => typeof f === "string")
          : [],
        statusChange,
        pixelDiff: canPixelDiff ? decodePixelDiff(rawCell.pixelDiff) : undefined,
      });
    }
    screens.push({ path, cells });
  }
  return {
    beforeRunKey: asString(raw.beforeRunKey) ?? "",
    afterRunKey: asString(raw.afterRunKey) ?? "",
    summary,
    screens,
  };
}
