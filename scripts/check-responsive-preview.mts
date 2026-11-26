// Exercise the public card and the same live renderer while changing the app's breakpoints.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { once } from "node:events";
import { cpSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { withChromiumPage } from "./chromium-page.mts";

const binary = resolve("target/debug/dui");
const root = mkdtempSync(join(tmpdir(), "dui-responsive-"));
const project = join(root, "project");
const env = { ...process.env, HOME: root };
type Layout = {
  width: number;
  height: number;
  scrollHeight: number;
  path: string;
  fields: { id: number; value: string }[];
  containers: { id: number; width: number }[];
  renderTree: { id: string; rect: { x: number; y: number; width: number; height: number } }[];
};
// Execute inside the viewer tab. The live frame dimensions must change after using its form.
async function checkDialog() {
  async function until<T>(read: () => T | undefined | null | false): Promise<T> {
    const end = performance.now() + 30000;
    while (performance.now() < end) {
      const value = read();
      if (value) return value;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    throw Error(
      "Timed out waiting for the responsive preview: " + document.body.innerText.slice(-400),
    );
  }
  const trigger = await until(() =>
    Array.from(document.querySelectorAll("button")).find(
      (b) => b.textContent === "Responsive preview",
    ),
  );
  trigger.click();
  const image = () => document.querySelector<HTMLImageElement>(".responsive-frame");
  await until(() => image()?.naturalWidth);
  const input = document.querySelector<HTMLInputElement>('input[name="width"]')!;
  const desired = Number(input.value) + 17;
  input.value = String(desired);
  input.form!.requestSubmit();
  await until(() => image()?.naturalWidth === desired);
  const dialog = document.querySelector('[role="dialog"]')!;
  if (document.documentElement.scrollWidth > innerWidth) throw Error("horizontal overflow");
  for (const el of dialog.querySelectorAll("input, select, button")) {
    const box = el.getBoundingClientRect();
    if (box.left < 0 || box.right > innerWidth) throw Error("clipped preview control");
  }
  for (const form of dialog.querySelectorAll("form")) {
    const button = form.querySelector("button")!.getBoundingClientRect();
    for (const control of form.querySelectorAll("input,select")) {
      const box = control.getBoundingClientRect();
      if (
        button.top < box.bottom &&
        box.top < button.bottom &&
        Math.abs(box.bottom - button.bottom) > 1
      )
        throw Error("Apply button is misaligned with its control");
    }
  }
  dialog.querySelector<HTMLElement>('[data-slot="dialog-close"]')!.click();
  await until(() => !document.querySelector('[role="dialog"]'));
  return true;
}

// Width preview renders the screen from its source in the running app (#323): the live frame
// follows the chosen width, and the status says it is rendered, not captured.
async function checkWidthPreview(widths: number[]) {
  async function until<T>(read: () => T | undefined | null | false): Promise<T> {
    const end = performance.now() + 30000;
    while (performance.now() < end) {
      const value = read();
      if (value) return value;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    throw Error("Timed out waiting for the width preview: " + document.body.innerText.slice(-400));
  }
  const trigger = await until(() =>
    document.querySelector<HTMLButtonElement>("[data-width-preview-open]"),
  );
  trigger.click();
  const status = () => document.querySelector<HTMLElement>("[data-width-preview-status]");
  await until(() => status()?.dataset.widthPreviewStatus === "live");
  const input = await until(() =>
    document.querySelector<HTMLInputElement>("[data-width-preview-input]"),
  );
  const shown: number[] = [];
  for (const width of widths) {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(
      input,
      String(width),
    );
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await until(
      () =>
        document.querySelector<HTMLElement>(".width-preview-stage")?.dataset.liveWidth ===
          String(width) &&
        document.querySelector<HTMLImageElement>(".width-preview-live")?.naturalWidth === width,
    );
    shown.push(document.querySelector<HTMLImageElement>(".width-preview-live")!.naturalWidth);
  }
  if (document.querySelector('[role="alert"]')) throw Error("live session failed");
  if (document.documentElement.scrollWidth > innerWidth) throw Error("horizontal overflow");
  for (const el of document.querySelectorAll(".width-preview-dialog :is(input, button)")) {
    const box = el.getBoundingClientRect();
    if (box.left < 0 || box.right > innerWidth) throw Error("clipped width preview control");
  }
  return shown;
}
async function check(media: number, container: number) {
  writeFileSync(
    join(project, "index.html"),
    `<meta name="viewport" content="width=device-width"><div id="app"></div><style>
  body{margin:0}main{min-height:110vh}.toolbar{display:flex;flex-direction:column}
  @media(min-width:${media}px){.toolbar{flex-direction:row}}
  #pane{container-type:inline-size;width:${container + 37}px}
  .actions{display:grid}#more{display:none}
  @container(min-width:${container}px){.actions{grid-template-columns:1fr 1fr}#more{display:block}}
  #note{width:100%}footer{position:fixed;bottom:0}
  </style><script type="module" src="/main.tsx"></script>`,
  );
  writeFileSync(
    join(project, "main.tsx"),
    `import {createRoot} from 'react-dom/client';
  createRoot(document.getElementById('app')!).render(<main><h1>Responsive</h1><label>Note<input id="note"/></label><label>Party<select defaultValue="one"><option value="one">One</option><option value="two">Two</option></select></label><div className="toolbar"><button id="first">One</button><button id="second">Two</button></div><section id="pane"><div className="actions"><button>Actions</button><button id="more">More</button></div><p id="wrap">{'Visible words '.repeat(15)}</p></section><footer id="bottom">Footer</footer></main>);`,
  );
  writeFileSync(
    join(project, "dui.json"),
    JSON.stringify({ readySelector: "main h1", widths: [media + 79] }),
  );
  const report = JSON.parse(
    execFileSync(binary, ["map", ".", "--capture"], { cwd: project, env, encoding: "utf8" }),
  );
  const row = report.cells[0];
  assert.equal(row.status, "verified");
  const archive = join(report.output, "report.json"),
    original = readFileSync(archive);
  const processViewer = spawn(binary, ["open", "--no-open", report.output], {
    cwd: project,
    env,
    stdio: ["ignore", "pipe", "inherit"],
  });
  const lines = createInterface({ input: processViewer.stdout });
  try {
    const [url] = (await once(lines, "line", { signal: AbortSignal.timeout(15000) })) as [string];
    const [run] = (await (await fetch(url + "/archive/runs.json")).json()) as {
      key: string;
      responsiveAvailable: boolean;
    }[];
    assert.ok(run?.responsiveAvailable);
    const denied = await fetch(url + "/api/responsive", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ op: "open", run: run.key }),
    });
    assert.equal(denied.status, 403);
    const cell = { state: row.state, width: row.width };
    const api = async (body: object) => {
      const r = await fetch(url + "/api/responsive", {
        method: "POST",
        headers: { Origin: url, "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      const result = await r.json();
      assert.ok(r.ok, JSON.stringify(result));
      return result;
    };
    const opened = await api({ op: "open", run: run.key, cell, requestId: randomUUID() });
    const addr = { session: opened.session, cell };
    let revision = opened.revision;
    try {
      const update = async (data: object): Promise<Layout> => {
        const req = { ...addr, ...data, expectedRevision: revision, requestId: randomUUID() };
        const res = await api(req);
        revision = res.revision;
        return res.layout;
      };
      let layout: Layout = await api({ ...addr, op: "layout" });
      const note = randomUUID();
      layout = await update({ op: "fill", field: layout.fields[0]!.id, text: note });
      layout = await update({ op: "fill", field: layout.fields[1]!.id, text: "two" });
      const initial = layout;
      // A sampled interval records evidence, reports unsampled widths and restores prior constraints.
      const probe = {
        op: "sample",
        ...addr,
        axis: "width",
        min: 128,
        max: 1024,
        budget: 24,
        expectedRevision: revision,
        requestId: randomUUID(),
      };
      const survey = await api(probe);
      assert.deepEqual(await api(probe), survey);
      revision = survey.revision;
      assert.ok(survey.observedChanges.some(([a, b]: number[]) => a === media - 1 && b === media));
      assert.ok(survey.unresolvedIntervals.length);
      assert.deepEqual(await api({ ...addr, op: "layout" }), initial);
      const containerProbe = await api({
        ...probe,
        container: layout.containers[0]!.id,
        budget: 2,
        expectedRevision: revision,
        requestId: randomUUID(),
      });
      revision = containerProbe.revision;
      assert.deepEqual(containerProbe.unresolvedIntervals, [[probe.min + 1, probe.max - 1]]);
      assert.deepEqual(await api({ ...addr, op: "layout" }), initial);

      const id = layout.containers[0]!.id;
      const rect = (l: Layout, id: string) => l.renderTree.find((n) => n.id === id)!.rect;
      const sample = async (width: number, parent: boolean) => {
        const next = await update({ op: "resize", width, ...(parent ? { container: id } : {}) });
        assert.equal(next.path, initial.path);
        assert.equal(next.fields[0]?.value, note);
        assert.equal(next.fields[1]?.value, "two");
        assert.equal(next.height, initial.height);
        assert.equal(parent ? next.containers[0]?.width : next.width, width);
        if (parent) assert.equal(next.width, initial.width);
        return next;
      };
      // Locate the first changed layout from observations; the search never receives the CSS threshold.
      async function transition(parent: boolean, predicate: (l: Layout) => boolean) {
        let lo = 128,
          hi = 1024;
        assert.equal(predicate(await sample(lo, parent)), false);
        assert.equal(predicate(await sample(hi, parent)), true);
        while (hi - lo > 1) {
          const mid = Math.floor((lo + hi) / 2);
          if (predicate(await sample(mid, parent))) hi = mid;
          else lo = mid;
        }
        return hi;
      }
      assert.equal(
        await transition(false, (l) => rect(l, "#first").y === rect(l, "#second").y),
        media,
      );
      await sample(initial.width, false);
      assert.equal(
        await transition(true, (l) => l.renderTree.some((n) => n.id === "#more")),
        container,
      );
      const narrow = await sample(128, true),
        wide = await sample(512, true);
      assert.ok(rect(narrow, "#wrap").height > rect(wide, "#wrap").height);
      const before = wide;
      const after = await update({ op: "resize", height: before.height + 97 });
      assert.equal(after.width, before.width);
      assert.equal(after.height - before.height, 97);
      assert.equal(rect(after, "#bottom").y - rect(before, "#bottom").y, 97);
      assert.ok(after.scrollHeight > before.scrollHeight);
      assert.equal(after.fields[0]?.value, note);
      assert.equal(after.fields[1]?.value, "two");
      const frame = await api({ ...addr, op: "frame" });
      const png = Buffer.from(frame.imageBase64, "base64");
      assert.equal(png.readUInt32BE(16), after.width);
      assert.equal(png.readUInt32BE(20), after.height);
      console.log({
        media,
        container,
        runtime: opened.environment.renderer,
        path: after.path,
        revision,
      });
    } finally {
      await api({ ...addr, op: "close" });
    }
    await withChromiumPage(async ({ call }) => {
      for (const [width, height] of [
        [390, 844],
        [1440, 920],
      ] as const) {
        await call("Emulation.setDeviceMetricsOverride", {
          width,
          height,
          deviceScaleFactor: 1,
          mobile: false,
        });
        await call("Page.navigate", {
          url: url + "/?" + new URLSearchParams({ run: run.key, screen: "route:/", uiLang: "en" }),
        });
        const { exceptionDetails } = await call("Runtime.evaluate", {
          expression: `(${checkDialog.toString()})()`,
          awaitPromise: true,
          returnByValue: true,
        });
        assert.ok(!exceptionDetails, JSON.stringify(exceptionDetails));
        await call("Page.navigate", {
          url: url + "/?" + new URLSearchParams({ run: run.key, screen: "route:/", uiLang: "en" }),
        });
        const widths = [media - 20, media + 20];
        const preview = (await call("Runtime.evaluate", {
          expression: `(${checkWidthPreview.toString()})(${JSON.stringify(widths)})`,
          awaitPromise: true,
          returnByValue: true,
        })) as { result: { value: number[] }; exceptionDetails?: unknown };
        assert.ok(!preview.exceptionDetails, JSON.stringify(preview.exceptionDetails));
        assert.deepEqual(preview.result.value, widths);
      }
    });
    assert.deepEqual(readFileSync(archive), original);
  } finally {
    lines.close();
    const stopped = once(processViewer, "exit");
    processViewer.kill();
    await stopped;
  }
}
try {
  cpSync("fixtures/replay", project, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(resolve("fixtures/replay/node_modules"), join(project, "node_modules"), "dir");
  execFileSync("git", ["init", "-q"], { cwd: project });
  for (const [media, container] of [
    [463, 311],
    [713, 421],
  ] as const)
    await check(media, container);
} finally {
  rmSync(root, { recursive: true, force: true });
}
