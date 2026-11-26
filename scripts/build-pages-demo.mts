/**
 * Builds the static interactive multi-project `dui` 2D canvas demo under `site/demo/`
 * so GitHub Pages (`https://de-novo.github.io/dui/demo/`) serves the exact `apps/canvas`
 * viewer pre-populated with 6 verified MIT-licensed trending open-source projects + `dui.sh`.
 *
 * Usage:
 *   npm run build:pages-demo
 *   OSS_CACHE_DIR=/tmp/oss-showcase npm run build:pages-demo
 */
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";

const root = process.cwd();
const duiBin = process.env.DUI_BIN || path.join(root, "target", "release", "dui");
const fallbackBin = path.join(root, "target", "debug", "dui");
const binary = fs.existsSync(duiBin) ? duiBin : fallbackBin;
if (!fs.existsSync(binary)) {
  throw new Error(`dui binary not found at ${duiBin} or ${fallbackBin}; build cargo first`);
}

function pngDimensions(filePath: string): { width: number; height: number } | null {
  try {
    const fd = fs.openSync(filePath, "r");
    const buf = Buffer.alloc(24);
    const bytesRead = fs.readSync(fd, buf, 0, 24, 0);
    fs.closeSync(fd);
    if (bytesRead < 24) return null;
    const sig = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    if (!sig.every((b, idx) => buf[idx] === b)) return null;
    const width = buf.readUInt32BE(16);
    const height = buf.readUInt32BE(20);
    return width > 0 && height > 0 ? { width, height } : null;
  } catch {
    return null;
  }
}

function enrichGraph(runDir: string): Record<string, unknown> {
  const graph = JSON.parse(fs.readFileSync(path.join(runDir, "graph.json"), "utf8")) as Record<
    string,
    unknown
  >;
  const reportPath = path.join(runDir, "report.json");
  if (!fs.existsSync(reportPath)) return graph;
  const report = JSON.parse(fs.readFileSync(reportPath, "utf8")) as Record<string, unknown>;
  const rawCells = Array.isArray(report.cells) ? report.cells : [];
  const enrichedCells = rawCells.map((rawCell) => {
    const c = rawCell as Record<string, unknown>;
    const img = typeof c.image === "string" ? c.image : null;
    const dims = img ? pngDimensions(path.join(runDir, img)) : null;
    const obs =
      c.observation && typeof c.observation === "object"
        ? (c.observation as Record<string, unknown>)
        : {};
    const entry: Record<string, unknown> = {
      path: c.path,
      scenario: c.scenario,
      state: c.state,
      width: c.width,
      planIndex: c.planIndex,
      status: c.status,
      reason: c.reason,
      image: img,
      locale: c.locale,
      overlay: c.overlay,
    };
    if (dims) {
      entry.imageWidth = dims.width;
      entry.imageHeight = dims.height;
    }
    if (obs.screenEvidence) entry.screenEvidence = obs.screenEvidence;
    if (typeof obs.redirectedTo === "string") entry.redirectedTo = obs.redirectedTo;
    if (Array.isArray(obs.renderTree)) entry.renderTree = obs.renderTree;
    if (obs.localeAudit && typeof obs.localeAudit === "object") {
      entry.localeAudit = obs.localeAudit;
    }
    return entry;
  });
  graph.cells = enrichedCells;
  if (report.screenDiscovery) graph.screenDiscovery = report.screenDiscovery;
  if (report.locales) graph.locales = report.locales;
  return graph;
}

async function waitForHttp(url: string, timeoutMs = 90_000): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const ok = await new Promise<boolean>((resolve) => {
      const req = http.get(url, (res) => {
        res.resume();
        resolve((res.statusCode ?? 500) < 500);
      });
      req.on("error", () => resolve(false));
      req.setTimeout(1500, () => {
        req.destroy();
        resolve(false);
      });
    });
    if (ok) return;
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(`Timed out waiting for ${url}`);
}

