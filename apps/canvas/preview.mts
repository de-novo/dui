// Local development entry point for exercising the map against a finished run.
import { createReadStream } from "node:fs";
import { readFile, realpath } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { createServer } from "vite";

const run = process.argv[2];
if (!run)
  throw new Error("Usage: npm run preview:archive -- <dui run directory>");
const root = await realpath(run);
const graph = JSON.parse(await readFile(join(root, "graph.json"), "utf8"));
const images = new Set<string>();
for (const cell of graph.overview) {
  if (
    typeof cell.image === "string" &&
    basename(cell.image) === cell.image &&
    cell.image.endsWith(".png")
  )
    images.add(cell.image);
}
const server = await createServer({
  root: import.meta.dirname,
  configFile: false,
  server: {
    host: "127.0.0.1",
    port: 0,
    fs: { strict: true, allow: [import.meta.dirname] },
  },
  plugins: [
    {
      name: "local-dui-archive",
      configureServer(vite) {
        vite.middlewares.use("/archive", async (req, res) => {
          const name = (req.url ?? "").slice(1);
          res.setHeader("Cache-Control", "no-store");
          if (name === "graph.json") {
            res.setHeader("Content-Type", "application/json");
            res.end(JSON.stringify(graph));
            return;
          }
          if (images.has(name)) {
            const image = await realpath(join(root, name)).catch(
              () => undefined,
            );
            if (image && dirname(image) === root) {
              res.setHeader("Content-Type", "image/png");
              createReadStream(image)
                .on("error", () => res.destroy())
                .pipe(res);
              return;
            }
          }
          res.statusCode = 404;
          res.end();
        });
      },
    },
  ],
});
await server.listen();
server.printUrls();
