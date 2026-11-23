export type AuditGap = {
  path: string;
  state?: string;
  width: number;
  status: "unverified" | "unrenderable";
  reason: string;
  locale?: string;
};

export type AuditSummary = {
  totalCells: number;
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
  verifiedCells: number;
  renderableCells: number;
  unverifiedCells: number;
  unrenderableCells: number;
  verifiedPercent: number;
  openComments: number;
};

export type DesignFinding = {
  kind: string;
  severity: "error" | "warning" | "info";
  nodeId: string;
  tag: string;
  rect: { x: number; y: number; width: number; height: number };
  relatedNodeId?: string;
  summary: string;
  measured: string;
  expected: string;
  path: string;
  width: number;
  occurrences?: number;
};

export type DesignAudit = {
  total: number;
  rawTotal?: number;
  errors: number;
  warnings: number;
  info: number;
  findings: DesignFinding[];
};

export type AuditReport = {
  goalPercent: number;
  meetsGoal: boolean;
  summary: AuditSummary;
  gaps: AuditGap[];
  screens: ScreenAudit[];
  designAudit?: DesignAudit;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function num(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

export function decodeAuditReport(raw: unknown): AuditReport | null {
  if (!isRecord(raw) || !isRecord(raw.summary)) return null;
  const s = raw.summary;
  const screens: ScreenAudit[] = Array.isArray(raw.screens)
    ? raw.screens.filter(isRecord).map((sc) => ({
        path: typeof sc.path === "string" ? sc.path : "/",
        totalCells: num(sc.totalCells),
        verifiedCells: num(sc.verifiedCells),
        renderableCells: num(sc.renderableCells),
        unverifiedCells: num(sc.unverifiedCells),
        unrenderableCells: num(sc.unrenderableCells),
        verifiedPercent: num(sc.verifiedPercent),
        openComments: num(sc.openComments),
      }))
    : [];
  const gaps: AuditGap[] = Array.isArray(raw.gaps)
    ? raw.gaps.filter(isRecord).map((g) => ({
        path: typeof g.path === "string" ? g.path : "/",
        state: typeof g.state === "string" ? g.state : undefined,
        width: num(g.width),
        status: g.status === "unrenderable" ? "unrenderable" : "unverified",
        reason: typeof g.reason === "string" ? g.reason : "missing-verification-evidence",
        locale: typeof g.locale === "string" ? g.locale : undefined,
      }))
    : [];
  const rawDesign = isRecord(raw.designAudit) ? raw.designAudit : null;
  const designFindings: DesignFinding[] =
    rawDesign && Array.isArray(rawDesign.findings)
      ? rawDesign.findings.filter(isRecord).map((f) => {
          const r = isRecord(f.rect) ? f.rect : {};
          const sev = f.severity === "error" || f.severity === "warning" ? f.severity : "info";
          return {
            kind: typeof f.kind === "string" ? f.kind : "unknown",
            severity: sev,
            nodeId: typeof f.nodeId === "string" ? f.nodeId : "",
            tag: typeof f.tag === "string" ? f.tag : "",
            rect: {
              x: num(r.x),
              y: num(r.y),
              width: num(r.width),
              height: num(r.height),
            },
            relatedNodeId: typeof f.relatedNodeId === "string" ? f.relatedNodeId : undefined,
            summary: typeof f.summary === "string" ? f.summary : "",
            measured: typeof f.measured === "string" ? f.measured : "",
            expected: typeof f.expected === "string" ? f.expected : "",
            path: typeof f.path === "string" ? f.path : "/",
            width: num(f.width),
            occurrences: typeof f.occurrences === "number" ? f.occurrences : undefined,
          };
        })
      : [];
  const designAudit: DesignAudit | undefined = rawDesign
    ? {
        total: num(rawDesign.total),
        rawTotal: typeof rawDesign.rawTotal === "number" ? rawDesign.rawTotal : undefined,
        errors: num(rawDesign.errors),
        warnings: num(rawDesign.warnings),
        info: num(rawDesign.info),
        findings: designFindings,
      }
    : undefined;
  return {
    goalPercent: num(raw.goalPercent) || 99,
    meetsGoal: Boolean(raw.meetsGoal),
    summary: {
      totalCells: num(s.totalCells),
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
    screens,
    designAudit,
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

export function designFindingsByPath(
  report: AuditReport | null,
  width?: number,
): Record<string, DesignFinding[]> {
  const grouped: Record<string, DesignFinding[]> = {};
  if (!report?.designAudit) return grouped;
  for (const f of report.designAudit.findings) {
    if (width && f.width && f.width !== width) continue;
    (grouped[f.path] ??= []).push(f);
  }
  return grouped;
}
