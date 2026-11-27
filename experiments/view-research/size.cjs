// Offline file payload comparison. Brotli sizes are not observed transfer times.
const fs = require("node:fs"),
  path = require("node:path"),
  z = require("node:zlib");
const esbuild = require("esbuild");
const [gpui, dom] = process.argv.slice(2);
for (const [name, dir] of [
  ["DOM", dom],
  ["GPUI", gpui],
  ["GPUI-minJS", gpui],
]) {
  let raw = 0,
    gzip = 0,
    brotli = 0;
  for (const f of fs.readdirSync(dir)) {
    let buf = fs.readFileSync(path.join(dir, f));
    if (name === "GPUI-minJS" && f.endsWith(".js"))
      buf = Buffer.from(
        esbuild.transformSync(buf.toString(), { format: "esm", minify: true, target: "es2022" })
          .code,
      );
    raw += buf.length;
    gzip += z.gzipSync(buf, { level: 9 }).length;
    brotli += z.brotliCompressSync(buf, {
      params: { [z.constants.BROTLI_PARAM_QUALITY]: 11 },
    }).length;
  }
  console.log(JSON.stringify({ name, raw, gzip, brotli }));
}
