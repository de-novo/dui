// Viewer design audit: run with `dui audit RUN --design apps/canvas/audit/dui.json` on a dui
// capture of the viewer, the site or the Pages demo (docs/en/architecture.md §6.3).
import { readFileSync } from "node:fs";

const { renderTree, designSpec, width } = JSON.parse(readFileSync(0, "utf8"));
const { localPath, tapTarget } = designSpec.content;
const paths = localPath.map((pattern) => new RegExp(pattern));
const findings = [];
const byId = new Map(renderTree.map((node) => [node.id, node]));
// WCAG 2.5.8 exempts targets inside a sentence: the parent carries more text than the link.
const inline = (node) => {
  const parent = byId.get(node.parentId);
  const own = (node.text ?? "").trim().length;
  return Boolean(parent) && (parent.text ?? "").trim().length > own + 10;
};

for (const node of renderTree) {
  const text = (node.text ?? "").trim();
  // Archives must show project-relative sources, never a machine's build or checkout path.
  if (text && paths.some((pattern) => pattern.test(text))) {
    findings.push({
      rule: "local-path",
      severity: "error",
      nodeId: node.id,
      rect: node.rect,
      summary: "A local build or checkout path is shown to users",
      measured: text.slice(0, 120),
      expected: "a project-relative source path",
    });
  }
  // WCAG 2.2 target size (minimum) on phone-sized captures.
  const interactive =
    node.tag === "button" || node.tag === "a" || node.role === "button" || node.role === "link";
  const { width: w = 0, height: h = 0 } = node.rect ?? {};
  if (
    interactive &&
    !inline(node) &&
    width <= tapTarget.maxViewportWidth &&
    w > 0 &&
    h > 0 &&
    (w < tapTarget.minSize || h < tapTarget.minSize)
  ) {
    findings.push({
      rule: "tap-target",
      severity: "error",
      nodeId: node.id,
      rect: node.rect,
      summary: "Tap target is smaller than 24 × 24 CSS px on a phone",
      measured: `${w} × ${h}`,
      expected: `≥ ${tapTarget.minSize} × ${tapTarget.minSize}`,
      specRef: "WCAG 2.2 SC 2.5.8",
    });
  }
}
process.stdout.write(JSON.stringify({ findings }));
