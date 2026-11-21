export type LocaleOverflow = {
  selector: string;
  text: string;
  scrollWidth: number;
  clientWidth: number;
  scrollHeight: number;
  clientHeight: number;
};

export type LocaleAudit = {
  locale: string;
  missingKeys: string[];
  overflows: LocaleOverflow[];
  rtlExpected: boolean;
  documentDir: string;
  rtlMismatch: { expected: string; observed: string } | null;
};

export type Cell = {
  path: string;
  width: number;
  status: "verified" | "unverified" | "unrenderable";
  reason?: string;
  image?: string;
  state?: string;
  scenario?: string;
  locale?: string;
  localeAudit?: LocaleAudit;
};
export type ScreenOrigin =
  | { kind: "entry" }
  | { kind: "anchor"; from: string; selector?: string }
  | { kind: "router"; file: string };
export type Screen = {
  id: string;
  path: string;
  cells: Cell[];
  entries?: Cell[];
  discoveredFrom?: ScreenOrigin;
};
export type EdgeProvenance = "extracted" | "inferred" | "verified";
export type EdgeAction = {
  kind: string;
  selector: string;
  label?: string;
};
export type ScreenLink = {
  source: string;
  target: string;
  observedWidths?: number[];
  confirmedWidths?: number[];
  failedWidths?: Record<string, string>;
  origin?: string;
  provenance: EdgeProvenance;
  selector?: string;
  action?: EdgeAction;
  replayVerified: boolean;
};
export type ScreenMap = {
  screens: Screen[];
  links: ScreenLink[];
};

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function decodeOrigin(raw: unknown): ScreenOrigin | undefined {
  if (typeof raw === "string" && raw.length > 0) return { kind: "router", file: raw };
  if (!record(raw)) return undefined;
  if (raw.kind === "entry") return { kind: "entry" };
  if (raw.kind === "anchor" && typeof raw.from === "string") {
    return {
      kind: "anchor",
      from: raw.from,
      selector: typeof raw.selector === "string" ? raw.selector : undefined,
    };
  }
  return undefined;
}

function decodeLocaleAudit(raw: unknown): LocaleAudit | undefined {
  if (!record(raw) || typeof raw.locale !== "string") return undefined;
  const missingKeys = Array.isArray(raw.missingKeys)
    ? raw.missingKeys.filter((k): k is string => typeof k === "string")
    : [];
  const overflows: LocaleOverflow[] = Array.isArray(raw.overflows)
    ? raw.overflows
        .filter((o): o is Record<string, unknown> => record(o) && typeof o.selector === "string")
        .map((o) => ({
          selector: o.selector as string,
          text: typeof o.text === "string" ? o.text : "",
          scrollWidth: typeof o.scrollWidth === "number" ? o.scrollWidth : 0,
          clientWidth: typeof o.clientWidth === "number" ? o.clientWidth : 0,
          scrollHeight: typeof o.scrollHeight === "number" ? o.scrollHeight : 0,
          clientHeight: typeof o.clientHeight === "number" ? o.clientHeight : 0,
        }))
    : [];
  const rtlMismatch =
    record(raw.rtlMismatch) &&
    typeof raw.rtlMismatch.expected === "string" &&
    typeof raw.rtlMismatch.observed === "string"
      ? { expected: raw.rtlMismatch.expected, observed: raw.rtlMismatch.observed }
      : null;
  return {
    locale: raw.locale,
    missingKeys,
    overflows,
    rtlExpected: raw.rtlExpected === true,
    documentDir: typeof raw.documentDir === "string" ? raw.documentDir : "ltr",
    rtlMismatch,
  };
}

