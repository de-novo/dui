import type { BoxModel, BoxSpacing, Cell, RenderNode } from "../data/overview";

export type LiveSurfaceMode = "capture" | "live";

export type LiveSurfaceTelemetry = {
  path: string;
  width: number;
  renderTree: RenderNode[];
  capturedAtMs: number;
};

export type LiveFrameTransform = {
  viewportWidth: number;
  viewportHeight: number;
  scale: number;
};

export function isLoopbackOrigin(rawUrl: string): boolean {
  try {
    const parsed = new URL(rawUrl);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return false;
    return parsed.hostname === "127.0.0.1" || parsed.hostname === "localhost";
  } catch {
    return false;
  }
}

export function resolveLiveBridgeUrl(
  bridgePath: string | undefined,
  screenPath: string,
): string | null {
  if (!bridgePath || !bridgePath.startsWith("/live-surface")) return null;
  const cleanBridge = bridgePath.replace(/\/+$/, "");
  const normalizedPath = screenPath.startsWith("/") ? screenPath : `/${screenPath}`;
  if (normalizedPath.startsWith("//")) return null;
  return `${cleanBridge}${normalizedPath}`;
}

export function resolveLiveSurfaceUrl(
  baseUrl: string | undefined,
  screenPath: string,
): string | null {
  if (!baseUrl || !isLoopbackOrigin(baseUrl)) return null;
  try {
    const cleanBase = baseUrl.replace(/\/+$/, "");
    const normalizedPath = screenPath.startsWith("/") ? screenPath : `/${screenPath}`;
    const resolved = new URL(normalizedPath, `${cleanBase}/`);
    if (!isLoopbackOrigin(resolved.href)) return null;
    return resolved.href;
  } catch {
    return null;
  }
}

export function computeLiveFrameTransform(
  viewportWidth: number,
  cardWidth: number,
  frameHeight: number,
): LiveFrameTransform {
  const safeVpW = Math.max(240, Math.round(viewportWidth));
  const safeCardW = Math.max(1, cardWidth);
  const safeFrameH = Math.max(1, frameHeight);
  const scale = safeCardW / safeVpW;
  const viewportHeight = Math.max(320, Math.round(safeFrameH / Math.max(scale, 0.05)));
  return {
    viewportWidth: safeVpW,
    viewportHeight,
    scale,
  };
}

export function liveSurfaceKey(path: string, width: number): string {
  return `${path}::${width}`;
}

function asRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null;
}

function decodeSpacing(raw: unknown): BoxSpacing {
  if (!asRecord(raw)) return { top: 0, right: 0, bottom: 0, left: 0 };
  const n = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? Math.round(v) : 0);
  return { top: n(raw.top), right: n(raw.right), bottom: n(raw.bottom), left: n(raw.left) };
}

function decodeBoxModel(raw: unknown): BoxModel | undefined {
  if (!asRecord(raw)) return undefined;
  return {
    margin: decodeSpacing(raw.margin),
    padding: decodeSpacing(raw.padding),
    border: decodeSpacing(raw.border),
    borderRadius: typeof raw.borderRadius === "string" ? raw.borderRadius : undefined,
    gap: typeof raw.gap === "string" ? raw.gap : undefined,
  };
}

function decodeNodes(raw: unknown): RenderNode[] {
  if (!Array.isArray(raw)) return [];
  const out: RenderNode[] = [];
  for (const item of raw.slice(0, 250)) {
    if (!asRecord(item) || typeof item.id !== "string" || typeof item.tag !== "string") continue;
    const rectRaw = asRecord(item.rect) ? item.rect : null;
    if (!rectRaw) continue;
    const style: Record<string, string> = {};
    if (asRecord(item.style)) {
      for (const [k, v] of Object.entries(item.style)) {
        if (typeof v === "string") style[k] = v;
      }
    }
    const attributes: Record<string, string> = {};
    if (asRecord(item.attributes)) {
      for (const [k, v] of Object.entries(item.attributes)) {
        if (typeof v === "string") attributes[k] = v;
      }
    }
    out.push({
      id: item.id,
      tag: item.tag,
      role: typeof item.role === "string" ? item.role : undefined,
      text: typeof item.text === "string" ? item.text : "",
      rect: {
        x: typeof rectRaw.x === "number" ? Math.round(rectRaw.x) : 0,
        y: typeof rectRaw.y === "number" ? Math.round(rectRaw.y) : 0,
        width: typeof rectRaw.width === "number" ? Math.round(rectRaw.width) : 0,
        height: typeof rectRaw.height === "number" ? Math.round(rectRaw.height) : 0,
      },
      style,
      parentId: typeof item.parentId === "string" ? item.parentId : undefined,
      boxModel: decodeBoxModel(item.boxModel),
      attributes: Object.keys(attributes).length > 0 ? attributes : undefined,
    });
  }
  return out;
}

/**
 * Decode a `postMessage` payload streamed from a live-mounted component card:
 * `{ type: "dui:live-render-tree", path: "/editor", width: 1280, renderTree: [...] }`.
 */
export function parseLiveTelemetryMessage(raw: unknown): LiveSurfaceTelemetry | null {
  if (!asRecord(raw) || raw.type !== "dui:live-render-tree") return null;
  if (typeof raw.path !== "string" || !raw.path.startsWith("/")) return null;
  if (typeof raw.width !== "number" || !Number.isFinite(raw.width) || raw.width <= 0) return null;
  const renderTree = decodeNodes(raw.renderTree);
  if (renderTree.length === 0) return null;
  return {
    path: raw.path,
    width: Math.round(raw.width),
    renderTree,
    capturedAtMs:
      typeof raw.capturedAtMs === "number" && Number.isFinite(raw.capturedAtMs)
        ? raw.capturedAtMs
        : Date.now(),
  };
}

/**
 * Overlay real-time streamed live `RenderNode[]` telemetry onto a cell while preserving
 * the exact same `ElementInspector` interface used for static captures.
 */
export function mergeLiveRenderTree(cell: Cell, liveTrees: Record<string, RenderNode[]>): Cell {
  const live = liveTrees[liveSurfaceKey(cell.path, cell.width)];
  if (!live || live.length === 0) return cell;
  return { ...cell, renderTree: live };
}

export type LiveSelectNodeMessage = {
  path: string;
  width: number;
  nodeId: string;
};

export function parseLiveSelectMessage(raw: unknown): LiveSelectNodeMessage | null {
  if (!asRecord(raw) || raw.type !== "dui:live-select-node") return null;
  if (typeof raw.path !== "string" || !raw.path.startsWith("/")) return null;
  if (typeof raw.width !== "number" || !Number.isFinite(raw.width) || raw.width <= 0) return null;
  if (typeof raw.nodeId !== "string" || raw.nodeId.length === 0) return null;
  return { path: raw.path, width: Math.round(raw.width), nodeId: raw.nodeId };
}

export function buildLiveHighlightMessage(nodeId: string | null): {
  type: "dui:live-highlight-node";
  nodeId: string | null;
} {
  return { type: "dui:live-highlight-node", nodeId };
}
