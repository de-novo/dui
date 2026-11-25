// Map real source declarations, then operate each observed transition in the embedded viewer.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { once } from "node:events";
import { mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { withChromiumPage } from "./chromium-page.mts";
import { viewerViewports } from "./verify-viewer-themes.mts";

const binary = resolve("target/debug/dui");
const root = mkdtempSync(join(tmpdir(), "dui-preview-map-"));
const project = join(root, "project");
mkdirSync(project);
const env = { ...process.env, HOME: root };
const map = (cwd = project) =>
  JSON.parse(execFileSync(binary, ["map", "."], { cwd, env, encoding: "utf8" }));
type Card = {
  id: string;
  nativeSource: { source: string; name: string };
  sourcePreview: { status: string; sourceSha256?: string };
};
const graph = (dir: string) =>
  JSON.parse(readFileSync(join(dir, "graph.json"), "utf8")) as { nodes: Card[] };
const entries = [
  { source: "ios/Settings.swift", entry: "Settings" },
  { source: "android/Settings.kt", entry: "Settings" },
  { source: "missing.swift", entry: "Missing" },
  { source: "unsupported.swift", entry: "Unavailable" },
];
const titles = ["Swift " + randomUUID(), "Compose <img src=x> " + randomUUID()];
try {
  for (const [i, folder] of ["ios", "android"].entries()) {
    mkdirSync(join(project, folder));
    const suffix = i === 0 ? "swift" : "kt";
    const code = readFileSync("fixtures/source-preview/Settings." + suffix, "utf8")
      .replace("package example", "")
      .replaceAll("Notifications", titles[i]!);
    writeFileSync(join(project, entries[i]!.source), code);
  }
  writeFileSync(
    join(project, entries[3]!.source),
    "import SwiftUI\nstruct Unavailable: View { var body: some View { NotAPreviewPrimitive() } }",
  );
  const configure = () =>
    writeFileSync(join(project, "dui.json"), JSON.stringify({ sourcePreviews: entries }));
  configure();
  const first = map();
  assert.deepEqual(first.coverage, { attempted: 0, renderable: 0, unrenderable: 0, verified: 0 });
  const nodes = graph(first.output).nodes;
  assert.equal(nodes.length, entries.length);
  assert.equal(new Set(nodes.map((n) => n.id)).size, nodes.length);
  assert.deepEqual(
    nodes.map((n) => n.sourcePreview.status),
    ["ready", "ready", "missing", "unsupported"],
    JSON.stringify(
      nodes.map((n) => (n.sourcePreview.status === "ready" ? "ready" : n.sourcePreview)),
    ),
  );
  await withViewer(first.output, async (url) => {
    const image = await exerciseMap(url, nodes);
    writeFileSync(join(root, "frame.png"), image);
    const snapshot = JSON.parse(readFileSync(join(first.output, "snapshot.json"), "utf8"));
    const manifest = join(root, "recordings.json");
    const record = (id: string) =>
      writeFileSync(
        manifest,
        JSON.stringify({
          sourceSnapshot: snapshot.id,
          captures: [
            {
              screenId: id,
              runtime: "web",
              image: "frame.png",
              width: image.readUInt32BE(16),
              observedPath: "/settings",
            },
          ],
        }),
      );
    const attach = () =>
      JSON.parse(
        execFileSync(binary, ["map", "attach", first.output, manifest], {
          cwd: project,
          env,
          encoding: "utf8",
          stdio: ["ignore", "pipe", "pipe"],
        }),
      );
    record(randomUUID());
    assert.throws(attach);
    record(nodes[1]!.id);
    const attached = attach();
    assert.equal(attached.cells.length, 1);
    assert.equal(
      attached.cells[0].path,
      nodes[1]!.nativeSource.source + "#" + nodes[1]!.nativeSource.name,
    );
    assert.equal(attached.cells[0].status, "unverified");
    assert.equal(attached.coverage.verified, 0);
    const [run] = await (await fetch(url + "/archive/runs.json")).json();
    const get = async () =>
      (await (await fetch(url + "/archive/graph.json?run=" + run.key)).json()).nodes as Card[];
    const file = join(project, entries[0]!.source);
    const changed = readFileSync(file, "utf8").replaceAll("Settings", "Preferences");
    writeFileSync(file, changed);
    assert.equal((await get())[0]!.sourcePreview.status, "stale");
    const old = entries[0]!.source;
    entries[0] = { source: "ios/Preferences.swift", entry: "Preferences" };
    renameSync(join(project, old), join(project, entries[0].source));
    configure();
    const next = map();
    const after = graph(next.output).nodes;
    assert.notEqual(after[0]!.id, nodes[0]!.id);
    assert.equal(after[0]!.sourcePreview.status, "ready");
    assert.equal(after[1]!.id, nodes[1]!.id);
    assert.equal(after[1]!.sourcePreview.sourceSha256, nodes[1]!.sourcePreview.sourceSha256);
    await withViewer(next.output, async (newUrl) => {
      await exerciseMap(newUrl, after);
    });
    assert.equal((await get())[0]!.sourcePreview.status, "stale");
    assert.equal(graph(first.output).nodes[0]!.sourcePreview.status, "ready");
    // Identical declarations in another project must never resolve to the same card.
    const other = join(root, "other");
    mkdirSync(other);
    await withViewer(
      first.output,
      async (portableUrl) => {
        const saved = await (await fetch(portableUrl + "/archive/graph.json")).json();
        assert.deepEqual(
          saved.nodes.map((n: Card) => n.sourcePreview),
          nodes.map((n) => n.sourcePreview),
        );
      },
      other,
    );
    writeFileSync(join(other, "dui.json"), JSON.stringify({ sourcePreviews: [entries[2]] }));
    assert.notEqual(graph(map(other).output).nodes[0]!.id, nodes[2]!.id);
  });
  console.log(
    "Verified two same-name component cards, real switches in both viewports, stale and missing sources, unsupported calls, renamed entries, archive isolation and zero native coverage.",
  );
} finally {
  rmSync(root, { recursive: true, force: true });
}

async function withViewer(output: string, check: (url: string) => Promise<void>, cwd = project) {
  const proc = spawn(binary, ["open", "--no-open", output], {
    cwd,
    env,
    stdio: ["ignore", "pipe", "inherit"],
  });
  const lines = createInterface({ input: proc.stdout });
  try {
    const [url] = (await once(lines, "line", { signal: AbortSignal.timeout(15000) })) as [string];
    await check(url);
  } finally {
    lines.close();
    const stopped = once(proc, "exit");
    proc.kill();
    await stopped;
  }
}

async function exerciseMap(url: string, nodes: Card[]) {
  return await withChromiumPage(async ({ call }) => {
    for (const { width, height, mobile } of viewerViewports) {
      await call("Emulation.setDeviceMetricsOverride", {
        width,
        height,
        deviceScaleFactor: 1,
        mobile,
      });
      for (const [i, node] of nodes.entries()) {
        await call("Page.navigate", { url: url + "/?uiLang=en" });
        const result = await call("Runtime.evaluate", {
          expression: `(${exerciseCard.toString()})(${JSON.stringify({ mobile, id: node.id, title: titles[i], source: node.nativeSource.source, status: node.sourcePreview.status })})`,
          awaitPromise: true,
          returnByValue: true,
        });
        assert.ok(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
      }
    }
    const { data } = await call("Page.captureScreenshot", { format: "png" });
    return Buffer.from(String(data), "base64");
  });
}

async function exerciseCard({
  mobile,
  id,
  title,
  source,
  status,
}: {
  mobile: boolean;
  id: string;
  title?: string;
  source: string;
  status: string;
}) {
  async function until<T>(read: () => T | false | null | undefined): Promise<T> {
    const end = performance.now() + 20000;
    while (performance.now() < end) {
      const v = read();
      if (v) return v;
      await new Promise((resolve) => setTimeout(resolve, 40));
    }
    throw Error("Missing preview UI for " + source + ": " + document.body.innerText.slice(-500));
  }
  const card = await until(() =>
    Array.from(
      document.querySelectorAll<HTMLElement>(mobile ? ".mobile-deck-card" : ".react-flow__node"),
    ).find((el) => (el.dataset.screenId || el.dataset.id) === id),
  );
  (mobile ? card.querySelector("button")! : card).click();
  const panel = await until(() => document.querySelector(".source-preview-panel"));
  if (!document.querySelector(".inspector .native-source-evidence")?.textContent?.includes(source))
    throw Error("Wrong source");
  if (status === "ready") {
    const controls = () => Array.from(panel.querySelectorAll<HTMLInputElement>('[role="switch"]'));
    if (controls().length !== 2 || !panel.textContent?.includes(title + " off"))
      throw Error("Wrong initial component");
    controls()[0]!.click();
    await until(() => panel.textContent?.includes(title + " on"));
    if (!controls()[0]!.checked || !controls()[1]!.checked) throw Error("Wrong switch state");
    controls()[1]!.click();
    await until(() => !controls()[1]?.checked);
    if (!controls()[0]!.checked) throw Error("Switches were coupled");
    if (panel.querySelector("img,iframe,script,a"))
      throw Error("Archive content created active markup");
  } else if (panel.querySelector("input") || !panel.querySelector("p code")?.textContent)
    throw Error("Missing per-card failure");
  if (document.documentElement.scrollWidth > innerWidth) throw Error("Horizontal overflow");
  return true;
}
