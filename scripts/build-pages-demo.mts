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
    serverCmd: (port) => ["pnpm", ["exec", "vite", "--host", "127.0.0.1", "--port", String(port)]],
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
    startPath: "/dashboard",
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
    serverCmd: (port) => ["pnpm", ["exec", "vite", "--host", "127.0.0.1", "--port", String(port)]],
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
      const res = await runCommandAsync(binary, ["map", distDir], {
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
    const res = await runCommandAsync(binary, ["map", repoDir], {
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
      const res = await runCommandAsync(binary, ["map", currentSiteDir], {
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
        "Render every screen, state, modal, and width from real code.",
        "Deterministic UI review from your real compiler.",
      );
      fs.writeFileSync(baseIndexPath, baseHtml);

      const baseHome = path.join(tmpHome, "home-dui-site-baseline");
      fs.mkdirSync(baseHome, { recursive: true });
      const baseRes = await runCommandAsync(binary, ["map", baselineSiteDir], {
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
          version: "v0.2.0-current (Compare diff!)",
          liveAvailable: false,
        });
        runs.push({
          key: "dui-site-baseline",
          projectId: "oss-dui-site",
          project: "de-novo/dui (dui.sh)",
          branch: "Apache-2.0 · Zero-dep HTML",
          version: "v0.2.0-baseline",
          liveAvailable: false,
        });
      } else {
        runs.push({
          key: "dui-site",
          projectId: "oss-dui-site",
          project: "de-novo/dui (dui.sh)",
          branch: "Apache-2.0 · Zero-dep HTML",
          version: "Self-dogfooded · 2 modals",
          liveAvailable: false,
        });
      }
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
