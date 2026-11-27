import assert from "node:assert/strict";
import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = mkdtempSync(join(tmpdir(), "dui-hot-"));
const project = join(root, "project");
const home = join(root, "home");
cpSync("fixtures/source-model", project, { recursive: true });
execFileSync("mkdir", ["-p", home]);
execFileSync("cargo", ["build", "-p", "dui", "--locked"], { stdio: "inherit" });
const bin = join(process.cwd(), "target/debug/dui");
const env = { ...process.env, HOME: home };
const mapped = JSON.parse(
  execFileSync(bin, ["map", project, "--entry", "src/App.tsx"], { env, encoding: "utf8" }),
) as { output: string };
const archive = mapped.output;
let server: ChildProcess | undefined;
try {
  server = spawn(bin, ["open", archive, "--no-open"], { env, stdio: ["ignore", "pipe", "pipe"] });
  const url = await new Promise<string>((resolve, reject) => {
    let out = "";
    server?.stdout?.on("data", (chunk) => {
      out += String(chunk);
      const match = out.match(/http:\/\/127\.0\.0\.1:\d+/);
      if (match) resolve(match[0]);
    });
    server?.on("exit", () => reject(Error("dui open exited")));
    setTimeout(() => reject(Error("dui open did not print a URL")), 5000);
  });
  const runs = (await (await fetch(`${url}/archive/runs.json`)).json()) as { key: string }[];
  const run = runs[0]?.key;
  assert.ok(run, "run key");
  const api = `${url}/api/source-model?run=${encodeURIComponent(run)}`;
  type Hot = {
    revision: number;
    statusRevision: number;
    watch?: { status?: string; reason?: string | null };
    patch?: {
      parsedFiles?: string[];
      changedFiles?: string[];
      affectedScreens?: string[];
      files?: { upsert?: { path?: string }[]; remove?: string[] };
      components?: { upsert?: { id?: string }[] };
      diagnostics?: { code?: string }[];
    };
  };
  const get = async () => (await (await fetch(api)).json()) as Hot;
  const waitReady = async () => {
    for (let i = 0; i < 100; i++) {
      const hot = await get();
      if (hot.watch?.status === "ready") return hot;
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
    throw Error("watcher did not become ready");
  };
  const waitRevision = async (revision: number, timeout = 3000) => {
    const start = performance.now();
    for (;;) {
      const hot = await get();
      if (hot.revision > revision) return { hot, ms: performance.now() - start };
      if (performance.now() - start > timeout) throw Error(`no hot revision after ${revision}`);
      await new Promise((resolve) => setTimeout(resolve, 8));
    }
  };
  const digest = (file: string) => createHash("sha256").update(readFileSync(file)).digest("hex");
  const archiveHashes = Object.fromEntries(
    ["source-model.json", "graph.json", "report.json"].map((file) => [
      file,
      digest(join(archive, file)),
    ]),
  );
  let state = await waitReady();
  const ordersPath = join(project, "src/Orders.tsx");
  const original = readFileSync(ordersPath, "utf8");
  const latencies: number[] = [];
  for (let i = 0; i < 10; i++) {
    const marker = `Orders hot ${i}`;
    const source = readFileSync(ordersPath, "utf8").replace(
      />Orders(?: hot \d+)?<\/h1>/,
      `>${marker}</h1>`,
    );
    writeFileSync(ordersPath, source);
    const written = performance.now();
    const result = await waitRevision(state.revision);
    // The write promise is synchronous in this test, so the timer starts at write completion and
    // ends when the viewer server's patched in-memory model is observable over HTTP.
    latencies.push(performance.now() - written);
    assert.deepEqual(result.hot.patch?.parsedFiles, ["src/Orders.tsx"]);
    assert.deepEqual(result.hot.patch?.changedFiles, ["src/Orders.tsx"]);
    assert.ok(result.hot.patch?.affectedScreens?.includes("route:/"));
    assert.ok(!result.hot.patch?.affectedScreens?.includes("route:/settings"));
    state = result.hot;
  }

  // Added import and newly reachable source file.
  writeFileSync(
    join(project, "src/format.ts"),
    "export const formatMoney = (n: number) => `$${n}`;\n",
  );
  writeFileSync(
    ordersPath,
    original
      .replace(
        'import { useOrders } from "./api";',
        'import { useOrders } from "./api";\nimport { formatMoney } from "./format";',
      )
      .replace(">Orders</h1>", ">{formatMoney(2481)}</h1>"),
  );
  let result = await waitRevision(state.revision);
  assert.ok(result.hot.patch?.files?.upsert?.some((file) => file.path === "src/format.ts"));
  assert.ok(result.hot.patch?.parsedFiles?.includes("src/format.ts"));
  state = result.hot;

  // Deleted import and deleted file: extraction recovers through the import graph.
  writeFileSync(ordersPath, original);
  rmSync(join(project, "src/format.ts"));
  result = await waitRevision(state.revision);
  assert.ok(result.hot.patch?.files?.remove?.includes("src/format.ts"));
  state = result.hot;

  // Transient invalid source must not kill the watcher; a valid save recovers.
  writeFileSync(ordersPath, "export function Orders( { return <main>");
  const beforeInvalid = state;
  let invalidObserved = false;
  for (let i = 0; i < 80; i++) {
    const hot = await get();
    if (
      hot.revision > beforeInvalid.revision ||
      hot.statusRevision > beforeInvalid.statusRevision
    ) {
      invalidObserved = true;
      state = hot;
      break;
    }
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  assert.ok(invalidObserved, "invalid source was observed without killing the server");
  writeFileSync(ordersPath, original);
  result = await waitRevision(state.revision, 4000).catch(async () => {
    // If invalid syntax only changed status and did not produce a model revision, compare against
    // the last valid revision instead.
    return waitRevision(beforeInvalid.revision, 4000);
  });
  assert.equal(result.hot.watch?.status, "ready");
  assert.ok(!result.hot.patch?.diagnostics?.some((d) => d.code === "parse-error"));

  for (const [file, before] of Object.entries(archiveHashes)) {
    assert.equal(digest(join(archive, file)), before, `${file} stayed immutable`);
  }
  latencies.sort((a, b) => a - b);
  const percentile = (p: number) =>
    latencies[Math.min(latencies.length - 1, Math.ceil((p / 100) * latencies.length) - 1)];
  console.log(
    JSON.stringify({
      ok: true,
      timer:
        "completion of each controlled fixture file write until the changed in-memory model revision was visible from /api/source-model; excludes browser paint, app rebuild, and server startup",
      samplesMs: latencies.map((ms) => Math.round(ms * 10) / 10),
      medianMs: Math.round(percentile(50) * 10) / 10,
      p95Ms: Math.round(percentile(95) * 10) / 10,
      maxMs: Math.round(latencies[latencies.length - 1] * 10) / 10,
    }),
  );
} finally {
  server?.kill();
  rmSync(root, { recursive: true, force: true });
}
