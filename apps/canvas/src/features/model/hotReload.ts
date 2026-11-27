import type { Screen, ScreenLink, ScreenMap } from "../../data/overview";
import type { Model } from "./modelDocument";

type Raw = Record<string, unknown>;
type EntityPatch = { upsert?: Raw[]; remove?: string[] };
export type SourceModelPatch = {
  baseRevision?: number;
  files?: EntityPatch;
  screens?: EntityPatch;
  components?: EntityPatch;
  itemSchemas?: EntityPatch;
  tokens?: EntityPatch;
  edges?: EntityPatch;
  diagnostics?: unknown[];
  affectedComponents?: string[];
  affectedScreens?: string[];
};
export type SourceGraphPatch = {
  screens?: EntityPatch;
  links?: EntityPatch;
  screenDiscovery?: { kind?: unknown; limitations?: unknown };
};

function record(value: unknown): value is Raw {
  return typeof value === "object" && value !== null;
}

function patchEntities<T extends Raw>(
  current: T[] | undefined,
  patch: EntityPatch | undefined,
  key: "id" | "path",
): T[] {
  const existing = current ?? [];
  if (!patch) return existing;
  const remove = new Set(patch.remove ?? []);
  const upsert = new Map<string, T>();
  for (const entity of patch.upsert ?? []) {
    if (typeof entity[key] === "string") upsert.set(entity[key] as string, entity as T);
  }
  const out: T[] = [];
  const seen = new Set<string>();
  for (const entity of existing) {
    const id = entity[key];
    if (typeof id !== "string" || remove.has(id)) continue;
    const next = upsert.get(id) ?? entity;
    upsert.delete(id);
    seen.add(id);
    out.push(next);
  }
  for (const [id, entity] of upsert) {
    if (!seen.has(id)) out.push(entity);
  }
  return out.sort((a, b) => String(a[key]).localeCompare(String(b[key])));
}

/** Apply the server's entity patch while retaining object identity for every unchanged entity. */
export function applyModelPatch(model: Model, patch: SourceModelPatch): Model {
  const raw = model as Model & Raw;
  return {
    ...raw,
    files: patchEntities(raw.files as Raw[] | undefined, patch.files, "path"),
    screens: patchEntities(raw.screens as Raw[] | undefined, patch.screens, "id"),
    components: patchEntities(raw.components as Raw[] | undefined, patch.components, "id"),
    itemSchemas: patchEntities(raw.itemSchemas as Raw[] | undefined, patch.itemSchemas, "id"),
    tokens: patchEntities(raw.tokens as Raw[] | undefined, patch.tokens, "id"),
    edges: patchEntities(raw.edges as Raw[] | undefined, patch.edges, "id"),
    diagnostics: patch.diagnostics ?? raw.diagnostics,
  } as Model;
}

function decodeScreenNode(raw: Raw, previous?: Screen): Screen {
  const location =
    record(raw.location) && typeof raw.location.file === "string"
      ? {
          file: raw.location.file,
          line: typeof raw.location.line === "number" ? raw.location.line : undefined,
          column: typeof raw.location.column === "number" ? raw.location.column : undefined,
        }
      : undefined;
  return {
    id: String(raw.id),
    path: String(raw.label),
    cells: previous?.cells ?? [],
    entries: previous?.entries,
    component: typeof raw.component === "string" ? raw.component : undefined,
    name: typeof raw.name === "string" ? raw.name : undefined,
    sourceLocation: location,
  };
}

function decodeGraphLink(raw: Raw): ScreenLink {
  return {
    source: String(raw.source),
    target: String(raw.target),
    provenance: "extracted",
    replayVerified: false,
    modelEdge: typeof raw.modelEdge === "string" ? raw.modelEdge : undefined,
  };
}

function linkKey(link: Pick<ScreenLink, "source" | "target" | "modelEdge">): string {
  return link.modelEdge ?? `${link.source}->${link.target}`;
}

function decodeDiscovery(raw: SourceGraphPatch["screenDiscovery"]): ScreenMap["screenDiscovery"] {
  if (!record(raw) || typeof raw.kind !== "string" || !Array.isArray(raw.limitations))
    return undefined;
  return {
    kind: raw.kind,
    limitations: raw.limitations.filter((x): x is string => typeof x === "string"),
  };
}

/** Apply a graph patch and clone affected screen objects even when their map metadata is unchanged.
 * The clone is the deliberate invalidation signal: memoized map cards keyed by a screen object redraw,
 * while every unaffected screen keeps object identity and stays memoized.
 */
export function applySourceGraphPatch(
  archive: ScreenMap,
  patch: SourceGraphPatch,
  affectedScreens: readonly string[] = [],
): ScreenMap {
  const screens = new Map(archive.screens.map((screen) => [screen.id, screen]));
  for (const id of patch.screens?.remove ?? []) screens.delete(id);
  for (const raw of patch.screens?.upsert ?? []) {
    if (typeof raw.id === "string") screens.set(raw.id, decodeScreenNode(raw, screens.get(raw.id)));
  }
  for (const id of affectedScreens) {
    const screen = screens.get(id);
    if (screen) screens.set(id, { ...screen });
  }
  const links = new Map(archive.links.map((link) => [linkKey(link), link]));
  for (const key of patch.links?.remove ?? []) links.delete(key);
  for (const raw of patch.links?.upsert ?? []) {
    const link = decodeGraphLink(raw);
    links.set(typeof raw.key === "string" ? raw.key : linkKey(link), link);
  }
  return {
    ...archive,
    screens: [...screens.values()].sort((a, b) => a.id.localeCompare(b.id)),
    links: [...links.values()],
    screenDiscovery: patch.screenDiscovery
      ? decodeDiscovery(patch.screenDiscovery)
      : archive.screenDiscovery,
  };
}

/** Rare recovery path after a client missed more than one server revision. */
export function replaceSourceGraph(archive: ScreenMap, graph: Raw): ScreenMap {
  const nodes = Array.isArray(graph.nodes) ? graph.nodes.filter(record) : [];
  const patch: SourceGraphPatch = {
    screens: {
      upsert: nodes,
      remove: archive.screens
        .filter((screen) => !nodes.some((node) => node.id === screen.id))
        .map((screen) => screen.id),
    },
    links: {
      upsert: Array.isArray(graph.edges) ? graph.edges.filter(record) : [],
      remove: archive.links.map(linkKey),
    },
    screenDiscovery: record(graph.screenDiscovery) ? graph.screenDiscovery : undefined,
  };
  // A snapshot has no per-revision invalidation metadata; clone every remaining screen once.
  return applySourceGraphPatch(
    archive,
    patch,
    archive.screens.map((screen) => screen.id),
  );
}
