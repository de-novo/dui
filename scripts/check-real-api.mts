// Verify optional real API verification mode (#56):
// 1. Start a real loopback HTTP API server requiring an auth header from `DUI_TEST_AUTH_HEADER`.
// 2. Capture in `apiMode: "live"` with `apiBaseUrl`, `apiProxyPrefix: "/api/"`, and `apiHeaderEnv`:
//    - Real API data renders and verifies (`apiProvenance["/api/shelf"] === "live-api"`).
//    - Read-only guard blocks `POST /api/mutate` (`blockedMutations` records it, 0 server mutations).
//    - Secret auth token value NEVER leaks into `report.json`, `manifest.json`, `facts.json`, or `dui.db`.
// 3. Capture in `apiMode: "hybrid"` with one mocked endpoint + one live endpoint and confirm distinct provenance.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const template = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-real-api-check-"));
const secretToken = "Bearer dui-ephemeral-secret-998123";

async function runDui(args: string[], envExtra: Record<string, string>) {
  return await new Promise<{ status: number; body: Record<string, unknown> }>(
    (resolveRes, reject) => {
      const child = spawn(binary, args, {
        env: { ...process.env, ...envExtra },
        stdio: ["ignore", "pipe", "pipe"],
      });
      let stdout = "";
      let stderr = "";
      child.stdout.setEncoding("utf8");
      child.stderr.setEncoding("utf8");
      child.stdout.on("data", (chunk: string) => {
        stdout += chunk;
      });
      child.stderr.on("data", (chunk: string) => {
        stderr += chunk;
      });
      child.on("error", reject);
      child.on("close", (code) => {
        try {
          assert(stdout.trim().length > 0, `empty stdout for ${args.join(" ")}: ${stderr}`);
          resolveRes({ status: code ?? 1, body: JSON.parse(stdout) });
        } catch (err) {
          reject(err);
        }
      });
    },
  );
}

let mutationHits = 0;
let liveShelfHits = 0;
const apiServer = createServer((req, res) => {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Headers", "Authorization, Content-Type");
  if (req.method === "OPTIONS") {
    res.statusCode = 204;
    res.end();
    return;
  }
  if (req.url === "/api/mutate" && req.method === "POST") {
    mutationHits += 1;
    res.statusCode = 200;
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify({ mutated: true }));
    return;
  }
  if (req.url === "/api/shelf" && req.method === "GET") {
    if (req.headers.authorization !== secretToken) {
      res.statusCode = 401;
      res.setHeader("Content-Type", "application/json");
      res.end(JSON.stringify({ error: "unauthorized" }));
      return;
    }
    liveShelfHits += 1;
    res.statusCode = 200;
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify({ title: "Live Shelf From Server" }));
    return;
  }
  if (req.url === "/api/banner" && req.method === "GET") {
    res.statusCode = 200;
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify({ banner: "Live Banner" }));
    return;
  }
  res.statusCode = 404;
  res.end();
});

