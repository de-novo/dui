// Verify v0.0.4 action-driven transition replay and edge provenance (#31):
// 1. Replay transitions from initial state in real Chromium via CLI `dui transitions` and MCP `transitions` (identical JSON).
// 2. Valid transitions promote to `provenance: "verified"`, `replayVerified: true` with confirmed widths.
// 3. Broken/mismatched inferred or extracted transitions stay unverified with per-width failure reasons.
// 4. Embedded `dui open` viewer renders edge provenance badges and replay evidence in Chrome.
import assert from "node:assert/strict";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

type TransitionVerdict = {
  from: string;
  to: string;
  origin: "extracted" | "inferred" | "verified";
  provenance: "extracted" | "inferred" | "verified";
  replayVerified: boolean;
  confirmedWidths: number[];
  failedWidths: Record<string, string>;
};

type TransitionReport = {
  summary: {
    total: number;
    verified: number;
    extracted: number;
    inferred: number;
    failed: number;
  };
  transitions: TransitionVerdict[];
};

const root = resolve(import.meta.dirname, "..");
const template = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-transitions-check-"));

function chromeExecutable(): string {
  const candidates = [
    process.env.DUI_CHROMIUM,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/chromium",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium-browser",
  ];
  for (const path of candidates) {
    if (path && existsSync(path)) return path;
  }
  throw new Error("Chrome/Chromium executable not found");
}

function runDui(args: string[], envExtra: Record<string, string>) {
  const res = spawnSync(binary, args, {
    env: { ...process.env, ...envExtra },
    encoding: "utf8",
  });
  assert(res.stdout.trim().length > 0, `empty stdout for ${args.join(" ")}: ${res.stderr}`);
  return { status: res.status ?? 1, body: JSON.parse(res.stdout) };
}

function callMcpTransitions(
  project: string,
  manifest: string,
  runDir: string,
  envExtra: Record<string, string>,
): TransitionReport {
  const rpc =
    JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "tools/call",
      params: { name: "transitions", arguments: { project, manifest, runDir } },
    }) + "\n";
  const res = spawnSync(binary, ["mcp"], {
    input: rpc,
    env: { ...process.env, ...envExtra },
    encoding: "utf8",
  });
  assert.equal(res.status, 0, `mcp exited ${res.status}: ${res.stderr}`);
  const parsed = JSON.parse(res.stdout.trim()) as {
    result: { isError: boolean; content: { text: string }[] };
  };
  assert.equal(parsed.result.isError, false, parsed.result.content[0]?.text);
  return JSON.parse(parsed.result.content[0]!.text) as TransitionReport;
}

