// Every web capture also saves a self-contained, script-free HTML snapshot taken at the same
// moment as its PNG, with its fidelity against the PNG (#310, docs/en/web-viewer.md#html-cards).
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { deflateSync } from "node:zlib";
import { createHash } from "node:crypto";

const root = resolve(import.meta.dirname, "..");
const binary = resolve(process.env.DUI_BIN ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-html-snapshots-"));

function solidPng(width: number, height: number, rgb: [number, number, number]) {
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (buf: Buffer) => {
    let c = 0xffffffff;
    for (const byte of buf) c = crcTable[(c ^ byte) & 0xff]! ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type: string, data: Buffer) => {
    const body = Buffer.concat([Buffer.from(type), data]);
    const out = Buffer.alloc(12 + data.length);
    out.writeUInt32BE(data.length, 0);
    body.copy(out, 4);
    out.writeUInt32BE(crc(body), 8 + data.length);
    return out;
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.set([8, 2, 0, 0, 0], 8);
  const row = Buffer.concat([Buffer.from([0]), Buffer.from(Array(width).fill(rgb).flat())]);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(Buffer.concat(Array(height).fill(row)))),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

type Snapshot = {
  file: string;
  bytes: number;
  fidelity: number;
  omitted: { resource: string; reason: string }[];
  assets?: string[];
};
type Report = {
  output: string;
  htmlReflow?: {
    pairs: { path: string; from: number; to: number; mismatch: number }[];
    measured: number;
    reused: number;
  };
  cells: {
    status: string;
    image: string;
    width: number;
    observation: { htmlSnapshot?: Snapshot };
  }[];
};

try {
  const site = join(stage, "site");
  mkdirSync(join(site, "fonts"), { recursive: true });
  writeFileSync(join(site, "logo.png"), solidPng(40, 40, [200, 40, 60]));
  writeFileSync(join(site, "fonts/brand.woff2"), Buffer.from("not a real font, only inlined"));
  // A large resource every screen uses is stored once per run, not inlined in each snapshot.
  const large = Buffer.from(Array.from({ length: 24 * 1024 }, (_, i) => (i * 7919) % 251));
  writeFileSync(join(site, "fonts/large.woff2"), large);
  const largeName = `asset-${createHash("sha256").update(large).digest("hex")}.woff2`;
  writeFileSync(
    join(site, "site.css"),
    `@font-face { font-family: Brand; src: url("fonts/brand.woff2") format("woff2"); }
@font-face { font-family: Large; src: url("fonts/large.woff2") format("woff2"); }
h1 { font-family: Large, sans-serif }
body { font-family: sans-serif; margin: 16px }
h1 { color: #333; border-bottom: 4px solid #c22 }
label { display: block; margin: 8px 0 }`,
  );
  writeFileSync(
    join(site, "index.html"),
    `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Snap</title>
<link rel="stylesheet" href="/site.css"></head><body>
<h1 onclick="alert(1)">Orders</h1>
<img src="/logo.png" alt="logo"> <img src="http://cdn.example.com/x.png" alt="remote" width="40" height="40">
<label>Name <input id="name"></label> <input type="checkbox" id="agree">
<x-badge></x-badge>
<canvas id="c" width="80" height="40"></canvas>
<div class="hero">Hero</div>
<a href="orders.html?tab=open#top">Open orders</a> <a href="https://example.com/help">Help</a>
<dialog id="d" style="width:300px;height:120px"><h2>Confirm</h2></dialog>
<script>
customElements.define("x-badge", class extends HTMLElement { connectedCallback() {
  const r = this.attachShadow({ mode: "open" });
  r.innerHTML = '<style>span{background:#0a0;color:white;padding:4px}</style><span>Shadow badge</span>';
} });
const sheet = document.createElement("style"); document.head.append(sheet);
sheet.sheet.insertRule(".hero{background:#246;color:#fff;padding:20px;width:200px}");
document.getElementById("name").value = "Ada";
document.getElementById("agree").checked = true;
const ctx = document.getElementById("c").getContext("2d"); ctx.fillStyle = "#f80"; ctx.fillRect(0, 0, 80, 40);
document.getElementById("d").show();
</script></body></html>`,
  );
  const manifest = join(site, "dui.json");
  const capture = (extra: Record<string, unknown>, home: string) => {
    writeFileSync(
      manifest,
      JSON.stringify({ host: "static", path: "/", widths: [800], readySelector: "h1", ...extra }),
    );
    const run = spawnSync(binary, ["run", site, manifest], {
      encoding: "utf8",
      env: { ...process.env, HOME: join(stage, home) },
    });
    assert.equal(run.status, 0, run.stdout.slice(0, 2000));
    return JSON.parse(run.stdout) as Report;
  };

  const first = capture({ widths: [800, 600], htmlReflow: true }, "home");
  const at800 = first.cells.find((c) => c.width === 800)!;
  const snap = at800.observation.htmlSnapshot!;
  assert.ok(snap?.file?.endsWith("-800.html"), JSON.stringify(at800.observation.htmlSnapshot));
  const html = readFileSync(join(first.output, snap.file), "utf8");
  assert.doesNotMatch(html, /<script|onclick=/i, "no scripts or handlers");
  assert.doesNotMatch(html, /<link[^>]+stylesheet/i, "styles are inlined, not linked");
  assert.match(html, /value="Ada"/, "input value kept");
  // The app's port changes every run; a link to it must not change the snapshot's name.
  assert.match(html, /href="\/orders\.html\?tab=open#top"/, "same-origin link without origin");
  assert.match(html, /href="https:\/\/example\.com\/help"/, "other origins kept whole");
  assert.match(html, /id="agree" checked/, "checkbox state kept");
  assert.match(
    html,
    /<template shadowrootmode="open"><style>span\{/,
    "open shadow root serialized",
  );
  assert.match(html, /\.hero \{ background: rgb\(34, 68, 102\)/, "CSSOM-inserted rule kept");
  assert.match(html, /alt="logo"[^>]*>|src="data:image\/png;base64,[^"]+" alt="logo"/);
  assert.match(
    html,
    /<img src="data:image\/png;base64,[^"]+"[^>]*id="c"/,
    "canvas became an image",
  );
  assert.match(html, /<dialog[^>]*open/, "open dialog stays open");
  assert.match(html, /src: url\("data:[^"]*base64,[^"]+"\) format\("woff2"\)/, "font inlined");
  // Both widths refer to the one shared copy of the large font.
  assert.ok(html.includes(`url("${largeName}")`), "large font shared by name");
  for (const cell of first.cells)
    assert.deepEqual(cell.observation.htmlSnapshot?.assets, [largeName], JSON.stringify(cell));
  assert.ok(readFileSync(join(first.output, largeName)).equals(large), "asset bytes kept");
  assert.ok(snap.bytes < large.length, `snapshot ${snap.bytes} bytes without the font`);
  assert.deepEqual(
    snap.omitted.map((o) => [o.resource, o.reason]),
    [["http://cdn.example.com/x.png", "cross-origin"]],
  );
  assert.ok(snap.fidelity <= 0.005, `fidelity ${snap.fidelity}`);

  // A page taller than the window with a centered column: the capture reserves the scrollbar
  // gutter, and the snapshot must lay out the same way (it did not when taken after the
  // gutter was released, shifting the column by half a scrollbar).
  writeFileSync(
    join(site, "tall.html"),
    `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Tall</title>
<style>body{margin:0;font-family:sans-serif;background:#111;color:#eee} main{max-width:720px;margin:0 auto;padding:24px;background:#3a3a5a}
section{height:600px;border-bottom:1px solid #444}</style></head><body><main><h1>Tall page</h1>
${Array.from({ length: 6 }, (_, i) => `<section><p>Section ${i} with centered text.</p></section>`).join("")}
</main></body></html>`,
  );
  const tall = capture({ path: "/tall.html" }, "home-tall");
  // The reflow comparison runs only when the manifest asks for it.
  assert.equal(tall.htmlReflow, undefined, "no reflow pass unless htmlReflow is set");
  const tallSnap = tall.cells[0]!.observation.htmlSnapshot!;
  assert.ok(tallSnap.fidelity <= 0.005, `tall page fidelity ${tallSnap.fidelity}`);

  // Each snapshot is laid out at the screen's other captured width and compared with its PNG
  // (#323): this page adapts with CSS alone, so it agrees.
  assert.deepEqual(
    first.htmlReflow?.pairs.map((p) => [p.from, p.to, p.mismatch <= 0.005]),
    [
      [600, 800, true],
      [800, 600, true],
    ],
    JSON.stringify(first.htmlReflow),
  );
  // A page whose script picks its layout by width disagrees: the snapshot has no scripts.
  writeFileSync(
    join(site, "script.html"),
    `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Script</title>
<style>body{margin:0;font-family:sans-serif} nav{padding:16px;background:#642;color:#fff} .rows div{background:#edd;margin:16px;height:80px}</style></head>
<body><nav id="nav"></nav><main class="rows" id="rows"></main><script>
const narrow = innerWidth < 700;
document.getElementById("nav").textContent = narrow ? "Menu" : "Shop · Orders · Reports";
document.getElementById("rows").innerHTML = Array.from({ length: narrow ? 2 : 6 }, (_, i) => "<div>Order " + i + "</div>").join("");
</script></body></html>`,
  );
  const scripted = capture(
    { path: "/script.html", widths: [800, 600], readySelector: "#rows div", htmlReflow: true },
    "home-script",
  );
  const scriptedPairs = scripted.htmlReflow?.pairs ?? [];
  assert.equal(scriptedPairs.length, 2, JSON.stringify(scripted.htmlReflow));
  assert.ok(
    scriptedPairs.every((p) => p.mismatch > 0.005),
    `script layout disagrees: ${JSON.stringify(scriptedPairs)}`,
  );

  // A cache hit brings the snapshot along.
  const again = capture({ widths: [800, 600], htmlReflow: true }, "home");
  const reused = again.cells.find((c) => c.width === 800)!.observation.htmlSnapshot!;
  assert.equal(reused?.file, snap.file);
  assert.ok(existsSync(join(again.output, reused.file)), "snapshot restored from the cache");
  assert.ok(existsSync(join(again.output, largeName)), "shared asset restored from the cache");
  // The same snapshots and PNGs: their reflow pairs are read back, not rendered again.
  assert.deepEqual(
    [again.htmlReflow?.measured, again.htmlReflow?.reused],
    [0, 2],
    JSON.stringify(again.htmlReflow),
  );
  assert.deepEqual(
    again.htmlReflow?.pairs.map((p) => p.mismatch),
    first.htmlReflow?.pairs.map((p) => p.mismatch),
  );

  // The manifest can turn snapshots off.
  const off = capture({ htmlSnapshots: false }, "home-off");
  assert.equal(off.cells[0]!.observation.htmlSnapshot, undefined);

  console.log(
    JSON.stringify({
      ok: true,
      bytes: snap.bytes,
      assets: snap.assets,
      fidelity: snap.fidelity,
      omitted: snap.omitted.length,
      reflow: first.htmlReflow?.pairs.map((p) => p.mismatch),
      scriptReflow: scriptedPairs.map((p) => p.mismatch),
    }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
