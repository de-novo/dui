// Exercise the public walkthrough in a disposable project and home.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { once } from "node:events";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";
import { pathToFileURL } from "node:url";
import { withChromiumPage, type ChromiumPage } from "./chromium-page.mts";

const root = path.resolve(import.meta.dirname, "..");
const stage = fs.mkdtempSync(path.join(os.tmpdir(), "dui-dispatch-"));
const project = path.join(stage, "dispatch");
const home = path.join(stage, "home");
const binary = path.join(root, "target/debug/dui");
const env = {
  ...process.env,
  HOME: home,
  DUI_DESKTOP_BIN:
    process.env.DUI_DESKTOP_BIN || path.join(root, "apps/desktop/target/debug/dui-desktop"),
};
const json = (file: string) => JSON.parse(fs.readFileSync(file, "utf8"));
function dui(...args: string[]) {
  return JSON.parse(
    execFileSync(binary, args, {
      cwd: project,
      env,
      encoding: "utf8",
      timeout: 120_000,
      maxBuffer: 64 * 1024 * 1024,
    }),
  );
}

type TreeNode = { id: string; tag: string; text: string; rect: { width: number; height: number } };
type Cell = { path: string; width: number; image: string; observation: { renderTree: TreeNode[] } };
type Edge = {
  kind: string;
  source: string;
  target: string;
  action?: { label: string };
  observedWidths: number[];
};
type Screen = { id: string; label: string; kind: string };

async function evaluate<T>(page: ChromiumPage, expression: string): Promise<T> {
  const answer = await page.call("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
    timeout: 15000,
  });
  if (answer.exceptionDetails) throw Error(JSON.stringify(answer.exceptionDetails));
  return (answer.result as { value: T }).value;
}

async function waitFor(page: ChromiumPage, condition: string) {
  await evaluate(
    page,
    `(async () => {
    const start = performance.now();
    while (!(${condition})) {
      if (performance.now() - start > 10000) throw Error(${JSON.stringify("Timed out waiting for " + condition)});
      await new Promise(requestAnimationFrame);
    }
  })()`,
  );
}

async function loaded(page: ChromiumPage, action: () => Promise<unknown>) {
  const signal = AbortSignal.timeout(15000);
  async function loadEvent() {
    while (true) {
      const [event] = await once(page.ws, "message", { signal });
      if (JSON.parse(String(event.data)).method === "Page.loadEventFired") return;
    }
  }
  await Promise.all([loadEvent(), action()]);
}

// Follow the example from its entry URL, clicking links and opening its real dialogs.
// The graph under test never supplies destinations, trigger labels or expected dialog contents.
async function verifyCapture(page: ChromiumPage, origin: string, archive: string, cells: Cell[]) {
  const graph = json(path.join(archive, "graph.json"));
  const screens = new Map<string, string>(
    (graph.nodes as Screen[]).filter((n) => n.kind === "screen").map((n) => [n.id, n.label]),
  );
  const edges = (graph.edges as Edge[]).filter((e) => e.kind === "linksTo");
  for (const width of new Set(cells.map((c) => c.width))) {
    await page.call("Emulation.setDeviceMetricsOverride", {
      width,
      height: 800,
      mobile: false,
      deviceScaleFactor: 1,
    });
    const routes = new Set(["/"]);
    const observedEdges = new Set<string>();
    const observedScreens = new Set<string>();
    const visit = (route: string) =>
      loaded(page, () => page.call("Page.navigate", { url: new URL(route, origin).href }));
    const cellFor = (route: string) => {
      const cell = cells.find((c) => c.path === route && c.width === width);
      assert.ok(cell?.image, `Missing capture for ${route} at ${width}`);
      const png = fs.readFileSync(path.join(archive, cell.image));
      assert.ok(png.subarray(0, 8).equals(Buffer.from("89504e470d0a1a0a", "hex")));
      return cell;
    };
    let dialogs = 0;
    for (const route of routes) {
      await visit(route);
      observedScreens.add(route);
      const heading = await evaluate<string>(page, 'document.querySelector("h1").textContent');
      assert.ok(
        cellFor(route).observation.renderTree.some((n) => n.tag === "h1" && n.text === heading),
      );
      const links = "Array.from(document.links).filter(a => a.origin === location.origin)";
      const count = await evaluate<number>(page, links + ".length");
      for (let i = 0; i < count; i++) {
        await loaded(page, () => evaluate(page, `void setTimeout(()=>${links}[${i}].click(),0)`));
        const target = await evaluate<string>(page, "location.pathname + location.search");
        routes.add(target);
        observedEdges.add(JSON.stringify([route, target]));
        await visit(route);
      }
      const triggers = "Array.from(document.querySelectorAll('button[aria-haspopup=\"dialog\"]'))";
      const triggerCount = await evaluate<number>(page, triggers + ".length");
      for (let i = 0; i < triggerCount; i++) {
        const label = await evaluate<string>(
          page,
          `(()=>{const b=${triggers}[${i}]; b.click();return b.innerText.trim()})()`,
        );
        await waitFor(page, 'document.querySelector("dialog[open]")');
        const title = await evaluate<string>(
          page,
          `document.querySelector("dialog[open] :is(h1,h2,h3)").textContent`,
        );
        const matching = edges.filter(
          (e) =>
            screens.get(e.source) === route &&
            e.action?.label === label &&
            e.observedWidths.includes(width),
        );
        assert.equal(matching.length, 1, `Expected one captured dialog for ${route}: ${label}`);
        const target = screens.get(matching[0]!.target)!;
        const overlay = cellFor(target);
        assert.ok(
          overlay.observation.renderTree.some((n) => n.text === title),
          `Missing open dialog: ${title}`,
        );
        assert.ok(
          !fs
            .readFileSync(path.join(archive, overlay.image))
            .equals(fs.readFileSync(path.join(archive, cellFor(route).image))),
          `Opening ${label} produced identical PNGs`,
        );
        observedScreens.add(target);
        observedEdges.add(JSON.stringify([route, target]));
        dialogs++;
        await visit(route);
      }
    }
    assert.ok(
      routes.size > 1 && dialogs > 0,
      "The walkthrough must exercise navigation and a dialog",
    );
    const archived = edges
      .filter((e) => e.observedWidths.includes(width))
      .map((e) => JSON.stringify([screens.get(e.source), screens.get(e.target)]));
    assert.deepEqual(new Set(archived), observedEdges);
    assert.deepEqual(new Set(screens.values()), observedScreens);
    console.log(
      `${width}px: matched ${routes.size} routes, ${dialogs} open dialogs and ${observedEdges.size} directed connections`,
    );
  }
}

