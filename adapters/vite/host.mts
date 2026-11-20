// Build and serve an instrumented Vite SPA in temporary storage.
import {
  readFileSync,
  writeFileSync,
  statSync,
  createReadStream,
} from "node:fs";
import { resolve, join, sep, extname } from "node:path";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import { createServer } from "node:http";
import { serveNitro } from "./nitro.mjs";
import { instrument, type InstrumentFact } from "./instrument.mjs";

const [root, manifest, work] = process.argv.slice(2);
const spec = JSON.parse(readFileSync(manifest, "utf8"));
const app = resolve(root, spec.appRoot ?? ".");
const require = createRequire(join(app, "package.json"));
if (require("vite/package.json").version.split(".")[0] !== "8")
  throw Error("Vite 8 required");
const vite = await import(pathToFileURL(require.resolve("vite")).href);
const facts: InstrumentFact[] = JSON.parse(
  readFileSync(join(work, "facts.json"), "utf8"),
).facts;
const byFile = new Map<string, InstrumentFact[]>();
for (const f of facts) {
  const path = resolve(root, f.file);
  if (!byFile.has(path)) byFile.set(path, []);
  byFile.get(path)!.push(f);
}
if (app !== root && !app.startsWith(root + sep))
  throw Error("appRoot must be inside project");
function plugins(stage: string) {
  return [
    {
      name: "dui-state-probes",
      enforce: "pre",
      load(id: string) {
        if (id.includes("?") || id.startsWith("\0")) return;
        const original = id.startsWith(stage + sep)
          ? resolve(app, id.slice(stage.length + 1))
          : id;
        const facts = byFile.get(original);
        if (facts) return instrument(readFileSync(id, "utf8"), facts, vite);
        if (id.endsWith(".css") && spec.offlineFonts)
          return readFileSync(id, "utf8").replace(
            /^@import url\("https:[^\n]+$/gm,
            "",
          );
      },
    },
  ];
}
if (spec.host === "nitro") {
  await serveNitro({ vite, root, app, spec, manifest, work, plugins });
} else {
  if (spec.host && spec.host !== "vite")
    throw Error("unsupported host: " + spec.host);
  const dist = join(work, "dist");
  let base = "/";
  await vite.build({
    root: app,
    cacheDir: join(work, "vite-cache"),
    build: { outDir: dist, emptyOutDir: true },
    plugins: [
      ...plugins(app),
      {
        name: "dui-base",
        configResolved(config: { base: string }) {
          base = new URL(config.base, "http://localhost/").pathname;
        },
      },
    ],
  });
  const types: Record<string, string> = {
    ".css": "text/css",
    ".js": "text/javascript",
    ".json": "application/json",
    ".html": "text/html",
    ".wasm": "application/wasm",
    ".svg": "image/svg+xml",
    ".png": "image/png",
    ".woff2": "font/woff2",
  };
  const server = createServer((req, res) => {
    try {
      const path = decodeURIComponent(
        new URL(req.url ?? "/", "http://localhost").pathname,
      );
      if (!path.startsWith(base)) {
        res.writeHead(404).end();
        return;
      }
      let file = resolve(dist, "./" + path.slice(base.length));
      if (file !== dist && !file.startsWith(dist + sep)) {
        res.writeHead(400).end();
        return;
      }
      if (
        !statSync(file, { throwIfNoEntry: false })?.isFile() &&
        req.headers.accept?.includes("text/html")
      )
        file = join(dist, "index.html");
      if (!statSync(file, { throwIfNoEntry: false })?.isFile()) {
        res.writeHead(404).end();
        return;
      }
      res.setHeader(
        "content-type",
        types[extname(file)] ?? "application/octet-stream",
      );
      createReadStream(file)
        .on("error", () => res.destroy())
        .pipe(res);
    } catch {
      res.writeHead(400).end();
    }
  });
  server.listen(0, "127.0.0.1", () => {
    const address = server.address();
    if (!address || typeof address === "string")
      throw Error("missing TCP port");
    writeFileSync(join(work, "port"), String(address.port));
  });
  process.stdin.resume();
  process.stdin.on("end", () => server.close(() => process.exit(0)));
}