interface OssShowcaseSpec {
  key: string;
  projectId: string;
  project: string;
  repo: string;
  commit: string;
  branch: string;
  version: string;
  license: string;
  kind: "vite-direct" | "static-build" | "url-server";
  installCmd: [string, string[]];
  buildCmd?: [string, string[]];
  serverCmd?: (port: number) => [string, string[]];
  staticSubdir?: string;
  maxRoutes?: number;
  startPath?: string;
}

const showcaseProjects: OssShowcaseSpec[] = [
  {
    key: "shadcn-admin",
    projectId: "oss-shadcn-admin",
    project: "satnaing/shadcn-admin",
    repo: "https://github.com/satnaing/shadcn-admin.git",
    commit: "e16c87f",
    branch: "MIT · React 19 + Vite 8",
    version: "★ 16.9k · TanStack Router",
    license: "MIT",
    kind: "url-server",
    installCmd: ["pnpm", ["install", "--frozen-lockfile", "--ignore-scripts"]],
    buildCmd: ["pnpm", ["exec", "vite", "build"]],
    serverCmd: (port) => [
      "pnpm",
      ["exec", "vite", "preview", "--host", "127.0.0.1", "--port", String(port), "--strictPort"],
    ],
    maxRoutes: 12,
  },
  {
    key: "astrowind",
    projectId: "oss-astrowind",
    project: "onwidget/astrowind",
    repo: "https://github.com/onwidget/astrowind.git",
    commit: "14e1a69",
    branch: "MIT · Astro 7 + Tailwind v4",
    version: "★ 6.1k · Landing Kit",
    license: "MIT",
    kind: "static-build",
    installCmd: ["npm", ["ci", "--ignore-scripts"]],
    buildCmd: ["npm", ["run", "build"]],
    staticSubdir: "dist",
    maxRoutes: 10,
  },
  {
    key: "astro-paper",
    projectId: "oss-astro-paper",
    project: "satnaing/astro-paper",
    repo: "https://github.com/satnaing/astro-paper.git",
    commit: "217c4e3",
    branch: "MIT · Astro 7 + Pagefind",
    version: "★ 5.2k · Editorial Blog",
    license: "MIT",
    kind: "static-build",
    installCmd: ["pnpm", ["install", "--frozen-lockfile", "--ignore-scripts"]],
    buildCmd: ["pnpm", ["run", "build"]],
    staticSubdir: "dist",
    maxRoutes: 10,
  },
  {
    key: "next-shadcn-admin",
    projectId: "oss-next-shadcn-admin",
    project: "arhamkhnz/next-shadcn-admin-dashboard",
    repo: "https://github.com/arhamkhnz/next-shadcn-admin-dashboard.git",
    commit: "9a84df6",
    branch: "MIT · Next.js 16 + Base UI",
    version: "★ 3.3k · App Router",
    license: "MIT",
    kind: "url-server",
    installCmd: ["npm", ["ci", "--ignore-scripts"]],
    serverCmd: (port) => ["npx", ["next", "dev", "-H", "127.0.0.1", "-p", String(port)]],
    startPath: "/dashboard/default",
    maxRoutes: 10,
  },
  {
    key: "shadcn-store",
    projectId: "oss-shadcn-store",
    project: "shadcnstore/shadcn-dashboard-landing-template",
    repo: "https://github.com/shadcnstore/shadcn-dashboard-landing-template.git",
    commit: "1482344",
    branch: "MIT · Next.js 16 + Base UI",
    version: "★ 1.3k · Landing + Admin",
    license: "MIT",
    kind: "url-server",
    installCmd: ["pnpm", ["install", "--frozen-lockfile", "--ignore-scripts"]],
    serverCmd: (port) => ["pnpm", ["exec", "next", "dev", "-H", "127.0.0.1", "-p", String(port)]],
    startPath: "/",
    maxRoutes: 10,
  },
  {
    key: "shadcn-vue-admin",
    projectId: "oss-shadcn-vue-admin",
    project: "Whbbit1999/shadcn-vue-admin",
    repo: "https://github.com/Whbbit1999/shadcn-vue-admin.git",
    commit: "8979c8e",
    branch: "MIT · Vue 3.5 + Vite 8",
    version: "★ 501 · shadcn-vue",
    license: "MIT",
    kind: "url-server",
    installCmd: ["pnpm", ["install", "--frozen-lockfile", "--ignore-scripts"]],
    buildCmd: ["pnpm", ["exec", "vite", "build"]],
    serverCmd: (port) => [
      "pnpm",
      ["exec", "vite", "preview", "--host", "127.0.0.1", "--port", String(port), "--strictPort"],
    ],
    maxRoutes: 10,
  },
];

