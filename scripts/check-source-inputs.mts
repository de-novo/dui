// Explicit source-input fixtures travel through `dui map --entry`, the immutable archive, the
// embedded viewer, and a hot source patch without network or browser payload injection.
import assert from "node:assert/strict";
import { execFileSync, spawn, spawnSync, type ChildProcess } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { withChromiumPage } from "./chromium-page.mts";

const root = resolve(import.meta.dirname, "..");
const binary = resolve(process.env.DUI_BIN ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-source-inputs-"));
const project = join(stage, "project");
const home = join(stage, "home");
cpSync(join(root, "fixtures/source-model"), project, { recursive: true });

const inputs = {
  defaultState: "populated",
  states: {
    populated: {
      slots: {
        "src/Orders.tsx#Orders.orders": [
          { id: "o1", title: "Fixture order", status: "open", total: 42 },
        ],
      },
    },
    empty: {
      slots: {
        "src/Orders.tsx#Orders.orders": [],
      },
    },
  },
};
writeFileSync(join(project, "inputs.json"), JSON.stringify(inputs, null, 2));
writeFileSync(join(project, "dui.json"), JSON.stringify({ sourceInputs: "inputs.json" }));

const env = { ...process.env, HOME: home, DUI_CHROMIUM: "/absent" };
const mapped = JSON.parse(
  execFileSync(binary, ["map", project, "--entry", "src/App.tsx"], { env, encoding: "utf8" }),
) as { output: string };
const archive = mapped.output;
const graph = JSON.parse(readFileSync(join(archive, "graph.json"), "utf8")) as {
  sourceInputs?: string;
};
const report = JSON.parse(readFileSync(join(archive, "report.json"), "utf8")) as {
  sourceInputs?: string;
};
assert.equal(graph.sourceInputs, "source-inputs.json");
assert.equal(report.sourceInputs, "source-inputs.json");
const archivedInputs = JSON.parse(readFileSync(join(archive, "source-inputs.json"), "utf8"));
assert.deepEqual(archivedInputs.states.populated.slots, inputs.states.populated.slots);
const inputDigest = () =>
  createHash("sha256")
    .update(readFileSync(join(archive, "source-inputs.json")))
    .digest("hex");
const immutableDigest = inputDigest();

const badProject = join(stage, "bad-project");
cpSync(project, badProject, { recursive: true });
writeFileSync(
  join(badProject, "inputs.json"),
  JSON.stringify({ states: { populated: { slots: { "missing#Nope.value": 1 } } } }),
);
const bad = spawnSync(binary, ["map", badProject, "--entry", "src/App.tsx"], {
  env: { ...env, HOME: join(stage, "bad-home") },
  encoding: "utf8",
});
assert.equal(bad.status, 2);
assert.match(bad.stderr + bad.stdout, /slot keys do not exist/);

let server: ChildProcess | undefined;
try {
  server = spawn(binary, ["open", archive, "--no-open"], {
    env,
    stdio: ["ignore", "pipe", "ignore"],
  });
  const url = await new Promise<string>((done, fail) => {
    const timer = setTimeout(() => fail(Error("dui open timed out")), 10_000);
    createInterface({ input: server!.stdout! }).on("line", (line) => {
      const match = line.match(/http:\/\/127\.0\.0\.1:\d+/);
      if (match) {
        clearTimeout(timer);
        done(match[0]);
      }
    });
  });

  await withChromiumPage(async (page) => {
    const ev = async <T,>(expression: string) =>
      (
        (await page.call("Runtime.evaluate", {
          expression,
          returnByValue: true,
          awaitPromise: true,
        })) as { result: { value: T } }
      ).result.value;
    const openViewer = async (state: string, width: number, height: number) => {
      const target = new URL(url);
      target.searchParams.set("view", "overview");
      target.searchParams.set("screen", "route:/");
      target.searchParams.set("state", state);
      target.searchParams.set("width", String(width));
      target.searchParams.set("height", String(height));
      await page.call("Emulation.setDeviceMetricsOverride", {
        width,
        height,
        deviceScaleFactor: 1,
        mobile: width < 600,
      });
      await page.call("Page.navigate", { url: String(target) });
    };
    const cardText = () =>
      ev<string>(`(() => {
        const cards=[...document.querySelectorAll('.screen-card,.mobile-deck-card')];
        const card=cards.find(c=>c.dataset.selected==='true'||c.classList.contains('selected'))??cards[0];
        const frame=card?.querySelector('.approximate-model-content');
        const shadow=frame?.shadowRoot;
        return shadow?[...shadow.children].filter(el=>el.tagName!=='STYLE').map(el=>el.textContent).join(' ').replace(/\\s+/g,' ').trim():'';
      })()`);
    const waitText = async (needle: string, timeout = 5000) => {
      const start = performance.now();
      for (;;) {
        const text = await cardText();
        if (text.includes(needle)) return text;
        if (performance.now() - start > timeout)
          throw Error(`missing ${needle}; card text was ${text.slice(0, 300)}`);
        await new Promise((resolve) => setTimeout(resolve, 40));
      }
    };

    await openViewer("populated", 1440, 920);
    const populated = await waitText("Fixture order");
    assert.ok(!populated.includes("No orders yet"), populated);

    const ordersPath = join(project, "src/Orders.tsx");
    const beforeHot = readFileSync(ordersPath, "utf8");
    writeFileSync(ordersPath, beforeHot.replace(">Orders</h1>", ">Orders hot input</h1>"));
    const hot = await waitText("Orders hot input");
    assert.ok(hot.includes("Fixture order"), hot);
    assert.equal(inputDigest(), immutableDigest, "hot reload must not rewrite archived inputs");

    await openViewer("empty", 390, 844);
    const empty = await waitText("No orders yet");
    assert.ok(!empty.includes("Fixture order"), empty);
  });
  console.log(JSON.stringify({ ok: true, archiveInputs: true, hotFixtureRetained: true }));
} finally {
  server?.kill("SIGTERM");
  rmSync(stage, { recursive: true, force: true });
}
