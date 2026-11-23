import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const repoRoot = process.cwd();
const duiBin = path.join(repoRoot, "target", "debug", "dui");
assert.ok(fs.existsSync(duiBin), `missing built dui binary at ${duiBin}`);

interface RunReport {
  coverage: { verified: number };
  cells: Array<{
    path: string;
    status: string;
    observation?: { text?: string };
  }>;
}

function runDuiText(args: string[], envOverrides: Record<string, string>): string {
  return execFileSync(duiBin, args, {
    encoding: "utf8",
    env: { ...process.env, ...envOverrides },
  });
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-auth-pr-diff-"));
const duiHome = path.join(tmp, ".dui-home");

try {
  const appDir = path.join(tmp, "auth-app");
  fs.mkdirSync(appDir, { recursive: true });

  // Multi-auth server simulating real-world OSS apps:
  // 1. Cookie session SSR routes (/dashboard, /projects)
  // 2. localStorage JWT SPA route (/editor)
  // 3. POST /api/login issuing both Set-Cookie + JSON { data: { token } }
  const writeServer = (headline: string) => {
    fs.writeFileSync(
      path.join(appDir, "server.mjs"),
      `import http from "node:http";
const port = Number(process.env.PORT);
http.createServer((req, res) => {
  const cookie = req.headers.cookie || "";
  if (req.method === "POST" && req.url === "/api/login") {
    let raw = "";
    req.on("data", (c) => (raw += c));
    req.on("end", () => {
      const body = JSON.parse(raw || "{}");
      if (body.email === "demo@example.com" && body.password === "s3cr3t-pass") {
        res.writeHead(200, {
          "content-type": "application/json",
          "set-cookie": "app_session=login-cookie-ok; Path=/; HttpOnly",
        });
        res.end(JSON.stringify({ data: { token: "jwt-from-login-ok" } }));
      } else {
        res.writeHead(401, { "content-type": "application/json" });
        res.end(JSON.stringify({ error: "unauthorized" }));
      }
    });
    return;
  }

  if (req.url === "/dashboard" || req.url === "/projects") {
    const authed =
      cookie.includes("app_session=storage-cookie-ok") ||
      cookie.includes("app_session=login-cookie-ok");
    if (!authed) {
      res.writeHead(401, { "content-type": "text/html; charset=utf-8" });
      res.end("<html><body><h1>Sign In Required</h1></body></html>");
      return;
    }
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    if (req.url === "/projects") {
      res.end("<html><body><h1>Authenticated Projects</h1><a href='/dashboard'>Dashboard</a></body></html>");
    } else {
      res.end("<html><body><h1>${headline}</h1><a href='/projects'>Projects</a></body></html>");
    }
    return;
  }

  if (req.url === "/editor") {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(\`<html><body><main id="root">loading</main><script>
      const tok = window.localStorage.getItem("jwtToken");
      document.getElementById("root").innerHTML = tok
        ? "<h1>Authenticated Editor</h1><p>Token loaded</p>"
        : "<h1>Guest Sign In Required</h1>";
    </script></body></html>\`);
    return;
  }

  res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
  res.end("<html><body><h1>Public Home</h1></body></html>");
}).listen(port, "127.0.0.1");
`,
    );
  };

  writeServer("Workspace Dashboard v1");

  // Pattern 1: Standard storageState.json (Cookie + localStorage)
  const storageStatePath = path.join(appDir, "storage-state.json");
  fs.writeFileSync(
    storageStatePath,
    JSON.stringify({
      cookies: [{ name: "app_session", value: "storage-cookie-ok", domain: "127.0.0.1" }],
      origins: [
        {
          origin: "http://127.0.0.1",
          localStorage: [{ name: "jwtToken", value: "storage-jwt-ok" }],
        },
      ],
    }),
  );

  const storageManifest = path.join(appDir, "dui-storage.json");
  fs.writeFileSync(
    storageManifest,
    JSON.stringify({
      host: "command",
      command: [process.execPath, "server.mjs"],
      path: "/dashboard",
      widths: [1280],
      storageState: "storage-state.json",
      scenarios: [{ path: "/projects" }, { path: "/editor" }],
    }),
  );

  const out1 = path.join(tmp, "out-storage-1");
  runDuiText(["run", appDir, storageManifest, out1], { HOME: duiHome });
  const rep1 = JSON.parse(fs.readFileSync(path.join(out1, "report.json"), "utf8")) as RunReport;
  assert.equal(
    rep1.coverage.verified,
    3,
    "storageState unlocks /dashboard, /projects, and /editor",
  );
  assert.ok(
    rep1.cells
      .find((c) => c.path === "/dashboard")
      ?.observation?.text?.includes("Workspace Dashboard v1"),
  );
  assert.ok(
    rep1.cells
      .find((c) => c.path === "/editor")
      ?.observation?.text?.includes("Authenticated Editor"),
  );

  // Pattern 2: POST authFlow + env: secret indirection + redaction on disk
  const flowManifest = path.join(appDir, "dui-flow.json");
  fs.writeFileSync(
    flowManifest,
    JSON.stringify({
      host: "command",
      command: [process.execPath, "server.mjs"],
      path: "/dashboard",
      widths: [1280],
      authFlow: {
        type: "request",
        method: "POST",
        path: "/api/login",
        body: { email: "demo@example.com", password: "env:DEMO_AUTH_PASSWORD" },
        tokenJsonPath: "data.token",
        localStorageKey: "jwtToken",
      },
      scenarios: [{ path: "/editor" }],
    }),
  );

  const outFlow = path.join(tmp, "out-flow");
  runDuiText(["run", appDir, flowManifest, outFlow], {
    HOME: duiHome,
    DEMO_AUTH_PASSWORD: "s3cr3t-pass",
  });
  const repFlow = JSON.parse(
    fs.readFileSync(path.join(outFlow, "report.json"), "utf8"),
  ) as RunReport;
  assert.equal(repFlow.coverage.verified, 2, "authFlow POST login unlocks cookie + JWT routes");
  assert.ok(
    repFlow.cells
      .find((c) => c.path === "/editor")
      ?.observation?.text?.includes("Authenticated Editor"),
  );
  const persistedManifest = fs.readFileSync(path.join(outFlow, "manifest.json"), "utf8");
  assert.ok(
    !persistedManifest.includes("s3cr3t-pass"),
    "password secret never written to manifest.json",
  );
  assert.ok(persistedManifest.includes("<redacted>"), "authFlow body redacted in manifest.json");

  // Pattern 3: PR-ready Markdown diff (`dui diff <out1> <out2> --markdown`)
  writeServer("Workspace Dashboard v2");
  const out2 = path.join(tmp, "out-storage-2");
  runDuiText(["run", appDir, storageManifest, out2, "--force"], { HOME: duiHome });
  const md = runDuiText(["diff", out1, out2, "--markdown"], { HOME: duiHome });
  assert.ok(md.includes("## dui UI Diff Summary"), "markdown header present");
  assert.ok(
    md.includes("| `/dashboard` | `1280px` | `changed` |"),
    "changed dashboard row present",
  );
  assert.ok(
    md.includes("Workspace Dashboard v1") && md.includes("Workspace Dashboard v2"),
    `expected element-level renderDiff summary in Markdown:\n${md}`,
  );

  console.log(
    JSON.stringify({
      status: "ok",
      storageStateVerified: rep1.coverage.verified,
      authFlowVerified: repFlow.coverage.verified,
      secretRedacted: true,
      markdownDiffLines: md.trim().split("\n").length,
    }),
  );
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
