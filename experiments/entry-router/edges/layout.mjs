export function files(owner, load) {
  const seen = new Set([owner.file, ...owner.layouts]);
  function select(file, name, visited = new Set()) {
    if (visited.has(file)) return;
    visited.add(file);
    const m = load(file);
    if (!m.exports.some(Boolean)) {
      seen.add(file);
      return;
    }
    for (const f of m.exports.filter(Boolean)) {
      const target = load(f);
      if (
        target.program.body.some(
          (n) =>
            n.type === "ExportNamedDeclaration" &&
            (n.declaration?.id?.name === name ||
              n.declaration?.declarations?.some((d) => d.id.name === name) ||
              n.specifiers?.some((s) => s.exported.name === name)),
        )
      )
        seen.add(f);
      else if (target.exports.length) select(f, name, visited);
    }
  }
  for (const f of seen) {
    const m = load(f);
    for (const [name, target, symbol] of m.imports)
      if (target && m.used.has(name)) select(target, symbol ?? name);
  }
  return seen;
}
