import fs from "node:fs";
import path from "node:path";
import { createCanvas, GlobalFonts, Path2D } from "@napi-rs/canvas";
import { chromium } from "../web-basics/node_modules/playwright-core/index.mjs";
import { PNG } from "../web-basics/node_modules/pngjs/lib/png.js";
const [flag, w, chrome] = process.argv.slice(2);
if (flag !== "--capture") throw Error("opt-in required");
const runs = JSON.parse(fs.readFileSync(path.join(w, "paths.json")));
const fontBytes = fs.readFileSync(
  path.join(
    w,
    "cache",
    fs.readdirSync(path.join(w, "cache")).find((n) => n.endsWith(".woff2")),
  ),
);
if (!GlobalFonts.register(fontBytes, "Oracle")) throw Error("Skia font registration failed");
const width = 660,
  height = 160,
  base = 125;
function paths(ctx, Path, run) {
  ctx.fillStyle = "black";
  for (const [d, x, y] of run.paths) {
    if (!d) continue;
    ctx.save();
    ctx.translate(4, 125);
    const scale = run.size / run.upem;
    ctx.scale(scale, -scale);
    ctx.translate(x, y);
    ctx.fill(new Path(d));
    ctx.restore();
  }
}
function diff(a, b) {
  const A = PNG.sync.read(a),
    B = PNG.sync.read(b);
  let changed = 0,
    maskXor = 0,
    ink = 0,
    abs = 0;
  for (let i = 0; i < A.data.length; i += 4) {
    const x = A.data[i],
      y = B.data[i];
    if (x === 255 && y === 255) continue;
    ink++;
    abs += Math.abs(x - y);
    if (x !== y) changed++;
    if (x < 128 !== y < 128) maskXor++;
  }
  return { ink, changed, maskXor, abs };
}
const browser = await chromium.launch({ executablePath: chrome }),
  p = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
const result = {};
try {
  await p.setContent(
    '<style>body{margin:0;background:white}#line{position:absolute;left:4px;top:0;white-space:pre}#marker{display:inline-block;width:0;height:0;vertical-align:baseline}</style><div id="line"><span id="txt"></span><span id="marker"></span></div>',
  );
  await p.evaluate(async (b) => {
    const f = new FontFace("Oracle", "url(data:font/woff2;base64," + b + ")", {
      weight: "100 900",
    });
    document.fonts.add(await f.load());
  }, fontBytes.toString("base64"));
  await p.addScriptTag({ content: "window.plot=" + paths.toString() });
  for (const [key, r] of Object.entries(runs)) {
    const chromeResult = await p.evaluate((r) => {
      const line = document.querySelector("#line"),
        text = document.querySelector("#txt"),
        marker = document.querySelector("#marker");
      line.style.font = r.weight + " " + r.size + "px Oracle";
      line.style.letterSpacing = r.spacing + "px";
      line.style.top = "0";
      text.textContent = r.text;
      line.style.top = 125 - marker.getBoundingClientRect().top + "px";
      const range = document.createRange();
      range.selectNodeContents(text);
      const c = document.createElement("canvas");
      c.width = 660;
      c.height = 160;
      const ctx = c.getContext("2d");
      ctx.fillStyle = "white";
      ctx.fillRect(0, 0, 660, 160);
      window.plot(ctx, Path2D, r);
      return {
        dom: range.getBoundingClientRect().width,
        baseline: marker.getBoundingClientRect().top,
        png: c.toDataURL().split(",")[1],
      };
    }, r);
    const dom = await p.screenshot();
    const chromeMetrics = await p.evaluate((r) => {
      const c = document.createElement("canvas").getContext("2d");
      c.font = r.weight + " " + r.size + "px Oracle";
      const m = c.measureText(r.text);
      return [
        m.actualBoundingBoxAscent,
        m.actualBoundingBoxDescent,
        m.fontBoundingBoxAscent,
        m.fontBoundingBoxDescent,
      ];
    }, r);
    const wrap = await p.evaluate(() => {
      const line = document.querySelector("#line"),
        text = document.querySelector("#txt"),
        out = [];
      for (const limit of [120, 180, 240]) {
        line.style.width = limit + "px";
        line.style.whiteSpace = "normal";
        let result = [],
          y = null;
        for (let i = 0; i < text.firstChild.length; i++) {
          const range = document.createRange();
          range.setStart(text.firstChild, i);
          range.setEnd(text.firstChild, i + 1);
          const box = range.getBoundingClientRect();
          if (!box.width) continue;
          if (box.y !== y) {
            result.push("");
            y = box.y;
          }
          result[result.length - 1] += text.textContent[i];
        }
        out.push(result.map((x) => x.trim()));
      }
      line.style.width = "";
      line.style.whiteSpace = "pre";
      return out;
    });
    fs.writeFileSync(path.join(w, "text-" + Object.keys(result).length + ".png"), dom);
    const canvas = createCanvas(width, height),
      ctx = canvas.getContext("2d");
    ctx.fillStyle = "white";
    ctx.fillRect(0, 0, width, height);
    paths(ctx, Path2D, r);
    const shared = canvas.toBuffer("image/png"),
      chromePath = Buffer.from(chromeResult.png, "base64");
    ctx.fillStyle = "white";
    ctx.fillRect(0, 0, width, height);
    ctx.fillStyle = "black";
    ctx.font = r.weight + " " + r.size + "px Oracle";
    ctx.fontVariationSettings = '"wght" ' + r.weight + ', "opsz" ' + r.opsz;
    ctx.letterSpacing = r.spacing + "px";
    ctx.fillText(r.text, 4, base);
    const met = ctx.measureText(r.text);
    const skiaMetrics = [
      met.actualBoundingBoxAscent,
      met.actualBoundingBoxDescent,
      met.fontBoundingBoxAscent,
      met.fontBoundingBoxDescent,
    ];
    result[key] = {
      chromeMetrics,
      skiaMetrics,
      wrap,
      domWidth: chromeResult.dom,
      baseline: chromeResult.baseline,
      ot: r.advance.ot,
      coretext: r.advance.coretext,
      skia: ctx.measureText(r.text).width,
      sharedPath: diff(chromePath, shared),
      nativeText: diff(dom, canvas.toBuffer("image/png")),
    };
    if (r.text === "403") {
      for (const [name, bytes] of Object.entries({
        DOM: dom,
        Skia: canvas.toBuffer("image/png"),
        ChromePath: chromePath,
        SkiaPath: shared,
      }))
        fs.writeFileSync(path.join(w, "glyph-" + name + ".png"), bytes);
    }
  }
  fs.writeFileSync(path.join(w, "raster.json"), JSON.stringify(result, null, 2));
} finally {
  await browser.close();
}
