// Pass the local viewer URL, Chrome executable, and output directory.
const { chromium } = require("playwright-core");
const assert = require("node:assert/strict");
const path = require("node:path");
(async () => {
  const [url, executablePath, out] = process.argv.slice(2);
  const browser = await chromium.launch({ executablePath });
  try {
    for (const [width, height] of [
      [1440, 920],
      [390, 844],
    ]) {
      const page = await browser.newPage({ viewport: { width, height }, locale: "en-US" });
      const view = () => new URL(page.url()).searchParams.get("view");
      const shot = (name) =>
        page.screenshot({ path: path.join(out, `query-${width}-${name}.png`) });
      await page.goto(url);
      await page.locator(".workspace-switch").click();
      await page.locator('a[href="?view=model"]:visible').first().click();
      await page.locator(".model-workbench").waitFor();
      assert.equal(view(), "model");
      await shot("model");
      await page.locator('.model-workbench a[href="?"]').click();
      await page.locator(".workspace-switch").waitFor();
      assert.equal(view(), null);
      if (width === 390 && !(await page.locator(".map-options").isVisible())) {
        const map = page.getByRole("tab", { name: "2D Map", exact: true });
        await map.click();
      }
      await page.locator(".map-options:visible").first().waitFor();
      await shot("map");
      await page.locator(".map-options:visible").first().click();
      await page.getByRole("button", { name: "Compare captures" }).click();
      await page.waitForURL((u) => u.searchParams.get("view") === "diff");
      await page.locator(".diff-workspace").waitFor();
      await shot("diff");
      await page.keyboard.press("Escape");
      await page.waitForURL((u) => !u.searchParams.has("view"));
      await page.locator(".workspace-switch").waitFor();
      console.log(width, ["map→model", "model→map", "map→diff", "diff→map"]);
      await page.close();
    }
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
