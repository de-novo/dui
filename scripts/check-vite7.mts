import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

const repoRoot = process.cwd();
const duiBin = path.join(repoRoot, "target", "debug", "dui");
const toggleDir = path.join(repoRoot, "fixtures", "toggle");
const toggleNodeModules = path.join(toggleDir, "node_modules");

assert.ok(fs.existsSync(duiBin), `missing built dui binary at ${duiBin}`);
assert.ok(fs.existsSync(toggleNodeModules), `missing fixtures/toggle/node_modules`);

interface RunReport {
  coverage: { verified: number; renderable: number };
  cells: Array<{ path: string; width: number; status: string }>;
}

function runDui(
  args: string[],
  homeDir: string,
): { status: number | null; stdout: string; stderr: string; json: unknown } {
  const res = spawnSync(duiBin, args, {
    encoding: "utf8",
    env: { ...process.env, HOME: homeDir },
  });
  let json: unknown = null;
  if (res.stdout.trim()) {
    try {
      json = JSON.parse(res.stdout) as unknown;
    } catch {
      json = null;
    }
  }
  return { status: res.status, stdout: res.stdout, stderr: res.stderr, json };
}

function makeFakeViteProject(destDir: string, viteVersion: string): void {
  fs.mkdirSync(path.join(destDir, "node_modules"), { recursive: true });
  for (const file of ["dui.json", "hooks.ts", "index.html", "main.tsx", "tsconfig.json"]) {
    fs.copyFileSync(path.join(toggleDir, file), path.join(destDir, file));
  }
  const pkg = JSON.parse(fs.readFileSync(path.join(toggleDir, "package.json"), "utf8")) as {
    devDependencies: Record<string, string>;
  };
  pkg.devDependencies.vite = viteVersion;
  fs.writeFileSync(path.join(destDir, "package.json"), JSON.stringify(pkg, null, 2));

  for (const entry of fs.readdirSync(toggleNodeModules)) {
    if (entry === "vite") continue;
    fs.symlinkSync(
      path.join(toggleNodeModules, entry),
      path.join(destDir, "node_modules", entry),
      "dir",
    );
  }

  // Simulate Vite 7: expose Vite's bundler APIs (`build`, `createBuilder`) without Vite 8's Oxc `parseSync` / `Visitor`.
  const realViteEntry = pathToFileURL(
    path.join(toggleNodeModules, "vite", "dist", "node", "index.js"),
  ).href;
  const fakeViteDir = path.join(destDir, "node_modules", "vite");
  fs.mkdirSync(fakeViteDir, { recursive: true });
  fs.writeFileSync(
    path.join(fakeViteDir, "package.json"),
    JSON.stringify(
      {
        name: "vite",
        version: viteVersion,
        type: "module",
        exports: {
          ".": "./index.mjs",
          "./package.json": "./package.json",
        },
      },
      null,
      2,
    ),
  );
  fs.writeFileSync(
    path.join(fakeViteDir, "index.mjs"),
    `import { build, createBuilder } from ${JSON.stringify(realViteEntry)};\nexport { build, createBuilder };\n`,
  );
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-vite7-"));
const duiHome = path.join(tmp, ".dui-home");

try {
  // 1. Vite 7.3.1 end-to-end scan + instrumented build + Chromium CDP capture (no vite.parseSync/Visitor).
  const vite7App = path.join(tmp, "vite7-app");
  makeFakeViteProject(vite7App, "7.3.1");

  const scan7 = runDui(["scan", vite7App, path.join(vite7App, "dui.json")], duiHome);
  assert.equal(scan7.status, 0, `expected scan to accept Vite 7.3.1, stderr: ${scan7.stderr}`);
  const scanFacts = scan7.json as {
    versions: Record<string, string>;
    facts: Array<{ kind: string }>;
  };
  assert.equal(scanFacts.versions.vite, "7.3.1", "expected versions.vite to record 7.3.1");
  assert.ok(scanFacts.facts.length > 0, "expected extracted facts on Vite 7 app");

  const out7 = path.join(tmp, "out-vite7");
  const run7 = runDui(["run", vite7App, path.join(vite7App, "dui.json"), out7], duiHome);
  assert.equal(
    run7.status,
    0,
    `expected Vite 7 run to succeed, stdout: ${run7.stdout}, stderr: ${run7.stderr}`,
  );
  const report7 = JSON.parse(fs.readFileSync(path.join(out7, "report.json"), "utf8")) as RunReport;
  assert.ok(
    report7.coverage.verified >= 4,
    `expected >= 4 verified cells, got ${report7.coverage.verified}`,
  );
  assert.equal(
    report7.coverage.verified,
    report7.coverage.renderable,
    "expected all renderable cells to verify under Vite 7",
  );

  // 2. Vite 7 route discovery (`discover.mts` + `parser.mts` TypeScript fallback, no vite.parseSync/Visitor).
  const routesDir = path.join(vite7App, "src", "routes");
  fs.mkdirSync(routesDir, { recursive: true });
  fs.writeFileSync(
    path.join(routesDir, "orders.tsx"),
    `import { createFileRoute } from "@tanstack/react-router";
const VIEWS = { active: "active", archived: "archived" } as const;
function OrdersPage() {
  const href = (k: keyof typeof VIEWS) => \`/orders/\${VIEWS[k]}\`;
  return href("active");
}
export const Route = createFileRoute("/orders/$")({ component: OrdersPage });
export const HomeRoute = createFileRoute("/dashboard")({ component: OrdersPage });
`,
  );
  // Compile adapters into a temp dir so we can run discover.mjs & parser parity checks directly.
  const adapterOut = path.join(tmp, "adapters-out");
  const tscBin = path.join(repoRoot, "node_modules", "typescript", "bin", "tsc");
  const tscRes = spawnSync(
    process.execPath,
    [tscBin, "--project", path.join(repoRoot, "tsconfig.json"), "--outDir", adapterOut],
    { encoding: "utf8" },
  );
  assert.equal(tscRes.status, 0, `tsc failed: ${tscRes.stderr}`);
  fs.copyFileSync(
    path.join(repoRoot, "adapters", "vite", "probe-helpers.ts"),
    path.join(adapterOut, "vite", "probe-helpers.ts"),
  );

  const discoverRes = spawnSync(
    process.execPath,
    [
      path.join(adapterOut, "vite", "discover.mjs"),
      vite7App,
      fs.readFileSync(path.join(repoRoot, "rules", "web.json"), "utf8"),
    ],
    { encoding: "utf8" },
  );
  assert.equal(discoverRes.status, 0, `discover.mjs failed under Vite 7: ${discoverRes.stderr}`);
  const discovered = JSON.parse(discoverRes.stdout) as {
    path: string;
    scenarios: Array<{ path: string }>;
    screenDiscovery: { mode: string; parseFailures: string[] };
  };
  const discoveredPaths = [discovered.path, ...discovered.scenarios.map((s) => s.path)].sort();
  assert.equal(discovered.screenDiscovery.mode, "router");
  assert.deepEqual(discovered.screenDiscovery.parseFailures, []);
  assert.deepEqual(discoveredPaths, ["/dashboard", "/orders/active", "/orders/archived"]);

  // 2b. React Router 7/8 static route discovery (`createBrowserRouter` + JSX `<Route>`) under BOTH Vite 7 and Vite 8.
  const rrApp = path.join(tmp, "rr-discovery-app");
  makeFakeViteProject(rrApp, "7.3.1");
  fs.writeFileSync(
    path.join(rrApp, "dui.json"),
    JSON.stringify({
      routeParams: { id: "42" },
      scenarios: [{ path: "/extra-standalone", readySelector: "main" }],
    }),
  );
  fs.mkdirSync(path.join(rrApp, "src"), { recursive: true });
  fs.writeFileSync(
    path.join(rrApp, "src", "router.tsx"),
    `import { createBrowserRouter, Navigate, Route, Routes } from "react-router";
import { reportRoutes } from "./reports";
const nestedRoutes = [
  { path: "/old-home", element: <Navigate to="/" replace /> },
  { path: "/billing", children: [{ path: "invoices" }, { path: "plans" }] },
  { path: "/reports", children: reportRoutes },
  {
    path: "/",
    children: [
      { index: true },
      { path: "login" },
      { path: "account", element: <Navigate to="../login" replace /> },
      {
        path: "items",
        children: [
          { index: true },
          { path: ":id" },
          { path: ":missingParam" },
        ],
      },
    ],
  },
];
export const router = createBrowserRouter(nestedRoutes);
export function AppRoutes() {
  return (
    <Routes>
      <Route path="/legacy-settings" element={<Navigate to="profile" />} />
      <Route path="/settings">
        <Route index />
        <Route path="profile" />
        <Route path={"members/:id"} />
      </Route>
    </Routes>
  );
}
`,
  );
  const rrDiscover7 = spawnSync(
    process.execPath,
    [
      path.join(adapterOut, "vite", "discover.mjs"),
      rrApp,
      fs.readFileSync(path.join(repoRoot, "rules", "web.json"), "utf8"),
    ],
    { encoding: "utf8" },
  );
  assert.equal(
    rrDiscover7.status,
    0,
    `React Router discover failed (Vite 7): ${rrDiscover7.stderr}`,
  );
  const rrParsed7 = JSON.parse(rrDiscover7.stdout) as {
    path: string;
    scenarios: Array<{ path: string }>;
    screenDiscovery: {
      mode: string;
      unresolved: string[];
      parseFailures: string[];
      layouts?: Record<string, string>;
      redirects?: Record<string, { to: string }>;
    };
  };
  const rrPaths7 = [rrParsed7.path, ...rrParsed7.scenarios.map((s) => s.path)].sort();
  // Layout-only parents and static redirects are not screens; both stay listed.
  assert.deepEqual(Object.keys(rrParsed7.screenDiscovery.layouts ?? {}), ["/billing", "/reports"]);
  assert.deepEqual(
    Object.fromEntries(
      Object.entries(rrParsed7.screenDiscovery.redirects ?? {}).map(([from, r]) => [from, r.to]),
    ),
    { "/account": "/login", "/legacy-settings": "/legacy-settings/profile", "/old-home": "/" },
  );
  assert.equal(rrParsed7.screenDiscovery.mode, "router");
  assert.deepEqual(rrParsed7.screenDiscovery.parseFailures, []);
  assert.deepEqual(rrPaths7, [
    "/",
    "/billing/invoices",
    "/billing/plans",
    "/extra-standalone",
    "/items",
    "/items/42",
    "/login",
    "/settings",
    "/settings/members/42",
    "/settings/profile",
  ]);
  assert.deepEqual(rrParsed7.screenDiscovery.unresolved, ["/items/:missingParam", "/reports/*"]);

  // Verify `dui --help` prints grouped CLI guide and exits 0.
  const helpRes = runDui(["--help"], duiHome);
  assert.equal(helpRes.status, 0, "dui --help must exit 0");
  assert.ok(
    helpRes.stdout.includes("dui map") &&
      helpRes.stdout.includes("dui run") &&
      helpRes.stdout.includes("dui auth login") &&
      helpRes.stdout.includes("dui open"),
    `expected grouped CLI help output, got:\n${helpRes.stdout}`,
  );

  // Verify identical React Router discovery on Vite 8 (Oxc parser) too.
  fs.rmSync(path.join(rrApp, "node_modules", "vite"), { recursive: true, force: true });
  fs.symlinkSync(
    path.join(toggleNodeModules, "vite"),
    path.join(rrApp, "node_modules", "vite"),
    "dir",
  );
  const rrDiscover8 = spawnSync(
    process.execPath,
    [
      path.join(adapterOut, "vite", "discover.mjs"),
      rrApp,
      fs.readFileSync(path.join(repoRoot, "rules", "web.json"), "utf8"),
    ],
    { encoding: "utf8" },
  );
  assert.equal(
    rrDiscover8.status,
    0,
    `React Router discover failed (Vite 8): ${rrDiscover8.stderr}`,
  );
  const rrParsed8 = JSON.parse(rrDiscover8.stdout) as typeof rrParsed7;
  const rrPaths8 = [rrParsed8.path, ...rrParsed8.scenarios.map((s) => s.path)].sort();
  assert.deepEqual(
    rrPaths8,
    rrPaths7,
    "React Router discovery must match between Vite 7 and Vite 8",
  );
  assert.deepEqual(rrParsed8.screenDiscovery.unresolved, rrParsed7.screenDiscovery.unresolved);

  // 3. Byte-for-byte instrumentation parity between Vite 8 Oxc and Vite 7 TypeScript fallback.
  const realVite = (await import(
    pathToFileURL(path.join(toggleNodeModules, "vite", "dist", "node", "index.js")).href
  )) as {
    parseSync: (file: string, text: string) => { program: unknown; errors: unknown[] };
    Visitor: new (v: Record<string, (n: never) => void>) => { visit(p: unknown): void };
  };
  const { createAstParser } = (await import(
    pathToFileURL(path.join(adapterOut, "vite", "parser.mjs")).href
  )) as {
    createAstParser: (opts: { vite: object; app: string; root: string }) => Promise<{
      parseSync: (file: string, text: string) => { program: unknown; errors: unknown[] };
      Visitor: new (v: Record<string, (n: never) => void>) => { visit(p: unknown): void };
      close?: () => void;
    }>;
  };
  const { instrument } = (await import(
    pathToFileURL(path.join(adapterOut, "vite", "instrument.mjs")).href
  )) as {
    instrument: (text: string, facts: unknown[], parser: unknown) => string;
  };
  const sourceText = fs.readFileSync(path.join(vite7App, "main.tsx"), "utf8");
  const vite8Parser = await createAstParser({ vite: realVite, app: vite7App, root: vite7App });
  const vite7Parser = await createAstParser({ vite: {}, app: vite7App, root: vite7App });
  try {
    const outVite8 = instrument(sourceText, scanFacts.facts, vite8Parser);
    const outVite7 = instrument(sourceText, scanFacts.facts, vite7Parser);
    assert.equal(
      outVite7,
      outVite8,
      "Vite 7 fallback instrumentation must match Vite 8 byte-for-byte",
    );

    assert.throws(
      () => instrument("const broken = (((;", scanFacts.facts, vite7Parser),
      /cannot instrument invalid source/,
    );
  } finally {
    vite8Parser.close?.();
    vite7Parser.close?.();
  }

  // 4. Vite 6.4.0 must still be rejected honestly as unsupported (exit 4).
  const vite6App = path.join(tmp, "vite6-app");
  makeFakeViteProject(vite6App, "6.4.0");
  const scan6 = runDui(["scan", vite6App, path.join(vite6App, "dui.json")], duiHome);
  assert.equal(scan6.status, 4, `expected Vite 6.4.0 to exit 4, got ${scan6.status}`);
  const unsupportedPayload = JSON.parse(scan6.stdout || scan6.stderr) as {
    status: string;
    reason: string;
  };
  assert.equal(unsupportedPayload.status, "unsupported");
  assert.match(unsupportedPayload.reason, /vite requires stable 7\.x\+; found 6\.4\.0/);

  console.log(
    JSON.stringify({
      status: "ok",
      vite7Version: scanFacts.versions.vite,
      verifiedCells: report7.coverage.verified,
      discoveredPaths,
      instrumentParity: true,
      vite6Rejected: unsupportedPayload.reason,
    }),
  );
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
