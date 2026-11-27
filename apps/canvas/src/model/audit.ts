export type AuditGap = {
  path: string;
  state?: string;
  width: number;
  status: "unverified" | "unrenderable" | "not-captured";
  reason: string;
  locale?: string;
};

export type AuditSummary = {
  totalCells: number;
  plannedCells: number;
  capturedCells: number;
  capturedRenderableCells: number;
  notCapturedCells: number;
  verifiedCells: number;
  renderableCells: number;
  unverifiedCells: number;
  unrenderableCells: number;
  verifiedPercent: number;
  renderablePercent: number;
  branchOutcomes: number;
  observedBranchOutcomes: number;
  missingBranchOutcomes: number;
  branchPercent: number;
  openComments: number;
  resolvedComments: number;
};

export type ScreenAudit = {
  path: string;
  totalCells: number;
  capturedCells: number;
  capturedRenderableCells: number;
  notCapturedCells: number;
  verifiedCells: number;
  renderableCells: number;
  unverifiedCells: number;
  unrenderableCells: number;
  verifiedPercent: number;
  openComments: number;
};

export type AuditReport = {
  goalPercent: number;
  meetsGoal: boolean;
  summary: AuditSummary;
  gaps: AuditGap[];
  notCaptured: AuditGap[];
  screens: ScreenAudit[];
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function num(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function decodeGaps(raw: unknown): AuditGap[] {
  return Array.isArray(raw)
    ? raw.filter(isRecord).map((g) => ({
        path: typeof g.path === "string" ? g.path : "/",
        state: typeof g.state === "string" ? g.state : undefined,
        width: num(g.width),
        status:
          g.status === "unrenderable" || g.status === "not-captured" ? g.status : "unverified",
        reason: typeof g.reason === "string" ? g.reason : "missing-verification-evidence",
        locale: typeof g.locale === "string" ? g.locale : undefined,
      }))
    : [];
}

export function decodeAuditReport(raw: unknown): AuditReport | null {
  if (!isRecord(raw) || !isRecord(raw.summary)) return null;
  const s = raw.summary;
  const screens: ScreenAudit[] = Array.isArray(raw.screens)
    ? raw.screens.filter(isRecord).map((sc) => ({
        path: typeof sc.path === "string" ? sc.path : "/",
        totalCells: num(sc.totalCells),
        capturedCells: num(sc.capturedCells),
        capturedRenderableCells: num(sc.capturedRenderableCells),
        notCapturedCells: num(sc.notCapturedCells),
        verifiedCells: num(sc.verifiedCells),
        renderableCells: num(sc.renderableCells),
        unverifiedCells: num(sc.unverifiedCells),
        unrenderableCells: num(sc.unrenderableCells),
        verifiedPercent: num(sc.verifiedPercent),
        openComments: num(sc.openComments),
      }))
    : [];
  const gaps = decodeGaps(raw.gaps);
  const notCaptured = decodeGaps(raw.notCaptured);
  return {
    goalPercent: num(raw.goalPercent) || 99,
    meetsGoal: Boolean(raw.meetsGoal),
    summary: {
      totalCells: num(s.totalCells),
      plannedCells: num(s.plannedCells) || num(s.totalCells),
      capturedCells: num(s.capturedCells),
      capturedRenderableCells: num(s.capturedRenderableCells),
      notCapturedCells: num(s.notCapturedCells),
      verifiedCells: num(s.verifiedCells),
      renderableCells: num(s.renderableCells),
      unverifiedCells: num(s.unverifiedCells),
      unrenderableCells: num(s.unrenderableCells),
      verifiedPercent: num(s.verifiedPercent),
      renderablePercent: num(s.renderablePercent),
      branchOutcomes: num(s.branchOutcomes),
      observedBranchOutcomes: num(s.observedBranchOutcomes),
      missingBranchOutcomes: num(s.missingBranchOutcomes),
      branchPercent: num(s.branchPercent),
      openComments: num(s.openComments),
      resolvedComments: num(s.resolvedComments),
    },
    gaps,
    notCaptured,
    screens,
  };
}

export function openCommentsByPath(report: AuditReport | null): Record<string, number> {
  const counts: Record<string, number> = {};
  if (!report) return counts;
  for (const sc of report.screens) {
    if (sc.openComments > 0) {
      counts[sc.path] = sc.openComments;
    }
  }
  return counts;
}
