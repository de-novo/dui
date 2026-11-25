/**
 * Self-dogfooding pre-deploy verification gate for `site/` and `site/demo/`.
 *
 * Verifies every exported showcase project and tests both mobile (390x844) and
 * desktop (1440x920) viewports in real headless Chromium before GitHub Pages
 * publishes the artifact:
 * - Zero blank white captures (every project must have >= 2 cells with >= 10 renderTree nodes)
 * - Every referenced PNG exists and is non-empty
 * - Zero horizontal overflow (`scrollWidth <= innerWidth`) on `site/` and `site/demo/`
 *   at both mobile (390px) and desktop (1440px)
 * - Mobile `site/demo/` renders the MobileScreenDeck cards (`.mobile-deck-card`)
 */
import { execFile } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";

import { withChromiumPage, type ChromiumPage } from "./chromium-page.mts";
import { verifyMapControls } from "./verify-map-controls.mts";
import { verifyViewerThemes, viewerViewports } from "./verify-viewer-themes.mts";

const root = process.cwd();
const siteDir = path.resolve(root, process.env.DUI_SITE_DIR || "site");
const demoDir = path.join(siteDir, "demo");
const archiveDir = path.join(demoDir, "archive");

function verifyArchives(): Array<{
  key: string;
  cells: number;
  populated: number;
  readyPreviews: number;
}> {
  const runsPath = path.join(archiveDir, "runs.json");
  if (!fs.existsSync(runsPath)) {
    throw new Error(`Missing ${runsPath}; run npm run build:pages-demo first`);
  }
  const runs = JSON.parse(fs.readFileSync(runsPath, "utf8")) as Array<{ key: string }>;
  if (runs.length === 0) throw new Error("runs.json is empty");

  const summary: Array<{ key: string; cells: number; populated: number; readyPreviews: number }> =
    [];
  for (const run of runs) {
    const dir = path.join(archiveDir, run.key);
    const graphPath = path.join(dir, "graph.json");
    if (!fs.existsSync(graphPath)) {
      throw new Error(`Missing graph.json for ${run.key}`);
    }
    const graph = JSON.parse(fs.readFileSync(graphPath, "utf8")) as {
      nodes?: Array<{
        sourcePreview?: { status?: string; scene?: { frames?: unknown[] } };
      }>;
      cells?: Array<{
        image?: string | null;
        renderTree?: unknown[];
        width?: number;
        path?: string;
        scenario?: string;
        status?: string;
      }>;
    };
    const cells = graph.cells ?? [];
    if (run.key === "native-source-preview") {
      // An attached recording keeps its runtime as the cell scenario.
      const runtimes = cells.map((cell) => `${cell.scenario}:${cell.status}`).sort();
      if (runtimes.join() !== "android:unverified,ios:unverified")
        throw new Error("Expected unverified iOS and Android captures, got " + runtimes.join());
    }
    let populated = 0;
    for (const cell of cells) {
      if (cell.image) {
        const imgPath = path.join(dir, cell.image);
        if (!fs.existsSync(imgPath) || fs.statSync(imgPath).size < 1000) {
          throw new Error(`Missing or empty PNG ${cell.image} in ${run.key}`);
        }
      }
      if (Array.isArray(cell.renderTree) && cell.renderTree.length >= 10) {
        populated += 1;
      }
    }
    const readyPreviews = (graph.nodes ?? []).filter(
      (node) =>
        node.sourcePreview?.status === "ready" &&
        Array.isArray(node.sourcePreview.scene?.frames) &&
        node.sourcePreview.scene.frames.length >= 2,
    ).length;
    const nativeDeclarations = (graph.nodes ?? []).filter(
      (node) => (node as { nativeSource?: unknown }).nativeSource,
    ).length;
    if (run.key === "home-assistant-ios") {
      const shots = cells.filter((cell) => cell.scenario === "ios" && cell.status === "unverified");
      if (nativeDeclarations < 200 || shots.length < 1 || shots.length !== cells.length)
        throw new Error(
          `home-assistant-ios: expected 200+ declarations and unverified iOS captures, got ${nativeDeclarations} and ${cells.length} cells`,
        );
    }
    if (populated < 2 && readyPreviews < 2 && nativeDeclarations < 2) {
      throw new Error(
        `Self-dogfood gate failed: ${run.key} has only ${populated}/${cells.length} populated cells and ${readyPreviews} ready source previews (blank capture detected)`,
      );
    }
    summary.push({ key: run.key, cells: cells.length, populated, readyPreviews });
  }
  return summary;
}