function ensureRepo(spec: OssShowcaseSpec, cacheRoot: string): string {
  const dir = path.join(cacheRoot, path.basename(spec.repo, ".git"));
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(cacheRoot, { recursive: true });
    execFileSync("git", ["clone", "--depth", "1", spec.repo, dir], { stdio: "inherit" });
  }
  if (!fs.existsSync(path.join(dir, "node_modules"))) {
    execFileSync(spec.installCmd[0], spec.installCmd[1], { cwd: dir, stdio: "inherit" });
  }
  // Verify MIT license exists in upstream repo before capturing
  const licenseFile = fs.readdirSync(dir).find((name) => /^licen[sc]e(\.|$)/i.test(name));
  if (!licenseFile) {
    throw new Error(`Missing LICENSE file in ${spec.project}`);
  }
  const licenseText = fs.readFileSync(path.join(dir, licenseFile), "utf8");
  if (!/\bMIT License\b/i.test(licenseText)) {
    throw new Error(`Refusing non-MIT showcase repo ${spec.project}`);
  }
  return dir;
}

/**
 * Attach the committed iOS simulator and Android emulator captures of the source preview
 * fixture. Fails when a fixture changed after its capture, so a stale image is never shown.
 */
function attachNativeRecordings(sourceRun: string, env: NodeJS.ProcessEnv, tmpHome: string) {
  const recordings = path.join(root, "fixtures/source-preview-native/recordings");
  const sha256 = (file: string) => createHash("sha256").update(fs.readFileSync(file)).digest("hex");
  const graph = JSON.parse(fs.readFileSync(path.join(sourceRun, "graph.json"), "utf8")) as {
    nodes: Array<{ id: string; label: string; nativeSource?: { source: string } }>;
  };
  const snapshot = JSON.parse(fs.readFileSync(path.join(sourceRun, "snapshot.json"), "utf8"));
  const manifestDir = fs.mkdtempSync(path.join(tmpHome, "native-recordings-"));
  const captures = ["ios", "android"].map((runtime) => {
    const record = JSON.parse(fs.readFileSync(path.join(recordings, runtime + ".json"), "utf8"));
    const image = path.join(recordings, record.image);
    if (sha256(path.join(root, record.fixture)) !== record.sourceSha256)
      throw new Error(
        `${record.fixture} changed after its ${runtime} capture. Run the "Source preview native captures" workflow and commit its ${runtime}.png and ${runtime}.json.`,
      );
    if (sha256(path.join(root, record.host)) !== record.hostSha256)
      throw new Error(`${record.host} changed after its ${runtime} capture.`);
    if (sha256(image) !== record.imageSha256)
      throw new Error(`${record.image} does not match its recorded SHA-256.`);
    const node = graph.nodes.find((n) => n.nativeSource?.source === record.source);
    if (!node) throw new Error(`No source card for ${record.source}`);
    fs.copyFileSync(image, path.join(manifestDir, record.image));
    return {
      screenId: node.id,
      runtime,
      observedPath: node.label,
      width: record.width,
      image: record.image,
    };
  });
  const manifest = path.join(manifestDir, "recordings.json");
  fs.writeFileSync(manifest, JSON.stringify({ sourceSnapshot: snapshot.id, captures }));
  const result = spawnSync(binary, ["map", "attach", sourceRun, manifest], {
    encoding: "utf8",
    env,
  });
  if (result.status !== 0)
    throw new Error(`dui map attach failed: ${result.stderr || result.stdout}`);
  const run = JSON.parse(result.stdout) as { output: string; cells: Array<{ status: string }> };
  if (run.cells.length !== 2 || run.cells.some((cell) => cell.status !== "unverified"))
    throw new Error("Native recordings must stay unverified: " + result.stdout);
  return run.output;
}

