export function files(owner, load) {
  const seen = new Set([owner.file]);
  for (const f of seen) {
    const m = load(f);
    for (const next of [...m.imports.map((i) => i[1]), ...m.exports]) if (next) seen.add(next);
  }
  return seen;
}
