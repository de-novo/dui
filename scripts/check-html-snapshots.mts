// Every web capture also saves a self-contained, script-free HTML snapshot taken at the same
// moment as its PNG, with its fidelity against the PNG (#310, docs/en/web-viewer.md#html-cards).
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { deflateSync } from "node:zlib";

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
};
type Report = {
  output: string;
  cells: { status: string; image: string; observation: { htmlSnapshot?: Snapshot } }[];
};

try {
  const site = join(stage, "site");
  mkdirSync(join(site, "fonts"), { recursive: true });
  writeFileSync(join(site, "logo.png"), solidPng(40, 40, [200, 40, 60]));
  writeFileSync(join(site, "fonts/brand.woff2"), Buffer.from("not a real font, only inlined"));
  writeFileSync(
    join(site, "site.css"),
    `@font-face { font-family: Brand; src: url("fonts/brand.woff2") format("woff2"); }
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

  const first = capture({}, "home");
  const snap = first.cells[0]!.observation.htmlSnapshot!;
  assert.ok(
    snap?.file?.endsWith("-800.html"),
    JSON.stringify(first.cells[0]!.observation.htmlSnapshot),
  );
  const html = readFileSync(join(first.output, snap.file), "utf8");
  assert.doesNotMatch(html, /<script|onclick=/i, "no scripts or handlers");
  assert.doesNotMatch(html, /<link[^>]+stylesheet/i, "styles are inlined, not linked");
  assert.match(html, /value="Ada"/, "input value kept");
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
  assert.deepEqual(
    snap.omitted.map((o) => [o.resource, o.reason]),
    [["http://cdn.example.com/x.png", "cross-origin"]],
  );
  assert.ok(snap.fidelity <= 0.005, `fidelity ${snap.fidelity}`);

  // A cache hit brings the snapshot along.
  const again = capture({}, "home");
  const reused = again.cells[0]!.observation.htmlSnapshot!;
  assert.equal(reused?.file, snap.file);
  assert.ok(existsSync(join(again.output, reused.file)), "snapshot restored from the cache");

  // The manifest can turn snapshots off.
  const off = capture({ htmlSnapshots: false }, "home-off");
  assert.equal(off.cells[0]!.observation.htmlSnapshot, undefined);

  console.log(
    JSON.stringify({
      ok: true,
      bytes: snap.bytes,
      fidelity: snap.fidelity,
      omitted: snap.omitted.length,
    }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