function exportRunToArchive(
  runDir: string,
  dest: string,
  homeDir: string,
  licenseSourceDir?: string,
): { cells: number; verified: number } {
  fs.mkdirSync(dest, { recursive: true });
  const enriched = enrichGraph(runDir);
  fs.writeFileSync(path.join(dest, "graph.json"), JSON.stringify(enriched));

  const auditRes = spawnSync(binary, ["audit", runDir], {
    encoding: "utf8",
    env: { ...process.env, HOME: homeDir },
    maxBuffer: 16 * 1024 * 1024,
  });
  if (auditRes.stdout.trim()) {
    fs.writeFileSync(path.join(dest, "audit.json"), auditRes.stdout);
  }

  for (const entry of fs.readdirSync(runDir)) {
    if (entry.endsWith(".png")) {
      fs.copyFileSync(path.join(runDir, entry), path.join(dest, entry));
    }
  }

  if (licenseSourceDir) {
    const lic = fs.readdirSync(licenseSourceDir).find((name) => /^licen[sc]e(\.|$)/i.test(name));
    if (lic) {
      fs.copyFileSync(path.join(licenseSourceDir, lic), path.join(dest, "LICENSE"));
    }
  }

  const cells = Array.isArray(enriched.cells) ? enriched.cells : [];
  const verified = cells.filter(
    (c) => c && typeof c === "object" && (c as Record<string, unknown>).status === "verified",
  ).length;
  const populated = cells.filter(
    (c) =>
      c &&
      typeof c === "object" &&
      Array.isArray((c as Record<string, unknown>).renderTree) &&
      ((c as Record<string, unknown>).renderTree as unknown[]).length >= 10,
  ).length;
  const nodes = Array.isArray(enriched.nodes) ? enriched.nodes : [];
  const readyPreviews = nodes.filter((n) => {
    if (!n || typeof n !== "object") return false;
    const preview = (n as Record<string, unknown>).sourcePreview;
    if (!preview || typeof preview !== "object") return false;
    const scene = (preview as Record<string, unknown>).scene;
    return (
      (preview as Record<string, unknown>).status === "ready" &&
      scene &&
      typeof scene === "object" &&
      Array.isArray((scene as Record<string, unknown>).frames) &&
      ((scene as Record<string, unknown>).frames as unknown[]).length >= 2
    );
  }).length;
  // A source map from a native discovery adapter is evidence on its own: declarations with sources.
  const nativeDeclarations = nodes.filter(
    (n) => n && typeof n === "object" && (n as Record<string, unknown>).nativeSource,
  ).length;
  if (populated < 2 && readyPreviews < 2 && nativeDeclarations < 2) {
    throw new Error(
      `Refusing blank capture for ${path.basename(dest)}: only ${populated}/${cells.length} cells had >=10 rendered DOM nodes and ${readyPreviews} ready source previews`,
    );
  }
  return { cells: cells.length, verified };
}

function runCommandAsync(
  cmd: string,
  args: string[],
  opts: { env: NodeJS.ProcessEnv; timeoutMs: number },
): Promise<{ status: number; stdout: string; stderr: string }> {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { env: opts.env, stdio: ["ignore", "pipe", "pipe"] });
    const outChunks: Buffer[] = [];
    const errChunks: Buffer[] = [];
    child.stdout.on("data", (d: Buffer) => outChunks.push(d));
    child.stderr.on("data", (d: Buffer) => errChunks.push(d));
    const timer = setTimeout(() => {
      child.kill("SIGTERM");
      reject(new Error(`Timed out after ${opts.timeoutMs}ms: ${cmd} ${args.join(" ")}`));
    }, opts.timeoutMs);
    child.on("error", (err) => {
      clearTimeout(timer);
      reject(err);
    });
    child.on("close", (code) => {
      clearTimeout(timer);
      resolve({
        status: code ?? 1,
        stdout: Buffer.concat(outChunks).toString("utf8"),
        stderr: Buffer.concat(errChunks).toString("utf8"),
      });
    });
  });
}

