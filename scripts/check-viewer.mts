// Verify embedded `apps/canvas` viewer (`dui open --no-open`), removal of legacy HTML galleries,
// and tag-based version / update-check / installer tag resolution.
import assert from "node:assert/strict";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

import { findChrome, withChromiumPage } from "./chromium-page.mts";
import { verifyViewerThemes, viewerViewports } from "./verify-viewer-themes.mts";

const root = resolve(import.meta.dirname, "..");
const fixture = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-viewer-check-"));

function runDui(args: string[], envExtra: Record<string, string>) {
  const res = spawnSync(binary, args, {
    env: { ...process.env, ...envExtra },
    encoding: "utf8",
  });
  assert(res.stdout.trim().length > 0, `empty stdout for ${args.join(" ")}: ${res.stderr}`);
  return { status: res.status ?? 1, body: JSON.parse(res.stdout) };
}

try {
  const home = join(stage, "home");
  const baseEnv = { HOME: home };

  // 1. Verify `--version` reports SemVer version and `vX.Y.Z` tag.
  const ver = runDui(["--version"], baseEnv);
  assert.equal(ver.status, 0);
  const verBody = ver.body as { name: string; version: string; tag: string };
  assert.equal(verBody.name, "dui");
  assert.match(verBody.version, /^\d+\.\d+\.\d+$/);
  assert.equal(verBody.tag, `v${verBody.version}`);

  // 2. Verify tag-based `dui update --check` and `scripts/install` tag resolution against a local git repo.
  const tagRepo = join(stage, "tag-repo");
  execFileSync("git", ["init", "-q", "-b", "main", tagRepo]);
  execFileSync("git", ["config", "user.name", "Test"], { cwd: tagRepo });
  execFileSync("git", ["config", "user.email", "test@example.com"], { cwd: tagRepo });
  writeFileSync(join(tagRepo, "README.md"), "release test\n");
  execFileSync("git", ["add", "README.md"], { cwd: tagRepo });
  execFileSync("git", ["commit", "-q", "-m", "initial"], { cwd: tagRepo });
  execFileSync("git", ["tag", verBody.tag], { cwd: tagRepo });

  const upToDate = runDui(["update", "--check", "--repo", tagRepo], baseEnv);
  assert.equal(upToDate.status, 0);
  assert.equal((upToDate.body as { updateAvailable: boolean }).updateAvailable, false);
  assert.equal((upToDate.body as { latest: string }).latest, verBody.tag);

  execFileSync("git", ["tag", "v99.0.2"], { cwd: tagRepo });
  execFileSync("git", ["tag", "v99.0.10"], { cwd: tagRepo });
  const hasUpdate = runDui(["update", "--check", "--repo", tagRepo], baseEnv);
  assert.equal(hasUpdate.status, 0);
  const updateBody = hasUpdate.body as {
    current: string;
    latest: string;
    nextTag: string;
    updateAvailable: boolean;
  };
  assert.equal(updateBody.updateAvailable, true);
  assert.equal(updateBody.latest, "v99.0.10");
  assert.equal(updateBody.nextTag, "v99.0.10");

  // Confirm `scripts/install` tag resolver picks v99.0.10 (numeric SemVer sort, not lexicographic).
  const installResolved = execFileSync(
    "sh",
    [
      "-c",
      `git ls-remote --tags --refs "$1" 'v*' | awk '{ sub("^refs/tags/", "", $2); if ($2 ~ /^v[0-9]+\\.[0-9]+\\.[0-9]+$/) print $2 }' | sort -t. -k1.2,1n -k2,2n -k3,3n | tail -n 1`,
      "sh",
      tagRepo,
    ],
    { encoding: "utf8" },
  ).trim();
  assert.equal(installResolved, "v99.0.10");

  // Confirm `scripts/install` PATH helper updates shell rc idempotently and respects DUI_NO_MODIFY_PATH=1.
  const fakeHome = join(stage, "fake-home");
  const fakeBin = join(fakeHome, ".local/bin");
  const installScript = join(root, "scripts/install");
  const helperTest = [
    `eval "$(sed -n '/^append_path_line()/,/^main()/p' "$1" | sed '$d')"`,
    `ensure_on_path "$2"`,
    `ensure_on_path "$2"`,
  ].join("\n");
  execFileSync("sh", ["-eu", "-c", helperTest, "sh", installScript, fakeBin], {
    env: { HOME: fakeHome, SHELL: "/bin/zsh", PATH: "/usr/bin:/bin" },
  });
  const zshrc = execFileSync("cat", [join(fakeHome, ".zshrc")], { encoding: "utf8" });
  assert.equal(
    zshrc.split(fakeBin).length - 1,
    1,
    `expected idempotent PATH entry in .zshrc, got:\n${zshrc}`,
  );
  const optOutHome = join(stage, "optout-home");
  execFileSync(
    "sh",
    ["-eu", "-c", helperTest, "sh", installScript, join(optOutHome, ".local/bin")],
    {
      env: {
        HOME: optOutHome,
        SHELL: "/bin/zsh",
        PATH: "/usr/bin:/bin",
        DUI_NO_MODIFY_PATH: "1",
      },
    },
  );
  assert(!existsSync(join(optOutHome, ".zshrc")), "DUI_NO_MODIFY_PATH=1 must not modify .zshrc");

  // 3. Capture a fixture and verify legacy graph.html / history.html are NOT generated.
  const appDir = join(stage, "app");
  cpSync(fixture, appDir, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(join(fixture, "node_modules"), join(appDir, "node_modules"), "dir");

  const captured = runDui(["run", appDir, join(appDir, "dui.json")], baseEnv);
  assert.equal(captured.status, 0);
  const runDir = (captured.body as { output: string }).output;
  assert(!existsSync(join(runDir, "graph.html")), "legacy graph.html must no longer be generated");

  const history = runDui(["history"], baseEnv);
  assert.equal(history.status, 0);
  assert(!existsSync(join(home, ".dui", "history.html")), "legacy history.html must not exist");
  assert.equal((history.body as { gallery?: string }).gallery, undefined);

  // 4. Launch embedded viewer via `dui open --no-open` (single binary, zero Vite dev server).
  const viewer = spawn(binary, ["open", "--no-open", runDir], {
    env: { ...process.env, ...baseEnv },
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    const url = await new Promise<string>((resolveUrl, rejectUrl) => {
      let buf = "";
      let errBuf = "";
      const timer = setTimeout(() => {
        rejectUrl(new Error(`timed out waiting for dui open URL: ${buf}\n${errBuf}`));
      }, 15_000);
      viewer.stdout.on("data", (chunk) => {
        buf += chunk.toString();
        const match = buf.match(/http:\/\/127\.0\.0\.1:\d+/);
        if (match) {
          clearTimeout(timer);
          resolveUrl(match[0]);
        }
      });
      viewer.stderr.on("data", (chunk) => {
        errBuf += chunk.toString();
      });
      viewer.on("exit", (code) => {
        clearTimeout(timer);
        rejectUrl(new Error(`dui open exited early (${code}): ${errBuf}`));
      });
    });

    const htmlRes = await fetch(url);
    assert.equal(htmlRes.status, 200);
    const html = await htmlRes.text();
    assert(html.includes('<div id="root"></div>'), "expected embedded apps/canvas HTML shell");

    const runs = (await (await fetch(`${url}/archive/runs.json`)).json()) as { key: string }[];
    assert.equal(runs.length, 1);
    const runKey = runs[0]!.key;

    const graphRes = await fetch(`${url}/archive/graph.json?run=${runKey}`);
    assert.equal(graphRes.status, 200);
    const graph = (await graphRes.json()) as {
      cells: { image?: string; imageWidth?: number; imageHeight?: number }[];
    };
    assert(graph.cells.length > 0);
    const firstCellWithImg = graph.cells.find((c) => c.image);
    assert(firstCellWithImg?.image);
    assert(
      (firstCellWithImg.imageWidth ?? 0) > 0 && (firstCellWithImg.imageHeight ?? 0) > 0,
      "archive graph.json exposes real PNG imageWidth and imageHeight",
    );
    const firstImg = firstCellWithImg.image;
    const imgRes = await fetch(`${url}/archive/${encodeURIComponent(firstImg)}?run=${runKey}`);
    assert.equal(imgRes.status, 200);
    assert.equal(imgRes.headers.get("content-type"), "image/png");

    const diffRes = await fetch(`${url}/api/diff?before=${runKey}&after=${runKey}`);
    assert.equal(diffRes.status, 200);
    const diff = (await diffRes.json()) as { summary: { totalChanged: number } };
    assert.equal(diff.summary.totalChanged, 0);

    // Render the embedded viewer in headless Chrome to verify the React + React Flow bundle mounts.
    const chrome = findChrome();
    const dom = execFileSync(
      chrome,
      [
        "--headless=new",
        "--disable-gpu",
        "--no-sandbox",
        "--virtual-time-budget=3000",
        "--dump-dom",
        `${url}/?screen=route%3A%2F&uiLang=en`,
      ],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], timeout: 30_000 },
    );
    assert(dom.includes("Archive"), "expected embedded viewer React UI to mount in Chrome");

    const themes = await withChromiumPage(async (page) => {
      const results = [];
      for (const { width, height, mobile, requireSelector } of viewerViewports) {
        await page.call("Emulation.setDeviceMetricsOverride", {
          width,
          height,
          mobile,
          deviceScaleFactor: mobile ? 2 : 1,
        });
        results.push({ width, themes: await verifyViewerThemes(page, url, requireSelector) });
      }
      return results;
    });

    console.log(
      JSON.stringify({
        version: verBody,
        updateCheck: updateBody,
        installResolvedTag: installResolved,
        embeddedViewerUrl: url,
        runsServed: runs.length,
        cellsServed: graph.cells.length,
        legacyHtmlRemoved: true,
        chromeRenderedEmbeddedViewer: true,
        themes,
      }),
    );
  } finally {
    viewer.kill();
  }
} finally {
  rmSync(stage, { recursive: true, force: true });
}
