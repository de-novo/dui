import fs from "node:fs";
import path from "node:path";
import { project } from "./engine.mjs";
const [mode, ...roots] = process.argv.slice(2);
const { files } = await import("./" + mode + ".mjs");
const result = {};
for (const [i, root] of roots.entries()) {
  const { load, owners } = project(path.resolve(root)),
    valid = new Set(owners.map((o) => o.id)),
    pairs = {};
  for (const owner of owners)
    for (const f of files(owner, load))
      for (const [to, offset] of load(f).links) {
        if (valid.has(to) && to !== owner.id)
          (pairs[JSON.stringify([owner.id, to])] ??= []).push([
            path.relative(fs.realpathSync(root), f),
            offset,
          ]);
      }
  result[["todo", "bp", "canvas"][i]] = { owners: owners.map((o) => o.id), pairs };
}
console.log(JSON.stringify(result));
