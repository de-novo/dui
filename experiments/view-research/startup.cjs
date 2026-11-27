// Chrome compositor filmstrips from new processes and their second fresh browser contexts.
const { chromium } = require("playwright-core"),
  jpeg = require("jpeg-js"),
  fs = require("node:fs"),
  path = require("node:path");
const [site, executablePath, out] = process.argv.slice(2);
function initMarks() {
  const wrap = (obj, k, label) => {
    if (!obj?.[k]) return;
    const fn = obj[k];
    obj[k] = function (...args) {
      performance.mark(label + "-start");
      return fn.apply(this, args).then((result) => {
        performance.mark(label + "-end");
        return result;
      });
    };
  };
  wrap(WebAssembly, "instantiateStreaming", "wasm-stream");
  wrap(globalThis.GPU?.prototype, "requestAdapter", "adapter");
  wrap(globalThis.GPUAdapter?.prototype, "requestDevice", "device");
  if (globalThis.GPUQueue) {
    const original = GPUQueue.prototype.submit;
    let first = true;
    GPUQueue.prototype.submit = function (...args) {
      if (first) {
        first = false;
        performance.mark("first-submit");
      }
      return original.apply(this, args);
    };
  }
}
function painted(event, mode) {
  const bytes = event.args.snapshot ?? event.args?.data?.snapshot;
  if (!bytes) return false;
  const { data, width, height } = jpeg.decode(Buffer.from(bytes, "base64"));
  if (mode === "gpui") {
    const i = 4 * (Math.floor(width / 2) + width * Math.floor(height / 2));
    return Math.abs(data[i] - 35) + Math.abs(data[i + 1] - 118) + Math.abs(data[i + 2] - 89) < 16;
  }
  let count = 0;
  for (let y = 0; y < Math.min(50, height); y++)
    for (let x = 0; x < Math.min(320, width); x++) {
      const i = 4 * (y * width + x);
      if (data[i] < 160 && data[i + 1] < 160 && data[i + 2] < 160) count++;
    }
  return count > 12;
}
(async () => {
  const results = [];
  for (let rep = 0; rep < 3; rep++)
    for (const width of [1440, 390])
      for (const mode of rep % 2 ? ["gpui", "dom"] : ["dom", "gpui"]) {
        const browser = await chromium.launch({
          executablePath,
          headless: true,
          args: ["--enable-unsafe-webgpu", "--ignore-gpu-blocklist"],
        });
        try {
          for (let ordinal = 1; ordinal <= 2; ordinal++) {
            const ctx = await browser.newContext({
              viewport: { width, height: width === 390 ? 844 : 920 },
            });
            const page = await ctx.newPage();
            await page.addInitScript(initMarks);
            const c = await ctx.newCDPSession(page),
              trace = [];
            c.on("Tracing.dataCollected", (d) => trace.push(...d.value));
            await c.send("Tracing.start", {
              categories: "loading,blink.user_timing,disabled-by-default-devtools.screenshot",
              transferMode: "ReportEvents",
            });
            const url = site + "/startup/" + mode + "/?sample=" + rep + "-" + ordinal;
            await page.goto(url);
            await page.waitForFunction(
              mode === "gpui"
                ? () =>
                    window.probe &&
                    document.querySelector("canvas") &&
                    performance.getEntriesByName("run-returned").length
                : () => document.body.shadowRoot?.querySelector("input"),
            );
            await page.waitForTimeout(1000);
            const image = await page.screenshot();
            if (rep === 0 && ordinal === 1)
              fs.writeFileSync(path.join(out, `startup-${width}-${mode}.png`), image);
            const done = new Promise((ok) => c.once("Tracing.tracingComplete", ok));
            await c.send("Tracing.end");
            await done;
            const nav = trace.find(
              (e) => e.name === "navigationStart" && e.args?.data?.documentLoaderURL === url,
            );
            const hit = trace
              .filter((e) => e.name === "Screenshot" && e.ts >= nav.ts && painted(e, mode))
              .sort((a, b) => a.ts - b.ts)[0];
            if (!hit) throw Error("No matching compositor screenshot for " + url);
            const marks = await page.evaluate(() =>
              Object.fromEntries(
                performance.getEntriesByType("mark").map((e) => [e.name, e.startTime]),
              ),
            );
            const resources = await page.evaluate(() =>
              performance
                .getEntriesByType("resource")
                .map((e) => [new URL(e.name).pathname, e.startTime, e.responseEnd, e.transferSize]),
            );
            results.push({
              rep,
              width,
              mode,
              ordinal,
              frameMs: (hit.ts - nav.ts) / 1000,
              marks,
              resources,
            });
            fs.writeFileSync(path.join(out, "startup.json"), JSON.stringify(results, null, 2));
            await ctx.close();
          }
        } finally {
          await browser.close();
        }
      }
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
