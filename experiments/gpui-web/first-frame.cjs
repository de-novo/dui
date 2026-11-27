const { chromium } = require("playwright-core");
const jpeg = require("jpeg-js");
(async () => {
  const browser = await chromium.launch({
    executablePath: process.argv[2],
    headless: true,
    args: ["--enable-unsafe-webgpu", "--ignore-gpu-blocklist"],
  });
  try {
    for (let i = 0; i < 5; i++) {
      const ctx = await browser.newContext({ viewport: { width: 900, height: 600 } });
      const page = await ctx.newPage();
      const c = await ctx.newCDPSession(page);
      let trace = [];
      c.on("Tracing.dataCollected", (d) => trace.push(...d.value));
      await c.send("Tracing.start", {
        categories: "loading,blink.user_timing,disabled-by-default-devtools.screenshot",
        transferMode: "ReportEvents",
      });
      await page.goto(process.argv[3] + "/?trace=" + i);
      await page.waitForTimeout(2400);
      const done = new Promise((ok) => c.once("Tracing.tracingComplete", ok));
      await c.send("Tracing.end");
      await done;
      const nav = trace
        .filter(
          (e) =>
            e.name === "navigationStart" && e.args?.data?.documentLoaderURL?.includes("?trace="),
        )
        .at(-1);
      let hits = [];
      for (const e of trace.filter((e) => e.name === "Screenshot")) {
        const b = e.args.snapshot ?? e.args?.data?.snapshot;
        if (!b) continue;
        const { data, width, height } = jpeg.decode(Buffer.from(b, "base64"));
        let j = 4 * (Math.floor(width / 2) + width * Math.floor(height / 2));
        if (Math.abs(data[j] - 35) + Math.abs(data[j + 1] - 118) + Math.abs(data[j + 2] - 89) < 16)
          hits.push(e.ts);
      }
      if (!hits.length || !nav) throw new Error("No matching card frame or navigationStart");
      console.log(
        JSON.stringify({ run: i + 1, firstObservedFrameMs: (Math.min(...hits) - nav.ts) / 1000 }),
      );
      await ctx.close();
    }
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