function mimeType(filePath: string): string {
  if (filePath.endsWith(".html")) return "text/html; charset=utf-8";
  if (filePath.endsWith(".css")) return "text/css; charset=utf-8";
  if (filePath.endsWith(".js") || filePath.endsWith(".mjs"))
    return "application/javascript; charset=utf-8";
  if (filePath.endsWith(".json")) return "application/json; charset=utf-8";
  if (filePath.endsWith(".png")) return "image/png";
  if (filePath.endsWith(".svg")) return "image/svg+xml";
  return "application/octet-stream";
}

function startStaticServer(): Promise<{ server: http.Server; port: number }> {
  return new Promise((resolve, reject) => {
    const server = http.createServer((req, res) => {
      const urlPath = decodeURIComponent((req.url || "/").split("?")[0] || "/");
      let rel = urlPath.replace(/^\/+/, "");
      if (!rel || rel.endsWith("/")) rel += "index.html";
      const filePath = path.resolve(siteDir, rel);
      if (
        !filePath.startsWith(siteDir) ||
        !fs.existsSync(filePath) ||
        fs.statSync(filePath).isDirectory()
      ) {
        res.writeHead(404);
        res.end("Not Found");
        return;
      }
      res.writeHead(200, { "content-type": mimeType(filePath) });
      fs.createReadStream(filePath).pipe(res);
    });
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const addr = server.address();
      if (!addr || typeof addr === "string") {
        reject(new Error("Failed to bind static server"));
        return;
      }
      resolve({ server, port: addr.port });
    });
  });
}

// Runs in Chromium after navigation. Compare the rendered boxes so the test
// follows the current gutter, font, language and action count.
async function checkContentPage() {
  if (location.pathname === "/index.html") {
    const showcaseTabs = Array.from(
      document.querySelectorAll<HTMLButtonElement>("[data-showcase-tab]"),
    );
    const showcaseImg = document.querySelector<HTMLImageElement>("[data-showcase-img]");
    if (showcaseTabs.length > 0 && showcaseImg) {
      const activeTab =
        showcaseTabs.find((tab) => tab.getAttribute("aria-selected") === "true") ??
        showcaseTabs[0]!;
      for (const tab of showcaseTabs) {
        tab.click();
        showcaseImg.loading = "eager";
        await showcaseImg.decode();
        if (showcaseImg.naturalWidth === 0) {
          throw Error(`Broken showcase image for tab: ${tab.textContent?.trim()}`);
        }
      }
      activeTab.click();
    }
    const previews = document.querySelectorAll<HTMLAnchorElement>("#ios-examples .native-preview");
    if (!previews.length) throw Error("Missing native example previews");
    for (const preview of previews) {
      const image = preview.querySelector("img")!;
      image.loading = "eager";
      await image.decode();
      if (
        image.currentSrc !== preview.href ||
        image.naturalWidth !== Number(image.getAttribute("width")) ||
        image.naturalHeight !== Number(image.getAttribute("height"))
      ) {
        throw Error(`Native example does not open its full-resolution capture: ${preview.href}`);
      }
    }
  }
  const rect = (selector: string) => document.querySelector(selector)!.getBoundingClientRect();
  const language = document.querySelector<HTMLSelectElement>("[data-lang-select]")!;
  const original = language.value;
  try {
    for (const option of language.options) {
      language.value = option.value;
      language.dispatchEvent(new Event("change", { bubbles: true }));
      if (location.pathname === "/index.html") {
        const ref = document.querySelector("[data-install-ref]")?.textContent?.trim();
        const guide = document.querySelector<HTMLAnchorElement>("[data-install-guide]");
        const quickstart = document.querySelector<HTMLAnchorElement>("[data-quickstart-guide]");
        const file = option.value === "en" ? "README.md" : `README.${option.value}.md`;
        if (!ref || guide?.href !== `https://github.com/de-novo/dui/blob/${ref}/${file}`) {
          throw Error(`Release guide must match the install ref and language: ${guide?.href}`);
        }
        if (quickstart?.href !== `https://github.com/de-novo/dui/blob/main/${file}#first-run`) {
          throw Error(`Quickstart must open the tested instructions: ${quickstart?.href}`);
        }
      }
      const content = rect("main > :first-child");
      const header = rect(".site-header");
      const brand = rect(".brand");
      const actions = rect(".header-actions");
      const viewport = document.documentElement.clientWidth;
      const main = document.querySelector("main")!;
      const rightEdge =
        main.getBoundingClientRect().right - parseFloat(getComputedStyle(main).paddingRight);
      if (
        Math.abs(brand.left - content.left) > 1 ||
        Math.abs(actions.right - rightEdge) > 1 ||
        Math.abs(brand.top + brand.height / 2 - (actions.top + actions.height / 2)) > 1 ||
        content.top <= header.bottom ||
        document.documentElement.scrollWidth > viewport + 1
      ) {
        throw Error(
          `Misaligned or overflowing layout: ${location.pathname}, ${option.value}, ${viewport}px`,
        );
      }
      const nav = rect(".site-nav");
      for (const link of document.querySelectorAll(".site-nav a")) {
        const bounds = link.getBoundingClientRect();
        if (bounds.left < nav.left - 1 || bounds.right > nav.right + 1) {
          throw Error(`Navigation link requires scrolling: ${link.textContent} at ${viewport}px`);
        }
      }
      for (const command of document.querySelectorAll(".card > pre.code-block:last-child")) {
        const card = command.parentElement!;
        const style = getComputedStyle(card);
        const bottom =
          card.getBoundingClientRect().bottom -
          parseFloat(style.paddingBottom) -
          parseFloat(style.borderBottomWidth);
        if (Math.abs(command.getBoundingClientRect().bottom - bottom) > 1) {
          throw Error(`Command panel above its card footer at ${viewport}px`);
        }
      }
      for (const button of document.querySelectorAll("main .cta-row .btn")) {
        const range = document.createRange();
        range.selectNodeContents(button);
        const text = range.getBoundingClientRect();
        const box = button.getBoundingClientRect();
        if (
          text.left < box.left - 1 ||
          text.right > box.right + 1 ||
          text.top < box.top - 1 ||
          text.bottom > box.bottom + 1
        ) {
          throw Error(`Clipped action: ${button.textContent?.trim()} at ${viewport}px`);
        }
      }
    }
    const theme = document.querySelector<HTMLButtonElement>("[data-theme-toggle]")!;
    const checkScheme = () => {
      const expected = document.documentElement.dataset.theme ?? "dark";
      if (getComputedStyle(document.documentElement).colorScheme !== expected) {
        throw Error(`Native controls do not match the ${expected} theme`);
      }
    };
    checkScheme();
    theme.click();
    try {
      checkScheme();
    } finally {
      theme.click();
    }
  } finally {
    language.value = original;
    language.dispatchEvent(new Event("change", { bubbles: true }));
  }
}

