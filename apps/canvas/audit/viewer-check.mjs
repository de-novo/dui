// Viewer design audit: run with `dui audit RUN --design apps/canvas/audit/dui.json` on a dui
// capture of the viewer, the site or the Pages demo (docs/en/architecture.md §6.3).
import { readFileSync } from "node:fs";

const { renderTree, designSpec, width, keyboard, dismissal } = JSON.parse(readFileSync(0, "utf8"));
const { localPath, tapTarget } = designSpec.content;
const unverified = [];
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
// Keyboard evidence from dui's Tab walk and Escape press (WCAG 2.2 SC 2.1.1, 2.4.7, 2.4.3).
const near = (a, b) =>
  ["x", "y", "width", "height"].every(
    (key) => Math.abs((a?.[key] ?? NaN) - (b?.[key] ?? NaN)) <= 1,
  );
const nodeFor = (record) =>
  renderTree.find((node) => node.tag === record.tag && near(node.rect, record.rect));
const report = (rule, record, summary, measured, expected, specRef) => {
  const node = nodeFor(record);
  if (!node) {
    unverified.push(`${rule}: <${record.tag}> "${record.label}" is not in the render tree`);
    return;
  }
  findings.push({
    rule,
    severity: "error",
    nodeId: node.id,
    rect: node.rect,
    summary,
    measured,
    expected,
    specRef,
  });
};
if (keyboard) {
  for (const stop of keyboard.stops) {
    if (!stop.focusVisible)
      report(
        "focus-visible",
        stop,
        "Keyboard focus has no visible indicator",
        stop.label || stop.tag,
        "an outline or ring on :focus-visible",
        "WCAG 2.2 SC 2.4.7",
      );
  }
  if (!keyboard.complete) {
    unverified.push("The Tab walk did not return to its start; reachability is not checked");
  } else {
    for (const target of keyboard.targets) {
      // Lazy images can shift the layout while Tab scrolls, so a labelled target also matches by name.
      const reached = keyboard.stops.some(
        (stop) =>
          stop.tag === target.tag &&
          (near(stop.rect, target.rect) || (target.label !== "" && stop.label === target.label)),
      );
      if (!reached)
        report(
          "keyboard-reachable",
          target,
          "Control cannot be reached with Tab",
          target.label || target.tag,
          "in the Tab order",
          "WCAG 2.2 SC 2.1.1",
        );
    }
  }
}
if (dismissal) {
  const root = renderTree[0];
  if (dismissal.closedByEscape === false)
    findings.push({
      rule: "escape-dismiss",
      severity: "error",
      nodeId: root.id,
      rect: root.rect,
      summary: "Escape does not close this overlay",
      measured: "still open",
      expected: "closed",
      specRef: "WAI-ARIA APG dialog pattern",
    });
  else if (dismissal.focusReturned === false)
    findings.push({
      rule: "focus-return",
      severity: "error",
      nodeId: root.id,
      rect: root.rect,
      summary: "Focus does not return to the control that opened this overlay",
      measured: "focus elsewhere",
      expected: "focus on the opener",
      specRef: "WCAG 2.2 SC 2.4.3",
    });
}
if (!keyboard && !dismissal)
  unverified.push("No interaction evidence: capture with a dui build that records the Tab walk");
process.stdout.write(JSON.stringify({ findings, unverified }));
