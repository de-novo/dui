// Local development entry point for the archive and live viewer.
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream, existsSync } from "node:fs";
import { access, mkdtemp, open, readFile, readdir, realpath, rm } from "node:fs/promises";
import type { IncomingMessage } from "node:http";
import { homedir, tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { promisify } from "node:util";
import { createServer } from "vite";
import type { Run } from "./src/data/run";

const execFileAsync = promisify(execFile);

type ArchiveEntry = {
  root: string;
  projectPath?: string;
  run: Run;
};

const cliPaths = process.argv.slice(2);
const extraSavedPaths = new Set<string>();
const activeSessions = new Set<string>();
const diffOverlayDirs = new Map<string, string>();

function resolveDuiBinary(): string {
  if (process.env.DUI_BIN) return process.env.DUI_BIN;
  const repoRoot = resolve(import.meta.dirname, "../..");
  for (const candidate of [
    join(repoRoot, "target/debug/dui"),
    join(repoRoot, "target/release/dui"),
  ]) {
    if (existsSync(candidate)) return candidate;
  }
  return "dui";
}

async function directories(root: string) {
  const entries = await readdir(root, { withFileTypes: true }).catch((error) => {
    if (error.code === "ENOENT") return [];
    throw error;
  });
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

async function resolveProjectPath(
  meta: Record<string, unknown> | undefined,
): Promise<string | undefined> {
  if (!meta) return undefined;
  const base =
    typeof meta.worktree === "string" && meta.worktree.length > 0
      ? meta.worktree
      : typeof meta.projectRoot === "string" && meta.projectRoot.length > 0
        ? meta.projectRoot
        : undefined;
  if (!base) return undefined;
  const app = typeof meta.app === "string" && meta.app.length > 0 ? meta.app : ".";
  const full = resolve(base, app);
  const exists = await access(full).then(
    () => true,
    () => false,
  );
  if (exists) return full;
  if (typeof meta.projectRoot === "string" && meta.projectRoot !== base) {
    const fallback = resolve(meta.projectRoot, app);
    const ok = await access(fallback).then(
      () => true,
      () => false,
    );
    if (ok) return fallback;
  }
  return undefined;
}

async function listArchives(): Promise<ArchiveEntry[]> {
  const paths = [...cliPaths, ...extraSavedPaths];
  if (!cliPaths.length) {
    for (const project of await directories(join(homedir(), ".dui", "projects"))) {
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
  const resolved = await Promise.all(paths.map((path) => realpath(path).catch(() => undefined)));
  const unique = [...new Set(resolved.filter((p): p is string => typeof p === "string"))];
  return Promise.all(
    unique.map(async (root) => {
      const meta = await optionalJson(root, "snapshot.json");
      const key = createHash("sha256").update(root).digest("hex");
      const projectDir = dirname(dirname(root));
      const localProject =
        dirname(root) === join(homedir(), ".dui", "projects", basename(projectDir), "runs") &&
        basename(projectDir) === meta?.projectId;
      const record = localProject ? await optionalJson(projectDir, "project.json") : undefined;
      const projectPath = await resolveProjectPath(meta);
      const run: Run = {
        key,
        projectId: meta?.projectId ?? key,
        project: record?.name ?? meta?.projectName ?? basename(meta?.projectRoot ?? root),
        branch: meta?.branch ?? null,
        version: meta?.id ?? basename(root),
        liveAvailable: Boolean(projectPath),
      };
      return { root, projectPath, run };
    }),
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
  locale?: string;
  observation?: {
    htmlSnapshot?: { file?: unknown; fidelity?: unknown; omitted?: unknown; assets?: unknown };
    renderTree?: unknown;
  };
};

const assetName = /^asset-[0-9a-f]{64}\.(woff2|woff|ttf|otf|png|jpg|gif|webp|avif|svg)$/;
const assetTypes: Record<string, string> = {
  woff2: "font/woff2",
  woff: "font/woff",
  ttf: "font/ttf",
  otf: "font/otf",
  png: "image/png",
  jpg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  avif: "image/avif",
  svg: "image/svg+xml",
};

/** Width and height from a PNG header, as `dui open` reads them (crates/dui/src/viewer.rs). */
async function pngSize(path: string) {
  const file = await open(path).catch(() => undefined);
  if (!file) return undefined;
  try {
    const header = Buffer.alloc(24);
    const { bytesRead } = await file.read(header, 0, 24, 0);
    if (bytesRead < 24 || header.toString("latin1", 12, 16) !== "IHDR") return undefined;
    return { imageWidth: header.readUInt32BE(16), imageHeight: header.readUInt32BE(20) };
  } finally {
    await file.close();
  }
}

async function openArchive(root: string) {
  const graph = JSON.parse(await readFile(join(root, "graph.json"), "utf8"));
  const report = await optionalJson(root, "report.json");
  const captures: Capture[] = report?.cells ?? graph.overview;
  const snapshotName = /^[0-9a-f]{64}-\d+\.html$/;
  // Fonts and images snapshots of the run share (crates/dui/src/snapshot_assets.rs).
  const assets = new Set<string>();
  for (const capture of captures) {
    const listed = capture.observation?.htmlSnapshot?.assets;
    if (Array.isArray(listed))
      for (const name of listed)
        if (typeof name === "string" && assetName.test(name)) assets.add(name);
  }
  // How each snapshot lays out at the screen's other captured widths, as `dui open` sends it.
  const reflow = new Map<string, { width: number; mismatch: number }[]>();
  for (const pair of report?.htmlReflow?.pairs ?? []) {
    if (typeof pair.to !== "number" || typeof pair.mismatch !== "number") continue;
    const key = JSON.stringify([pair.path, pair.state, pair.scenario, pair.locale, pair.from]);
    reflow.set(key, [...(reflow.get(key) ?? []), { width: pair.to, mismatch: pair.mismatch }]);
  }
  const cells = captures.map(
    ({ path, width, image, status, reason, state, scenario, locale, observation }) => {
      const snapshot = observation?.htmlSnapshot;
      const pairs = reflow.get(JSON.stringify([path, state, scenario, locale, width]));
      return {
        path,
        width,
        image,
        status,
        reason,
        state,
        scenario,
        // Element rectangles, as `dui open` sends them (crates/dui/src/viewer.rs).
        ...(Array.isArray(observation?.renderTree) ? { renderTree: observation.renderTree } : {}),
        ...(typeof snapshot?.file === "string" && snapshotName.test(snapshot.file)
          ? {
              snapshot: {
                file: snapshot.file,
                fidelity: snapshot.fidelity,
                omitted: Array.isArray(snapshot.omitted) ? snapshot.omitted.length : 0,
                ...(pairs ? { reflow: pairs } : {}),
              },
            }
          : {}),
      };
    },
  );
  const images = new Set<string>(assets);
  const hasSourceModel = existsSync(join(root, "source-model.json"));
  if (hasSourceModel) {
    graph.sourceModel = "source-model.json";
    images.add("source-model.json");
  }
  for (const { image } of [...cells, ...graph.overview]) {
    if (typeof image === "string" && basename(image) === image && image.endsWith(".png"))
      images.add(image);
  }
  const sizes = new Map(
    await Promise.all(
      [...images]
        .filter((name) => name.endsWith(".png"))
        .map(async (name) => [name, await pngSize(join(root, name))] as const),
    ),
  );
  const sized = <T extends { image?: string }>(item: T) => {
    const size = item.image ? sizes.get(item.image) : undefined;
    return size ? { ...item, ...size } : item;
  };
  for (const cell of cells) if (cell.snapshot) images.add(cell.snapshot.file);
  // Overview entries are the map cards; they get the snapshot of the capture with their image.
  const byImage = new Map(cells.filter((c) => c.snapshot).map((c) => [c.image, c.snapshot]));
  const overview = graph.overview.map((item: { image?: string }) =>
    sized(
      item.image && byImage.has(item.image) ? { ...item, snapshot: byImage.get(item.image) } : item,
    ),
  );
  return { graph: JSON.stringify({ ...graph, overview, cells: cells.map(sized) }), images };
}

function readJsonBody(req: IncomingMessage): Promise<Record<string, unknown>> {
  return new Promise((resolveBody, rejectBody) => {
    const chunks: Buffer[] = [];
    let size = 0;
    req.on("data", (chunk: Buffer) => {
      size += chunk.length;
      if (size > 65536) {
        rejectBody(Error("request-too-large"));
        req.destroy();
        return;
      }
      chunks.push(chunk);
    });
    req.on("end", () => {
      try {
        const parsed = JSON.parse(Buffer.concat(chunks).toString("utf8"));
        if (typeof parsed !== "object" || parsed === null) rejectBody(Error("invalid-json"));
        else resolveBody(parsed as Record<string, unknown>);
      } catch {
        rejectBody(Error("invalid-json"));
      }
    });
    req.on("error", rejectBody);
  });
}

function isSameOriginLoopback(req: IncomingMessage): boolean {
  const origin = req.headers.origin;
  if (!origin) return true;
  const host = req.headers.host;
  if (!host) return false;
  return origin === `http://${host}`;
}

async function runLiveCommand(
  payload: Record<string, unknown>,
): Promise<{ ok: boolean; status: number; body: Record<string, unknown> }> {
  const binary = resolveDuiBinary();
  try {
    const { stdout } = await execFileAsync(binary, ["live", JSON.stringify(payload)], {
      maxBuffer: 16 * 1024 * 1024,
      timeout: 180000,
    });
    return { ok: true, status: 200, body: JSON.parse(stdout) };
  } catch (error) {
    const failed = error as { stdout?: string; code?: number | string; message?: string };
    if (typeof failed.stdout === "string" && failed.stdout.trim()) {
      try {
        const parsed = JSON.parse(failed.stdout) as Record<string, unknown>;
        const code = typeof failed.code === "number" ? failed.code : 3;
        return { ok: false, status: code === 2 ? 400 : 500, body: parsed };
      } catch {
        // Fall through to generic error below.
      }
    }
    return {
      ok: false,
      status: 500,
      body: { status: "error", reason: failed.message ?? "live-command-failed" },
    };
  }
}

function validSessionId(id: unknown): id is string {
  return typeof id === "string" && /^[0-9a-f]{64}$/i.test(id);
}

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
            let name = decodeURIComponent(url.pathname.slice(1));
            // `/archive/r/<run>/<name>`, where a snapshot's relative asset names resolve.
            let pathKey: string | null = null;
            const scoped = url.pathname.match(/^\/r\/([^/]+)\/(.+)$/);
            if (scoped) {
              pathKey = decodeURIComponent(scoped[1]!);
              name = decodeURIComponent(scoped[2]!);
            }
            res.setHeader("Cache-Control", "no-store");
            const archives = await listArchives();
            if (name === "runs.json") {
              res.setHeader("Content-Type", "application/json");
              res.end(JSON.stringify(archives.map((a) => a.run)));
              return;
            }
            const key = pathKey ?? url.searchParams.get("run");
            const archive = key === null ? archives[0] : archives.find((a) => a.run.key === key);
            if (archive) {
              const { root } = archive;
              if (!cache.has(root)) cache.set(root, openArchive(root));
              const data = await cache.get(root)!;
              if (name === "graph.json") {
                res.setHeader("Content-Type", "application/json");
                res.end(data.graph);
                return;
              }
              if (name === "source-model.json" && data.images.has(name)) {
                res.setHeader("Content-Type", "application/json; charset=utf-8");
                createReadStream(join(root, name))
                  .on("error", () => res.destroy())
                  .pipe(res);
                return;
              }
              if (data.images.has(name)) {
                const image = await realpath(join(root, name)).catch(() => undefined);
                if (image && dirname(image) === root) {
                  if (name.endsWith(".html")) {
                    // Same policy and base as `dui open` (crates/dui/src/viewer.rs,
                    // snapshot_policy and with_asset_base).
                    const host = `http://${req.headers.host}/archive/`;
                    res.setHeader("Content-Type", "text/html; charset=utf-8");
                    res.setHeader(
                      "Content-Security-Policy",
                      `default-src 'none'; style-src 'unsafe-inline'; img-src data: ${host}; font-src data: ${host}; sandbox`,
                    );
                    res.setHeader("X-Content-Type-Options", "nosniff");
                    const html = await readFile(image, "utf8");
                    const base = `<base href="/archive/r/${encodeURIComponent(archive.run.key)}/">`;
                    const head = html.search(/<head>/i);
                    res.end(
                      head < 0
                        ? base + html
                        : html.slice(0, head + 6) + base + html.slice(head + 6),
                    );
                    return;
                  }
                  if (assetName.test(name)) {
                    res.setHeader("Content-Type", assetTypes[name.split(".").pop()!]!);
                    res.setHeader("Access-Control-Allow-Origin", "*");
                    res.setHeader("X-Content-Type-Options", "nosniff");
                  } else res.setHeader("Content-Type", "image/png");
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

        vite.middlewares.use("/api/diff", async (req, res, next) => {
          try {
            res.setHeader("Cache-Control", "no-store");
            if (!isSameOriginLoopback(req)) {
              res.statusCode = 403;
              res.setHeader("Content-Type", "application/json");
              res.end(JSON.stringify({ status: "error", reason: "forbidden-origin" }));
              return;
            }
            const url = new URL(req.url ?? "/", "http://localhost");
            const beforeKey = url.searchParams.get("before");
            const afterKey = url.searchParams.get("after");
            if (!beforeKey || !afterKey) {
              res.statusCode = 400;
              res.setHeader("Content-Type", "application/json");
              res.end(JSON.stringify({ status: "error", reason: "missing-before-or-after" }));
              return;
            }
            const pairKey = `${beforeKey}:${afterKey}`;
            if (req.method === "GET" && url.pathname === "/overlay") {
              const file = url.searchParams.get("file");
              const dir = diffOverlayDirs.get(pairKey);
              if (!dir || !file || !/^[0-9a-f]{16}-diff\.png$/i.test(file)) {
                res.statusCode = 404;
                res.end();
                return;
              }
              const target = join(dir, file);
              const resolved = await realpath(target).catch(() => undefined);
              const resolvedDir = await realpath(dir).catch(() => undefined);
              if (!resolved || !resolvedDir || dirname(resolved) !== resolvedDir) {
                res.statusCode = 404;
                res.end();
                return;
              }
              res.setHeader("Content-Type", "image/png");
              createReadStream(resolved)
                .on("error", () => res.destroy())
                .pipe(res);
              return;
            }
            if (req.method !== "GET" || (url.pathname !== "/" && url.pathname !== "")) {
              res.statusCode = 405;
              res.end();
              return;
            }
            const archives = await listArchives();
            const beforeArchive = archives.find((a) => a.run.key === beforeKey);
            const afterArchive = archives.find((a) => a.run.key === afterKey);
            if (!beforeArchive || !afterArchive) {
              res.statusCode = 404;
              res.setHeader("Content-Type", "application/json");
              res.end(JSON.stringify({ status: "error", reason: "run-not-found" }));
              return;
            }
            let overlayDir = diffOverlayDirs.get(pairKey);
            if (!overlayDir) {
              overlayDir = await mkdtemp(join(tmpdir(), "dui-diff-overlay-"));
              diffOverlayDirs.set(pairKey, overlayDir);
            }
            const binary = resolveDuiBinary();
            const { stdout } = await execFileAsync(
              binary,
              ["diff", beforeArchive.root, afterArchive.root, "--overlay-dir", overlayDir],
              { maxBuffer: 16 * 1024 * 1024, timeout: 60000 },
            );
            const parsed = JSON.parse(stdout) as Record<string, unknown>;
            res.statusCode = 200;
            res.setHeader("Content-Type", "application/json");
            res.end(
              JSON.stringify({
                beforeRunKey: beforeKey,
                afterRunKey: afterKey,
                summary: parsed.summary,
                screens: parsed.screens,
              }),
            );
          } catch (error) {
            next(error);
          }
        });

        vite.middlewares.use("/api/audit", async (req, res, next) => {
          try {
            res.setHeader("Cache-Control", "no-store");
            if (!isSameOriginLoopback(req)) {
              res.statusCode = 403;
              res.setHeader("Content-Type", "application/json");
              res.end(JSON.stringify({ status: "error", reason: "forbidden-origin" }));
              return;
            }
            if (req.method !== "GET") {
              res.statusCode = 405;
              res.end();
              return;
            }
            const url = new URL(req.url ?? "/", "http://localhost");
            const runKey = url.searchParams.get("run");
            const goal = url.searchParams.get("goal");
            const archives = await listArchives();
            const found = runKey ? archives.find((a) => a.run.key === runKey) : archives[0];
            if (!found) {
              res.statusCode = 404;
              res.setHeader("Content-Type", "application/json");
              res.end(JSON.stringify({ status: "error", reason: "run-not-found" }));
              return;
            }
            const args = ["audit", found.root];
            if (goal) args.push("--goal", goal);
            const { stdout } = await execFileAsync(resolveDuiBinary(), args);
            const parsed = JSON.parse(stdout) as Record<string, unknown>;
            delete parsed.run;
            parsed.runKey = found.run.key;
            res.statusCode = 200;
            res.setHeader("Content-Type", "application/json");
            res.end(JSON.stringify(parsed));
          } catch (error) {
            next(error);
          }
        });

        vite.middlewares.use("/api/comments", async (req, res, next) => {
          try {
            res.setHeader("Cache-Control", "no-store");
            if (!isSameOriginLoopback(req)) {
              res.statusCode = 403;
              res.setHeader("Content-Type", "application/json");
              res.end(JSON.stringify({ status: "error", reason: "forbidden-origin" }));
              return;
            }
            const url = new URL(req.url ?? "/", "http://localhost");
            const bin = resolveDuiBinary();
            if (req.method === "GET") {
              const filter: Record<string, unknown> = {};
              const pid = url.searchParams.get("projectId");
              const runKey = url.searchParams.get("run");
              const status = url.searchParams.get("status");
              const screenPath = url.searchParams.get("path");
              const widthRaw = url.searchParams.get("width");
              if (pid) filter.projectId = pid;
              if (runKey) {
                const archives = await listArchives();
                const found = archives.find((a) => a.run.key === runKey);
                if (found) filter.run = found.root;
              }
              if (status) filter.status = status;
              if (screenPath) filter.path = screenPath;
              if (widthRaw && Number(widthRaw) > 0) filter.width = Number(widthRaw);
              const { stdout } = await execFileAsync(bin, [
                "comment",
                "list",
                JSON.stringify(filter),
              ]);
              res.statusCode = 200;
              res.setHeader("Content-Type", "application/json");
              res.end(stdout);
              return;
            }
            if (req.method !== "POST") {
              res.statusCode = 405;
              res.end();
              return;
            }
            const body = await readJsonBody(req);
            const op = body.op;
            if (op === "add") {
              const payload: Record<string, unknown> = { ...body };
              if (typeof body.run === "string" && body.run.length > 0) {
                const archives = await listArchives();
                const found = archives.find((a) => a.run.key === body.run);
                if (!found) {
                  res.statusCode = 404;
                  res.setHeader("Content-Type", "application/json");
                  res.end(JSON.stringify({ status: "error", reason: "run-not-found" }));
                  return;
                }
                payload.run = found.root;
              }
              const { stdout } = await execFileAsync(bin, [
                "comment",
                "add",
                JSON.stringify(payload),
              ]);
              res.statusCode = 200;
              res.setHeader("Content-Type", "application/json");
              res.end(stdout);
              return;
            }
            if ((op === "resolve" || op === "reopen") && typeof body.id === "string") {
              const { stdout } = await execFileAsync(bin, ["comment", op, body.id]);
              res.statusCode = 200;
              res.setHeader("Content-Type", "application/json");
              res.end(stdout);
              return;
            }
            res.statusCode = 400;
            res.setHeader("Content-Type", "application/json");
            res.end(JSON.stringify({ status: "error", reason: "unknown-operation" }));
          } catch (error) {
            next(error);
          }
        });

        vite.middlewares.use("/api/live", async (req, res, next) => {
          try {
            res.setHeader("Cache-Control", "no-store");
            if (!isSameOriginLoopback(req)) {
              res.statusCode = 403;
              res.setHeader("Content-Type", "application/json");
              res.end(JSON.stringify({ status: "error", reason: "forbidden-origin" }));
              return;
            }
            const url = new URL(req.url ?? "/", "http://localhost");
            if (req.method === "GET" && url.pathname === "/frame") {
              const session = url.searchParams.get("session");
              const state = url.searchParams.get("state");
              const width = Number(url.searchParams.get("width"));
              if (!validSessionId(session) || !state || !Number.isInteger(width) || width <= 0) {
                res.statusCode = 400;
                res.setHeader("Content-Type", "application/json");
                res.end(JSON.stringify({ status: "error", reason: "invalid-frame-query" }));
                return;
              }
              const reply = await runLiveCommand({
                op: "frame",
                session,
                cell: { state, width },
              });
              if (!reply.ok || typeof reply.body.imageBase64 !== "string") {
                res.statusCode = reply.status;
                res.setHeader("Content-Type", "application/json");
                res.end(JSON.stringify(reply.body));
                return;
              }
              const png = Buffer.from(reply.body.imageBase64, "base64");
              res.setHeader("Content-Type", "image/png");
              res.end(png);
              return;
            }
            if (req.method !== "POST" || (url.pathname !== "/" && url.pathname !== "")) {
              res.statusCode = 405;
              res.end();
              return;
            }
            let body: Record<string, unknown>;
            try {
              body = await readJsonBody(req);
            } catch (error) {
              res.statusCode = 400;
              res.setHeader("Content-Type", "application/json");
              res.end(JSON.stringify({ status: "error", reason: (error as Error).message }));
              return;
            }
            const op = body.op;
            if (op === "open") {
              const archives = await listArchives();
              const archive = archives.find((a) => a.run.key === body.run);
              if (!archive) {
                res.statusCode = 404;
                res.setHeader("Content-Type", "application/json");
                res.end(JSON.stringify({ status: "error", reason: "run-not-found" }));
                return;
              }
              if (!archive.projectPath) {
                res.statusCode = 400;
                res.setHeader("Content-Type", "application/json");
                res.end(JSON.stringify({ status: "error", reason: "project-unavailable" }));
                return;
              }
              const reply = await runLiveCommand({
                op: "open",
                project: archive.projectPath,
                run: archive.root,
                cell: body.cell,
                requestId: body.requestId,
              });
              if (reply.ok && typeof reply.body.session === "string") {
                activeSessions.add(reply.body.session);
              }
              res.statusCode = reply.status;
              res.setHeader("Content-Type", "application/json");
              res.end(JSON.stringify(reply.body));
              return;
            }
            if (
              op === "inspect" ||
              op === "set" ||
              op === "reset" ||
              op === "save" ||
              op === "close"
            ) {
              if (!validSessionId(body.session)) {
                res.statusCode = 400;
                res.setHeader("Content-Type", "application/json");
                res.end(JSON.stringify({ status: "error", reason: "invalid-session" }));
                return;
              }
              const forwarded: Record<string, unknown> = { op, session: body.session };
              if (op !== "close") forwarded.cell = body.cell;
              if (op === "set" || op === "reset" || op === "save") {
                forwarded.expectedRevision = body.expectedRevision;
                forwarded.requestId = body.requestId;
              }
              if (op === "set" || op === "reset") {
                forwarded.slot = body.slot;
                forwarded.instance = body.instance;
                if (op === "set") forwarded.value = body.value;
                if (body.expect) forwarded.expect = body.expect;
              }
              const reply = await runLiveCommand(forwarded);
              if (op === "close" && reply.ok) {
                activeSessions.delete(body.session);
              }
              if (op === "save" && reply.ok && typeof reply.body.output === "string") {
                const savedRoot = await realpath(reply.body.output).catch(
                  () => reply.body.output as string,
                );
                extraSavedPaths.add(savedRoot);
                const runKey = createHash("sha256").update(savedRoot).digest("hex");
                const { output: _output, ...rest } = reply.body;
                res.statusCode = 200;
                res.setHeader("Content-Type", "application/json");
                res.end(JSON.stringify({ ...rest, runKey }));
                return;
              }
              res.statusCode = reply.status;
              res.setHeader("Content-Type", "application/json");
              res.end(JSON.stringify(reply.body));
              return;
            }
            res.statusCode = 400;
            res.setHeader("Content-Type", "application/json");
            res.end(JSON.stringify({ status: "error", reason: "unknown-operation" }));
          } catch (error) {
            next(error);
          }
        });
      },
    },
  ],
});

let closing = false;
async function cleanup() {
  if (closing) return;
  closing = true;
  await Promise.allSettled([
    ...[...activeSessions].map((session) => runLiveCommand({ op: "close", session })),
    ...[...diffOverlayDirs.values()].map((dir) => rm(dir, { recursive: true, force: true })),
  ]);
  await server.close();
  process.exit(0);
}
process.on("SIGINT", () => void cleanup());
process.on("SIGTERM", () => void cleanup());

await server.listen();
server.printUrls();
