import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

const templates = new Map<string, string>();
function helperSource(typed: boolean) {
  const file = typed ? "probe-helpers.ts" : "probe-helpers.js";
  let template = templates.get(file);
  if (!template) {
    template = readFileSync(new URL(file, import.meta.url), "utf8");
    templates.set(file, template);
  }
  return template;
}

// All ranges are UTF-16 offsets from the project compiler's original source.
type Span = [number, number];
type Arms =
  | { kind: "if"; yes: Span; no: Span | null }
  | { kind: "conditional"; yes: Span; no: Span }
  | { kind: "logical"; right: Span; and: boolean };
export type InstrumentFact = {
  file: string;
  start: number;
  end: number;
  kind: string;
  label: string;
  condition: Span;
  arms?: Arms;
  init?: Span;
  after?: number;
  shape: { kind: string };
  setter?: string;
};

interface Call {
  start: number;
  end: number;
  arguments: { start: number; end: number }[];
}
interface Oxc {
  parseSync(file: string, text: string): { program: unknown; errors: unknown[] };
  Visitor: new (visitor: { CallExpression(call: Call): void }) => {
    visit(program: unknown): void;
  };
}

function initializers(oxc: Oxc, file: string, text: string) {
  const ast = oxc.parseSync(file, text);
  if (ast.errors.length) throw Error("cannot instrument invalid source: " + file);
  const spans = new Map<string, Span>();
  new oxc.Visitor({
    CallExpression(call) {
      if (call.arguments.length === 1) {
        const [argument] = call.arguments;
        spans.set(call.start + ":" + call.end, [argument.start, argument.end]);
      }
    },
  }).visit(ast.program);
  return spans;
}

export function instrument(text: string, facts: InstrumentFact[], oxc: Oxc) {
  // Keep module directives at the top and avoid shadowing application names.
  if (!facts.length) return text;
  const file = facts[0].file;
  let name = "__duiTools_" + createHash("sha256").update(file).digest("hex").slice(0, 12);
  while (text.includes(name)) name += "_";
  const probe = name + "().probe()";
  const slots = facts.some(
    (f) => f.kind === "slot" && f.init && f.after && f.shape.kind !== "unknown",
  )
    ? initializers(oxc, file, text)
    : undefined;
  const edits: { at: number; open: boolean; value: string; order: number }[] = [];
  function insert(at: number, open: boolean, value: string) {
    edits.push({ at, open, value, order: edits.length });
  }
  function wrap([a, b]: Span, prefix: string, suffix: string) {
    insert(a, true, prefix);
    insert(b, false, suffix);
  }
  // Parent wrappers open first and close last at shared offsets.
  for (const f of [...facts].sort((a, b) => a.start - b.start || b.end - a.end)) {
    const key = JSON.stringify([f.file + "#" + f.start, f.end, f.kind].join(":"));
    const hit = (value: boolean) => `${probe}?.branch(${key},${value})`;
    if (f.kind === "branch") {
      const [a, b] = f.condition;
      if (text.slice(a, b) !== f.label || !f.arms) throw Error("stale compiler positions: " + key);
      const arm = f.arms;
      if (arm.kind === "if") {
        wrap(arm.yes, "{" + hit(true) + ";", arm.no ? "}" : "}else{" + hit(false) + ";}");
        if (arm.no) wrap(arm.no, "{" + hit(false) + ";", "}");
      } else if (arm.kind === "conditional") {
        wrap(arm.yes, "(" + hit(true) + ",(", "))");
        wrap(arm.no, "(" + hit(false) + ",(", "))");
      } else {
        // Leave the left guard in place so narrowing also applies inside the RHS.
        // Tag only the evaluated RHS. A skipped operand keeps its original value.
        wrap([f.start, f.end], `${name}().logical(${key},${arm.and},(`, "))");
        wrap(arm.right, `${name}().right((`, "))");
      }
    }
    if (f.kind === "slot" && f.init && f.after && f.shape.kind !== "unknown") {
      const span = slots?.get(f.start + ":" + f.end);
      if (!span || span[0] !== f.init[0] || span[1] !== f.init[1]) {
        throw Error("slot initializer disagrees with compiler: " + key);
      }
      wrap(span, `${name}().initial(${key},()=>(`, "))");
      insert(f.after, false, `;${probe}?.slot(${key},${f.label},${f.setter ?? "null"});`);
    }
  }
  if (!edits.length) return text;
  for (const edit of edits.sort(
    (a, b) =>
      b.at - a.at ||
      Number(b.open) - Number(a.open) ||
      (a.open ? b.order - a.order : a.order - b.order),
  ))
    text = text.slice(0, edit.at) + edit.value + text.slice(edit.at);
  return text + "\n" + helperSource(/\.[cm]?tsx?$/.test(file)).replaceAll("__duiTools", name);
}
