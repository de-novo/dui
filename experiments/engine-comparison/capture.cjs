const { chromium } = require("playwright-core"),
  fs = require("node:fs"),
  path = require("node:path");
(async () => {
  const [chrome, out] = process.argv.slice(2),
    data = require("./evidence/engines.json"),
    b = await chromium.launch({ executablePath: chrome }),
    refs = {};
  try {
    for (const w of [1440, 390]) {
      const p = await b.newPage({ viewport: { width: w, height: w === 390 ? 844 : 920 } });
      refs[w] = {};
      for (const [k, v] of Object.entries(data.layout)) {
        await p.setContent(
          "<style>body{margin:0}#box{background:#196d5e;" + v.css + '}</style><div id="box"></div>',
        );
        refs[w][k] = await p.locator("#box").evaluate((e) => {
          const { x, y, width, height } = e.getBoundingClientRect();
          return [x, y, width, height];
        });
        await p.screenshot({
          path: path.join(out, w + "-" + k + ".png"),
          clip: { x: 0, y: 0, width: 110, height: 48 },
        });
      }
      await p.setContent(
        '<style>body{margin:0}#home {padding:12px} .missing + .card {padding:8px}</style><div class="card" id="home">Example</div>',
      );
      refs[w].padding = await p
        .locator("#home")
        .evaluate((e) =>
          ["Top", "Right", "Bottom", "Left"].map((k) =>
            parseFloat(getComputedStyle(e)["padding" + k]),
          ),
        );
      await p.screenshot({
        path: path.join(out, w + "-padding.png"),
        clip: { x: 0, y: 0, width: 110, height: 48 },
      });
      await p.close();
    }
  } finally {
    await b.close();
  }
  fs.writeFileSync(path.join(__dirname, "evidence/chrome.json"), JSON.stringify(refs, null, 2));
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
