import assert from "node:assert/strict";
import type { ChromiumPage } from "./chromium-page.mts";

// Runs in the viewer so the checks use its current language, fonts and composed hit targets.
async function checkMapControls(mobile: boolean) {
  const issues: string[] = [];
  const frame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  async function waitFor<T extends HTMLElement>(selector: string): Promise<T> {
    const deadline = performance.now() + 10000;
    while (performance.now() < deadline) {
      const el = document.querySelector<T>(selector);
      if (el) return el;
      await frame();
    }
    throw Error("Missing " + selector);
  }
  function checkTargets(group: string, elements: Element[]) {
    const targets = elements.map((el) => ({
      el,
      name: el.getAttribute("aria-label") || el.textContent?.trim() || el.tagName,
      rect: el.getBoundingClientRect(),
    }));
    for (const [index, { el, name, rect: a }] of targets.entries()) {
      const issue = (message: string) =>
        issues.push(document.documentElement.lang + " " + group + " " + name + ": " + message);
      if (
        !a.width ||
        !a.height ||
        a.left < 0 ||
        a.top < 0 ||
        a.right > innerWidth + 0.5 ||
        a.bottom > innerHeight + 0.5
      ) {
        issue("outside viewport");
      }
      if (!el.contains(document.elementFromPoint(a.x + a.width / 2, a.y + a.height / 2))) {
        issue("center covered");
      }
      for (const { name: other, rect: b } of targets.slice(index + 1)) {
        if (a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom)
          issue("overlaps " + other);
      }
    }
  }
  const language = await waitFor<HTMLSelectElement>(
    '.rail select:has(option[value="' + CSS.escape(document.documentElement.lang) + '"])',
  );
  await waitFor(mobile ? ".mobile-deck-card img" : ".screen-card img");
  await document.fonts.ready;
  const original = language.value;
  const locales = [...language.options].map((option) => option.value);
  const otherTab = '.mobile-view-switch [role="tab"][aria-selected="false"]';
  for (const locale of locales) {
    language.value = locale;
    language.dispatchEvent(new Event("change", { bubbles: true }));
    await frame();
    if (document.documentElement.lang !== locale)
      throw Error("Language did not switch to " + locale);
    if (mobile) {
      const deck = await waitFor(".mobile-deck-toolbar");
      checkTargets("header", [
        ...document.querySelectorAll(".rail .workspace-switch, .rail select"),
      ]);
      checkTargets("deck", [...deck.querySelectorAll("button")]);
      (await waitFor(otherTab)).click();
    }
    const controls = await waitFor(".react-flow__controls");
    const options = await waitFor(".map-options");
    const credit = await waitFor(".react-flow__attribution a");
    const buttons = [...controls.querySelectorAll("button")];
    if (!buttons.length) throw Error("Map navigation is empty");
    checkTargets("map", [
      ...buttons,
      options,
      credit,
      ...document.querySelectorAll(".mobile-map-return button"),
    ]);
    const row = options.getBoundingClientRect();
    const nav = controls.getBoundingClientRect();
    if (Math.abs(nav.bottom - row.bottom) > 0.5 || (mobile && Math.abs(nav.top - row.top) > 0.5)) {
      issues.push(
        locale +
          " map toolbar edges differ: controls " +
          [nav.top, nav.bottom] +
          ", options " +
          [row.top, row.bottom],
      );
    }
    if (credit.parentElement!.getBoundingClientRect().top < Math.max(nav.bottom, row.bottom)) {
      issues.push(locale + " attribution intersects toolbar row");
    }
    options.focus();
    options.click();
    const close = await waitFor('[role="dialog"] [data-slot="dialog-close"]');
    const dialog = close.closest<HTMLElement>('[role="dialog"]')!;
    await Promise.all(dialog.getAnimations().map((animation) => animation.finished));
    checkTargets("options", [...dialog.querySelectorAll("select, button")]);
    close.focus();
    close.click();
    await waitFor('.map-options[aria-expanded="false"]:focus');
    if (mobile) {
      (await waitFor(otherTab)).click();
      await waitFor(".mobile-deck-card img");
    }
  }
  language.value = original;
  language.dispatchEvent(new Event("change", { bubbles: true }));
  return { locales, issues };
}

// Compare the camera after a continuous resize with the viewer's own Fit view action.
async function verifyMapResize(page: ChromiumPage) {
  async function camera(action?: string) {
    const response = await page.call("Runtime.evaluate", {
      expression: `(async () => {
        const frame = () => new Promise(requestAnimationFrame);
        const action = ${JSON.stringify(action ?? "")};
        if (action) document.querySelector(action).click();
        const deadline = performance.now() + 10000;
        let previous, stable = 0;
        while (performance.now() < deadline) {
          await frame();
          const viewport = document.querySelector(".react-flow__viewport");
          if (!viewport || !document.querySelector(".screen-card img")) continue;
          const current = getComputedStyle(viewport).transform;
          stable = previous === current ? stable + 1 : 0;
          if (stable >= 6) return current;
          previous = current;
        }
        throw Error("Map camera did not settle");
      })()`,
      awaitPromise: true,
      returnByValue: true,
    });
    if (response.exceptionDetails) throw Error(JSON.stringify(response.exceptionDetails));
    return (response.result as { value: string }).value;
  }
  const phone = await camera('.mobile-deck [role="tab"][aria-selected="false"]');
  await page.call("Emulation.setDeviceMetricsOverride", {
    width: 1440,
    height: 920,
    mobile: false,
    deviceScaleFactor: 1,
  });
  const expanded = await camera();
  assert.notEqual(expanded, phone, "Map retained the phone camera on desktop");
  assert.equal(
    expanded,
    await camera(".react-flow__controls-fitview"),
    "Resized overview differs from Fit view",
  );
  const manual = await camera(
    ".react-flow__controls-zoomin:not(:disabled), .react-flow__controls-zoomout:not(:disabled)",
  );
  assert.notEqual(manual, expanded, "Zoom control did not move the camera");
  await page.call("Emulation.setDeviceMetricsOverride", {
    width: 390,
    height: 844,
    mobile: true,
    deviceScaleFactor: 2,
  });
  assert.equal(await camera(), manual, "Resize reset manual zoom");
}

export async function verifyMapControls(page: ChromiumPage, url: string) {
  await page.call("Page.bringToFront");
  const failures: string[] = [];
  const results = [];
  for (const { width, height, mobile } of [
    { width: 320, height: 568, mobile: true },
    { width: 390, height: 844, mobile: true },
    { width: 768, height: 390, mobile: true },
    { width: 1440, height: 920, mobile: false },
  ]) {
    await page.call("Emulation.setDeviceMetricsOverride", {
      width,
      height,
      mobile,
      deviceScaleFactor: mobile ? 2 : 1,
    });
    await page.call("Page.navigate", { url });
    const response = await page.call("Runtime.evaluate", {
      expression: `(${checkMapControls.toString()})(${mobile})`,
      awaitPromise: true,
      returnByValue: true,
      timeout: 30000,
    });
    if (response.exceptionDetails) {
      failures.push(width + "×" + height + ": " + JSON.stringify(response.exceptionDetails));
      continue;
    }
    const { locales, issues } = (
      response.result as { value: Awaited<ReturnType<typeof checkMapControls>> }
    ).value;
    failures.push(...issues.map((issue) => width + "×" + height + " " + issue));
    if (width === 390) await verifyMapResize(page);
    results.push({ width, height, locales });
  }
  if (failures.length) throw Error("Map controls:\n" + failures.join("\n"));
  return results;
}