export function decodeOverview(raw: unknown): ScreenMap {
  if (
    !record(raw) ||
    !Array.isArray(raw.nodes) ||
    !Array.isArray(raw.overview) ||
    !Array.isArray(raw.edges)
  )
    throw Error("Invalid archive graph");
  const screens: Screen[] = [];
  const byPath = new Map<string, Screen>();
  for (const node of raw.nodes) {
    if (!record(node) || node.kind !== "screen") continue;
    if (typeof node.id !== "string" || typeof node.label !== "string")
      throw Error("Invalid screen");
    const screen: Screen = {
      id: node.id,
      path: node.label,
      cells: [],
      discoveredFrom: decodeOrigin(node.discoveredFrom),
    };
    byPath.set(screen.path, screen);
    screens.push(screen);
  }
  const detailed = raw.cells !== undefined;
  if (detailed && !Array.isArray(raw.cells)) throw Error("Invalid archive cells");
  const sources = detailed ? [raw.overview, raw.cells as unknown[]] : [raw.overview];
  for (const [index, source] of sources.entries()) {
    for (const cell of source) {
      if (
        !record(cell) ||
        typeof cell.path !== "string" ||
        typeof cell.width !== "number" ||
        !Number.isInteger(cell.width) ||
        cell.width <= 0 ||
        (cell.status !== "verified" &&
          cell.status !== "unverified" &&
          cell.status !== "unrenderable")
      )
        throw Error("Invalid capture in archive overview");
      const screen = byPath.get(cell.path);
      if (!screen) throw Error("Capture references a missing screen");
      const values = detailed && index === 0 ? (screen.entries ??= []) : screen.cells;
      values.push({
        state: typeof cell.state === "string" ? cell.state : undefined,
        scenario: typeof cell.scenario === "string" ? cell.scenario : undefined,
        path: cell.path,
        width: cell.width,
        status: cell.status,
        image: typeof cell.image === "string" ? cell.image : undefined,
        reason: typeof cell.reason === "string" ? cell.reason : undefined,
        locale: typeof cell.locale === "string" ? cell.locale : undefined,
        localeAudit: decodeLocaleAudit(cell.localeAudit),
      });
    }
  }
  const ids = new Set(screens.map((s) => s.id));
  const links: ScreenLink[] = [];
  for (const edge of raw.edges) {
    if (!record(edge) || edge.kind !== "linksTo") continue;
    if (
      typeof edge.source !== "string" ||
      typeof edge.target !== "string" ||
      !ids.has(edge.source) ||
      !ids.has(edge.target)
    )
      throw Error("Invalid discovery link");
    const widths = edge.observedWidths;
    if (
      widths !== undefined &&
      (!Array.isArray(widths) ||
        !widths.every((w) => typeof w === "number" && Number.isInteger(w) && w > 0))
    )
      throw Error("Invalid discovery link widths");
    const confirmedWidths =
      Array.isArray(edge.confirmedWidths) &&
      edge.confirmedWidths.every((w) => typeof w === "number" && Number.isInteger(w) && w > 0)
        ? edge.confirmedWidths
        : undefined;
    const failedWidths = record(edge.failedWidths)
      ? Object.fromEntries(
          Object.entries(edge.failedWidths).filter(
            (entry): entry is [string, string] => typeof entry[1] === "string",
          ),
        )
      : undefined;
    const action =
      record(edge.action) &&
      typeof edge.action.kind === "string" &&
      typeof edge.action.selector === "string"
        ? {
            kind: edge.action.kind,
            selector: edge.action.selector,
            label: typeof edge.action.label === "string" ? edge.action.label : undefined,
          }
        : undefined;
    const replayVerified = edge.replayVerified === true && edge.provenance === "verified";
    const provenance: EdgeProvenance = replayVerified
      ? "verified"
      : edge.provenance === "inferred" || edge.origin === "inferred"
        ? "inferred"
        : "extracted";
    const link: ScreenLink = {
      source: edge.source,
      target: edge.target,
      observedWidths: widths,
      origin: typeof edge.origin === "string" ? edge.origin : undefined,
      provenance,
      selector: typeof edge.selector === "string" ? edge.selector : action?.selector,
      replayVerified,
    };
    if (confirmedWidths) link.confirmedWidths = confirmedWidths;
    if (failedWidths && Object.keys(failedWidths).length > 0) link.failedWidths = failedWidths;
    if (action) link.action = action;
    links.push(link);
  }
  return { screens, links };
}
