import assert from "node:assert/strict";
import { once } from "node:events";
import type { ChromiumPage } from "./chromium-page.mts";

export const viewerViewports = [
  { width: 390, height: 844, mobile: true, requireSelector: ".mobile-deck-card img" },
  { width: 1440, height: 920, mobile: false, requireSelector: ".screen-card img" },
];

async function graphRequest(ws: WebSocket): Promise<string> {
  const signal = AbortSignal.timeout(15_000);
  while (true) {
    const [event] = await once(ws, "message", { signal });
    const message = JSON.parse(String(event.data));
    if (message.method === "Fetch.requestPaused") return message.params.requestId;
  }
}

export async function verifyViewerThemes(page: ChromiumPage, url: string, card: string) {
  const { call, ws } = page;
  async function evaluate(expression: string) {
    const response = await call("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
      timeout: 15000,
    });
    if (response.exceptionDetails) throw Error(JSON.stringify(response.exceptionDetails));
    return (response.result as { value: unknown }).value;
  }
  const setSystemTheme = (value: string) =>
    call("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value }] });
  const computedTheme = `({
    scheme: getComputedStyle(document.documentElement).colorScheme,
    background: getComputedStyle(document.body).backgroundColor,
  })`;
  async function changeSystemTheme(value: string) {
    await evaluate(`void (window.mediaChange = new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error("No color scheme change event")), 10000);
      matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
        clearTimeout(timer);
        requestAnimationFrame(() => resolve(${computedTheme}));
      }, { once: true });
    }))`);
    await setSystemTheme(value);
    return evaluate("window.mediaChange");
  }
  const results = [];
  const { identifier } = await call("Page.addScriptToEvaluateOnNewDocument", {
    source: `
      window.firstViewerFrame = new Promise(resolve => {
        function sample() {
          if (document.body) resolve(${computedTheme});
          else requestAnimationFrame(sample);
        }
        requestAnimationFrame(sample);
      });
    `,
  });
  await call("Fetch.enable", {
    patterns: [{ urlPattern: "*/graph.json*", resourceType: "Fetch" }],
  });
  try {
    for (const system of ["dark", "light"] as const) {
      await setSystemTheme(system);
      const opposite = system === "dark" ? "light" : "dark";
      for (const preference of ["system", opposite]) {
        const expected = preference === "system" ? system : preference;
        const scenario = `system=${system}, preference=${preference}`;
        const [requestId] = await Promise.all([graphRequest(ws), call("Page.navigate", { url })]);
        const first = (await evaluate("window.firstViewerFrame")) as {
          scheme: string;
          background: string;
        };
        assert.equal(first.scheme, expected, `Wrong theme on the first frame: ${scenario}`);
        const pending = await evaluate(computedTheme);
        assert.equal(
          await evaluate(`Boolean(document.querySelector(${JSON.stringify(card)}))`),
          false,
        );
        const switched = await changeSystemTheme(opposite);
        await changeSystemTheme(system);
        await call("Fetch.continueRequest", { requestId });
        const loaded = (await evaluate(`(async () => {
          const start = performance.now();
          while (!document.querySelector(${JSON.stringify(card)})) {
            if (performance.now() - start > 15000) throw Error("Archive cards did not appear");
            await new Promise(requestAnimationFrame);
          }
          return ${computedTheme};
        })()`)) as typeof first;
        assert.notEqual(loaded.background, "rgba(0, 0, 0, 0)", scenario);
        assert.deepEqual(first, loaded, `First frame changed: ${scenario}`);
        assert.deepEqual(pending, loaded, `Pending archive changed: ${scenario}`);
        assert.equal(
          (switched as typeof loaded).scheme,
          preference === "system" ? opposite : preference,
          scenario,
        );
        // Persist through the real control. The next navigation exercises its saved selection.
        const next = preference === "system" ? opposite : "system";
        const selected = await evaluate(`(() => {
          const picker = document.querySelector('select:has(option[value="system"])');
          const previous = picker.value;
          picker.value = ${JSON.stringify(next)};
          picker.dispatchEvent(new Event("change", { bubbles: true }));
          return previous;
        })()`);
        assert.equal(selected, preference, scenario);
        results.push({ system, preference, ...loaded });
      }
    }
    assert.equal(
      new Set(results.map((r) => r.background)).size,
      2,
      "Expected distinct theme backgrounds",
    );
    return results;
  } finally {
    await call("Page.removeScriptToEvaluateOnNewDocument", { identifier });
    await call("Fetch.disable");
    await call("Emulation.setEmulatedMedia", { features: [] });
  }
}
