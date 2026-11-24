import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { decodeOverview } from "../apps/canvas/src/data/overview.ts";
import { withChromiumPage } from "./chromium-page.mts";
import { viewerViewports } from "./verify-viewer-themes.mts";

export async function verifyNativeMapViewer(archiveDir: string) {
  const { screens, screenDiscovery } = decodeOverview(
    JSON.parse(readFileSync(join(archiveDir, "graph.json"), "utf8")),
  );
  assert(screenDiscovery, "Expected source discovery metadata");
  const binary = process.env.DUI_BIN || resolve(import.meta.dirname, "../target/debug/dui");
  const viewer = spawn(binary, ["open", "--no-open", resolve(archiveDir)], {
    stdio: ["ignore", "pipe", "inherit"],
  });
  try {
    const url = await new Promise<string>((done, fail) => {
      const timer = setTimeout(() => fail(Error("Timed out opening source map")), 15_000);
      const lines = createInterface({ input: viewer.stdout });
      viewer.once("error", fail);
      lines.once("close", () => {
        clearTimeout(timer);
        fail(Error("Viewer exited before opening the archive"));
      });
      lines.on("line", (line) => {
        const match = line.match(/http:\/\/127\.0\.0\.1:\d+/);
        if (match) {
          done(match[0]);
          lines.close();
        }
      });
    });
    await withChromiumPage(async ({ call }) => {
      async function evaluate(expression: string) {
        const { result, exceptionDetails } = await call("Runtime.evaluate", {
          expression,
          returnByValue: true,
          awaitPromise: true,
          timeout: 15_000,
        });
        assert(!exceptionDetails, JSON.stringify(exceptionDetails));
        return (result as { value: unknown }).value;
      }
      async function waitFor(selector: string) {
        await evaluate(`(async () => {
          while (!document.querySelector(${JSON.stringify(selector)})) await new Promise(requestAnimationFrame);
        })()`);
      }
      for (const { width, height, mobile } of viewerViewports) {
        await call("Emulation.setDeviceMetricsOverride", {
          width,
          height,
          mobile,
          deviceScaleFactor: 1,
        });
        await call("Page.navigate", { url: url + "/?uiLang=en" });
        if (screens.length === 0) {
          await waitFor(".native-empty h2");
          assert.equal(
            await evaluate("document.querySelector('.native-empty h2').textContent"),
            "No screen declarations found",
          );
          assert.deepEqual(
            await evaluate(
              "Array.from(document.querySelectorAll('.discovery-limitations li'), el => el.textContent)",
            ),
            screenDiscovery.limitations,
          );
          assert.equal(await evaluate("document.documentElement.scrollWidth <= innerWidth"), true);
          continue;
        }
        const surface = mobile ? ".mobile-deck-card" : ".react-flow__node";
        await waitFor(surface + " .native-source-evidence");
        const cards =
          (await evaluate(`Array.from(document.querySelectorAll(${JSON.stringify(surface)}), el => ({
          id: el.dataset.screenId || el.dataset.id,
          text: el.textContent,
          dynamic: Array.from(el.querySelectorAll('.native-source-evidence > span')).some(row => row.textContent === 'Dynamic route'),
          unverified: Boolean(el.querySelector('[data-status="unverified"]')),
          images: el.querySelectorAll('img').length,
        }))`)) as Array<{
            id: string;
            text: string;
            unverified: boolean;
            dynamic: boolean;
            images: number;
          }>;
        const byId = new Map(cards.map((card) => [card.id, card]));
        assert.equal(cards.length, screens.length);
        for (const screen of screens) {
          const source = screen.nativeSource;
          assert(source, screen.path);
          const card = byId.get(screen.id);
          assert(card?.unverified, screen.path);
          assert.equal(card.images, 0);
          assert(card.text.includes(source.name));
          assert(card.text.includes(source.source + ":" + source.line));
          assert.equal(card.dynamic, source.dynamic === true);
          if (source.condition) assert(card.text.includes(source.condition));
        }
        assert.equal(await evaluate("document.documentElement.scrollWidth <= innerWidth"), true);
        const selected =
          screens.find((s) => s.nativeSource?.condition || s.nativeSource?.dynamic) ?? screens[0];
        const selector = surface + (mobile ? " button" : "");
        await evaluate(`Array.from(document.querySelectorAll(${JSON.stringify(selector)})).find(el => {
          const card = el.closest(${JSON.stringify(surface)});
          return (card.dataset.screenId || card.dataset.id) === ${JSON.stringify(selected.id)};
        }).click()`);
        await waitFor(".inspector .native-source-evidence");
        assert.equal(
          await evaluate("document.querySelector('.panel-heading h2').textContent"),
          selected.nativeSource?.name,
        );
        assert.equal(
          await evaluate(
            "document.querySelector('.inspector .native-source-evidence code').textContent",
          ),
          selected.nativeSource!.source + ":" + selected.nativeSource!.line,
        );
        assert.equal(
          await evaluate("new URLSearchParams(location.search).get('screen')"),
          selected.id,
        );
        assert.equal(
          await evaluate(
            "document.querySelectorAll('.inspector img, .inspector .cell-row, .inspector > label select').length",
          ),
          0,
        );
        assert.equal(await evaluate("document.documentElement.scrollWidth <= innerWidth"), true);
        if (mobile) {
          await evaluate(
            "document.querySelector('.inspector button[aria-label=\"Back to overview\"]').click()",
          );
          await evaluate(
            "Array.from(document.querySelectorAll('[role=tab]')).find(el => el.textContent.trim() === '2D Map').click()",
          );
          await waitFor(".react-flow__node .native-source-evidence");
          assert.equal(
            await evaluate(
              "document.querySelectorAll('.react-flow__node .native-source-evidence').length",
            ),
            screens.length,
          );
        }
      }
    });
  } finally {
    viewer.kill();
  }
}

if (import.meta.main) {
  assert(process.argv.length > 2, "Pass a source-only native map archive directory");
  for (const archiveDir of process.argv.slice(2)) {
    await verifyNativeMapViewer(archiveDir);
    console.log("Verified native map in desktop and mobile Chromium:", archiveDir);
  }
}