try {
  const home = join(stage, "home");
  const baseEnv = { HOME: home };
  const appDir = join(stage, "order-flow");
  cpSync(template, appDir, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(join(template, "node_modules"), join(appDir, "node_modules"), "dir");

  writeFileSync(
    join(appDir, "main.tsx"),
    [
      'import { useState, useEffect } from "react";',
      'import { createRoot } from "react-dom/client";',
      "",
      "function App() {",
      "  const [, setTick] = useState(0);",
      "  useEffect(() => {",
      "    const onPop = () => setTick((n) => n + 1);",
      '    window.addEventListener("popstate", onPop);',
      '    return () => window.removeEventListener("popstate", onPop);',
      "  }, []);",
      "  const path = window.location.pathname || '/order';",
      "  const go = (next: string) => {",
      "    window.history.pushState({}, '', next);",
      "    setTick((n) => n + 1);",
      "  };",
      '  if (path === "/confirm") {',
      "    return (",
      '      <main data-testid="confirm-screen">',
      "        <h1>Confirm cancellation</h1>",
      '        <button type="button" data-testid="confirm-cancel" onClick={() => go("/cancelled")}>Confirm cancel</button>',
      "      </main>",
      "    );",
      "  }",
      '  if (path === "/cancelled") {',
      "    return (",
      '      <main data-testid="cancelled-screen">',
      "        <h1>Order cancelled</h1>",
      "      </main>",
      "    );",
      "  }",
      '  if (path === "/help") {',
      '    return <main data-testid="help-screen"><h1>Help</h1></main>;',
      "  }",
      "  return (",
      '    <main data-testid="order-screen">',
      "      <h1>Order details</h1>",
      '      <button type="button" data-testid="cancel-order" onClick={() => go("/confirm")}>Cancel order</button>',
      '      <button type="button" data-testid="noop-help" onClick={() => {}}>Broken help</button>',
      "    </main>",
      "  );",
      "}",
      'createRoot(document.getElementById("app")!).render(<App />);',
      "",
    ].join("\n"),
  );

  const manifestPath = join(appDir, "dui.json");
  writeFileSync(
    manifestPath,
    JSON.stringify(
      {
        tsconfig: "tsconfig.json",
        path: "/order",
        widths: [375, 1280],
        readySelector: "[data-testid=order-screen]",
        stateBudget: 1,
        scenarios: [
          { path: "/confirm", readySelector: "[data-testid=confirm-screen]" },
          { path: "/cancelled", readySelector: "[data-testid=cancelled-screen]" },
          { path: "/help", readySelector: "[data-testid=help-screen]" },
        ],
        transitions: [
          {
            from: "/order",
            to: "/confirm",
            origin: "extracted",
            action: {
              kind: "click",
              selector: "[data-testid=cancel-order]",
              label: "Cancel order",
            },
            expectSelector: "[data-testid=confirm-screen]",
            expectText: "Confirm cancellation",
          },
          {
            from: "/confirm",
            to: "/cancelled",
            origin: "extracted",
            action: {
              kind: "click",
              selector: "[data-testid=confirm-cancel]",
              label: "Confirm cancel",
            },
            expectSelector: "[data-testid=cancelled-screen]",
            expectText: "Order cancelled",
          },
          {
            from: "/order",
            to: "/help",
            origin: "extracted",
            action: { kind: "click", selector: "[data-testid=noop-help]", label: "Broken help" },
          },
          {
            from: "/confirm",
            to: "/help",
            origin: "inferred",
            action: {
              kind: "click",
              selector: "[data-testid=missing-btn]",
              label: "Inferred help",
            },
          },
        ],
      },
      null,
      2,
    ),
  );

  const captured = runDui(["run", appDir, manifestPath], baseEnv);
  assert.equal(captured.status, 0, JSON.stringify(captured.body));
  const runDir = (captured.body as { output: string }).output;

  const cliRes = runDui(["transitions", appDir, manifestPath, runDir], baseEnv);
  assert.equal(cliRes.status, 0, JSON.stringify(cliRes.body));
  const cliReport = cliRes.body as TransitionReport;
  const mcpReport = callMcpTransitions(appDir, manifestPath, runDir, baseEnv);
  assert.deepEqual(cliReport, mcpReport);

  assert.equal(cliReport.summary.total, 4);
  assert.equal(cliReport.summary.verified, 2);
  assert.equal(cliReport.summary.extracted, 1);
  assert.equal(cliReport.summary.inferred, 1);
  assert.equal(cliReport.summary.failed, 2);

  const toConfirm = cliReport.transitions.find((t) => t.from === "/order" && t.to === "/confirm")!;
  const toCancelled = cliReport.transitions.find(
    (t) => t.from === "/confirm" && t.to === "/cancelled",
  )!;
  const brokenHelp = cliReport.transitions.find((t) => t.from === "/order" && t.to === "/help")!;
  const inferredHelp = cliReport.transitions.find(
    (t) => t.from === "/confirm" && t.to === "/help",
  )!;

  assert.equal(toConfirm.provenance, "verified");
  assert.equal(toConfirm.replayVerified, true);
  assert.deepEqual(toConfirm.confirmedWidths, [375, 1280]);

  assert.equal(toCancelled.provenance, "verified");
  assert.equal(toCancelled.replayVerified, true);
  assert.deepEqual(toCancelled.confirmedWidths, [375, 1280]);

  assert.equal(brokenHelp.provenance, "extracted");
  assert.equal(brokenHelp.replayVerified, false);
  assert(brokenHelp.failedWidths["375"]?.includes("destination-mismatch"));

  assert.equal(inferredHelp.provenance, "inferred");
  assert.equal(inferredHelp.replayVerified, false);
  assert(inferredHelp.failedWidths["375"]?.includes("selector-not-found"));

  assert(existsSync(join(runDir, "transitions.json")));
  const updatedGraph = JSON.parse(readFileSync(join(runDir, "graph.json"), "utf8")) as {
    edges: { source: string; target: string; provenance?: string; replayVerified?: boolean }[];
  };
  assert(
    updatedGraph.edges.some(
      (e) =>
        e.source === "route:/order" &&
        e.target === "route:/confirm" &&
        e.provenance === "verified" &&
        e.replayVerified === true,
    ),
  );

  // Verify embedded `dui open` renders verified & unverified edge provenance in real Chrome.
  const viewer = spawn(binary, ["open", "--no-open", runDir], {
    env: { ...process.env, ...baseEnv },
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    const url = await new Promise<string>((resolveUrl, rejectUrl) => {
      let buf = "";
      const timer = setTimeout(() => rejectUrl(new Error(`viewer timeout: ${buf}`)), 15_000);
      viewer.stdout.on("data", (chunk) => {
        buf += chunk.toString();
        const match = buf.match(/http:\/\/127\.0\.0\.1:\d+/);
        if (match) {
          clearTimeout(timer);
          resolveUrl(match[0]);
        }
      });
    });

    const chrome = chromeExecutable();
    const dom = execFileSync(
      chrome,
      [
        "--headless=new",
        "--disable-gpu",
        "--no-sandbox",
        "--virtual-time-budget=3000",
        "--dump-dom",
        `${url}/?screen=route%3A%2Forder&uiLang=en`,
      ],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], timeout: 30_000 },
    );
    assert(
      dom.includes('data-edge-provenance="verified"') && dom.includes("Verified by replay"),
      "expected replay-verified edge provenance in viewer DOM",
    );
    assert(
      dom.includes('data-edge-provenance="extracted"'),
      "expected unverified extracted edge provenance in viewer DOM",
    );

    console.log(
      JSON.stringify({
        summary: cliReport.summary,
        cliMcpIdentical: true,
        viewerProvenanceRendered: true,
      }),
    );
  } finally {
    viewer.kill();
  }
} finally {
  rmSync(stage, { recursive: true, force: true });
}
