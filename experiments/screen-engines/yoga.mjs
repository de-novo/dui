import Yoga from "../engine-comparison/node_modules/yoga-layout/dist/src/index.js";

// Input is exclusively the source-model drawer's inert tree. Browser canvas shapes
// leaf text. Yoga computes container and leaf boxes before the DOM paints them.
export function arrange(host, width) {
  const root =
    host.shadowRoot.querySelector(".dui-document-body") ??
    host.shadowRoot.querySelector("[data-model-node]");
  if (!root) throw Error("missing projected root");
  const ctx = document.createElement("canvas").getContext("2d"),
    boxes = [];
  function visit(el) {
    const node = Yoga.Node.create(),
      cs = getComputedStyle(el),
      st = el.style;
    boxes.push([el, node]);
    for (const [prop, set] of [
      ["width", "setWidth"],
      ["height", "setHeight"],
      ["maxWidth", "setMaxWidth"],
    ]) {
      if (st[prop].endsWith("px")) node[set](parseFloat(st[prop]));
      if (st[prop].endsWith("%")) node[set + "Percent"](parseFloat(st[prop]));
    }
    const row = cs.display === "flex" && cs.flexDirection === "row";
    node.setFlexDirection(row ? Yoga.FLEX_DIRECTION_ROW : Yoga.FLEX_DIRECTION_COLUMN);
    node.setGap(Yoga.GUTTER_ALL, parseFloat(cs.gap) || 0);
    if (cs.alignItems === "center") node.setAlignItems(Yoga.ALIGN_CENTER);
    if (cs.justifyContent === "center") node.setJustifyContent(Yoga.JUSTIFY_CENTER);
    for (const [edge, k] of [
      [Yoga.EDGE_LEFT, "Left"],
      [Yoga.EDGE_RIGHT, "Right"],
      [Yoga.EDGE_TOP, "Top"],
      [Yoga.EDGE_BOTTOM, "Bottom"],
    ]) {
      node.setPadding(edge, parseFloat(cs["padding" + k]) || 0);
      node.setMargin(edge, parseFloat(cs["margin" + k]) || 0);
    }
    if (el.children.length)
      Array.from(el.children).forEach((c, i) => node.insertChild(visit(c), i));
    else if (el.textContent)
      node.setMeasureFunc((available, mode) => {
        ctx.font = cs.font || cs.fontSize + " " + cs.fontFamily;
        const limit = mode === Yoga.MEASURE_MODE_UNDEFINED ? Infinity : available;
        const words = el.textContent.trim().split(/\s+/);
        let line = "",
          max = 0,
          lines = 1;
        for (const word of words) {
          const next = line ? line + " " + word : word;
          if (line && ctx.measureText(next).width > limit) {
            max = Math.max(max, ctx.measureText(line).width);
            line = word;
            lines++;
          } else line = next;
        }
        return {
          width: Math.min(limit, Math.max(max, ctx.measureText(line).width)),
          height: lines * (parseFloat(cs.lineHeight) || parseFloat(cs.fontSize) * 1.2),
        };
      });
    return node;
  }
  const tree = visit(root);
  try {
    tree.calculateLayout(width, undefined, Yoga.DIRECTION_LTR);
    for (const [el, node] of boxes) {
      const r = node.getComputedLayout();
      Object.assign(el.style, {
        position: el === root ? "relative" : "absolute",
        left: r.left + "px",
        top: r.top + "px",
        width: r.width + "px",
        height: r.height + "px",
        margin: "0",
        boxSizing: "border-box",
      });
    }
  } finally {
    tree.freeRecursive();
  }
}