async function captureOssProject(
  spec: OssShowcaseSpec,
  repoDir: string,
  tmpHome: string,
  port: number,
): Promise<string> {
  if (spec.kind === "static-build") {
    const distDir = path.join(repoDir, spec.staticSubdir || "dist");
    if (!fs.existsSync(distDir) && spec.buildCmd) {
      execFileSync(spec.buildCmd[0], spec.buildCmd[1], { cwd: repoDir, stdio: "inherit" });
    }
    const manifestPath = path.join(distDir, "dui.json");
    const manifest = {
      host: "static",
      path: spec.startPath || "/",
      widths: [375, 1280],
      routeSelector: "a[href]",
      discoverOverlays: true,
      maxRoutes: spec.maxRoutes ?? 10,
    };
    fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
    try {
      const res = await runCommandAsync(binary, ["map", distDir, "--capture"], {
        env: { ...process.env, HOME: tmpHome },
        timeoutMs: 180_000,
      });
      const parsed = JSON.parse(res.stdout) as { output?: string };
      if (!parsed.output) {
        throw new Error(`Missing output for ${spec.key}: ${res.stderr || res.stdout}`);
      }
      return parsed.output;
    } finally {
      fs.rmSync(manifestPath, { force: true });
    }
  }

  if (!spec.serverCmd) {
    throw new Error(`Missing serverCmd for ${spec.key}`);
  }
  if (spec.buildCmd && !fs.existsSync(path.join(repoDir, spec.staticSubdir || "dist"))) {
    execFileSync(spec.buildCmd[0], spec.buildCmd[1], { cwd: repoDir, stdio: "inherit" });
  }
  const [cmd, cmdArgs] = spec.serverCmd(port);
  const child = spawn(cmd, cmdArgs, {
    cwd: repoDir,
    stdio: "ignore",
    detached: true,
  });
  const manifestPath = path.join(repoDir, "dui.json");
  try {
    const baseUrl = `http://127.0.0.1:${port}`;
    await waitForHttp(baseUrl);
    const manifest = {
      host: "url",
      url: baseUrl,
      path: spec.startPath || "/",
      widths: [375, 1280],
      routeSelector: "a[href]",
      discoverOverlays: true,
      maxRoutes: spec.maxRoutes ?? 10,
    };
    fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
    const res = await runCommandAsync(binary, ["map", repoDir, "--capture"], {
      env: { ...process.env, HOME: tmpHome },
      timeoutMs: 240_000,
    });
    const parsed = JSON.parse(res.stdout) as { output?: string };
    if (!parsed.output) {
      throw new Error(`Missing output for ${spec.key}: ${res.stderr || res.stdout}`);
    }
    return parsed.output;
  } finally {
    fs.rmSync(manifestPath, { force: true });
    if (child.pid) {
      try {
        process.kill(-child.pid, "SIGTERM");
      } catch {
        // Process already exited.
      }
    }
  }
}

