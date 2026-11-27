import fs from "node:fs";
import path from "node:path";
import { project, walk } from "./engine.mjs";
import { files as rendered } from "./layout.mjs";
const [rootIn, mode] = process.argv.slice(2),
  root = fs.realpathSync(rootIn),
  { load } = project(root);
const owners = [],
  base = fs.existsSync(root + "/app")
    ? root + "/app"
    : fs.existsSync(root + "/src/app")
      ? root + "/src/app"
      : root + "/src/routes",
  next = !base.endsWith("/routes");
const sources = fs.readdirSync(base, { recursive: true }).filter((s) => s.endsWith(".tsx"));
const clean = (s) =>
  "/" +
  s
    .split("/")
    .filter(
      (x) => x && !x.startsWith("(") && !x.startsWith("@") && !x.startsWith("_") && x !== "index",
    )
    .join("/");
for (const f of sources) {
  const file = base + "/" + f,
    dir = path.dirname(file);
  let id;
  if (next) {
    if (!["page.tsx", "page.further.tsx"].includes(path.basename(f))) continue;
    id = clean(path.relative(base, dir));
  } else {
    walk(load(file).program, (n) => {
      if (
        n.type === "CallExpression" &&
        n.callee.name === "createFileRoute" &&
        typeof n.arguments[0]?.value === "string"
      )
        id = clean(n.arguments[0].value);
    });
    if (!id || path.basename(f) === "route.tsx") continue;
  }
  const layouts = [];
  for (let d = dir; d.startsWith(base); d = path.dirname(d)) {
    const layout = d + "/" + (next ? "layout.tsx" : "route.tsx");
    if (fs.existsSync(layout)) layouts.push(layout);
  }
  if (!next && fs.existsSync(base + "/__root.tsx")) layouts.push(base + "/__root.tsx");
  owners.push({ id, file, layouts });
}
const valid = new Set(owners.map((o) => o.id)),
  pairs = new Set(),
  cache = new Map();
function targets(f) {
  if (cache.has(f)) return cache.get(f);
  const module = load(f),
    out = new Set(module.links.map((v) => v[0]));
  if (mode === "data")
    walk(module.program, (n) => {
      if (
        n.type === "Property" &&
        ["href", "to", "url"].includes(n.key?.name) &&
        typeof n.value?.value === "string"
      )
        out.add(n.value.value);
    });
  cache.set(f, out);
  return out;
}
for (const owner of owners) {
  let files;
  if (mode === "local") files = [owner.file];
  else if (mode === "render") files = rendered(owner, load);
  else {
    files = new Set([owner.file, ...owner.layouts]);
    for (const f of files) {
      let m = load(f);
      for (const target of [...m.imports.map((i) => i[1]), ...m.exports])
        if (target) files.add(target);
    }
  }
  for (const f of files)
    for (const dest of targets(f))
      if (dest !== owner.id && valid.has(dest)) pairs.add(JSON.stringify([owner.id, dest]));
}
console.log(
  JSON.stringify({
    screens: [...valid].sort(),
    pairs: [...pairs].map(JSON.parse),
    scanned: cache.size,
  }),
);
