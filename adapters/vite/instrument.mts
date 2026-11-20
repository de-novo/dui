// Edits are insertions at compiler positions. Nested predicates retain evaluation order.
type Span = [number, number];
export type InstrumentFact = {
  file: string;
  start: number;
  end: number;
  kind: string;
  label: string;
  condition: Span;
  init?: Span;
  after?: number;
  shape: { kind: string };
  setter?: string;
};
export function instrument(text: string, facts: InstrumentFact[]) {
  const edits: [number, number, string][] = [];
  for (const f of facts) {
    const key = JSON.stringify(
      [f.file + "#" + f.start, f.end, f.kind].join(":"),
    );
    if (f.kind === "branch") {
      const [a, b] = f.condition;
      if (text.slice(a, b) !== f.label)
        throw Error("stale compiler positions: " + key);
      edits.push(
        [a, 1, `(globalThis.__duiProbe?.branch ?? ((_id,v)=>v))(${key},(`],
        [b, 0, "))"],
      );
    }
    if (f.kind === "slot" && f.init && f.after && f.shape.kind !== "unknown") {
      const [a, b] = f.init;
      edits.push(
        [
          a,
          1,
          `(Object.hasOwn(globalThis.__duiProbe?.input ?? {},${key}) ? globalThis.__duiProbe.input[${key}] : (`,
        ],
        [b, 0, "))"],
      );
      edits.push([
        f.after,
        0,
        `;globalThis.__duiProbe?.slot(${key},${f.label},${f.setter ?? "null"});`,
      ]);
    }
  }
  for (const [at, , str] of edits.sort((a, b) => b[0] - a[0] || b[1] - a[1]))
    text = text.slice(0, at) + str + text.slice(at);
  return text;
}