async function main() {
  fs.copyFileSync(path.join(root, "scripts/install"), path.join(root, "site/install"));
  const demoDir = path.join(root, "site", "demo");
  const archiveDir = path.join(demoDir, "archive");
  fs.rmSync(demoDir, { recursive: true, force: true });
  fs.mkdirSync(archiveDir, { recursive: true });

  // 1. Build apps/canvas with relative base (`./`) into site/demo/
  execFileSync("npm", ["run", "build", "--prefix", path.join(root, "apps", "canvas")], {
    stdio: "inherit",
    env: { ...process.env, DUI_CANVAS_BASE: "./" },
  });
  fs.cpSync(path.join(root, "apps", "canvas", "dist"), demoDir, { recursive: true });

  // Restore standard `/` canvas dist so local `cargo build` embeds normal `/` paths.
  execFileSync("npm", ["run", "build", "--prefix", path.join(root, "apps", "canvas")], {
    stdio: "inherit",
    env: { ...process.env, DUI_CANVAS_BASE: "/" },
  });

  // Inject static archive meta tag into site/demo/index.html
  const demoHtmlPath = path.join(demoDir, "index.html");
  let demoHtml = fs.readFileSync(demoHtmlPath, "utf8");
  demoHtml = demoHtml.replace(
    "</head>",
    '  <meta name="dui-static-archive" content="./archive" />\n  </head>',
  );
  fs.writeFileSync(demoHtmlPath, demoHtml);

  const tmpHome = fs.mkdtempSync(path.join(os.tmpdir(), "dui-pages-demo-"));
  const ossCacheRoot = process.env.OSS_CACHE_DIR || path.join(tmpHome, "oss-repos");
  const filterRaw = process.env.SHOWCASE_PROJECTS?.trim();
  const filterSet = filterRaw ? new Set(filterRaw.split(",").map((s) => s.trim())) : null;
  const runs: Array<Record<string, unknown>> = [];

  try {
    const selected = showcaseProjects.filter((spec) => !filterSet || filterSet.has(spec.key));
    const prepared = selected.map((spec, idx) => {
      console.log(`[pages-demo] preparing ${spec.project} (${spec.license})...`);
      const repoDir = ensureRepo(spec, ossCacheRoot);
      const subHome = path.join(tmpHome, `home-${spec.key}`);
      fs.mkdirSync(subHome, { recursive: true });
      return { spec, repoDir, subHome, port: 4310 + idx };
    });

    const batchSize = 3;
    for (let i = 0; i < prepared.length; i += batchSize) {
      const batch = prepared.slice(i, i + batchSize);
      const results = await Promise.all(
        batch.map(async ({ spec, repoDir, subHome, port }) => {
          const runDir = await captureOssProject(spec, repoDir, subHome, port);
          const stats = exportRunToArchive(
            runDir,
            path.join(archiveDir, spec.key),
            subHome,
            repoDir,
          );
          console.log(
            `[pages-demo] exported ${spec.key}: ${stats.cells} cells (${stats.verified} verified)`,
          );
          return {
            key: spec.key,
            projectId: spec.projectId,
            project: spec.project,
            branch: spec.branch,
            version: spec.version,
            diffWith: [],
            liveAvailable: false,
          };
        }),
      );
      runs.push(...results);
    }

    // Also include dui.sh itself (self-dogfooded) PLUS a baseline run + precomputed
    // Render-Tree & Pixel Diff so visitors can test 2D Canvas Inline Diff right on GitHub Pages.
    if (!filterSet || filterSet.has("dui-site")) {
      console.log("[pages-demo] capturing dui.sh official site (current + baseline diff)...");
      const currentSiteDir = path.join(tmpHome, "site-current");
      fs.cpSync(path.join(root, "site"), currentSiteDir, {
        recursive: true,
        filter: (src) => !src.endsWith(path.join("site", "demo")),
      });
      const siteHome = path.join(tmpHome, "home-dui-site");
      fs.mkdirSync(siteHome, { recursive: true });
      const res = await runCommandAsync(binary, ["map", currentSiteDir, "--capture"], {
        env: { ...process.env, HOME: siteHome },
        timeoutMs: 120_000,
      });
      const parsed = JSON.parse(res.stdout) as { output?: string };
      if (!parsed.output) {
        throw new Error(`Failed capturing dui.sh site: ${res.stderr || res.stdout}`);
      }
      const currentRunDir = parsed.output;
      const stats = exportRunToArchive(
        currentRunDir,
        path.join(archiveDir, "dui-site"),
        siteHome,
        root,
      );
      console.log(
        `[pages-demo] exported dui-site: ${stats.cells} cells (${stats.verified} verified)`,
      );

      // Build a baseline variant of site/ (pre-showcase hero + without the 6-OSS grid)
      // so visitors can interactively toggle Before / After / Render / Pixel Overlay diff.
      const baselineSiteDir = path.join(tmpHome, "site-baseline");
      fs.cpSync(path.join(root, "site"), baselineSiteDir, {
        recursive: true,
        filter: (src) => !src.endsWith(path.join("site", "demo")),
      });
      const baseIndexPath = path.join(baselineSiteDir, "index.html");
      let baseHtml = fs.readFileSync(baseIndexPath, "utf8");
      baseHtml = baseHtml.replace(
        /<section class="section" id="live-oss-showcase">[\s\S]*?<\/section>/,
        "",
      );
      baseHtml = baseHtml.replace(
        '<h1 data-i18n="heroTitle">Make the code the single source of truth for development and UI.</h1>',
        "<h1>Deterministic UI review from your real compiler.</h1>",
      );
      fs.writeFileSync(baseIndexPath, baseHtml);

      const baseHome = path.join(tmpHome, "home-dui-site-baseline");
      fs.mkdirSync(baseHome, { recursive: true });
      const baseRes = await runCommandAsync(binary, ["map", baselineSiteDir, "--capture"], {
        env: { ...process.env, HOME: baseHome },
        timeoutMs: 120_000,
      });
      const baseParsed = JSON.parse(baseRes.stdout) as { output?: string };
      if (baseParsed.output) {
        const baseStats = exportRunToArchive(
          baseParsed.output,
          path.join(archiveDir, "dui-site-baseline"),
          baseHome,
          root,
        );
        console.log(
          `[pages-demo] exported dui-site-baseline: ${baseStats.cells} cells (${baseStats.verified} verified)`,
        );

        const diffsDir = path.join(archiveDir, "diffs");
        fs.mkdirSync(diffsDir, { recursive: true });
        const diffRes = await runCommandAsync(
          binary,
          ["diff", baseParsed.output, currentRunDir, "--overlay-dir", diffsDir],
          { env: { ...process.env, HOME: siteHome }, timeoutMs: 60_000 },
        );
        if (diffRes.stdout.trim()) {
          const diffJson = JSON.parse(diffRes.stdout) as Record<string, unknown>;
          diffJson.beforeRunKey = "dui-site-baseline";
          diffJson.afterRunKey = "dui-site";
          const normalizeOverlay = (entry: unknown) => {
            if (!entry || typeof entry !== "object") return;
            const obj = entry as Record<string, unknown>;
            const pd = obj.pixelDiff as Record<string, unknown> | undefined;
            if (pd && typeof pd.overlayImage === "string") {
              pd.overlayImage = path.basename(pd.overlayImage);
            }
          };
          for (const key of ["changed", "added", "removed", "unchanged"]) {
            const arr = diffJson[key];
            if (Array.isArray(arr)) arr.forEach(normalizeOverlay);
          }
          if (Array.isArray(diffJson.screens)) {
            for (const screen of diffJson.screens) {
              if (screen && typeof screen === "object") {
                const screenCells = (screen as Record<string, unknown>).cells;
                if (Array.isArray(screenCells)) screenCells.forEach(normalizeOverlay);
              }
            }
          }
          fs.writeFileSync(
            path.join(diffsDir, "dui-site-baseline__dui-site.json"),
            JSON.stringify(diffJson),
          );
          console.log("[pages-demo] exported static diff: dui-site-baseline__dui-site.json");
        }

        runs.push({
          key: "dui-site",
          projectId: "oss-dui-site",
          project: "de-novo/dui (dui.sh)",
          branch: "Apache-2.0 · Zero-dep HTML",
          version: "Current capture",
          diffWith: ["dui-site-baseline"],
          liveAvailable: false,
        });
        runs.push({
          key: "dui-site-baseline",
          projectId: "oss-dui-site",
          project: "de-novo/dui (dui.sh)",
          branch: "Apache-2.0 · Zero-dep HTML",
          version: "Baseline capture",
          diffWith: [],
          liveAvailable: false,
        });
      } else {
        runs.push({
          key: "dui-site",
          projectId: "oss-dui-site",
          project: "de-novo/dui (dui.sh)",
          branch: "Apache-2.0 · Zero-dep HTML",
          version: "Self-dogfooded · 1 modal",
          liveAvailable: false,
        });
      }
    }

    if (!filterSet || filterSet.has("native-source-preview")) {
      console.log("[pages-demo] compiling SwiftUI & Compose portable source preview showcase...");
      const previewProjectDir = path.join(tmpHome, "native-source-preview");
      fs.mkdirSync(path.join(previewProjectDir, "ios"), { recursive: true });
      fs.mkdirSync(path.join(previewProjectDir, "android"), { recursive: true });
      fs.copyFileSync(
        path.join(root, "fixtures/source-preview/Settings.swift"),
        path.join(previewProjectDir, "ios/Settings.swift"),
      );
      fs.copyFileSync(
        path.join(root, "fixtures/source-preview/Settings.kt"),
        path.join(previewProjectDir, "android/Settings.kt"),
      );
      fs.writeFileSync(
        path.join(previewProjectDir, "dui.json"),
        JSON.stringify(
          {
            sourcePreviews: [
              { source: "ios/Settings.swift", entry: "Settings" },
              { source: "android/Settings.kt", entry: "example.Settings" },
            ],
          },
          null,
          2,
        ),
      );
      const previewHome = path.join(tmpHome, "home-native-source-preview");
      fs.mkdirSync(previewHome, { recursive: true });
      const previewEnv: NodeJS.ProcessEnv = { ...process.env, HOME: previewHome };
      delete previewEnv.ANDROID_HOME;
      delete previewEnv.ANDROID_SDK_ROOT;
      const previewRes = await runCommandAsync(binary, ["map", previewProjectDir, "--capture"], {
        env: previewEnv,
        timeoutMs: 180_000,
      });
      const previewParsed = JSON.parse(previewRes.stdout) as { output?: string };
      if (!previewParsed.output) {
        throw new Error(
          `Failed compiling native source preview showcase: ${previewRes.stderr || previewRes.stdout}`,
        );
      }
      const attached = attachNativeRecordings(previewParsed.output, previewEnv, tmpHome);
      exportRunToArchive(
        attached,
        path.join(archiveDir, "native-source-preview"),
        previewHome,
        root,
      );
      console.log(
        "[pages-demo] exported native-source-preview: SwiftUI + Compose with native captures",
      );
      runs.push({
        key: "native-source-preview",
        projectId: "oss-native-source-preview",
        project: "SwiftUI & Compose (native captures)",
        branch: "Apache-2.0 · Swift 6 + Kotlin 2",
        version: "iPhone simulator + Android emulator",
        diffWith: [],
        liveAvailable: false,
      });
    }

    if (!filterSet || filterSet.has("home-assistant-ios")) {
      // Mapped and captured on macOS by the "SwiftUI screen discovery" workflow; see
      // fixtures/home-assistant-ios/provenance.json.
      const fixture = path.join(root, "fixtures/home-assistant-ios");
      const provenance = JSON.parse(fs.readFileSync(path.join(fixture, "provenance.json"), "utf8"));
      for (const capture of provenance.captures as Array<{ imageSha256: string }>) {
        const matches = fs
          .readdirSync(fixture)
          .filter((name) => name.endsWith(".png"))
          .filter(
            (name) =>
              createHash("sha256")
                .update(fs.readFileSync(path.join(fixture, name)))
                .digest("hex") === capture.imageSha256,
          );
        if (matches.length !== 1)
          throw new Error("home-assistant-ios capture does not match provenance");
      }
      exportRunToArchive(fixture, path.join(archiveDir, "home-assistant-ios"), tmpHome, fixture);
      console.log(
        `[pages-demo] exported home-assistant-ios: ${provenance.declarations} SwiftUI declarations`,
      );
      runs.push({
        key: "home-assistant-ios",
        projectId: "oss-home-assistant-ios",
        project: "home-assistant/iOS (SwiftUI)",
        branch: `Apache-2.0 · ${String(provenance.commit).slice(0, 7)}`,
        version: "Index store map + simulator capture",
        diffWith: [],
        liveAvailable: false,
      });
    }

    fs.writeFileSync(path.join(archiveDir, "runs.json"), JSON.stringify(runs, null, 2));
    console.log(
      JSON.stringify({
        status: "ok",
        demoDir,
        count: runs.length,
        projects: runs.map((r) => r.key),
      }),
    );
  } finally {
    fs.rmSync(tmpHome, { recursive: true, force: true });
  }
}

await main();