async function verifyViewer(
  page: ChromiumPage,
  before: { output: string; snapshot: { id: string } },
  after: { snapshot: { id: string } },
  cell: Cell,
  heading: TreeNode,
  widths: number[],
) {
  const viewer = spawn(binary, ["open", "--no-open"], {
    env,
    stdio: ["ignore", "pipe", "inherit"],
  });
  try {
    const reader = createInterface({ input: viewer.stdout });
    const [line] = await once(reader, "line", { signal: AbortSignal.timeout(15000) });
    const url = String(line).match(/http:\/\/127\.0\.0\.1:\d+/)?.[0];
    assert.ok(url, "Missing viewer URL");
    const runs = (await (await fetch(url + "/archive/runs.json")).json()) as {
      version: string;
      key: string;
    }[];
    const key = (id: string) => {
      const run = runs.find((r) => r.version === id);
      assert.ok(run);
      return run.key;
    };
    const graph = json(path.join(before.output, "graph.json"));
    const screen = (graph.nodes as Screen[]).find(
      (n) => n.kind === "screen" && n.label === cell.path,
    )!;
    const select = new URLSearchParams({
      run: key(before.snapshot.id),
      screen: screen.id,
      width: String(cell.width),
    });
    await loaded(page, () => page.call("Page.navigate", { url: url + "/?" + select }));
    await waitFor(page, 'document.querySelector(".element-node-btn")');
    await evaluate(
      page,
      `(()=>{const b=Array.from(document.querySelectorAll(".element-node-btn")).find(b=>b.querySelector("code").textContent===${JSON.stringify(heading.id)});b.closest("details").open=true;b.click()})()`,
    );
    await waitFor(
      page,
      `document.querySelector(".element-spec-title code")?.textContent===${JSON.stringify(heading.id)} && document.querySelector(".box-model-diagram")`,
    );
    const dimensions = await evaluate<string>(
      page,
      'document.querySelector(".element-spec-title span").textContent',
    );
    assert.ok(dimensions.startsWith(`${heading.rect.width} × ${heading.rect.height} px`));
    const pair = new URLSearchParams({
      view: "diff",
      before: key(before.snapshot.id),
      after: key(after.snapshot.id),
    });
    await loaded(page, () => page.call("Page.navigate", { url: url + "/?" + pair }));
    await waitFor(
      page,
      `Number(document.querySelector("[data-diff-total-changed]")?.dataset.diffTotalChanged)===${widths.length} && document.querySelector("[data-diff-screen]")`,
    );
    const rows = await evaluate<[string, number][]>(
      page,
      'Array.from(document.querySelectorAll("[data-diff-screen]"), e => [e.dataset.diffScreen, Number(e.dataset.diffWidth)])',
    );
    assert.deepEqual(rows.sort(), widths.map((w) => [cell.path, w]).sort());
  } finally {
    viewer.kill();
  }
}

