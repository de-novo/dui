import type { Cell } from "../data/overview";
import type { Run } from "../data/run";

export type LiveCellRef = {
  state: string;
  width: number;
};

export type ProvenanceSnapshot = {
  commit: string | null;
  dirty: boolean | null;
  branch?: string | null;
};

export type LiveProvenance = {
  archive: ProvenanceSnapshot;
  live: ProvenanceSnapshot;
};

export type LiveControl = {
  slot: string;
  owner: string;
  instance: string | null;
  label: string;
  type: string;
  source: { file: string; start?: number; end?: number };
  editable: boolean;
  reason: string | null;
  value?: unknown;
  hasValue: boolean;
  initialValue?: unknown;
  hasInitialValue: boolean;
  options?: unknown[];
};

export type LiveOpenResult = {
  session: string;
  revision: number;
  cell: LiveCellRef;
  provenance: LiveProvenance;
};

export type LiveInspectResult = {
  session: string;
  cell: LiveCellRef;
  revision: number;
  controls: LiveControl[];
};

export type LiveEditEvidence = {
  selector?: string;
  expectedText?: string;
  visibleText?: string | null;
  mode?: string;
  changed?: boolean;
};

export type LiveEditResult = {
  session: string;
  cell: LiveCellRef;
  slot: string;
  instance: string;
  revision: number;
  status: "verified" | "unverified";
  reason: string | null;
  observedValue?: unknown;
  expectedValue?: unknown;
  elapsedMs?: number;
  evidence: LiveEditEvidence;
};