// All public content pages use the same stylesheet. Chromium resolves its media
// lengths (including rem) in CSS pixels, so the probes follow stylesheet edits.
async function contentWidths(page: ChromiumPage, url: string) {
  await page.call("DOM.enable");
  await page.call("CSS.enable");
  await page.call("Page.navigate", { url });
  await page.call("Runtime.evaluate", {
    expression:
      'document.readyState === "complete" || new Promise(resolve => addEventListener("load", () => resolve(true), { once: true }))',
    awaitPromise: true,
  });
  const { medias } = (await page.call("CSS.getMediaQueries")) as {
    medias: Array<{
      mediaList?: Array<{ expressions: Array<{ feature: string; computedLength?: number }> }>;
    }>;
  };
  await page.call("CSS.disable");
  await page.call("DOM.disable");
  const widths = new Set([320, 390, 768, 1440]);
  for (const media of medias.flatMap((rule) => rule.mediaList ?? [])) {
    for (const { feature, computedLength } of media.expressions) {
      if (computedLength === undefined || !["width", "min-width", "max-width"].includes(feature))
        continue;
      const boundary = Math.floor(computedLength);
      for (const width of [boundary - 1, boundary, boundary + 1]) {
        if (width >= 320 && width <= 1440) widths.add(width);
      }
    }
  }
  return [...widths].sort((a, b) => a - b);
}

