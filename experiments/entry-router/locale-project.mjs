import { parseSync } from "oxc-parser";
import fs from "node:fs";
const [source, input, locale, mode, out] = process.argv.slice(2);
const p = parseSync("messages.ts", fs.readFileSync(source, "utf8")).program;
const messages = p.body
  .flatMap((n) => n.declaration?.declarations ?? [])
  .find((d) => d.id.name === "MESSAGES").init;
const dict = messages.properties.find((p) => (p.key.name ?? p.key.value) === locale).value
  .properties;
const lookup = new Map(
  dict.filter((p) => typeof p.value.value === "string").map((p) => [p.key.name, p.value.value]),
);
const model = JSON.parse(fs.readFileSync(input));
function visit(n, parent) {
  if (mode === "weight")
    for (const d of n.style ?? [])
      if (
        d.property === "fontWeight" &&
        d.value.kind === "unresolved" &&
        d.value.text === "inherit"
      )
        d.value = { kind: "keyword", value: "inherit" };
  if (n.kind === "text" && (parent === "h1" || (mode !== "title" && parent === "a")))
    for (const [i, p] of n.parts.entries()) {
      const key = /^t\.(\w+)$/.exec(p.text ?? "")?.[1];
      if (key && lookup.has(key)) n.parts[i] = { kind: "literal", value: lookup.get(key) };
    }
  for (const c of n.children ?? []) visit(c, n.tag);
}
const header = model.components
  .find((c) => c.name === "ModelView")
  .root.children.find((n) => n.tag === "header");
visit(header);
fs.writeFileSync(out, JSON.stringify(model));
