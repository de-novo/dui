// The renderer receives numeric model values only. Layout resolves unit bases in the iframe.
function css(value) {
  if (value.clamp) return `clamp(${value.clamp.map(css).join(", ")})`;
  if (!Number.isFinite(value.n) || !["px", "em", "rem", "vw", "%"].includes(value.unit))
    throw Error("unsupported experimental length");
  return String(value.n) + value.unit;
}
export function applyExtension(doc, extension) {
  if (extension.rootFont) doc.documentElement.style.fontSize = css(extension.rootFont);
  doc.body.style.fontSize = "inherit";
  for (const node of doc.querySelectorAll("[data-model-node]")) {
    const width = extension.widths[node.dataset.modelNode];
    if (width) node.style.width = css(width);
  }
}