async function verifyRenderedPages(
  port: number,
  archives: Array<{ key: string; populated: number; readyPreviews: number }> = [],
) {
  const firstRunKey = archives.find((a) => a.populated >= 2)?.key;
  const hasDuiDiff =
    archives.some((a) => a.key === "dui-site") &&
    archives.some((a) => a.key === "dui-site-baseline");
  const hasSourcePreview = archives.some((a) => a.key === "native-source-preview");
  return withChromiumPage(async (page) => {
    const { call } = page;
    const widths = await contentWidths(page, `http://127.0.0.1:${port}/index.html`);
    const checks: Array<{
      name: string;
      url: string;
      width: number;
      height: number;
      mobile: boolean;
      requireSelector: string;
      verifyThemes?: boolean;
    }> = [
      ...fs
        .readdirSync(siteDir)
        .filter((file) => file.endsWith(".html"))
        .flatMap((file) =>
          widths.map((width) => ({
            name: file + "-" + width,
            width,
            height: width < 768 ? 844 : 920,
            mobile: width <= 390,
            url: `http://127.0.0.1:${port}/${file}`,
            requireSelector:
              file === "index.html" ? (width < 768 ? "[data-copy]" : ".oss-card") : ".hero h1",
          })),
        ),
      ...(firstRunKey
        ? viewerViewports.map((viewport) => ({
            ...viewport,
            verifyThemes: true,
            name: viewport.mobile ? "demo-mobile-deck" : "demo-desktop-canvas",
            url: `http://127.0.0.1:${port}/demo/?run=${encodeURIComponent(firstRunKey)}`,
          }))
        : []),
    ];

    async function verifyNodeHover() {
      const evaluate = async (expression: string) => {
        const response = await call("Runtime.evaluate", { expression, returnByValue: true });
        if (response.exceptionDetails) throw new Error(JSON.stringify(response.exceptionDetails));
        return (response.result as { value: unknown }).value;
      };
      const point = (await evaluate(`(() => {
        const nodes = Array.from(document.querySelectorAll(".react-flow__node"));
        const point = nodes.map(node => {
          const rect = node.getBoundingClientRect();
          const x = rect.left + rect.width / 2;
          const y = rect.top + rect.height / 2;
          return node.contains(document.elementFromPoint(x, y)) ? { x, y } : null;
        }).find(Boolean);
        if (!point) throw Error("No screen node available for hover");
        const edges = Array.from(document.querySelectorAll(".react-flow__edge-path"));
        let hidden = false;
        const observer = new MutationObserver(records => {
          if (records.some(r => r.oldValue?.includes("visibility: hidden") || r.target.style.visibility === "hidden")) hidden = true;
        });
        for (const node of nodes) observer.observe(node, { attributes: true, attributeFilter: ["style"], attributeOldValue: true });
        window.checkHover = () => {
          observer.disconnect();
          return !hidden && [...nodes, ...edges].every(el => el.isConnected);
        };
        return point;
      })()`)) as { x: number; y: number };
      for (const position of [point, { x: 1, y: 1 }]) {
        await call("Input.dispatchMouseEvent", { type: "mouseMoved", ...position });
        await new Promise((r) => setTimeout(r, 200));
      }
      if (!(await evaluate("window.checkHover()"))) {
        throw new Error("Hovering the map hid measured screen nodes or replaced connected edges");
      }
    }

    async function verifyMapClearance() {
      const result = await call("Runtime.evaluate", {
        expression: `(async () => {
          function checkBounds() {
            const cards = Array.from(document.querySelectorAll('.react-flow__node .screen-card')).map(card => ({
              rect: card.getBoundingClientRect(), id: card.closest('.react-flow__node').dataset.id
            }));
            for (const [i, a] of cards.entries()) {
              for (const b of cards.slice(i + 1)) {
                if (a.rect.left < b.rect.right && b.rect.left < a.rect.right &&
                    a.rect.top < b.rect.bottom && b.rect.top < a.rect.bottom) {
                  throw Error('Overlapping screen cards: ' + a.id + ' and ' + b.id);
                }
              }
            }
            for (const edge of document.querySelectorAll('.react-flow__edge-path')) {
              const obstacles = cards.filter(card => !JSON.parse(edge.id).includes(card.id));
              const length = edge.getTotalLength();
              const matrix = edge.getScreenCTM();
              for (let distance = 0; distance < length; distance += 12) {
                const point = edge.getPointAtLength(distance).matrixTransform(matrix);
                const crossed = obstacles.find(({rect}) => point.x > rect.left && point.x < rect.right &&
                  point.y > rect.top && point.y < rect.bottom);
                if (crossed) throw Error('Connection ' + edge.id + ' crosses ' + crossed.id);
              }
            }
          }
          checkBounds();
          const style = document.createElement("style");
          style.textContent = ".screen-card .card-caption { min-height: 200vh }";
          document.head.append(style);
          try {
            for (let frame = 0; frame < 6; frame++) await new Promise(requestAnimationFrame);
            checkBounds();
          } finally {
            style.remove();
          }
        })()`,
        awaitPromise: true,
      });
      if (result.exceptionDetails) throw Error(JSON.stringify(result.exceptionDetails));
    }

    const results: Array<Record<string, unknown>> = [];
    for (const c of checks) {
      await call("Emulation.setDeviceMetricsOverride", {
        width: c.width,
        height: c.height,
        deviceScaleFactor: c.mobile ? 2 : 1,
        mobile: c.mobile,
      });
      const themes = c.verifyThemes
        ? await verifyViewerThemes(page, c.url, c.requireSelector)
        : undefined;
      await call("Page.navigate", { url: c.url });
      await new Promise((r) => setTimeout(r, 1500));
      if (!c.verifyThemes) {
        const layout = await call("Runtime.evaluate", {
          expression: `(${checkContentPage.toString()})()`,
          awaitPromise: true,
        });
        if (layout.exceptionDetails) throw Error(JSON.stringify(layout.exceptionDetails));
      }
      const evalRes = (await call("Runtime.evaluate", {
        expression: `(() => {
          const el = document.querySelector(${JSON.stringify(c.requireSelector)});
          const rect = el ? el.getBoundingClientRect() : null;
          const brokenImgs = Array.from(document.querySelectorAll('img')).filter(i => i.complete && i.naturalWidth === 0).length;
          return {
            innerWidth: window.innerWidth,
            scrollWidth: document.documentElement.scrollWidth,
            found: Boolean(el),
            inViewportX: rect ? (rect.left >= 0 && rect.right <= window.innerWidth + 1) : false,
            brokenImgs,
          };
        })()`,
        returnByValue: true,
      })) as { result?: { value?: Record<string, unknown> }; exceptionDetails?: unknown };
      if (evalRes.exceptionDetails) throw Error(JSON.stringify(evalRes.exceptionDetails));
      const val = evalRes.result?.value ?? {};
      if (c.name === "demo-desktop-canvas") {
        await verifyNodeHover();
        val.stableNodeHover = true;
        await verifyMapClearance();
        val.clearanceAfterResize = true;
      }
      results.push({ name: c.name, ...val, themes });
      if (!val.found) {
        throw new Error(`Self-dogfood gate failed (${c.name}): missing ${c.requireSelector}`);
      }
      if (Number(val.scrollWidth) > Number(val.innerWidth) + 1) {
        throw new Error(
          `Self-dogfood gate failed (${c.name}): horizontal overflow scrollWidth=${val.scrollWidth} > innerWidth=${val.innerWidth}`,
        );
      }
      if (!val.inViewportX) {
        throw new Error(
          `Self-dogfood gate failed (${c.name}): ${c.requireSelector} clipped outside viewport`,
        );
      }
      if (Number(val.brokenImgs) > 0) {
        throw new Error(`Self-dogfood gate failed (${c.name}): ${val.brokenImgs} broken images`);
      }
    }
    const demo = checks.find((check) => check.verifyThemes);
    if (demo) results.push({ mapControls: await verifyMapControls(page, demo.url) });

    if (hasDuiDiff) {
      await call("Emulation.setDeviceMetricsOverride", {
        width: 1440,
        height: 920,
        deviceScaleFactor: 1,
        mobile: false,
      });
      // 1. Inline diff: top bar clearance, unique project labels, no truncated compare labels.
      await call("Page.navigate", {
        url: `http://127.0.0.1:${port}/demo/?run=dui-site&compare=dui-site-baseline`,
      });
      await new Promise((r) => setTimeout(r, 1500));
      const inlineDiffCheck = await call("Runtime.evaluate", {
        expression: `(async () => {
          for (let i = 0; i < 6; i++) await new Promise(requestAnimationFrame);
          const projectSelect = document.querySelector(".showcase-project-bar select");
          const projectLabels = projectSelect ? Array.from(projectSelect.options).map(o => o.text) : [];
          if (new Set(projectLabels).size !== projectLabels.length) {
            throw Error("Duplicate labels in showcase project switcher: " + projectLabels.join(", "));
          }
          const compareSelect = document.querySelector('.canvas-diff-bar:not(.showcase-project-bar) select');
          const compareLabels = compareSelect ? Array.from(compareSelect.options).map(o => o.text) : [];
          if (compareLabels.some(label => /\\bapture\\b/.test(label))) {
            throw Error("Truncated compare label found: " + compareLabels.join(", "));
          }
          const topBar = document.querySelector(".map-top-bar")?.getBoundingClientRect();
          const cards = Array.from(document.querySelectorAll(".react-flow__node .screen-card")).map(c => c.getBoundingClientRect());
          if (topBar && cards.some(card => card.left < topBar.right && topBar.left < card.right && card.top < topBar.bottom && topBar.top < card.bottom)) {
            throw Error("Floating map-top-bar overlaps a fitted screen card in inline diff mode");
          }
          return { projectLabels: projectLabels.length, compareLabels };
        })()`,
        awaitPromise: true,
        returnByValue: true,
      });
      if (inlineDiffCheck.exceptionDetails) {
        throw new Error(JSON.stringify(inlineDiffCheck.exceptionDetails));
      }
      results.push({ inlineDiff: (inlineDiffCheck.result as { value: unknown }).value });

      // 2. Full CaptureDiffView: zero broken before/after/overlay images.
      await call("Page.navigate", {
        url: `http://127.0.0.1:${port}/demo/?run=dui-site&view=diff&before=dui-site-baseline&after=dui-site`,
      });
      await new Promise((r) => setTimeout(r, 1500));
      const fullDiffCheck = await call("Runtime.evaluate", {
        expression: `(async () => {
          const imgs = Array.from(document.querySelectorAll(".diff-workspace img"));
          if (imgs.length < 3) throw Error("Expected before, after, and overlay images in CaptureDiffView");
          for (const img of imgs) {
            img.loading = "eager";
            await img.decode();
            if (img.naturalWidth === 0) throw Error("Broken CaptureDiffView image: " + img.src);
          }
          return { diffImages: imgs.length };
        })()`,
        awaitPromise: true,
        returnByValue: true,
      });
      if (fullDiffCheck.exceptionDetails) {
        throw new Error(JSON.stringify(fullDiffCheck.exceptionDetails));
      }
      results.push({ fullDiff: (fullDiffCheck.result as { value: unknown }).value });
    }

    if (hasSourcePreview) {
      for (const vp of viewerViewports) {
        await call("Emulation.setDeviceMetricsOverride", {
          width: vp.width,
          height: vp.height,
          deviceScaleFactor: vp.mobile ? 2 : 1,
          mobile: vp.mobile,
        });
        await call("Page.navigate", {
          url: `http://127.0.0.1:${port}/demo/?run=native-source-preview`,
        });
        await new Promise((r) => setTimeout(r, 1500));
        const previewCheck = await call("Runtime.evaluate", {
          expression: `(async () => {
            const cards = Array.from(document.querySelectorAll(${JSON.stringify(vp.mobile ? ".mobile-deck-card" : ".react-flow__node")}));
            if (cards.length < 2) throw Error("Expected SwiftUI & Compose source preview cards");
            if (cards.some((card) => card.querySelector(".preview-switch"))) {
              throw Error("A map card drew a portable preview as if it were a screen");
            }
            // Each card shows its attached native simulator or emulator capture.
            for (const card of cards) {
              const img = card.querySelector("img");
              if (!img) throw Error("Source card is missing its native capture");
              img.loading = "eager";
              await img.decode();
              if (img.naturalWidth === 0) throw Error("Native capture did not decode");
            }
            const hit = ${vp.mobile} ? cards[0].querySelector("button") : cards[0];
            hit.click();
            const deadline = performance.now() + 8000;
            let sw = null;
            while (performance.now() < deadline) {
              sw = document.querySelector('.source-preview-panel [role="switch"]');
              if (sw) break;
              await new Promise(requestAnimationFrame);
            }
            if (!sw) throw Error("Source preview switches did not appear");
            if (!document.querySelector(".source-preview-panel .source-preview-note")?.textContent) {
              throw Error("Source preview is missing its not-native disclaimer");
            }
            const before = sw.checked;
            sw.click();
            await new Promise(requestAnimationFrame);
            if (sw.checked === before) throw Error("Source preview switch did not toggle");
            if (document.documentElement.scrollWidth > window.innerWidth + 1) {
              throw Error("Horizontal overflow on native source preview");
            }
            return { cards: cards.length, toggled: true };
          })()`,
          awaitPromise: true,
          returnByValue: true,
        });
        if (previewCheck.exceptionDetails) {
          throw new Error(JSON.stringify(previewCheck.exceptionDetails));
        }
        results.push({
          name: vp.mobile ? "source-preview-mobile" : "source-preview-desktop",
          ...(previewCheck.result as { value?: Record<string, unknown> }).value,
        });
      }
    }

    return results;
  });
}

