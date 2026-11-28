#!/usr/bin/env node
// Capture reference screens with explicit data-state rewrites and pre-screenshot assertions.
// This harness never copies reference output into the candidate model path.
import { createRequire } from "node:module";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const shared = createRequire(import.meta.url);
const { chromium } = shared("playwright-core");

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? process.argv[i + 1] : fallback;
}
const target = arg("target", "calcom");
const url = arg("url", process.env.STAGE3_REFERENCE_URL);
const storageState = arg("storage-state", process.env.STAGE3_REFERENCE_STORAGE_STATE);
const out = path.resolve(arg("out", `/tmp/dui-stage3-references/${target}`));
const chrome = process.env.CHROME_PATH || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
if (!url) throw new Error("--url or STAGE3_REFERENCE_URL is required");
fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });

const builtinStates = {
  calcom: {
    populated: { text: ["Benchmark Intro", "Benchmark Deep Dive"], absentText: ["Create your first event type"] },
    loading: { selectors: [".animate-pulse"], absentText: ["Benchmark Intro", "Benchmark Deep Dive", "Create your first event type"] },
    empty: { text: ["Create your first event type"], absentText: ["Benchmark Intro", "Benchmark Deep Dive"] },
    // Cal.com's event-type query has no rendered error sentinel. The harness therefore requires
    // the response status and negative visible evidence rather than pretending that shell pixels prove error rendering.
    error: { absentText: ["Benchmark Intro", "Benchmark Deep Dive", "Create your first event type"], requireStatus: 500 },
  },
  plane: {
    populated: { text: ["Benchmark issue one", "Benchmark issue two"] },
    loading: { selectors: [".animate-pulse", "[class*='animate']"], absentText: ["Benchmark issue one", "Benchmark issue two"] },
    empty: { absentText: ["Benchmark issue one", "Benchmark issue two"] },
    error: { absentText: ["Benchmark issue one", "Benchmark issue two"], requireStatus: 500 },
  },
}[target];
const external = process.env.STAGE3_REFERENCE_ADAPTER
  ? (await import(pathToFileURL(path.resolve(process.env.STAGE3_REFERENCE_ADAPTER)).href)).default
  : undefined;
const states = external?.states ?? builtinStates;
if (!states) throw new Error(`No reference adapter for ${target}; provide STAGE3_REFERENCE_ADAPTER`);

const viewports = [
  { name: "desktop", width: 1440, height: 920 },
  { name: "mobile", width: 390, height: 844 },
];
const sha256 = (value) => crypto.createHash("sha256").update(value).digest("hex");

function calcomDataUrl(requestUrl) {
  return requestUrl.includes("/api/trpc/eventTypes/getEventTypesFromGroup") && requestUrl.includes("batch=1");
}
function emptyCalcomPayload(body) {
  const json = JSON.parse(body);
  const row = Array.isArray(json) ? json[0] : json?.["0"];
  if (!row?.result?.data?.json) {
    throw new Error(`unexpected Cal.com tRPC envelope: ${body.slice(0, 160)}`);
  }
  row.result.data.json.eventTypes = [];
  if ("nextCursor" in row.result.data.json) row.result.data.json.nextCursor = null;
  return JSON.stringify(json);
}
function planeDataUrl(requestUrl) {
  return requestUrl.includes("/issues/") && requestUrl.includes("group_by=");
}
function emptyPlanePayload(body) {
  const json = JSON.parse(body);
  if (Array.isArray(json?.results)) json.results = [];
  else if (json?.results && typeof json.results === "object") {
    for (const key of Object.keys(json.results)) json.results[key] = [];
  } else if (Array.isArray(json?.grouped_by)) json.grouped_by = [];
  else if ("results" in (json ?? {})) json.results = {};
  json.count = 0;
  json.total_count = 0;
  json.total_results = 0;
  json.next_page_results = false;
  return JSON.stringify(json);
}
const adapter = external?.adapter ?? (target === "calcom"
  ? { match: calcomDataUrl, empty: emptyCalcomPayload }
  : { match: planeDataUrl, empty: emptyPlanePayload });

