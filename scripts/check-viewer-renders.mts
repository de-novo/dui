// The map viewer re-renders only what an interaction changes. A React DevTools-style commit
// hook counts the components that actually ran, per interaction, on a 24-page map with dialogs:
// moving over the map and selecting a card must not re-render every edge and card.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { withChromiumPage } from "./chromium-page.mts";

const root = resolve(import.meta.dirname, "..");
const binary = resolve(process.env.DUI_BIN ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-viewer-renders-"));
const home = join(stage, "home");

// Counts components whose function ran in a commit: a fiber object not committed in the
// previous commit (React alternates objects for re-rendered fibers) with PerformedWork set.
// Vite prints its URL with color codes.
const stripAnsi = (text: string) =>
  text.replaceAll(String.fromCharCode(27), "").replace(/\[[0-9;]*m/g, "");

const hook = `(() => {
  const state = { commits: 0, renders: {}, seen: new WeakSet() };
  window.__renders = state;
  const nameOf = (f) => {
    const t = f.type;
    if (typeof t === "function") return t.displayName || t.name || null;
    if (t && typeof t === "object") { const inner = t.render || t.type; return t.displayName || (inner && (inner.displayName || inner.name)) || null; }
    return null;
  };
  window.__REACT_DEVTOOLS_GLOBAL_HOOK__ = {
    supportsFiber: true, renderers: new Map(),
    inject(r) { const id = this.renderers.size + 1; this.renderers.set(id, r); return id; },
    onScheduleFiberRoot() {}, onCommitFiberUnmount() {}, onPostCommitFiberRoot() {}, checkDCE() {},
    onCommitFiberRoot(_id, root) {
      state.commits++;
      const next = new WeakSet();
      const stack = [root.current];
      while (stack.length) {
        const f = stack.pop();
        next.add(f);
        if (!state.seen.has(f) && (f.flags & 1)) { const n = nameOf(f); if (n) state.renders[n] = (state.renders[n] || 0) + 1; }
        if (f.sibling) stack.push(f.sibling);
        if (f.child) stack.push(f.child);
      }
      state.seen = next;
    },
  };
})();`;

const site = join(stage, "site");
mkdirSync(site);
const pages = Array.from({ length: 24 }, (_, i) => i);
for (const i of pages) {
  writeFileSync(
    join(site, i === 0 ? "index.html" : `p${i}.html`),
    `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Page ${i}</title></head><body>
<header><button type="button" aria-haspopup="dialog" onclick="profile.showModal()">Profile</button></header>
<nav>${pages.map((p) => `<a href="/p${p}.html">Page ${p}</a>`).join(" ")}</nav>
<main><h1>Page ${i}</h1><button type="button" aria-haspopup="dialog" onclick="create.showModal()">Create item ${i}</button></main>
<dialog id="profile" aria-label="Profile" style="width:420px;height:220px"><h2>Profile</h2></dialog>
<dialog id="create" aria-label="Create item ${i}" style="width:420px;height:220px"><h2>Create item ${i}</h2></dialog>
</body></html>`,
  );
}
let server: ReturnType<typeof spawn> | undefined;
try {
  // Exit 5 is fine here: the map only needs to exist.
  const run = spawnSync(binary, ["map", site, "--build-dir", site, "--capture"], {
    encoding: "utf8",
    env: { ...process.env, HOME: home },
  });
  assert.ok(run.status === 0 || run.status === 5, run.stderr);
  const mapped = JSON.parse(run.stdout) as { output?: string };
  assert.ok(mapped.output, "dui map produced a run");
  server = spawn(process.execPath, [join(root, "apps/canvas/preview.mts"), mapped.output], {
    cwd: join(root, "apps/canvas"),
    env: { ...process.env, HOME: home },
    stdio: ["ignore", "pipe", "ignore"],
  });
  const url = await new Promise<string>((done, fail) => {
    const timer = setTimeout(() => fail(new Error("viewer dev server timed out")), 60_000);
    createInterface({ input: server!.stdout! }).on("line", (line) => {
      const m = stripAnsi(line).match(/(http:\/\/(?:localhost|127\.0\.0\.1):\d+\/?)/);
      if (m) {
        clearTimeout(timer);
        done(m[1]!);
      }
    });
  });
  const result = await withChromiumPage(async (page) => {
    const ev = async <T,>(expression: string) =>
      (
        (await page.call("Runtime.evaluate", {
          expression,
          returnByValue: true,
          awaitPromise: true,
        })) as {
          result: { value: T };
        }
      ).result.value;
    const mouse = (type: string, x: number, y: number, extra: Record<string, unknown> = {}) =>
      page.call("Input.dispatchMouseEvent", { type, x, y, ...extra });
    await page.call("Page.enable");
    await page.call("Emulation.setDeviceMetricsOverride", {
      width: 1440,
      height: 920,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await page.call("Page.addScriptToEvaluateOnNewDocument", { source: hook });
    await page.call("Page.navigate", { url });
    for (
      let i = 0;
      i < 150 && !(await ev<number>("document.querySelectorAll('.react-flow__node').length"));
      i++
    )
      await new Promise((r) => setTimeout(r, 200));
    await new Promise((r) => setTimeout(r, 3000));
    const counts = {
      cards: await ev<number>("document.querySelectorAll('.react-flow__node').length"),
      edges: await ev<number>("document.querySelectorAll('.react-flow__edge').length"),
    };
    // Link edges start at the anchor that leads to their target (#321): every anchor dot has an
    // edge whose drawn path begins on it.
    const anchors = await ev<{ dots: number; worst: number }>(`(() => {
      const starts = [...document.querySelectorAll('.react-flow__edge path.react-flow__edge-path')].map(p => {
        const m = p.getScreenCTM(); const pt = p.getPointAtLength(0);
        return { x: m.a * pt.x + m.c * pt.y + m.e, y: m.b * pt.x + m.d * pt.y + m.f };
      });
      const dots = [...document.querySelectorAll('.link-anchor-dot')].map(d => { const r = d.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; });
      const worst = Math.max(0, ...dots.map(d => Math.min(...starts.map(s => Math.hypot(s.x - d.x, s.y - d.y)))));
      return { dots: dots.length, worst };
    })()`);
    const measure = async (act: () => Promise<void>) => {
      await ev("(window.__renders.commits = 0, window.__renders.renders = {}, true)");
      await act();
      await new Promise((r) => setTimeout(r, 1000));
      return ev<{ commits: number; renders: Record<string, number> }>(
        "({ commits: window.__renders.commits, renders: window.__renders.renders })",
      );
    };
    const pane = await ev<{ x: number; y: number; w: number; h: number }>(
      "(() => { const r = document.querySelector('.react-flow').getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height }; })()",
    );
    const idle = await measure(() => new Promise((r) => setTimeout(r, 2000)));
    // Enter and leave the visible cards one after another, as a pointer crossing the map does.
    const centers = await ev<{ x: number; y: number }[]>(
      "[...document.querySelectorAll('.react-flow__node')].map(e => e.getBoundingClientRect()).filter(r => r.width > 20 && r.x > 300 && r.y > 80 && r.right < innerWidth && r.bottom < innerHeight).slice(0, 10).map(r => ({ x: r.x + r.width / 2, y: r.y + r.height / 2 }))",
    );
    const sweep = await measure(async () => {
      for (const c of centers) {
        await mouse("mouseMoved", c.x, c.y);
        await mouse("mouseMoved", c.x, c.y - 200);
      }
    });
    await mouse("mouseMoved", pane.x + 5, pane.y + 5);
    await new Promise((r) => setTimeout(r, 400));
    const card = await ev<{ x: number; y: number }>(
      "(() => { const n = [...document.querySelectorAll('.react-flow__node')].find(e => { const r = e.getBoundingClientRect(); return r.width > 20 && r.x > 300 && r.y > 80 && r.right < innerWidth && r.bottom < innerHeight; }); const r = n.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()",
    );
    const select = await measure(async () => {
      await mouse("mousePressed", card.x, card.y, { button: "left", clickCount: 1 });
      await mouse("mouseReleased", card.x, card.y, { button: "left", clickCount: 1 });
    });
    // Map options switch where links start: the card border, then the link element again.
    const edgeStart = async (value: string) => {
      await ev("document.querySelector('.map-options').click(), true");
      await new Promise((r) => setTimeout(r, 500));
      await ev(
        `(() => { const s = document.querySelector('[data-map-edge-start]'); s.value = ${JSON.stringify(value)}; s.dispatchEvent(new Event('change', { bubbles: true })); return true; })()`,
      );
      await page.call("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape" });
      await page.call("Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape" });
      await new Promise((r) => setTimeout(r, 1500));
      return ev<{ search: string; dots: number; edges: number }>(
        "({ search: location.search, dots: document.querySelectorAll('.link-anchor-dot').length, edges: document.querySelectorAll('.react-flow__edge').length })",
      );
    };
    const cardMode = await edgeStart("card");
    const elementMode = await edgeStart("element");
    return { counts, anchors, idle, sweep, select, cardMode, elementMode };
  });
  const { counts, anchors, idle, sweep, select, cardMode, elementMode } = result;
  assert.ok(
    cardMode.search.includes("edges=card") && cardMode.dots === 0 && cardMode.edges > 0,
    `card border mode: ${JSON.stringify(cardMode)}`,
  );
  assert.ok(
    !elementMode.search.includes("edges=") && elementMode.dots === anchors.dots,
    `link element mode: ${JSON.stringify(elementMode)}`,
  );
  assert.ok(anchors.dots > 0, `link edges leave from anchors: ${JSON.stringify(anchors)}`);
  assert.ok(anchors.worst <= 1, `edges start on their anchors: ${JSON.stringify(anchors)}`);
  assert.ok(counts.cards >= 40 && counts.edges >= 20, JSON.stringify(counts));
  assert.equal(idle.commits, 0, "an idle map does not re-render");
  assert.ok(sweep.commits > 0, "moving over cards changes the hover focus");
  // Hover touches only the edges of the cards entered and left, never all of them each time.
  assert.ok(
    (sweep.renders.GutterEdge ?? 0) < counts.edges * 2,
    `edges re-rendered on hover: ${JSON.stringify(sweep)}`,
  );
  // Selecting re-renders the cards whose state changed, not every card on every commit.
  assert.ok(
    (select.renders.CaptureCard ?? 0) < counts.cards,
    `cards re-rendered on select: ${JSON.stringify(select)}`,
  );
  console.log(
    JSON.stringify({
      ok: true,
      ...counts,
      anchors,
      edgeStart: { card: cardMode.dots, element: elementMode.dots },
      sweep: { commits: sweep.commits, edges: sweep.renders.GutterEdge ?? 0 },
      select: { commits: select.commits, cards: select.renders.CaptureCard ?? 0 },
    }),
  );
} finally {
  server?.kill("SIGTERM");
  rmSync(stage, { recursive: true, force: true });
}