try {
  await new Promise<void>((resolveReady) => apiServer.listen(0, "127.0.0.1", resolveReady));
  const apiPort = (apiServer.address() as AddressInfo).port;
  const apiBaseUrl = `http://127.0.0.1:${apiPort}`;

  const home = join(stage, "home");
  const env = { HOME: home, DUI_TEST_AUTH_HEADER: secretToken, DUI_TEST_API_BASE: apiBaseUrl };
  const appDir = join(stage, "real-api-app");
  cpSync(template, appDir, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(join(template, "node_modules"), join(appDir, "node_modules"), "dir");

  writeFileSync(
    join(appDir, "main.tsx"),
    [
      'import { useEffect, useState } from "react";',
      'import { createRoot } from "react-dom/client";',
      "",
      "function App() {",
      '  const [title, setTitle] = useState("");',
      '  const [banner, setBanner] = useState("");',
      "  useEffect(() => {",
      '    fetch("/api/shelf")',
      "      .then((r) => r.json())",
      "      .then((d: { title: string }) => setTitle(d.title));",
      '    fetch("/api/banner")',
      "      .then((r) => r.json())",
      "      .then((d: { banner: string }) => setBanner(d.banner));",
      '    fetch("/api/mutate", { method: "POST", body: "{}" }).catch(() => {});',
      "  }, []);",
      "  if (!title || !banner) return <div>Loading...</div>;",
      "  return (",
      '    <main data-testid="live-api-ready">',
      "      <h1>{title}</h1>",
      "      <p>{banner}</p>",
      "    </main>",
      "  );",
      "}",
      "",
      'createRoot(document.getElementById("app")!).render(<App />);',
      "",
    ].join("\n"),
  );

  // 1. Live mode with env-resolved auth header + read-only mutation guard.
  const liveManifest = join(appDir, "dui.live.json");
  writeFileSync(
    liveManifest,
    JSON.stringify(
      {
        tsconfig: "tsconfig.json",
        path: "/",
        widths: [768],
        readySelector: "[data-testid=live-api-ready]",
        apiMode: "live",
        apiBaseUrl: "env:DUI_TEST_API_BASE",
        apiProxyPrefix: "/api/",
        apiHeaderEnv: { Authorization: "DUI_TEST_AUTH_HEADER" },
        mockResponses: { "/api/shelf": { title: "Mocked Shelf Ignored In Live Mode" } },
      },
      null,
      2,
    ),
  );

  const liveRun = await runDui(["run", appDir, liveManifest], env);
  assert(liveRun.body.output, JSON.stringify(liveRun.body));
  assert(liveShelfHits >= 1, "real API server received authenticated GET /api/shelf");
  assert.equal(mutationHits, 0, "read-only guard blocked POST /api/mutate");

  const liveCells = liveRun.body.cells as {
    status: string;
    observation: {
      text: string;
      apiMode: string;
      apiProvenance: Record<string, string>;
      blockedMutations: string[];
    };
  }[];
  const liveCell = liveCells[0]!;
  assert.equal(liveCell.status, "verified", JSON.stringify(liveCell));
  assert(
    liveCell.observation.text.includes("Live Shelf From Server"),
    `expected live API text in DOM, got: ${liveCell.observation.text}`,
  );
  assert.equal(liveCell.observation.apiMode, "live");
  assert.equal(liveCell.observation.apiProvenance["/api/shelf"], "live-api");
  assert.equal(liveCell.observation.apiProvenance["/api/mutate"], "blocked-mutation");
  assert(liveCell.observation.blockedMutations.includes("POST /api/mutate"));

  // Secret token must NEVER leak into archived files or SQLite database.
  const runDir = liveRun.body.output as string;
  for (const file of ["report.json", "manifest.json", "facts.json", "snapshot.json"]) {
    const p = join(runDir, file);
    if (existsSync(p)) {
      const content = readFileSync(p, "utf8");
      assert(!content.includes(secretToken), `secret leaked into ${file}`);
    }
  }
  const dbBytes = readFileSync(join(home, ".dui", "dui.db"));
  assert(!dbBytes.includes(Buffer.from(secretToken)), "secret leaked into dui.db");

  // 2. Hybrid mode: /api/banner mocked, /api/shelf served from live API.
  const hybridManifest = join(appDir, "dui.hybrid.json");
  writeFileSync(
    hybridManifest,
    JSON.stringify(
      {
        tsconfig: "tsconfig.json",
        path: "/",
        widths: [768],
        readySelector: "[data-testid=live-api-ready]",
        apiMode: "hybrid",
        apiBaseUrl,
        apiProxyPrefix: "/api/",
        apiHeaderEnv: { Authorization: "DUI_TEST_AUTH_HEADER" },
        mockResponses: { "/api/banner": { banner: "Hybrid Mocked Banner" } },
      },
      null,
      2,
    ),
  );
  const hybridRun = await runDui(["run", appDir, hybridManifest], env);
  assert(hybridRun.body.output, JSON.stringify(hybridRun.body));
  const hybridCells = hybridRun.body.cells as typeof liveCells;
  const hybridCell = hybridCells[0]!;
  assert.equal(hybridCell.status, "verified", JSON.stringify(hybridCell));
  assert(hybridCell.observation.text.includes("Live Shelf From Server"));
  assert(hybridCell.observation.text.includes("Hybrid Mocked Banner"));
  assert.equal(hybridCell.observation.apiProvenance["/api/shelf"], "live-api");
  assert.equal(hybridCell.observation.apiProvenance["/api/banner"], "mock");

  console.log(
    JSON.stringify({
      ok: true,
      liveShelfHits,
      mutationHits,
      liveProvenance: liveCell.observation.apiProvenance,
      hybridProvenance: hybridCell.observation.apiProvenance,
      secretRedacted: true,
    }),
  );
} finally {
  apiServer.close();
  rmSync(stage, { recursive: true, force: true });
}
