// Where on a capture a link edge starts (docs/en/web-viewer.md, links): the anchors in the
// capture's recorded element tree whose `href` resolves to the target screen.
import type { Cell, Screen } from "../data/overview";

/** A rectangle as fractions of the capture's width and height. */
export type AnchorBox = { x: number; y: number; width: number; height: number };

const base = "http://dui.invalid";

/** The path a screen is captured at, as links compare it: no hash, `index.html` dropped. */
export function linkPath(path: string): string {
  const url = new URL(path, base);
  const pathname = url.pathname.replace(/\/index\.html$/, "/");
  return pathname + url.search;
}

/** The capture with a render tree behind what a card shows: map cards show overview entries,
 * which carry the image but not the tree. */
export function treeCell(screen: Screen, shown: Cell | undefined): Cell | undefined {
  if (!shown) return undefined;
  if (shown.renderTree?.length) return shown;
  return (
    screen.cells.find((c) => c.image === shown.image && c.renderTree?.length) ??
    screen.cells.find((c) => c.width === shown.width && !c.state && c.renderTree?.length)
  );
}

/** Anchors on `cell` (captured at `sourcePath`) that lead to `targetPath`, in document order. */
export function anchorsTo(cell: Cell | undefined, sourcePath: string, targetPath: string) {
  const tree = cell?.renderTree;
  if (!cell || !tree?.length) return [];
  // The PNG may be taken at a device scale other than 1; rectangles are in CSS pixels.
  const cssHeight =
    cell.imageWidth && cell.imageHeight
      ? (cell.imageHeight * cell.width) / cell.imageWidth
      : undefined;
  if (!cssHeight) return [];
  const target = linkPath(targetPath);
  const from = new URL(sourcePath, base);
  const boxes: AnchorBox[] = [];
  for (const node of tree) {
    const href = node.attributes?.href;
    if (node.tag !== "a" || !href) continue;
    let url: URL;
    try {
      url = new URL(href, from);
    } catch {
      continue;
    }
    if (url.origin !== base || linkPath(url.pathname + url.search) !== target) continue;
    const { x, y, width, height } = node.rect;
    if (width <= 0 || height <= 0 || x + width <= 0 || y < 0 || y >= cssHeight) continue;
    const left = Math.max(0, x);
    boxes.push({
      x: left / cell.width,
      y: y / cssHeight,
      width: Math.min(width - (left - x), cell.width - left) / cell.width,
      height: Math.min(height, cssHeight - y) / cssHeight,
    });
  }
  return boxes;
}
