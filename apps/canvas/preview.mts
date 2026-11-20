// Local development entry point for the archive viewer.
import { createReadStream } from "node:fs";
import { readFile, readdir, realpath, access } from "node:fs/promises";
import { createHash } from "node:crypto";
import { homedir } from "node:os";
import { basename, dirname, join } from "node:path";
import { createServer } from "vite";
import type { Run } from "./src/data/run";

async function directories(root: string) {
  const entries = await readdir(root, { withFileTypes: true }).catch(
    (error) => {
      if (error.code === "ENOENT") return [];
      throw error;
    },
  );
  return entries
    .filter((e) => e.isDirectory() && !e.name.startsWith("."))
    .map((e) => join(root, e.name));
}

async function optionalJson(root: string, name: string) {
  try {
    return JSON.parse(await readFile(join(root, name), "utf8"));
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw error;
  }
}

async function listArchives() {
  const paths = process.argv.slice(2);
  if (!paths.length) {
    for (const project of await directories(
      join(homedir(), ".dui", "projects"),
    )) {
      for (const run of await directories(join(project, "runs"))) {
        if (
          await access(join(run, "graph.json")).then(
            () => true,
            () => false,
          )
        )
          paths.push(run);
      }
    }
    paths.sort((a, b) => basename(b).localeCompare(basename(a)));
  }
  return Promise.all(
    [...new Set(await Promise.all(paths.map((path) => realpath(path))))].map(
      async (root) => {
        const meta = await optionalJson(root, "snapshot.json");
        const key = createHash("sha256").update(root).digest("hex");
        const run: Run = {
          key,
          projectId: meta?.projectId ?? key,
          project: basename(meta?.projectRoot ?? root),
          branch: meta?.branch ?? null,
          version: meta?.id ?? basename(root),
        };
        return { root, run };
      },
    ),
  );
}

type Capture = {
  path: string;
  width: number;
  image?: string;
  status: string;
  reason?: string;
  state?: string;
  scenario?: string;
};
async function openArchive(root: string) {
  const graph = JSON.parse(await readFile(join(root, "graph.json"), "utf8"));
  const report = await optionalJson(root, "report.json");
  const captures: Capture[] = report?.cells ?? graph.overview;
  const cells = captures.map(
    ({ path, width, image, status, reason, state, scenario }) => ({
      path,
      width,
      image,
      status,
      reason,
      state,
      scenario,
    }),
  );
  const images = new Set<string>();
  for (const { image } of [...cells, ...graph.overview]) {
    if (
      typeof image === "string" &&
      basename(image) === image &&
      image.endsWith(".png")
    )
      images.add(image);
  }
  return { graph: JSON.stringify({ ...graph, cells }), images };
}
const archives = await listArchives();
const cache = new Map<string, ReturnType<typeof openArchive>>();
const server = await createServer({
  root: import.meta.dirname,
  configFile: join(import.meta.dirname, "vite.config.ts"),
  server: {
    host: "127.0.0.1",
    port: 0,
    fs: { strict: true, allow: [import.meta.dirname] },
  },
  plugins: [
    {
      name: "local-dui-archive",
      configureServer(vite) {
        vite.middlewares.use("/archive", async (req, res, next) => {
          try {
            const url = new URL(req.url ?? "/", "http://localhost");
            const name = decodeURIComponent(url.pathname.slice(1));
            res.setHeader("Cache-Control", "no-store");
            if (name === "runs.json") {
              res.setHeader("Content-Type", "application/json");
              res.end(JSON.stringify(archives.map((a) => a.run)));
              return;
            }
            const key = url.searchParams.get("run");
            const archive =
              key === null
                ? archives[0]
                : archives.find((a) => a.run.key === key);
            if (archive) {
              const { root } = archive;
              if (!cache.has(root)) cache.set(root, openArchive(root));
              const data = await cache.get(root)!;
              if (name === "graph.json") {
                res.setHeader("Content-Type", "application/json");
                res.end(data.graph);
                return;
              }
              if (data.images.has(name)) {
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
            }
            res.statusCode = 404;
            res.end();
          } catch (error) {
            next(error);
          }
        });
      },
    },
  ],
});
await server.listen();
server.printUrls();
