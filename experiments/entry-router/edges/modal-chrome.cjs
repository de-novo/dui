const { chromium } = require("playwright-core"),
  fs = require("node:fs"),
  path = require("node:path");
(async () => {
  const [url, chrome, out] = process.argv.slice(2),
    b = await chromium.launch({ executablePath: chrome }),
    log = [];
  try {
    for (const width of [1440, 390]) {
      const p = await b.newPage({ viewport: { width, height: width === 1440 ? 920 : 844 } });
      p.setDefaultTimeout(6000);
      await p.goto(url + "/invoices");
      await p.getByRole("link", { name: "Add", exact: true }).waitFor();
      await p.screenshot({ path: path.join(out, "modal-" + width + "-list.png") });
      for (const name of ["Add", "Edit"]) {
        await p.getByRole("link", { name, exact: true }).first().click();
        const dialog = p.getByRole("dialog");
        await dialog.waitFor();
        const dest = new URL(p.url()).pathname;
        const hidden = (await p.getByRole("link", { name: "Add", exact: true }).count()) === 0;
        await p.screenshot({ path: path.join(out, "modal-" + width + "-" + name + ".png") });
        await dialog.getByRole("button", { name: "Cancel" }).click();
        await p.waitForURL((u) => u.pathname === "/invoices");
        log.push({
          width,
          dest,
          returned: true,
          backgroundAddExcludedFromAccessibilityTree: hidden,
        });
      }
      await p.close();
    }
  } finally {
    fs.writeFileSync(path.join(out, "modal-chrome.json"), JSON.stringify(log, null, 2));
    await b.close();
  }
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
