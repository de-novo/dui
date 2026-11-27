import Yoga from "yoga-layout";
import fs from "node:fs";
const model = JSON.parse(fs.readFileSync(0, "utf8"));
const root = model.components[0].root;
if (model.schemaVersion !== 1 || root.tag !== "div" || root.children?.length)
  throw Error("expected a single v1 div");
const node = Yoga.Node.create();
try {
  for (const d of root.style) {
    if (d.when || d.value.kind !== "length" || d.value.unit !== "px") continue;
    if (d.property === "width") node.setWidth(d.value.value);
    if (d.property === "height") node.setHeight(d.value.value);
    if (d.property === "maxWidth") node.setMaxWidth(d.value.value);
  }
  node.calculateLayout(390, 844, Yoga.DIRECTION_LTR);
  const { left, top, width, height } = node.getComputedLayout();
  console.log(JSON.stringify({ rect: [left, top, width, height] }));
} finally {
  node.free();
}