const site = http.createServer((req, res) => {
  const file = new URL(req.url ?? "/", "http://localhost").pathname.slice(1) || "index.html";
  if (file.includes("/") || file.includes("..") || !fs.existsSync(path.join(project, file))) {
    res.writeHead(404).end();
    return;
  }
  res.setHeader("Content-Type", file.endsWith(".css") ? "text/css" : "text/html; charset=utf-8");
  fs.createReadStream(path.join(project, file)).pipe(res);
});
try {
  fs.cpSync(path.join(root, "examples/dispatch"), project, { recursive: true });
  fs.mkdirSync(home);
  const before = dui("map", ".");
  const cells: Cell[] = json(path.join(before.output, "report.json")).cells;
  const entry = cells.find((c) => c.path === "/");
  assert.ok(entry);
  const heading = entry.observation.renderTree.find((n) => n.tag === "h1");
  assert.ok(heading);
  site.listen(0, "127.0.0.1");
  await once(site, "listening");
  const address = site.address();
  assert.ok(address && typeof address === "object");
  await withChromiumPage(async (page) => {
    await verifyCapture(page, `http://127.0.0.1:${address.port}`, before.output, cells);
    const index = path.join(project, "index.html");
    const revised = heading.text + " · revised";
    const html = fs.readFileSync(index, "utf8");
    assert.ok(html.includes(`<h1>${heading.text}</h1>`));
    fs.writeFileSync(index, html.replace(`<h1>${heading.text}</h1>`, `<h1>${revised}</h1>`));
    const after = dui("map", ".");
    const diff = dui("diff", before.output, after.output);
    type Change = { tag: string; beforeText?: string; afterText?: string };
    type DiffCell = { path: string; width: number; renderDiff?: { changes: Change[] } };
    const changed: DiffCell[] = diff.screens.flatMap((s: { cells: DiffCell[] }) => s.cells);
    const expectedWidths = cells.filter((c) => c.path === entry.path).map((c) => c.width);
    assert.deepEqual(changed.map((c) => c.width).sort(), expectedWidths.sort());
    for (const c of changed) {
      assert.equal(c.path, entry.path);
      assert.ok(
        c.renderDiff?.changes.some(
          (n) => n.tag === heading.tag && n.beforeText === heading.text && n.afterText === revised,
        ),
      );
    }
    if (process.argv.includes("--desktop")) {
      const images = ["overview", "detail"].map((view) => {
        const file = path.join(stage, view + ".png");
        const snapshot = dui(
          "desktop",
          "--home",
          path.join(home, ".dui"),
          "--run",
          before.snapshot.id,
          "--view",
          view,
          "--screen",
          entry.path,
          "--snapshot-out",
          file,
        );
        assert.equal(snapshot.ok, true);
        assert.equal(snapshot.summary.cells, cells.length + after.cells.length);
        const image = fs.readFileSync(file);
        assert.equal(image.readUInt32BE(16), snapshot.pixelWidth);
        assert.equal(image.readUInt32BE(20), snapshot.pixelHeight);
        console.log(
          `GPUI ${view}: ${snapshot.renderer}, ${snapshot.pixelWidth} × ${snapshot.pixelHeight}`,
        );
        return image;
      });
      assert.ok(!images[0]!.equals(images[1]!), "Desktop selection must change the painted view");
    }
    await verifyViewer(page, before, after, entry, heading, expectedWidths);
    const clip = pathToFileURL(path.join(root, "examples/recordings/dispatch.mp4")).href;
    await loaded(page, () => page.call("Page.navigate", { url: clip }));
    await waitFor(page, 'document.querySelector("video")?.readyState >= 2');
    const seconds = await evaluate<number>(
      page,
      `(async () => {
      const v = document.querySelector("video");
      v.pause();
      if (!(Number.isFinite(v.duration) && v.duration > 0 && v.videoWidth && v.videoHeight)) throw Error("Invalid walkthrough video");
      await new Promise(resolve => { v.addEventListener("seeked",resolve,{once:true});v.currentTime = v.duration * 0.9; });
      return v.duration;
    })()`,
    );
    console.log(`Decoded walkthrough video (${seconds.toFixed(1)} s)`);
    console.log(
      `Matched ${heading.id} in Element Inspector and ${changed.length} changed cells in CLI and viewer Diff`,
    );
  });
} finally {
  site.close();
  fs.rmSync(stage, { recursive: true, force: true });
}
