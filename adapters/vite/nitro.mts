import { cpSync, symlinkSync, mkdirSync, existsSync } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";

// Framework builds may regenerate routes and server assets beside the app.
export async function serveNitro({
  vite,
  root,
  app,
  spec,
  manifest,
  work,
  plugins,
}: {
  vite: {
    createBuilder(config: unknown): Promise<{ buildApp(): Promise<void> }>;
  };
  root: string;
  app: string;
  manifest: string;
  work: string;
  spec: {
    workspaceLinks?: string[];
    hostEnv?: Record<string, string>;
    mockPreload?: string;
  };
  plugins(stage: string): unknown[];
}) {
  const tree = join(work, "workspace"),
    stage = join(tree, relative(root, app));
  const skip = new Set([
    "node_modules",
    ".git",
    ".output",
    ".nitro",
    ".tanstack",
    ".turbo",
    ".wrangler",
    "dist",
    "target",
  ]);
  cpSync(app, stage, {
    recursive: true,
    filter: (path) =>
      !skip.has(basename(path)) && !basename(path).startsWith(".env"),
  });
  for (let dir = app; ; dir = dirname(dir)) {
    const copy = join(tree, relative(root, dir));
    mkdirSync(copy, { recursive: true });
    for (const name of [
      "package.json",
      "node_modules",
      ...(dir === root ? (spec.workspaceLinks ?? []) : []),
    ]) {
      const dest = join(copy, name),
        from = join(dir, name);
      if (existsSync(from) && !existsSync(dest)) symlinkSync(from, dest);
    }
    if (dir === root) break;
  }
  Object.assign(process.env, spec.hostEnv ?? {});
  process.chdir(stage);
  await (
    await vite.createBuilder({
      root: stage,
      nitro: { inlineDynamicImports: true },
      cacheDir: join(work, "vite-cache"),
      plugins: plugins(stage),
      logLevel: "warn",
    })
  ).buildApp();
  if (spec.mockPreload)
    await import(
      pathToFileURL(resolve(dirname(manifest), spec.mockPreload)).href
    );
  process.env.NITRO_HOST = "127.0.0.1";
  process.env.NITRO_PORT = "0";
  await import(pathToFileURL(join(stage, ".output/server/index.mjs")).href);
  process.stdin.resume();
  process.stdin.on("end", () => process.exit(0));
}
