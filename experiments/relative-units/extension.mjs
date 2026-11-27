// Narrow sidecar extractor for a single unconditioned html rule and proven width winners.
import { generate, parse, toPlainObject } from "css-tree";

function length(node) {
  if (node.type === "Percentage") return { n: Number(node.value), unit: "%" };
  if (node.type === "Dimension" && ["px", "em", "rem", "vw"].includes(node.unit))
    return { n: Number(node.value), unit: node.unit };
  return null;
}
function value(source) {
  const nodes = toPlainObject(parse(source, { context: "value" })).children;
  if (nodes.length !== 1) return null;
  const node = nodes[0];
  if (node.type !== "Function") return length(node);
  if (node.name !== "clamp" || node.children.length !== 5) return null;
  const [a, comma, b, comma2, c] = node.children;
  if (![comma, comma2].every((v) => v.type === "Operator" && v.value === ",")) return null;
  const clamp = [a, b, c].map(length);
  return clamp.every(Boolean) ? { clamp } : null;
}
export function extractExtension(model, css) {
  const roots = [...parse(css).children].filter(
    (r) => r.type === "Rule" && generate(r.prelude) === "html",
  );
  const rootSizes = roots.flatMap((r) =>
    [...r.block.children].filter((d) => d.property === "font-size"),
  );
  const rootFont = rootSizes.length === 1 ? value(generate(rootSizes[0].value)) : null;
  const widths = {};
  function visit(node) {
    for (const d of node.style ?? []) {
      if (
        d.property !== "width" ||
        d.when ||
        d.value.kind !== "unresolved" ||
        d.value.reason !== "Unsupported CSS value"
      )
        continue;
      const result = value(d.value.text);
      if (result) widths[node.id] = result;
    }
    for (const child of node.children ?? []) visit(child);
  }
  visit(model.components[0].root);
  return { rootFont, widths };
}
