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

export type BoxSpacing = {
  top: number;
  right: number;
  bottom: number;
  left: number;
};

export type BoxModel = {
  margin: BoxSpacing;
  padding: BoxSpacing;
  border: BoxSpacing;
  borderRadius?: string;
  gap?: string;
};

export type RenderNode = {
  id: string;
  tag: string;
  role?: string;
  text: string;
  rect: { x: number; y: number; width: number; height: number };
  style: Record<string, string>;
  parentId?: string;
  boxModel?: BoxModel;
  attributes?: Record<string, string>;
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
  renderTree?: RenderNode[];
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

function decodeSpacing(raw: unknown): BoxSpacing {
  if (!record(raw)) return { top: 0, right: 0, bottom: 0, left: 0 };
  const n = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? Math.round(v) : 0);
  return { top: n(raw.top), right: n(raw.right), bottom: n(raw.bottom), left: n(raw.left) };
}

function decodeRenderTree(raw: unknown): RenderNode[] | undefined {
  if (!Array.isArray(raw) || raw.length === 0) return undefined;
  const nodes: RenderNode[] = [];
  for (const item of raw) {
    if (!record(item) || typeof item.id !== "string" || typeof item.tag !== "string") continue;
    if (!record(item.rect)) continue;
    const r = item.rect;
    const num = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? v : 0);
    const style: Record<string, string> = {};
    if (record(item.style)) {
      for (const [k, v] of Object.entries(item.style)) {
        if (typeof v === "string") style[k] = v;
      }
    }
    let attributes: Record<string, string> | undefined;
    if (record(item.attributes)) {
      const attrs: Record<string, string> = {};
      for (const [k, v] of Object.entries(item.attributes)) {
        if (typeof v === "string") attrs[k] = v;
      }
      if (Object.keys(attrs).length > 0) attributes = attrs;
    }
    let boxModel: BoxModel | undefined;
    if (record(item.boxModel)) {
      boxModel = {
        margin: decodeSpacing(item.boxModel.margin),
        padding: decodeSpacing(item.boxModel.padding),
        border: decodeSpacing(item.boxModel.border),
        borderRadius:
          typeof item.boxModel.borderRadius === "string" ? item.boxModel.borderRadius : undefined,
        gap: typeof item.boxModel.gap === "string" ? item.boxModel.gap : undefined,
      };
    }
    nodes.push({
      id: item.id,
      tag: item.tag,
      role: typeof item.role === "string" ? item.role : undefined,
      text: typeof item.text === "string" ? item.text : "",
      rect: { x: num(r.x), y: num(r.y), width: num(r.width), height: num(r.height) },
      style,
      parentId: typeof item.parentId === "string" ? item.parentId : undefined,
      boxModel,
      attributes,
    });
  }
  return nodes.length > 0 ? nodes : undefined;
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
        renderTree: decodeRenderTree(cell.renderTree),
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
