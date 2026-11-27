import fs from "node:fs";
import path from "node:path";
import { project, walk } from "./engine.mjs";
const [mode, ...roots] = process.argv.slice(2);
const reach = ["referenced", "reachable"].includes(mode);
const { activeRanges } = await import("./reach.mjs");
const { files } = await import("./" + (reach ? "layout" : mode) + ".mjs");
const result = {};
for (const [i, root] of roots.entries()) {
  const { load: raw, owners } = project(path.resolve(root)),
    valid = new Set(owners.map((o) => o.id)),
    pairs = {};
  const cache = new Map();
  function load(file) {
    const mod = raw(file);
    if (!reach) return mod;
    if (!cache.has(file)) {
      const ranges = activeRanges(mod.program, mode);
      const live = (offset) => ranges.some(([a, b]) => a <= offset && offset < b);
      const used = new Set();
      for (const n of mod.program.body)
        if (live(n.start))
          walk(n, (c) => {
            if (c.type === "JSXOpeningElement") used.add(c.name.name);
          });
      cache.set(file, { ...mod, used, links: mod.links.filter(([, offset]) => live(offset)) });
    }
    return cache.get(file);
  }
  for (const owner of owners)
    for (const f of files(owner, load))
      for (const [to, offset] of load(f).links) {
        if (valid.has(to) && to !== owner.id)
          (pairs[JSON.stringify([owner.id, to])] ??= []).push([
            path.relative(fs.realpathSync(root), f),
            offset,
          ]);
      }
  result[["todo", "bp", "canvas", "external"][i]] = { owners: owners.map((o) => o.id), pairs };
}
console.log(JSON.stringify(result));
