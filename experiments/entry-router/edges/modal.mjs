import { project, walk } from "./engine.mjs";
import { files } from "./layout.mjs";
import fs from "node:fs";
import path from "node:path";
const [root, method] = process.argv.slice(2),
  { owners, load } = project(root);
const targets = owners.map((o) => o.id),
  pairs = {};
function destination(expr) {
  if (!expr) return;
  if (typeof expr.value === "string") return expr.value.replace(/\/$/, "") || "/";
  if (expr.type === "TemplateLiteral") {
    const text = expr.quasis.map((q) => q.value.cooked).join(":?");
    const compatible = targets.filter(
      (route) =>
        route.split("/").length === text.split("/").length &&
        route
          .split("/")
          .every(
            (s, i) =>
              s === text.split("/")[i] || (s.startsWith(":") && text.split("/")[i] === ":?"),
          ),
    );
    if (compatible.length === 1) return compatible[0];
  }
}
function blocking(module) {
  const dialogNames = new Set(
    module.imports
      .filter(
        (i) =>
          i[0] &&
          module.program.body.some(
            (n) =>
              n.source?.value === "@reach/dialog" &&
              n.specifiers?.some((s) => s.local?.name === i[0]),
          ),
      )
      .map((i) => i[0]),
  );
  let found = false;
  for (const n of module.program.body)
    if (n.type === "ExportDefaultDeclaration")
      walk(n, (c) => {
        if (
          c.type === "JSXOpeningElement" &&
          dialogNames.has(c.name.name) &&
          c.attributes.some((a) => a.name?.name === "isOpen" && a.value?.expression?.value === true)
        )
          found = true;
      });
  return found;
}
for (const owner of owners) {
  const active =
    method === "modal" && blocking(load(owner.file)) ? { ...owner, layouts: [] } : owner;
  for (const f of files(active, load)) {
    const mod = load(f),
      links = [...mod.links];
    if (method !== "literal")
      walk(mod.program, (n) => {
        if (n.type === "JSXAttribute" && ["to", "href"].includes(n.name.name))
          links.push([destination(n.value?.expression), n.start]);
        if (n.type === "CallExpression" && ["navigate", "redirect"].includes(n.callee.name))
          links.push([destination(n.arguments[0]), n.start]);
      });
    for (const [to, offset] of links)
      if (targets.includes(to) && to !== owner.id)
        (pairs[JSON.stringify([owner.id, to])] ??= []).push([
          path.relative(fs.realpathSync(root), f),
          offset,
        ]);
  }
}
console.log(JSON.stringify(pairs));
