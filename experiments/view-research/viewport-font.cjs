// Source-side projection for unqualified document typography in the workbench fixture.
const css = require("css-tree");
exports.projectFont = (source) => {
  const ast = css.parse(source, { positions: true });
  const root = [],
    body = [];
  for (const rule of ast.children) {
    if (rule.type !== "Rule" || rule.prelude.type !== "SelectorList") continue;
    const selectors = rule.prelude.children.toArray().map(css.generate);
    const dest = selectors.includes("body")
      ? body
      : selectors.some((s) => s === "html" || s === ":root")
        ? root
        : null;
    if (!dest) continue;
    for (const d of rule.block.children) {
      if (
        d.type === "Declaration" &&
        (d.property.startsWith("--") || /^(font($|-)|line-height$)/.test(d.property))
      )
        dest.push(source.slice(d.loc.start.offset, d.loc.end.offset));
    }
  }
  return { root: root.join(";"), body: body.join(";") };
};
exports.viewportModel = (model, width, height) => {
  const copy = structuredClone(model);
  function visit(n) {
    if (!n || typeof n !== "object") return;
    for (const d of n.style ?? []) {
      if (d.value?.kind !== "unresolved") continue;
      const m = /^([+-]?(?:\d*\.)?\d+)(dvh|vh|vw|dvw)$/.exec(d.value.text);
      if (m)
        d.value = {
          kind: "length",
          unit: "px",
          value: (Number(m[1]) * (m[2].endsWith("h") ? height : width)) / 100,
        };
    }
    for (const c of n.children ?? []) visit(c);
  }
  for (const c of copy.components) visit(c.root);
  return copy;
};
