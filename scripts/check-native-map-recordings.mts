import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { withChromiumPage } from "./chromium-page.mts";
import { verifyNativeMapViewer } from "./check-native-map-viewer.mts";

const binary = process.env.DUI_BIN || resolve(import.meta.dirname, "../target/debug/dui");

export async function verifyAttachedRecordings(source: string) {
  const graphBytes = await readFile(join(source, "graph.json"));
  const { nodes } = JSON.parse(graphBytes.toString()) as { nodes: { id: string; label: string }[] };
  const { id: sourceSnapshot, projectId } = JSON.parse(
    await readFile(join(source, "snapshot.json"), "utf8"),
  );
  assert(nodes.length >= 3);
  const temp = await mkdtemp(join(tmpdir(), "dui-native-recordings-"));
  const manifest = join(temp, "recordings.json");
  const runs = join(temp, ".dui", "projects", projectId, "runs");
  const localSource = join(runs, sourceSnapshot);
  const attach = () =>
    execFileSync(binary, ["map", "attach", localSource, manifest], {
      encoding: "utf8",
      env: { ...process.env, HOME: temp },
      stdio: ["ignore", "pipe", "pipe"],
    });
  try {
    await mkdir(runs, { recursive: true });
    await cp(source, localSource, { recursive: true });
    const captures = await withChromiumPage(async ({ call }) => {
      const captures = [];
      for (const [index, node] of nodes.slice(0, 2).entries()) {
        const width = 390;
        await call("Emulation.setDeviceMetricsOverride", {
          width,
          height: 844,
          mobile: false,
          deviceScaleFactor: 1,
        });
        await call("Runtime.evaluate", {
          expression: "document.body.textContent = " + JSON.stringify(node.label),
        });
        const { data } = await call("Page.captureScreenshot", { format: "png" });
        const image = `frame-${index}.png`;
        await writeFile(join(temp, image), Buffer.from(String(data), "base64"));
        captures.push({
          screenId: node.id,
          runtime: "web",
          image,
          width,
          observedPath: `/sample/${index}`,
        });
      }
      return captures;
    });
    await writeFile(manifest, JSON.stringify({ sourceSnapshot: "stale", captures }));
    assert.throws(attach);
    await writeFile(
      manifest,
      JSON.stringify({ sourceSnapshot, captures: [{ ...captures[0], screenId: "missing" }] }),
    );
    assert.throws(attach);
    await writeFile(manifest, JSON.stringify({ sourceSnapshot, captures }));
    const run = JSON.parse(attach());
    assert.deepEqual(await readFile(join(source, "graph.json")), graphBytes);
    assert.notEqual(run.output, source);
    assert.equal(run.coverage.attempted, 0);
    assert.equal(run.coverage.verified, 0);
    assert.equal(run.cells.length, captures.length);
    const db = new DatabaseSync(join(temp, ".dui", "dui.db"), { readOnly: true });
    try {
      for (const cap of captures) {
        const cell = run.cells.find((c: { screenId: string }) => c.screenId === cap.screenId);
        assert.equal(cell?.status, "unverified");
        assert.equal(cell.runtime, cap.runtime);
        assert.equal(cell.observedPath, cap.observedPath);
        assert.deepEqual(
          await readFile(join(run.output, cell.image)),
          await readFile(join(temp, cap.image)),
        );
        const row = db
          .prepare(
            "SELECT s.origin_json, c.image_blob_id FROM cells c JOIN screens s ON s.id=c.screen_id WHERE c.run_id=? AND c.screen_id=?",
          )
          .get(run.snapshot.id, cap.screenId);
        assert(row?.image_blob_id);
        assert.equal(JSON.parse(String(row.origin_json)).kind, "native");
      }
      const counts = db
        .prepare("SELECT count(*) AS n FROM run_screens WHERE run_id=?")
        .get(run.snapshot.id);
      assert.equal(counts?.n, nodes.length);
    } finally {
      db.close();
    }
    await verifyNativeMapViewer(run.output);
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
}

if (import.meta.main) {
  assert(process.argv[2], "Pass a native source archive");
  await verifyAttachedRecordings(process.argv[2]);
}