const browser = await chromium.launch({ executablePath: chrome, headless: true });
const report = { target, url, schemaVersion: 1, states: {} };
for (const [state, assertion] of Object.entries(states)) {
  report.states[state] = { viewports: {}, assertion };
  for (const viewport of viewports) {
    const requests = [];
    const context = await browser.newContext({
      viewport,
      storageState: storageState && fs.existsSync(storageState) ? storageState : undefined,
      bypassCSP: true,
    });
    await context.route("**/*", async (route) => {
      const request = route.request();
      if (!adapter.match(request.url())) return route.continue();
      const started = Date.now();
      if (!adapter.statefulUpstream && state === "loading") {
        requests.push({ url: request.url(), action: "held-open" });
        return new Promise(() => {});
      }
      if (!adapter.statefulUpstream && state === "error") {
        requests.push({ url: request.url(), action: "rewritten-error", status: 500, durationMs: Date.now() - started });
        return route.fulfill({ status: 500, contentType: "application/json", body: JSON.stringify({ error: "benchmark forced error" }) });
      }
      const response = await route.fetch();
      const body = await response.text();
      let outBody = body;
      let action = adapter.statefulUpstream ? "observed-stateful-upstream" : "observed";
      if (!adapter.statefulUpstream && state === "empty") {
        outBody = adapter.empty(body);
        action = "rewritten-empty";
      }
      requests.push({
        url: request.url(),
        action,
        status: response.status(),
        requestSha256: sha256(request.postData() ?? ""),
        responseSha256: sha256(outBody),
        responseBytes: Buffer.byteLength(outBody),
        durationMs: Date.now() - started,
      });
      return route.fulfill({ response, body: outBody });
    });
    if (external?.cookies) await context.addCookies(external.cookies(state, viewport, url));
    const page = await context.newPage();
    await page.goto(url, { waitUntil: state === "loading" ? "domcontentloaded" : "networkidle", timeout: 30000 }).catch((error) => {
      if (state !== "loading") throw error;
    });
    await page.waitForTimeout(state === "loading" ? 1500 : 750);
    const visible = await page.evaluate(() => ({
      text: document.body.innerText.replace(/\s+/g, " ").trim(),
      selectorMatches: (selectors) => selectors.map((selector) => [...document.querySelectorAll(selector)].some((el) => {
        const box = el.getBoundingClientRect();
        const style = getComputedStyle(el);
        return box.width > 0 && box.height > 0 && style.visibility !== "hidden" && style.display !== "none";
      })),
    }));
    const selectorResults = assertion.selectors?.length
      ? await page.evaluate((selectors) => selectors.map((selector) => [...document.querySelectorAll(selector)].some((el) => {
          const box = el.getBoundingClientRect();
          const style = getComputedStyle(el);
          return box.width > 0 && box.height > 0 && style.visibility !== "hidden" && style.display !== "none";
        })), assertion.selectors)
      : [];
    const missingText = (assertion.text ?? []).filter((text) => !visible.text.includes(text));
    const forbiddenText = (assertion.absentText ?? []).filter((text) => visible.text.includes(text));
    const missingSelectors = (assertion.selectors ?? []).filter((_, index) => !selectorResults[index]);
    const statusOk = !assertion.requireStatus || requests.some((request) => request.status === assertion.requireStatus);
    const passed = !missingText.length && !forbiddenText.length && !missingSelectors.length && statusOk && (adapter.allowNoBrowserRequests || requests.length > 0);
    const png = path.join(out, `${target}-${state}-${viewport.name}.png`);
    await page.screenshot({ path: png, fullPage: false });
    report.states[state].viewports[viewport.name] = {
      png: path.basename(png),
      passed,
      missingText,
      forbiddenText,
      missingSelectors,
      textSha256: sha256(visible.text),
      textLength: visible.text.length,
      requests,
    };
    await Promise.race([context.close(), new Promise((resolve) => setTimeout(resolve, 1000))]);
    if (!passed) {
      console.error(`${target}/${state}/${viewport.name} assertion failed`, { missingText, forbiddenText, missingSelectors, statusOk });
    }
  }
}
await Promise.race([browser.close(), new Promise((resolve) => setTimeout(resolve, 1000))]);
fs.writeFileSync(path.join(out, "reference-provenance.json"), JSON.stringify(report, null, 2));
console.log(JSON.stringify({ target, out, states: Object.fromEntries(Object.entries(report.states).map(([state, r]) => [state, Object.fromEntries(Object.entries(r.viewports).map(([viewport, v]) => [viewport, v.passed]))])) }, null, 2));

process.exit(0);
