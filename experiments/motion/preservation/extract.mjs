import { parse, generate } from "css-tree";
import { createHash } from "node:crypto";
export function candidates(model, source) {
  const id = model.components[0].root.props.id.value;
  const hash = createHash("sha256").update(source).digest("hex");
  const file = model.files.find((f) => f.sha256 === hash && f.language === "css");
  if (!file) throw Error("model/CSS mismatch");
  const rules = parse(source, { positions: true }).children.toArray();
  if (rules.length !== 1 || generate(rules[0].prelude) !== "#" + id)
    throw Error("requires one unambiguous ID rule");
  const d = rules[0].block.children.toArray().find((d) => d.property === "transform");
  if (!d) return null;
  return {
    declarations: rules[0].block.children.toArray(),
    value: d.value,
    source: source.slice(d.value.loc.start.offset, d.value.loc.end.offset),
    location: [file.path, d.value.loc.start.offset, d.value.loc.end.offset],
  };
}
export function raw(c) {
  return c && { text: c.source, location: c.location };
}
export function ordered(c) {
  if (!c) return null;
  const operations = [];
  for (const f of c.value.children) {
    if (f.type !== "Function") return { unresolved: c.source };
    const args = f.children
      .toArray()
      .filter((n) => n.type !== "Operator" && n.type !== "WhiteSpace");
    const units = {
      translate: ["px", "%"],
      translateX: ["px", "%"],
      translateY: ["px", "%"],
      rotate: ["deg"],
      scale: [""],
    };
    if (
      !units[f.name] ||
      !args.length ||
      args.length > (["translate", "scale"].includes(f.name) ? 2 : 1)
    )
      return { unresolved: c.source };
    const values = [];
    for (const a of args) {
      const unit = a.type === "Percentage" ? "%" : a.type === "Number" ? "" : a.unit;
      if (!units[f.name].includes(unit) || !Number.isFinite(Number(a.value)))
        return { unresolved: c.source };
      values.push({ n: Number(a.value), unit });
    }
    operations.push({ op: f.name, values });
  }
  return { operations, location: c.location };
}

export function withVariables(c) {
  if (!c) return null;
  const variables = {},
    pending = [c.source],
    missing = [];
  for (const text of pending)
    for (const [, name] of text.matchAll(/var\(\s*(--[\w-]+)/g)) {
      if (Object.hasOwn(variables, name) || missing.includes(name)) continue;
      const d = c.declarations.findLast((d) => d.property === name);
      if (!d) {
        missing.push(name);
        continue;
      }
      variables[name] = generate(d.value).trim();
      pending.push(variables[name]);
    }
  return { ...raw(c), variables, missing };
}