export type LiveSaveResult = {
  project: string;
  run: string;
  runKey?: string;
  cell: LiveCellRef;
  revision: number;
  status: "verified" | "unverified";
  reason: string | null;
  image: string;
  imageSha256: string;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function decodeCellRef(raw: unknown): LiveCellRef {
  if (
    !isRecord(raw) ||
    typeof raw.state !== "string" ||
    typeof raw.width !== "number" ||
    !Number.isInteger(raw.width) ||
    raw.width <= 0
  ) {
    throw Error("Invalid live cell reference");
  }
  return { state: raw.state, width: raw.width };
}

function decodeSnapshot(raw: unknown): ProvenanceSnapshot {
  if (!isRecord(raw)) return { commit: null, dirty: null };
  return {
    commit: typeof raw.commit === "string" ? raw.commit : null,
    dirty: typeof raw.dirty === "boolean" ? raw.dirty : null,
    branch: typeof raw.branch === "string" ? raw.branch : null,
  };
}

export function decodeLiveOpen(raw: unknown): LiveOpenResult {
  if (
    !isRecord(raw) ||
    typeof raw.session !== "string" ||
    typeof raw.revision !== "number" ||
    !Number.isInteger(raw.revision) ||
    raw.revision < 0
  ) {
    throw Error("Invalid live session response");
  }
  const provenance = isRecord(raw.provenance)
    ? {
        archive: decodeSnapshot(raw.provenance.archive),
        live: decodeSnapshot(raw.provenance.live),
      }
    : { archive: { commit: null, dirty: null }, live: { commit: null, dirty: null } };
  return {
    session: raw.session,
    revision: raw.revision,
    cell: decodeCellRef(raw.cell),
    provenance,
  };
}

export function decodeLiveInspect(raw: unknown): LiveInspectResult {
  if (
    !isRecord(raw) ||
    typeof raw.session !== "string" ||
    typeof raw.revision !== "number" ||
    !Array.isArray(raw.controls)
  ) {
    throw Error("Invalid live inspect response");
  }
  const controls: LiveControl[] = raw.controls.map((item) => {
    if (!isRecord(item) || typeof item.slot !== "string" || typeof item.editable !== "boolean") {
      throw Error("Invalid live control");
    }
    const source = isRecord(item.source) ? item.source : {};
    return {
      slot: item.slot,
      owner: typeof item.owner === "string" ? item.owner : "Component",
      instance: typeof item.instance === "string" ? item.instance : null,
      label: typeof item.label === "string" ? item.label : item.slot,
      type: typeof item.type === "string" ? item.type : "unknown",
      source: {
        file: typeof source.file === "string" ? source.file : "",
        start: typeof source.start === "number" ? source.start : undefined,
        end: typeof source.end === "number" ? source.end : undefined,
      },
      editable: item.editable,
      reason: typeof item.reason === "string" ? item.reason : null,
      value: item.value,
      hasValue: Object.hasOwn(item, "value"),
      initialValue: item.initialValue,
      hasInitialValue: Object.hasOwn(item, "initialValue"),
      options: Array.isArray(item.options) ? item.options : undefined,
    };
  });
  return {
    session: raw.session,
    cell: decodeCellRef(raw.cell),
    revision: raw.revision,
    controls,
  };
}

export function decodeLiveEdit(raw: unknown): LiveEditResult {
  if (
    !isRecord(raw) ||
    typeof raw.session !== "string" ||
    typeof raw.slot !== "string" ||
    typeof raw.instance !== "string" ||
    typeof raw.revision !== "number" ||
    (raw.status !== "verified" && raw.status !== "unverified")
  ) {
    throw Error("Invalid live edit response");
  }
  const ev = isRecord(raw.evidence) ? raw.evidence : {};
  return {
    session: raw.session,
    cell: decodeCellRef(raw.cell),
    slot: raw.slot,
    instance: raw.instance,
    revision: raw.revision,
    status: raw.status,
    reason: typeof raw.reason === "string" ? raw.reason : null,
    observedValue: raw.observedValue,
    expectedValue: raw.expectedValue,
    elapsedMs: typeof raw.elapsedMs === "number" ? raw.elapsedMs : undefined,
    evidence: {
      selector: typeof ev.selector === "string" ? ev.selector : undefined,
      expectedText: typeof ev.expectedText === "string" ? ev.expectedText : undefined,
      visibleText:
        typeof ev.visibleText === "string" || ev.visibleText === null ? ev.visibleText : undefined,
      mode: typeof ev.mode === "string" ? ev.mode : undefined,
      changed: typeof ev.changed === "boolean" ? ev.changed : undefined,
    },
  };
}

export function decodeLiveSave(raw: unknown): LiveSaveResult {
  if (
    !isRecord(raw) ||
    typeof raw.project !== "string" ||
    typeof raw.run !== "string" ||
    typeof raw.revision !== "number" ||
    typeof raw.image !== "string" ||
    typeof raw.imageSha256 !== "string" ||
    (raw.status !== "verified" && raw.status !== "unverified")
  ) {
    throw Error("Invalid live save response");
  }
  return {
    project: raw.project,
    run: raw.run,
    runKey: typeof raw.runKey === "string" ? raw.runKey : undefined,
    cell: decodeCellRef(raw.cell),
    revision: raw.revision,
    status: raw.status,
    reason: typeof raw.reason === "string" ? raw.reason : null,
    image: raw.image,
    imageSha256: raw.imageSha256,
  };
}

export function valuesEqual(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function formatControlValue(value: unknown): string {
  if (typeof value === "boolean" || typeof value === "number") return String(value);
  if (typeof value === "string") return value;
  if (value === null) return "null";
  if (value === undefined) return "—";
  return JSON.stringify(value);
}

export function provenanceDiffers(provenance: LiveProvenance): boolean {
  return (
    provenance.archive.commit !== provenance.live.commit ||
    provenance.archive.dirty !== provenance.live.dirty
  );
}

export type ControlGroup = {
  id: string;
  title: string;
  controls: LiveControl[];
};

export function partitionControls(controls: LiveControl[]): {
  editableGroups: ControlGroup[];
  readOnly: LiveControl[];
} {
  const editable: LiveControl[] = [];
  const readOnly: LiveControl[] = [];
  for (const control of controls) {
    if (control.editable && control.instance !== null) editable.push(control);
    else readOnly.push(control);
  }
  const ownerInstances = new Map<string, string[]>();
  const byKey = new Map<string, { owner: string; instance: string; controls: LiveControl[] }>();
  for (const control of editable) {
    const instance = control.instance!;
    const list = ownerInstances.get(control.owner) ?? [];
    if (!list.includes(instance)) {
      list.push(instance);
      ownerInstances.set(control.owner, list);
    }
    const key = `${control.owner}\0${instance}`;
    const group = byKey.get(key) ?? { owner: control.owner, instance, controls: [] };
    group.controls.push(control);
    byKey.set(key, group);
  }
  const editableGroups: ControlGroup[] = [...byKey.entries()].map(([id, group]) => {
    const instances = ownerInstances.get(group.owner) ?? [];
    const index = instances.indexOf(group.instance);
    const title = instances.length > 1 ? `${group.owner} ${index + 1}` : group.owner;
    return { id, title, controls: group.controls };
  });
  return { editableGroups, readOnly };
}

export function canOpenLiveCell(
  cell: Cell | undefined,
  run: Run,
): { ok: true; cell: LiveCellRef } | { ok: false; reason: string } {
  if (run.liveAvailable === false) {
    return { ok: false, reason: "Project source directory is not available locally." };
  }
  if (!cell) {
    return { ok: false, reason: "No captured cell at this width." };
  }
  if (!cell.state) {
    return { ok: false, reason: "This archive does not record a state ID for live opening." };
  }
  if (cell.scenario === "live") {
    return { ok: false, reason: "Saved live revision — open the source cell to edit live." };
  }
  if (cell.status === "unrenderable") {
    return {
      ok: false,
      reason: cell.reason ? `Cell is unrenderable (${cell.reason}).` : "Cell is unrenderable.",
    };
  }
  return { ok: true, cell: { state: cell.state, width: cell.width } };
}
