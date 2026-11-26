// Web map cards draw their HTML snapshot when large enough on screen and faithful, and their
// PNG otherwise (#311, docs/en/web-viewer.md#html-cards). Runs the embedded viewer of `dui open`.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { withChromiumPage } from "./chromium-page.mts";

const root = resolve(import.meta.dirname, "..");
const binary = resolve(process.env.DUI_BIN ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-html-cards-"));
const home = join(stage, "home");

const site = join(stage, "site");
mkdirSync(site);
const pages = Array.from({ length: 24 }, (_, i) => i);
// Every page uses one large font: the snapshots share it, and the viewer must serve it to the
// sandboxed frames.
writeFileSync(
  join(site, "large.woff2"),
  Buffer.from(Array.from({ length: 24 * 1024 }, (_, i) => (i * 7919) % 251)),
);
for (const i of pages) {
  writeFileSync(
    join(site, i === 0 ? "index.html" : `p${i}.html`),
    `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Page ${i}</title></head>
<style>@font-face{font-family:Large;src:url(/large.woff2) format("woff2")} h1{font-family:Large,sans-serif}</style>
<body style="font-family:sans-serif"><nav>${pages.map((p) => `<a href="/p${p}.html">Page ${p}</a>`).join(" ")}</nav>
<main><h1>Page ${i}</h1><p>Orders for page ${i}.</p></main></body></html>`,
  );
}
let server: ReturnType<typeof spawn> | undefined;
try {
  const run = spawnSync(binary, ["map", site, "--build-dir", site], {
    encoding: "utf8",
    env: { ...process.env, HOME: home },
  });
  assert.ok(run.status === 0 || run.status === 5, run.stderr);
  const output = (JSON.parse(run.stdout) as { output: string }).output;
  server = spawn(binary, ["open", "--no-open", output], {
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
  type Frames = { frames: number; loaded: number; showing: string[]; overflow: number };
  const results: Record<string, unknown> = {};
  for (const [vw, vh] of [
    [1440, 920],
    [390, 844],
  ] as const) {
    await withChromiumPage(async (page) => {
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
      const key = async (k: string, code: string) => {
        await page.call("Input.dispatchKeyEvent", { type: "keyDown", key: k, code, text: k });
        await page.call("Input.dispatchKeyEvent", { type: "keyUp", key: k, code });
      };
      const frames = () =>
        ev<Frames>(
          "({ frames: document.querySelectorAll('iframe.card-snapshot').length, loaded: document.querySelectorAll('iframe.card-snapshot-loaded').length, showing: [...new Set([...document.querySelectorAll('.capture-frame')].map(f => f.dataset.showing))], overflow: document.scrollingElement.scrollWidth - innerWidth })",
        );
      // Requests from every frame, including the sandboxed snapshot frames when they run in
      // their own process.
      const assetLoads: { url: string; status?: number; failed?: string }[] = [];
      page.ws.addEventListener("message", (event) => {
        const msg = JSON.parse(String(event.data)) as {
          method?: string;
          params?: Record<string, any>;
          sessionId?: string;
        };
        if (msg.method === "Target.attachedToTarget") {
          const sessionId = msg.params!.sessionId as string;
          for (const method of ["Network.enable", "Runtime.runIfWaitingForDebugger"])
            page.ws.send(
              JSON.stringify({ id: Math.floor(Math.random() * 1e9), sessionId, method }),
            );
        }
        if (
          msg.method === "Network.requestWillBeSent" &&
          /\/asset-[0-9a-f]{64}\./.test(msg.params!.request.url)
        )
          assetLoads.push({ url: msg.params!.request.url, id: msg.params!.requestId } as never);
        const load = assetLoads.find(
          (l) => (l as never as { id: string }).id === msg.params?.requestId,
        );
        if (load && msg.method === "Network.responseReceived")
          load.status = msg.params!.response.status;
        if (load && msg.method === "Network.loadingFailed")
          load.failed =
            msg.params!.blockedReason ??
            msg.params!.corsErrorStatus?.corsError ??
            msg.params!.errorText;
      });
      await page.call("Network.enable");
      await page.call("Target.setAutoAttach", {
        autoAttach: true,
        waitForDebuggerOnStart: true,
        flatten: true,
      });
      await page.call("Page.enable");
      await page.call("Emulation.setDeviceMetricsOverride", {
        width: vw,
        height: vh,
        deviceScaleFactor: 1,
        mobile: vw < 600,
      });
      await page.call("Page.navigate", { url });
      for (
        let i = 0;
        i < 150 && !(await ev<number>("document.querySelectorAll('.capture-frame').length"));
        i++
      )
        await new Promise((r) => setTimeout(r, 200));
      await new Promise((r) => setTimeout(r, 2000));
      const overview = await frames();
      assert.equal(overview.overflow, 0, `${vw}px: no horizontal overflow`);
      if (vw < 600) {
        results.mobile = overview;
        return;
      }
      // Small cards show their PNG.
      assert.equal(overview.frames, 0, JSON.stringify(overview));
      // 1:1: the card in view shows its snapshot, served with the snapshot policy.
      await key("0", "Digit0");
      await new Promise((r) => setTimeout(r, 2500));
      const oneToOne = await frames();
      assert.ok(
        oneToOne.frames >= 1 && oneToOne.loaded === oneToOne.frames,
        JSON.stringify(oneToOne),
      );
      assert.ok(oneToOne.showing.includes("snapshot"), JSON.stringify(oneToOne));
      const policy = await ev<{ type: string; csp: string }>(
        "fetch(document.querySelector('iframe.card-snapshot').src).then(r => ({ type: r.headers.get('content-type'), csp: r.headers.get('content-security-policy') }))",
      );
      assert.equal(policy.type, "text/html; charset=utf-8");
      assert.match(policy.csp, /default-src 'none'.*sandbox/);
      // The frames load the run's shared font from the viewer, allowed by policy and CORS.
      // A frame can unmount while its font is in flight; finished loads must succeed and none
      // may be blocked.
      assert.ok(
        assetLoads.some((l) => l.status === 200),
        `snapshot frames load the shared font: ${JSON.stringify(assetLoads)}`,
      );
      assert.ok(
        assetLoads.every(
          (l) =>
            (l.status === undefined || l.status === 200) &&
            (!l.failed || l.failed === "net::ERR_ABORTED"),
        ),
        JSON.stringify(assetLoads),
      );
      assert.equal(
        new Set(assetLoads.map((l) => new URL(l.url).pathname.split("/").pop())).size,
        1,
      );
      assert.equal(
        await ev<string>("document.querySelector('iframe.card-snapshot').getAttribute('sandbox')"),
        "",
      );
      // A large screen zoomed out until dozens of cards are still large: never more than 12
      // frames.
      await page.call("Emulation.setDeviceMetricsOverride", {
        width: 3200,
        height: 2000,
        deviceScaleFactor: 1,
        mobile: false,
      });
      await new Promise((r) => setTimeout(r, 500));
      for (let i = 0; i < 7; i++) {
        await key("-", "Minus");
        await new Promise((r) => setTimeout(r, 300));
      }
      await new Promise((r) => setTimeout(r, 1500));
      const many = await frames();
      const large = await ev<number>(
        "[...document.querySelectorAll('.react-flow__node')].filter(n => { const r = n.getBoundingClientRect(); return r.width >= 320 && r.right > 0 && r.left < innerWidth && r.bottom > 0 && r.top < innerHeight; }).length",
      );
      assert.ok(large > 12, `the budget is exercised: ${large} large cards in view`);
      assert.equal(many.frames, 12, JSON.stringify({ many, large }));
      await page.call("Emulation.setDeviceMetricsOverride", {
        width: vw,
        height: vh,
        deviceScaleFactor: 1,
        mobile: false,
      });
      // The inspector names what the selected card shows.
      await key("0", "Digit0");
      await new Promise((r) => setTimeout(r, 1500));
      const center = await ev<{ x: number; y: number }>(
        "(() => { const n = document.querySelector('.react-flow__node iframe.card-snapshot').closest('.react-flow__node').getBoundingClientRect(); return { x: Math.max(n.x, 300) + 40, y: Math.max(n.y, 100) + 40 }; })()",
      );
      await page.call("Input.dispatchMouseEvent", {
        type: "mousePressed",
        ...center,
        button: "left",
        clickCount: 1,
      });
      await page.call("Input.dispatchMouseEvent", {
        type: "mouseReleased",
        ...center,
        button: "left",
        clickCount: 1,
      });
      await new Promise((r) => setTimeout(r, 1500));
      const source = await ev<string | null>(
        "document.querySelector('[data-card-source]')?.textContent ?? null",
      );
      assert.match(source ?? "", /HTML snapshot|HTML 스냅샷|HTML 快照/, String(source));
      results.desktop = {
        overview,
        oneToOne,
        assetLoads: assetLoads.length,
        zoomedOut: { ...many, largeCardsInView: large },
        source,
      };
    });
  }
  console.log(JSON.stringify({ ok: true, ...results }));
} finally {
  server?.kill("SIGTERM");
  rmSync(stage, { recursive: true, force: true });
}
