const { chromium } = require("playwright-core"),
  fs = require("node:fs"),
  path = require("node:path");
(async () => {
  const [chrome, todo, bp, out] = process.argv.slice(2),
    b = await chromium.launch({ executablePath: chrome }),
    observed = [];
  try {
    for (const width of [1440, 390]) {
      const p = await b.newPage({ viewport: { width, height: width === 1440 ? 920 : 844 } });
      await p.goto(todo);
      await p.locator(".new-todo").fill("Navigation audit");
      await p.locator(".new-todo").press("Enter");
      for (const from of ["#/", "#/active", "#/completed"])
        for (const to of ["#/", "#/active", "#/completed"])
          if (from !== to) {
            await p.goto(todo + "/" + from);
            await p.locator('a[href="' + to + '"]').click();
            await p.waitForURL((u) => u.hash === to);
            observed.push({ width, app: "todo", from: "/" + from, to: "/" + to });
          }
      await p.screenshot({ path: path.join(out, "todo-" + width + ".png") });
      await p.goto(bp);
      await p.getByRole("button", { name: /get started/i }).click();
      await p.waitForURL((u) => u.pathname === "/auth/login");
      observed.push({ width, app: "bp", from: "/", to: "/auth/login" });
      await p.screenshot({ path: path.join(out, "bp-" + width + ".png") });
      await p.goto(bp + "/nonexistent-probe");
      await p.getByRole("link", { name: /home/i }).click();
      await p.waitForURL((u) => u.pathname === "/");
      observed.push({ width, app: "bp", from: "*", to: "/" });
      await p.close();
    }
  } finally {
    fs.writeFileSync(path.join(out, "browser.json"), JSON.stringify(observed, null, 2));
    await b.close();
  }
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