/**
 * Self-dogfood: map every published demo page and the site with dui itself, then run the
 * project-owned viewer design audit (apps/canvas/audit) on that capture.
 */
// Run dui without blocking this process: its own static server must keep answering dui's browser.
function dui(binary: string, args: string[], env: NodeJS.ProcessEnv) {
  return new Promise<{ status: number; stdout: string; stderr: string }>((resolve, reject) => {
    execFile(
      binary,
      args,
      { encoding: "utf8", env, maxBuffer: 64 * 1024 * 1024 },
      (error, stdout, stderr) => {
        const status = error ? (typeof error.code === "number" ? error.code : -1) : 0;
        if (error && status === -1) reject(error);
        else resolve({ status, stdout, stderr });
      },
    );
  });
}

async function dogfoodAudit(port: number, keys: string[]) {
  const binary = [process.env.DUI_BIN, "target/release/dui", "target/debug/dui"]
    .filter((candidate): candidate is string => Boolean(candidate))
    .map((candidate) => path.resolve(root, candidate))
    .find((candidate) => fs.existsSync(candidate));
  if (!binary) throw new Error("Build dui before verifying Pages: the self-audit runs it");
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "dui-dogfood-home-"));
  const project = fs.mkdtempSync(path.join(os.tmpdir(), "dui-dogfood-"));
  const env = { ...process.env, HOME: home };
  const failures: string[] = [];
  let cells = 0;
  try {
    for (const key of keys) {
      const url = `http://127.0.0.1:${port}/demo/?run=${encodeURIComponent(key)}`;
      const mapped = await dui(binary, ["map", project, "--url", url], env);
      if (mapped.status !== 0 && !mapped.stdout.trim())
        throw new Error(`dui map failed for ${key}: ${mapped.stderr}`);
      const run = JSON.parse(mapped.stdout);
      cells += run.coverage.attempted;
      if (run.coverage.verified !== run.coverage.attempted)
        failures.push(`${key}: ${run.coverage.verified}/${run.coverage.attempted} cells verified`);
      const audit = await dui(
        binary,
        ["audit", run.output, "--design", path.join(root, "apps/canvas/audit/dui.json")],
        env,
      );
      if (audit.status === 5) {
        const report = JSON.parse(audit.stdout).designAudit;
        for (const finding of report.findings)
          failures.push(
            `${key} ${finding.cell.path} @${finding.cell.width}: ${finding.rule} ${finding.nodeId} (${finding.measured})`,
          );
        for (const reason of report.unverified ?? []) failures.push(`${key}: unverified ${reason}`);
      } else if (audit.status !== 0) {
        throw new Error(`dui audit failed for ${key}: ${audit.stderr || audit.stdout}`);
      }
    }
  } finally {
    fs.rmSync(home, { recursive: true, force: true });
    fs.rmSync(project, { recursive: true, force: true });
  }
  if (failures.length) throw new Error("dui self-audit:\n" + failures.slice(0, 40).join("\n"));
  return { pages: keys.length, cells };
}

async function main() {
  const { server, port } = await startStaticServer();
  try {
    if (process.argv.includes("--layout-only")) {
      console.log(JSON.stringify(await verifyRenderedPages(port)));
      return;
    }
    const archiveSummary = verifyArchives();
    const installer = await fetch(`http://127.0.0.1:${port}/install`);
    if (
      installer.status !== 200 ||
      !Buffer.from(await installer.arrayBuffer()).equals(
        fs.readFileSync(path.join(root, "scripts/install")),
      )
    ) {
      throw new Error("The published /install must serve scripts/install byte for byte");
    }
    const rendered = await verifyRenderedPages(port, archiveSummary);
    const dogfood = await dogfoodAudit(
      port,
      archiveSummary.map((archive) => archive.key),
    );
    console.log(
      JSON.stringify({ status: "ok", archives: archiveSummary, rendered, dogfood }, null, 2),
    );
  } finally {
    server.close();
  }
}

await main();
