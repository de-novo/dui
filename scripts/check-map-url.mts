// A running app needs only its loopback URL and rendered links for a screen map.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

const exec = promisify(execFile);
const binary = resolve("target/debug/dui");
const dir = await mkdtemp(join(tmpdir(), "dui-map-url-"));
const run = (...args: string[]) =>
  exec(binary, ["map", ...args], { cwd: dir, env: { ...process.env, HOME: dir } });

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
  console.log(JSON.stringify({ pages: paths.length, width: result.cells[0]?.width }));
} finally {
  await new Promise<void>((done) => server.close(() => done()));
  await rm(dir, { recursive: true, force: true });
}
