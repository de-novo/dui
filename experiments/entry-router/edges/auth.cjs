const { chromium } = require("playwright-core"),
  fs = require("node:fs"),
  path = require("node:path");
(async () => {
  const [chrome, url, out] = process.argv.slice(2),
    b = await chromium.launch({ executablePath: chrome }),
    rows = [];
  try {
    for (const width of [1440, 390]) {
      const p = await b.newPage({ viewport: { width, height: width === 390 ? 844 : 920 } });
      p.setDefaultTimeout(9000);
      const at = async (to, from, action) => {
        await p.waitForURL((u) => u.pathname === to);
        rows.push({ width, from, to, action });
      };
      const link = (name) =>
        p.getByRole("link", { name, exact: true }).filter({ visible: true }).first();
      const button = (name) =>
        p.getByRole("button", { name, exact: true }).filter({ visible: true }).first();
      const email = (admin) => (admin ? "owner" : "member") + "@example.test",
        pw = "local-fixture-only-123";
      async function logout() {
        if (width === 390) await p.keyboard.press("Escape");
        await button("Open user menu").click();
        await p.getByRole("menuitem", { name: "Sign Out" }).click();
      }
      async function register(admin) {
        await link("Register").click();
        await at("/auth/register", "/auth/login", "register link");
        for (const [k, v] of [
          ["First Name", admin ? "Owner" : "Member"],
          ["Last Name", "Probe"],
          ["Email Address", email(admin)],
          ["Password", pw],
        ])
          await p.getByLabel(k).fill(v);
        if (admin) await p.getByLabel("Team Name").fill("Local research");
        else {
          await p.locator("#choose-team").click();
          await p.locator("select").selectOption({ label: "Local research" });
        }
        await button("Register").click();
        await at("/app", "/auth/register", admin ? "new team form" : "join team form");
      }
      await p.goto(url + "/app");
      await at("/auth/login", "/app", "anonymous guard");
      await register(true);
      if (width === 390) await button("Toggle Menu").click();
      await link("Users").click();
      await at("/app/users", "/app", "admin menu");
      await p.screenshot({ path: path.join(out, "auth-" + width + "-admin.png") });
      await logout();
      await at("/auth/login", "/app/users", "sign out");
      await p.getByLabel("Email Address").fill(email(true));
      await p.getByLabel("Password").fill(pw);
      await button("Log in").click();
      await at("/app/users", "/auth/login", "login form with return URL");
      await logout();
      await at("/auth/login", "/app/users", "sign out");
      await p.goto(url);
      await button("Get started").click();
      await register(false);
      if (width === 390) await button("Toggle Menu").click();
      await link("Discussions").waitFor();
      if (await link("Users").count()) throw Error("member sees admin link");
      rows.push({ width, from: "/app", action: "member menu", usersLink: false });
      await p.screenshot({ path: path.join(out, "auth-" + width + "-member.png") });
      await p.close();
    }
  } finally {
    fs.writeFileSync(path.join(out, "auth.json"), JSON.stringify(rows, null, 2));
    await b.close();
  }
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
