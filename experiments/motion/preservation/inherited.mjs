import { parse } from "css-tree";
import { createHash } from "node:crypto";
// The fixture language is one literal ID per selector and a single branch of v1 elements.
export function scopes(model, css) {
  const file = model.files.find(
    (f) => f.sha256 === createHash("sha256").update(css).digest("hex") && f.language === "css",
  );
  if (!file) throw Error("input provenance");
  const branch = [];
  for (let n = model.components[0].root; n; n = n.children?.[0]) {
    if ((n.children ?? []).length > 1) throw Error("expected one branch");
    branch.push({ id: n.props.id.value, vars: {} });
  }
  let transform;
  for (const r of parse(css, { positions: true }).children) {
    const sel = r.prelude?.children?.first?.children;
    if (
      r.type !== "Rule" ||
      r.prelude.children.size !== 1 ||
      sel.size !== 1 ||
      sel.first.type !== "IdSelector"
    )
      throw Error("unsupported selector");
    const scope = branch.find((n) => n.id === sel.first.name);
    if (!scope) throw Error("unknown target");
    for (const d of r.block.children) {
      if (d.important) throw Error("importance outside fixture");
      const v = css.slice(d.value.loc.start.offset, d.value.loc.end.offset).trim();
      if (d.property.startsWith("--")) scope.vars[d.property] = v;
      if (d.property === "transform" && scope === branch.at(-1)) transform = v;
    }
  }
  return { branch, transform };
}
export function encode(input, mode) {
  const { branch, transform } = input;
  if (mode === "leaf") return { transform, levels: [branch.at(-1).vars] };
  if (mode === "flatten")
    return { transform, levels: [Object.assign({}, ...branch.map((s) => s.vars))] };
  if (mode === "scopes") return { transform, levels: branch.map((s) => s.vars) };
  throw Error(mode);
}
