// A running app needs only its loopback URL and rendered links for a screen map.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

const exec = promisify(execFile);
const binary = resolve("target/debug/dui");
const dir = await mkdtemp(join(tmpdir(), "dui-map-url-"));
const run = (...args: string[]) =>
  exec(binary, ["map", ...args, "--capture"], { cwd: dir, env: { ...process.env, HOME: dir } });

const paths = ["entry", "second", "third"].map((label) => "/" + label + "-" + randomUUID());
const pages = new Map(paths.map((path, i) => [path, i]));
let titlePrefix = "Page";
const server = createServer((req, res) => {
  const index = pages.get(req.url ?? "");
  if (index === undefined) {
    res.writeHead(404).end();
    return;
  }
  res.setHeader("Content-Type", "text/html");
  res.end(
    `<!doctype html><h1>${titlePrefix} ${index}</h1><a href="${paths[(index + 1) % paths.length]}">Next</a>`,
  );
});
// A Vite app that serves /runtime-config.js from a configureServer plugin renders only on its
// dev server. Mapping that server with --url still reads the router, including a route no
// link reaches; the built-in host renders blank and the report names the dev-server need.
async function checkViteRouterUnderUrl() {
  const app = join(dir, "router-app");
  await mkdir(join(app, "src"), { recursive: true });
  const toggle = resolve("fixtures/toggle");
  await symlink(join(toggle, "node_modules"), join(app, "node_modules"), "dir");
  await writeFile(
    join(app, "package.json"),
    JSON.stringify({
      name: "router-app",
      private: true,
      type: "module",
      dependencies: { react: "19.2.8", "react-dom": "19.2.8" },
      devDependencies: { vite: "8.3.8", typescript: "7.0.2" },
    }),
  );
  await writeFile(
    join(app, "tsconfig.json"),
    JSON.stringify({
      compilerOptions: {
        target: "ES2022",
        lib: ["ES2022", "DOM"],
        module: "ESNext",
        moduleResolution: "Bundler",
        jsx: "react-jsx",
        strict: true,
        skipLibCheck: true,
        paths: { "react-router": ["./src/mini-router.tsx"] },
      },
      include: ["src"],
    }),
  );
  await writeFile(
    join(app, "vite.config.ts"),
    `import { fileURLToPath } from "node:url";
export default {
  resolve: { alias: { "react-router": fileURLToPath(new URL("./src/mini-router.tsx", import.meta.url)) } },
  plugins: [{
    name: "runtime-config",
    configureServer(server: { middlewares: { use(path: string, fn: (req: unknown, res: { setHeader(k: string, v: string): void; end(s: string): void }) => void): void } }) {
      server.middlewares.use("/runtime-config.js", (_req, res) => {
        res.setHeader("content-type", "text/javascript");
        res.end("window.__CONFIG = { title: 'OMS' };");
      });
    },
  }],
};
`,
  );
  await writeFile(
    join(app, "index.html"),
    '<!doctype html><meta charset="utf-8"><div id="root"></div><script src="/runtime-config.js"></script><script type="module" src="/src/main.tsx"></script>',
  );
  await writeFile(
    join(app, "src/mini-router.tsx"),
    `import type { ReactNode } from "react";
type Route = { path: string; element: ReactNode };
export function createBrowserRouter(routes: Route[]) { return routes; }
export function RouterProvider({ router }: { router: Route[] }) {
  return <>{router.find((r) => r.path === location.pathname)?.element ?? <h1>Not found</h1>}</>;
}
`,
  );
  await writeFile(
    join(app, "src/main.tsx"),
    `import { createRoot } from "react-dom/client";
import { createBrowserRouter, RouterProvider } from "react-router";
const config = (window as { __CONFIG?: { title: string } }).__CONFIG;
if (!config) throw new Error("Invalid public runtime configuration");
const router = createBrowserRouter([
  { path: "/", element: <main><h1>{config.title} home</h1><a href="/orders">Orders</a></main> },
  { path: "/orders", element: <main><h1>{config.title} orders</h1><a href="/">Home</a></main> },
  { path: "/settings/billing", element: <main><h1>{config.title} billing</h1></main> },
]);
createRoot(document.getElementById("root")!).render(<RouterProvider router={router} />);
`,
  );
  await writeFile(join(app, "dui.json"), JSON.stringify({ widths: [412], readySelector: "h1" }));
  type Mapped = {
    screenDiscovery: {
      mode: string;
      sources: Record<string, { kind: string }>;
      devServer?: { configureServer: string[] };
      devServerHint?: string;
    };
    cells: { path: string; status: string; observation: { text?: string } }[];
  };
  const mapped = async (...args: string[]) => {
    const out = await exec(binary, ["map", app, ...args, "--capture"], {
      cwd: dir,
      env: { ...process.env, HOME: dir },
    }).catch((e: { stdout: string }) => e);
    return JSON.parse(out.stdout) as Mapped;
  };
  const builtIn = await mapped();
  assert.deepEqual(builtIn.screenDiscovery.devServer?.configureServer, ["runtime-config"]);
  assert.match(
    builtIn.screenDiscovery.devServerHint ?? "",
    /configureServer middleware \(runtime-config\)/,
  );
  assert.ok(
    builtIn.cells.every((c) => c.status !== "verified"),
    "the built-in host renders blank",
  );

  const vite = await import(
    pathToFileURL(join(toggle, "node_modules/vite/dist/node/index.js")).href
  );
  const devServer = await vite.createServer({
    root: app,
    configFile: join(app, "vite.config.ts"),
    logLevel: "silent",
    server: { host: "127.0.0.1", port: 0, strictPort: false },
  });
  await devServer.listen();
  try {
    const port = (devServer.httpServer.address() as { port: number }).port;
    const live = await mapped("--url", `http://127.0.0.1:${port}/`);
    assert.equal(live.screenDiscovery.mode, "router+links");
    assert.equal(live.screenDiscovery.devServerHint, undefined);
    const paths = [...new Set(live.cells.map((c) => c.path))].sort();
    assert.deepEqual(paths, ["/", "/orders", "/settings/billing"]);
    for (const cell of live.cells) {
      assert.equal(cell.status, "verified", `${cell.path}: ${JSON.stringify(cell)}`.slice(0, 400));
    }
    assert.match(
      live.cells.find((c) => c.path === "/settings/billing")?.observation.text ?? "",
      /OMS billing/,
    );
    return { routes: paths.length, mode: live.screenDiscovery.mode };
  } finally {
    await devServer.close();
  }
}

