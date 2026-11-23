import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";

const repoRoot = process.cwd();
const duiBin = path.join(repoRoot, "target", "debug", "dui");
assert.ok(fs.existsSync(duiBin), `missing built dui binary at ${duiBin}`);

async function waitForUrl(child: ReturnType<typeof spawn>): Promise<string> {
  return new Promise((resolve, reject) => {
    let buf = "";
    const timer = setTimeout(
      () => reject(new Error(`timed out waiting for viewer URL: ${buf}`)),
      15000,
    );
    child.stdout?.on("data", (chunk: Buffer) => {
      buf += chunk.toString("utf8");
      const m = buf.match(/http:\/\/127\.0\.0\.1:\d+/);
      if (m) {
        clearTimeout(timer);
        resolve(m[0]);
      }
    });
    child.on("exit", (code) => {
      clearTimeout(timer);
      reject(new Error(`viewer exited early (${code}): ${buf}`));
    });
  });
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-live-canvas-"));
const duiHome = path.join(tmp, ".dui-home");

const liveServer = http.createServer((_req, res) => {
  res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
  res.end("<html><body><h1>Live Surface Host</h1></body></html>");
});

try {
  await new Promise<void>((resolve) => liveServer.listen(0, "127.0.0.1", () => resolve()));
  const addr = liveServer.address();
  assert.ok(addr && typeof addr === "object");
  const liveOrigin = `http://127.0.0.1:${addr.port}`;

  const siteDir = path.join(tmp, "site");
  fs.mkdirSync(siteDir, { recursive: true });
  fs.writeFileSync(
    path.join(siteDir, "index.html"),
    "<html><body><h1>Static Host</h1></body></html>",
  );
  const manifest = path.join(siteDir, "dui.json");
  fs.writeFileSync(manifest, JSON.stringify({ host: "static", path: "/", widths: [1280] }));

  const outDir = path.join(tmp, "out");
  execFileSync(duiBin, ["run", siteDir, manifest, outDir], {
    env: { ...process.env, HOME: duiHome },
  });

  // 1. Loopback DUI_LIVE_URL is exposed cleanly on runs.json
  const viewer1 = spawn(duiBin, ["open", "--no-open", outDir], {
    env: { ...process.env, HOME: duiHome, DUI_LIVE_URL: liveOrigin },
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    const baseUrl = await waitForUrl(viewer1);
    const resp = await fetch(`${baseUrl}/archive/runs.json`);
    assert.equal(resp.status, 200);
    const runs = (await resp.json()) as Array<{ liveBaseUrl?: string; liveDefaultMode?: string }>;
    assert.equal(runs[0]?.liveBaseUrl, liveOrigin);
    assert.equal(runs[0]?.liveDefaultMode, undefined);
  } finally {
    viewer1.kill("SIGTERM");
  }

  // 2. Non-loopback DUI_LIVE_URL is rejected for security
  const viewer2 = spawn(duiBin, ["open", "--no-open", outDir], {
    env: { ...process.env, HOME: duiHome, DUI_LIVE_URL: "https://evil.example.com" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    const baseUrl = await waitForUrl(viewer2);
    const resp = await fetch(`${baseUrl}/archive/runs.json`);
    assert.equal(resp.status, 200);
    const runs = (await resp.json()) as Array<{ liveBaseUrl?: string }>;
    assert.equal(runs[0]?.liveBaseUrl, undefined);
  } finally {
    viewer2.kill("SIGTERM");
  }

  // 3. Explicit `--live-url <loopback>` exposes liveBaseUrl and defaults canvas to Live mount mode
  const viewer3 = spawn(duiBin, ["open", "--no-open", "--live-url", liveOrigin, outDir], {
    env: { ...process.env, HOME: duiHome },
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    const baseUrl = await waitForUrl(viewer3);
    const resp = await fetch(`${baseUrl}/archive/runs.json`);
    assert.equal(resp.status, 200);
    const runs = (await resp.json()) as Array<{ liveBaseUrl?: string; liveDefaultMode?: string }>;
    assert.equal(runs[0]?.liveBaseUrl, liveOrigin);
    assert.equal(runs[0]?.liveDefaultMode, "live");
  } finally {
    viewer3.kill("SIGTERM");
  }

  // 4. Non-loopback `--live-url` is rejected with exit code 2
  const { spawnSync } = await import("node:child_process");
  const badLive = spawnSync(
    duiBin,
    ["open", "--no-open", "--live-url", "https://evil.example.com", outDir],
    { env: { ...process.env, HOME: duiHome }, encoding: "utf8" },
  );
  assert.equal(
    badLive.status,
    2,
    `expected exit 2 for non-loopback --live-url, got ${badLive.status}`,
  );

  // 5. One-command `dui open --live <run>` auto-starts the static host on loopback and injects zero-modification telemetry bridge via /live-surface/
  let autoLiveUrl = "";
  let bridgeInjected = false;
  const viewer4 = spawn(duiBin, ["open", "--no-open", "--live", outDir], {
    env: { ...process.env, HOME: duiHome },
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    const baseUrl = await waitForUrl(viewer4);
    const resp = await fetch(`${baseUrl}/archive/runs.json`);
    assert.equal(resp.status, 200);
    const runs = (await resp.json()) as Array<{
      liveBaseUrl?: string;
      liveBridgePath?: string;
      liveDefaultMode?: string;
    }>;
    autoLiveUrl = runs[0]?.liveBaseUrl ?? "";
    assert.match(autoLiveUrl, /^http:\/\/127\.0\.0\.1:\d+$/, "expected auto-started loopback host");
    assert.equal(runs[0]?.liveBridgePath, "/live-surface");
    assert.equal(runs[0]?.liveDefaultMode, "live");
    const liveHtml = await (await fetch(autoLiveUrl)).text();
    assert.ok(liveHtml.includes("Static Host"), "auto-started live host serves live app HTML");

    const bridgedResp = await fetch(`${baseUrl}/live-surface/`);
    assert.equal(bridgedResp.status, 200);
    const bridgedHtml = await bridgedResp.text();
    assert.ok(bridgedHtml.includes("Static Host"), "bridged HTML retains upstream content");
    assert.ok(
      bridgedHtml.includes(`<base href="${autoLiveUrl}/" />`),
      "bridged HTML injects loopback <base> tag",
    );
    assert.ok(
      bridgedHtml.includes("dui:live-render-tree"),
      "bridged HTML injects zero-modification live telemetry bridge",
    );
    bridgeInjected = true;
  } finally {
    viewer4.kill("SIGTERM");
  }

  // 6. One-command `dui open --live <vite-run>` auto-starts Vite dev server on loopback & serves bridged HTML
  const toggleDir = path.join(repoRoot, "fixtures", "toggle");
  let viteAutoLiveUrl = "";
  if (fs.existsSync(path.join(toggleDir, "node_modules", ".bin", "vite"))) {
    const viteOutDir = path.join(tmp, "out-vite");
    fs.mkdirSync(viteOutDir, { recursive: true });
    fs.writeFileSync(
      path.join(viteOutDir, "manifest.json"),
      JSON.stringify({ path: "/", widths: [1280] }),
    );
    fs.writeFileSync(
      path.join(viteOutDir, "snapshot.json"),
      JSON.stringify({ projectRoot: toggleDir, app: "." }),
    );
    fs.writeFileSync(
      path.join(viteOutDir, "graph.json"),
      JSON.stringify({ screens: [], edges: [] }),
    );
    const viewer5 = spawn(duiBin, ["open", "--no-open", "--live", viteOutDir], {
      env: { ...process.env, HOME: duiHome },
      stdio: ["ignore", "pipe", "pipe"],
    });
    try {
      const baseUrl = await waitForUrl(viewer5);
      const runs = (await (await fetch(`${baseUrl}/archive/runs.json`)).json()) as Array<{
        liveBaseUrl?: string;
        liveBridgePath?: string;
      }>;
      viteAutoLiveUrl = runs[0]?.liveBaseUrl ?? "";
      assert.match(
        viteAutoLiveUrl,
        /^http:\/\/127\.0\.0\.1:\d+$/,
        "expected auto-started Vite server",
      );
      assert.equal(runs[0]?.liveBridgePath, "/live-surface");
      const viteBridgedHtml = await (await fetch(`${baseUrl}/live-surface/`)).text();
      assert.ok(
        viteBridgedHtml.includes("dui:live-render-tree"),
        "Vite dev server HTML includes injected live telemetry bridge",
      );
    } finally {
      viewer5.kill("SIGTERM");
    }
  }

  console.log(
    JSON.stringify({
      status: "ok",
      loopbackLiveBaseUrlExposed: liveOrigin,
      nonLoopbackRejected: true,
      cliLiveUrlFlag: true,
      autoLiveStartedUrl: autoLiveUrl,
      bridgeInjected,
      viteAutoLiveUrl,
    }),
  );
} finally {
  liveServer.close();
  fs.rmSync(tmp, { recursive: true, force: true });
}
