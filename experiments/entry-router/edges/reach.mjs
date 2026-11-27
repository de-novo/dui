import { walk } from "./engine.mjs";
// File-local declaration graph. JSX identifiers, callback props and imperative calls
// all retain bindings; exported declarations are conservative entry points.
export function activeRanges(program, mode) {
  const units = program.body.map((n) => {
    const d = n.declaration ?? n;
    const names =
      d.type === "VariableDeclaration" ? d.declarations.map((x) => x.id.name) : [d.id?.name];
    return {
      n,
      names: names.filter(Boolean),
      refs: new Set(),
      entry: n.type.startsWith("Export") || !names.some(Boolean),
    };
  });
  const binding = new Map();
  for (const u of units) for (const name of u.names) binding.set(name, u);
  for (const u of units)
    walk(u.n, (n) => {
      if (n.type === "Identifier" || n.type === "JSXIdentifier") {
        const v = binding.get(n.name);
        if (v && v !== u) u.refs.add(v);
      }
    });
  const live = new Set(units.filter((u) => u.entry));
  if (mode === "referenced") {
    for (const u of units) for (const v of u.refs) live.add(v);
  } else for (const u of live) for (const v of u.refs) live.add(v);
  return [...live].map((u) => [u.n.start, u.n.end]);
}
