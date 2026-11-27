// Serialize the proven scene back into literal JSX for measurement with the unchanged v1 drawer.
export function jsx(node) {
  if (typeof node !== "object") return "{" + JSON.stringify(node) + "}";
  const attrs = Object.entries(node.attrs)
    .map(([k, v]) => " " + k + "={" + JSON.stringify(v) + "}")
    .join("");
  return "<" + node.tag + attrs + ">" + node.children.map(jsx).join("") + "</" + node.tag + ">";
}
