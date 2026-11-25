// Interpret the portable format as data. Archives cannot supply markup, URLs or event handlers.
export type PreviewItem =
  | { kind: "row" | "column"; children: PreviewItem[] }
  | { kind: "text"; text: string }
  | { kind: "switch"; label: string; checked: boolean; control: number };
type Frame = { items: PreviewItem[]; next: number[] };
export type SourcePreview =
  | { status: "ready"; frames: Frame[]; digest: string }
  | { status: "missing" | "stale" | "unsupported"; reason: string };

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
function decodeFrame(value: unknown, count: number): Frame | undefined {
  if (!record(value) || !Array.isArray(value.tokens) || !Array.isArray(value.next)) return;
  const items: PreviewItem[] = [];
  const stack = [items];
  let controls = 0;
  for (const token of value.tokens) {
    if (!Array.isArray(token)) return;
    const parent = stack[stack.length - 1]!;
    const [kind, a, b] = token;
    switch (kind) {
      case "row":
      case "column": {
        if (stack.length > 128) return;
        const children: PreviewItem[] = [];
        parent.push({ kind, children });
        stack.push(children);
        break;
      }
      case "end":
        if (stack.length < 2) return;
        stack.pop();
        break;
      case "text":
        if (typeof a !== "string") return;
        parent.push({ kind, text: a });
        break;
      case "switch":
        if (typeof a !== "boolean" || typeof b !== "string") return;
        parent.push({ kind, checked: a, label: b, control: controls++ });
        break;
      default:
        return;
    }
  }
  if (
    stack.length !== 1 ||
    controls !== value.next.length ||
    controls > 16 ||
    !value.next.every((i) => Number.isSafeInteger(i) && i >= 0 && i < count)
  )
    return;
  return { items, next: value.next };
}
export function decodeSourcePreview(raw: unknown, entry: string, screenId: string): SourcePreview {
  if (!record(raw)) return { status: "missing", reason: "missing-preview" };
  if (raw.status === "missing" || raw.status === "stale" || raw.status === "unsupported")
    return { status: raw.status, reason: typeof raw.reason === "string" ? raw.reason : raw.status };
  const scene = raw.scene;
  if (
    raw.status !== "ready" ||
    !record(scene) ||
    scene.version !== 1 ||
    !Array.isArray(scene.frames) ||
    scene.frames.length < 1 ||
    scene.frames.length > 32
  )
    return { status: "unsupported", reason: "invalid-preview-format" };
  if (
    typeof raw.sourceSha256 !== "string" ||
    !/^[0-9a-f]{64}$/.test(raw.sourceSha256) ||
    raw.screenId !== screenId ||
    scene.entry !== entry ||
    scene.sourceSha256 !== raw.sourceSha256
  )
    return { status: "stale", reason: "source-identity-mismatch" };
  const frames: Frame[] = [];
  for (const frame of scene.frames) {
    const decoded = decodeFrame(frame, scene.frames.length);
    if (!decoded) return { status: "unsupported", reason: "invalid-preview-frame" };
    frames.push(decoded);
  }
  return { status: "ready", frames, digest: raw.sourceSha256 };
}
