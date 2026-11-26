// Map cards small on screen draw the capture's thumbnail, and large ones the PNG
// (docs/en/web-viewer.md#shared-screen-maps, #350). Checked at the fit view and at 1:1, on desktop
// and mobile viewports, with the bytes of images each view loads.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { withChromiumPage } from "./chromium-page.mts";

const root = resolve(import.meta.dirname, "..");
const binary = resolve(process.env.DUI_BIN ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-thumbnails-"));
const site = join(stage, "site");
const home = join(stage, "home");
mkdirSync(site);
const pages = Array.from({ length: 16 }, (_, i) => i);
for (const i of pages) {
  writeFileSync(
    join(site, i === 0 ? "index.html" : `p${i}.html`),
    `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Page ${i}</title></head>
<body style="font-family:sans-serif;margin:0"><nav style="padding:12px;background:#1d2330">${pages
      .map(
        (p) =>
          `<a style="color:#fff;margin-right:8px" href="/${p ? `p${p}.html` : ""}">Page ${p}</a>`,
      )
      .join("")}</nav>
<main style="padding:24px"><h1>Page ${i}</h1>${"<p>Orders, stock and prices for this page.</p>".repeat(8)}</main></body></html>`,
  );
}

type Card = { src: string; device: number; card: number; complete: boolean; natural: number };
type View = { cards: Card[]; overflow: number; bytes: { thumb: number; full: number } };

let server: ReturnType<typeof spawn> | undefined;
try {
  writeFileSync(join(site, "dui.json"), JSON.stringify({ widths: [1280] }));
  const run = spawnSync(binary, ["map", site, "--build-dir", site, "--capture"], {
    encoding: "utf8",
    env: { ...process.env, HOME: home },
  });
  assert.equal(run.status, 0, run.stderr.slice(-2000));
  const report = JSON.parse(run.stdout) as {
    output: string;
    thumbnails?: { made: number };
    cells: { image?: string }[];
  };
  const images = report.cells.flatMap((c) => (c.image ? [c.image] : []));
  assert.equal(report.thumbnails?.made, images.length, JSON.stringify(report.thumbnails));
  for (const image of images)
    assert.ok(existsSync(join(report.output, "thumb-" + image)), `thumbnail of ${image}`);

  server = spawn(binary, ["open", "--no-open", report.output], {
    env: { ...process.env, HOME: home },
    stdio: ["ignore", "pipe", "ignore"],
  });
  const url = await new Promise<string>((done, fail) => {
    const timer = setTimeout(() => fail(new Error("dui open timed out")), 30_000);
    createInterface({ input: server!.stdout! }).on("line", (line) => {
      const m = line.match(/http:\/\/127\.0\.0\.1:\d+/);
      if (m) {
        clearTimeout(timer);
        done(m[0]);
      }
    });
  });

  const results: Record<string, unknown> = {};
  for (const [vw, vh, label] of [
    [1440, 920, "desktop"],
    [390, 844, "mobile"],
  ] as const) {
    await withChromiumPage(async (page) => {
      const ev = async <T,>(expression: string) =>
        (
          (await page.call("Runtime.evaluate", {
            expression,
            returnByValue: true,
            awaitPromise: true,
          })) as { result: { value: T } }
        ).result.value;
      await page.call("Emulation.setDeviceMetricsOverride", {
        width: vw,
        height: vh,
        deviceScaleFactor: 2,
        mobile: label === "mobile",
      });
      await page.call("Page.navigate", { url });
      // The cards in view, each with the image it draws and its width on screen in device pixels.
      const view = async (): Promise<View> => {
        for (let i = 0; i < 100; i++) {
          const ready = await ev<boolean>(
            "(() => { const imgs = [...document.querySelectorAll('.capture-frame img, .mobile-deck-frame img')].filter(i => { const r = i.getBoundingClientRect(); return r.width > 0 && r.right > 0 && r.left < innerWidth && r.bottom > 0 && r.top < innerHeight; }); return imgs.length > 0 && imgs.every(i => i.complete); })()",
          );
          if (ready) break;
          await new Promise((r) => setTimeout(r, 100));
        }
        await new Promise((r) => setTimeout(r, 300));
        return ev<View>(`(() => {
          const cards = [...document.querySelectorAll('.capture-frame img, .mobile-deck-frame img')].flatMap(i => {
            const r = i.getBoundingClientRect();
            if (!(r.width > 0 && r.right > 0 && r.left < innerWidth && r.bottom > 0 && r.top < innerHeight)) return [];
            return [{ src: i.currentSrc || i.src, device: Math.round(r.width * devicePixelRatio), card: Math.round(i.closest('.screen-card, .mobile-deck-frame').getBoundingClientRect().width * devicePixelRatio), complete: i.complete, natural: i.naturalWidth }];
          });
          const sum = (thumb) => performance.getEntriesByType('resource')
            .filter(e => /\\.png/.test(e.name) && /\\/thumb-/.test(e.name) === thumb)
            .reduce((s, e) => s + (e.encodedBodySize || 0), 0);
          return { cards, overflow: document.scrollingElement.scrollWidth - innerWidth, bytes: { thumb: sum(true), full: sum(false) } };
        })()`);
      };
      const check = (name: string, v: View) => {
        assert.ok(v.cards.length > 0, `${label} ${name}: cards in view`);
        assert.equal(v.overflow, 0, `${label} ${name}: horizontal overflow`);
        for (const card of v.cards) {
          assert.ok(
            card.complete && card.natural > 0,
            `${label} ${name}: broken image ${card.src}`,
          );
          // A thumbnail is drawn no wider than its own pixels; a card wider than that draws the PNG.
          const thumb = /\/thumb-/.test(card.src);
          assert.ok(
            thumb ? card.device <= 200 : card.card > 200,
            `${label} ${name}: ${card.src} in a card ${card.card} device px wide`,
          );
        }
      };
      const fit = await view();
      check("fit", fit);
      if (label === "desktop") {
        // Zoomed out, every card is small.
        for (let i = 0; i < 4; i++) {
          await ev("document.querySelector('.react-flow__controls-zoomout')?.click()");
          await new Promise((r) => setTimeout(r, 250));
        }
        const out = await view();
        check("zoomed out", out);
        assert.ok(
          out.cards.every((c) => /\/thumb-/.test(c.src)),
          `every card is small zoomed out: ${JSON.stringify(out.cards.map((c) => c.device))}`,
        );
        // 1:1 makes every card the capture's own width: the PNG takes over.
        await ev("document.querySelector('.zoom-1x-label')?.closest('button')?.click()");
        await new Promise((r) => setTimeout(r, 600));
        const large = await view();
        check("1:1", large);
        assert.ok(
          large.cards.every((c) => !/\/thumb-/.test(c.src)),
          "large cards draw the PNG",
        );
        results[label] = { fit: fit.bytes, oneToOne: large.bytes, cards: fit.cards.length };
      } else {
        results[label] = { fit: fit.bytes, cards: fit.cards.length };
      }
    });
  }
  console.log(JSON.stringify({ ok: true, ...results }));
} finally {
  server?.kill("SIGTERM");
  rmSync(stage, { recursive: true, force: true });
}