try {
  await new Promise<void>((ready) => server.listen(0, "127.0.0.1", ready));
  const address = server.address();
  assert.ok(address && typeof address === "object");
  await writeFile(
    join(dir, "dui.json"),
    JSON.stringify({
      host: "vite",
      surface: "vite",
      path: "/old-entry",
      widths: [412],
      readySelector: "h1",
    }),
  );
  const url = `http://localhost:${address.port}${paths[0]}`;
  const result = JSON.parse((await run("--url", url)).stdout) as {
    output: string;
    screenDiscovery: { sources: Record<string, { kind: string; from?: string }> };
    cells: {
      path: string;
      width: number;
      status: string;
      image?: string;
      observation: {
        renderTree: { tag: string; text: string }[];
      };
    }[];
  };
  assert.deepEqual(result.cells.map((c) => c.path).sort(), [...paths].sort());
  assert.equal(result.screenDiscovery.sources[paths[0]!]!.kind, "entry");
  assert.equal(result.screenDiscovery.sources[paths[2]!]!.from, paths[1]);
  for (const cell of result.cells) {
    assert.equal(cell.width, 412);
    assert.equal(cell.status, "verified");
    assert.ok(cell.observation.renderTree.some((n) => n.text === "Page " + pages.get(cell.path)));
    assert.ok(cell.image);
    const png = await readFile(join(result.output, cell.image));
    assert.ok(png.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])));
  }
  titlePrefix = "Revised page";
  const updated = JSON.parse((await run(dir, "--url", url)).stdout) as typeof result;
  assert.equal(updated.cells.length, result.cells.length);
  for (const cell of updated.cells) {
    assert.ok(
      cell.observation.renderTree.some((n) => n.text === "Revised page " + pages.get(cell.path)),
    );
  }
  await assert.rejects(run(dir, "--url", "http://example.com/start"), { code: 2 });
  const router = await checkViteRouterUnderUrl();
  console.log(JSON.stringify({ pages: paths.length, width: result.cells[0]?.width, router }));
} finally {
  await new Promise<void>((done) => server.close(() => done()));
  await rm(dir, { recursive: true, force: true });
}
