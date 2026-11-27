import fs from "node:fs";
import path from "node:path";
import { execFileSync, spawn } from "node:child_process";
import { once } from "node:events";
import { createInterface } from "node:readline";
import { chromium } from "playwright-core";
import { entries } from "./edits.mjs";
const [dui, chrome, root, out, ...only] = process.argv.slice(2);
const env = { ...process.env, HOME: path.join(out, "home"), DUI_CHROMIUM: "/absent" };
fs.mkdirSync(env.HOME, { recursive: true });
const b = await chromium.launch({ executablePath: chrome }),
  results = {};
const state = ({ selector, prop }) => {
  const h = document.querySelector(".approximate-model-content"),
    e = h?.shadowRoot?.querySelector(selector);
  if (!e) return null;
  if (prop === "text") return e.textContent;
  if (["checked", "disabled", "value"].includes(prop)) return e[prop];
  return getComputedStyle(e)[prop];
};
try {
  for (const [id, edit] of Object.entries(entries)) {
    if (only.length && !only.includes(id)) continue;
    const scenario =
      {
        and: "true",
        ternary: "true",
        "map-key": "ABC",
        "default-prop": "omitted",
        spread: "before",
        "initial-state": "true",
        "class-composition": "true",
        clsx: "true",
        overflow: "hidden",
        checkbox: "true",
        button: "true",
      }[id] ?? "default";
    const src = path.join(root, id + "--" + scenario),
      dir = path.join(out, id);
    fs.mkdirSync(dir, { recursive: true });
    for (const f of fs.readdirSync(src))
      if (["App.tsx", "mark.png", "triangle.ttf"].includes(f) || f.endsWith(".css"))
        fs.copyFileSync(path.join(src, f), path.join(dir, f));
    const [before, after, selector, prop, expected, file = "App.tsx"] = edit,
      filename = path.join(dir, file);
    let content = fs.readFileSync(filename, "utf8");
    if (!content.includes(before)) throw Error("missing edit " + id);
    // Both widths must change regardless of whether the media rule is active.
    let changed = content.replace(before, after);
    if (id === "media-query") changed = changed.replace("padding:12px", "padding:16px");
    const archive = JSON.parse(execFileSync(dui, ["map", dir], { env }));
    const server = spawn(dui, ["serve", archive.output], {
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    const [url] = await once(createInterface({ input: server.stdout }), "line");
    const contexts = [];
    try {
      const prepared = await Promise.all(
        [1440, 390].map(async (width) => {
          const ctx = await b.newContext({
            viewport: { width, height: width === 390 ? 844 : 920 },
            locale: "en-US",
          });
          contexts.push(ctx);
          const page = await ctx.newPage();
          await page.goto(url);
          await page.waitForFunction(() =>
            document
              .querySelector(".approximate-model-content")
              ?.shadowRoot?.querySelector("[data-model-node]"),
          );
          await page.waitForTimeout(160);
          await page.addScriptTag({ content: "window.readFixtureState = " + state.toString() });
          const old = await page.evaluate(state, { selector, prop });
          return { page, width, old };
        }),
      );
      const wait = 2000;
      const start = performance.now();
      fs.writeFileSync(filename, changed);
      const trials = await Promise.all(
        prepared.map(async ({ page, width, old }) => {
          let status = "deadline",
            ms = null;
          try {
            await page.waitForFunction(
              ({ selector, prop, expected, old }) => {
                const v = window.readFixtureState({ selector, prop });
                if (v === old || v === null) return false;
                if (expected === "viewportWidth" || expected === "viewportHeight")
                  return parseFloat(v) > 0;
                return v === expected;
              },
              { selector, prop, expected, old },
              { timeout: wait, polling: "raf" },
            );
            await page.evaluate(
              () => new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok))),
            );
            ms = performance.now() - start;
            status = "reflected";
          } catch (e) {
            if (e.name !== "TimeoutError") throw e;
          }
          const final = await page.evaluate(state, { selector, prop });
          if (status === "deadline" && old === expected) status = "unchanged-already-equal";
          return { width, old, final, status, ms, observationEndMs: performance.now() - start };
        }),
      );
      results[id] = {
        scenario,
        file,
        before,
        after,
        selector,
        prop,
        expected,
        deadlineMs: wait,
        trials,
      };
      fs.writeFileSync(path.join(out, "live.json"), JSON.stringify(results, null, 2));
    } finally {
      await Promise.all(contexts.map((c) => c.close()));
      server.kill();
    }
  }
} finally {
  await b.close();
}
