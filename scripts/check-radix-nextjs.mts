import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const repoRoot = process.cwd();
const duiBin = path.join(repoRoot, "target", "debug", "dui");
assert.ok(fs.existsSync(duiBin), `missing built dui binary at ${duiBin}`);

interface ScanFact {
  kind: string;
  package?: string;
  label?: string;
  fields?: Array<{ name: string; type: string; shape?: { kind: string } }>;
}

interface ScanReport {
  status: string;
  versions: Record<string, string>;
  facts: ScanFact[];
}

interface NextRouteReport {
  path: string;
  file: string;
  isClientComponent: boolean;
  isDynamic: boolean;
  boundaryNote?: string;
}

interface NextRunReport {
  coverage: { verified: number };
  nextAppRoutes: NextRouteReport[];
  serverComponentBoundaries: NextRouteReport[];
  cells: Array<{ path: string; width: number; status: string }>;
}

function runDui(args: string[], envOverrides: Record<string, string>): unknown {
  const stdout = execFileSync(duiBin, args, {
    encoding: "utf8",
    env: { ...process.env, ...envOverrides },
  });
  return JSON.parse(stdout) as unknown;
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-radix-nextjs-"));
const duiHome = path.join(tmp, ".dui-home");

try {
  // Part A: Radix UI 1 / shadcn declarative rule packs via official TypeScript 7 compiler
  const radixDir = path.join(tmp, "radix-app");
  fs.mkdirSync(path.join(radixDir, "src"), { recursive: true });
  fs.symlinkSync(
    path.join(repoRoot, "node_modules", "typescript"),
    path.join(radixDir, "node_modules_ts_tmp"),
  );
  fs.rmSync(path.join(radixDir, "node_modules_ts_tmp"));

  const nm = path.join(radixDir, "node_modules");
  fs.mkdirSync(path.join(nm, "@radix-ui", "react-dialog"), { recursive: true });
  fs.mkdirSync(path.join(nm, "@radix-ui", "react-tabs"), { recursive: true });
  fs.mkdirSync(path.join(nm, "react"), { recursive: true });
  fs.symlinkSync(path.join(repoRoot, "node_modules", "typescript"), path.join(nm, "typescript"));

  fs.writeFileSync(
    path.join(nm, "react", "package.json"),
    JSON.stringify({ name: "react", version: "19.2.0", types: "index.d.ts" }),
  );
  fs.writeFileSync(
    path.join(nm, "react", "index.d.ts"),
    `declare namespace JSX { interface IntrinsicElements { [elem: string]: any; } }
export function useState<T>(initial: T): [T, (v: T) => void];`,
  );
  fs.writeFileSync(
    path.join(nm, "@radix-ui", "react-dialog", "package.json"),
    JSON.stringify({ name: "@radix-ui/react-dialog", version: "1.1.0", types: "index.d.ts" }),
  );
  fs.writeFileSync(
    path.join(nm, "@radix-ui", "react-dialog", "index.d.ts"),
    `export interface DialogProps { open?: boolean; defaultOpen?: boolean; children?: any; }
export function Root(props: DialogProps): any;
export const Dialog = Root;`,
  );
  fs.writeFileSync(
    path.join(nm, "@radix-ui", "react-tabs", "package.json"),
    JSON.stringify({ name: "@radix-ui/react-tabs", version: "1.1.0", types: "index.d.ts" }),
  );
  fs.writeFileSync(
    path.join(nm, "@radix-ui", "react-tabs", "index.d.ts"),
    `export interface TabsProps<V extends string = string> { value?: V; defaultValue?: V; children?: any; }
export function Root<V extends string = string>(props: TabsProps<V>): any;`,
  );

  fs.writeFileSync(
    path.join(radixDir, "package.json"),
    JSON.stringify({ name: "radix-fixture", private: true }),
  );
  fs.writeFileSync(
    path.join(radixDir, "tsconfig.json"),
    JSON.stringify({
      compilerOptions: {
        target: "ES2022",
        module: "ESNext",
        moduleResolution: "bundler",
        jsx: "react-jsx",
        strict: true,
        noEmit: true,
      },
      include: ["src"],
    }),
  );
  fs.writeFileSync(
    path.join(radixDir, "src", "SettingsModal.tsx"),
    `import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import * as Tabs from "@radix-ui/react-tabs";

type SettingsTab = "account" | "billing" | "security";

export function SettingsModal() {
  const [open] = useState<boolean>(false);
  const [tab] = useState<SettingsTab>("account");
  return (
    <Dialog.Root open={open}>
      <Tabs.Root value={tab} />
    </Dialog.Root>
  );
}
`,
  );
  const radixManifest = path.join(radixDir, "dui.json");
  fs.writeFileSync(
    radixManifest,
    JSON.stringify({
      tsconfig: "tsconfig.json",
      path: "/",
      widths: [375],
      readySelector: "body",
    }),
  );

  const scan = runDui(["scan", radixDir, radixManifest], { HOME: duiHome }) as ScanReport;
  assert.equal(scan.versions["@radix-ui/react-dialog"], "1.1.0");
  assert.equal(scan.versions["@radix-ui/react-tabs"], "1.1.0");
  const controlFacts = scan.facts.filter((f) => f.kind === "control");
  const dialogFact = controlFacts.find((f) => f.package === "@radix-ui/react-dialog");
  const tabsFact = controlFacts.find((f) => f.package === "@radix-ui/react-tabs");
  assert.ok(dialogFact, `expected Radix Dialog control fact: ${JSON.stringify(scan.facts)}`);
  assert.ok(
    dialogFact.fields?.some(
      (fld) =>
        fld.name === "open" &&
        fld.type === "boolean" &&
        (fld.shape?.kind === "boolean" || fld.shape?.kind === "union"),
    ),
    `expected Dialog open:boolean field: ${JSON.stringify(dialogFact)}`,
  );
  assert.ok(tabsFact, `expected Radix Tabs control fact: ${JSON.stringify(scan.facts)}`);
  assert.ok(
    tabsFact.fields?.some((fld) => fld.name === "value" && fld.shape?.kind === "union"),
    `expected Tabs value union field: ${JSON.stringify(tabsFact)}`,
  );

  // Part B: Next.js 16 App Router discovery + Server Component boundary + capture
  const nextDir = path.join(tmp, "next-app");
  fs.mkdirSync(path.join(nextDir, "app", "(marketing)", "pricing"), { recursive: true });
  fs.mkdirSync(path.join(nextDir, "app", "orders", "[id]"), { recursive: true });

  fs.writeFileSync(
    path.join(nextDir, "app", "page.tsx"),
    `export default async function HomePage() {
  return <main><h1>Next Server Home</h1></main>;
}
`,
  );
  fs.writeFileSync(
    path.join(nextDir, "app", "(marketing)", "pricing", "page.tsx"),
    `"use client";
export default function PricingPage() {
  return <main><h1>Client Pricing</h1></main>;
}
`,
  );
  fs.writeFileSync(
    path.join(nextDir, "app", "orders", "[id]", "page.tsx"),
    `export default async function OrderDetail() {
  return <main><h1>Order</h1></main>;
}
`,
  );
  fs.writeFileSync(
    path.join(nextDir, "serve-next.mjs"),
    `import http from "node:http";
const port = Number(process.env.PORT);
http.createServer((req, res) => {
  res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
  if (req.url === "/pricing") {
    res.end("<html><body><main><h1>Client Pricing</h1><a href='/'>Home</a></main></body></html>");
    return;
  }
  res.end("<html><body><main><h1>Next Server Home</h1><a href='/pricing'>Pricing</a></main></body></html>");
}).listen(port, "127.0.0.1");
`,
  );
  const nextManifest = path.join(nextDir, "dui.json");
  fs.writeFileSync(
    nextManifest,
    JSON.stringify({
      host: "next",
      command: [process.execPath, "serve-next.mjs"],
      widths: [375, 1280],
    }),
  );

  const nextOut = path.join(tmp, "out-next");
  runDui(["run", nextDir, nextManifest, nextOut], { HOME: duiHome });
  const nextReport = JSON.parse(
    fs.readFileSync(path.join(nextOut, "report.json"), "utf8"),
  ) as NextRunReport;
  assert.equal(
    nextReport.coverage.verified,
    4,
    `expected 4 verified Next.js cells (/ and /pricing x 2 widths), got ${nextReport.coverage.verified}`,
  );
  assert.equal(nextReport.nextAppRoutes.length, 3, "discovers /, /pricing, and /orders/:id");
  assert.ok(
    nextReport.serverComponentBoundaries.some(
      (r) => r.path === "/" && !r.isClientComponent && r.boundaryNote?.includes("server-component"),
    ),
    "reports honest Server Component boundary for app/page.tsx",
  );
  assert.ok(
    nextReport.nextAppRoutes.some((r) => r.path === "/pricing" && r.isClientComponent),
    "recognizes 'use client' on app/(marketing)/pricing/page.tsx",
  );

  // Missing `next` binary without url/command override reports unrenderable (no-next-binary)
  const missingManifest = path.join(nextDir, "dui-missing.json");
  fs.writeFileSync(missingManifest, JSON.stringify({ host: "next", widths: [375] }));
  const missingReply = runDui(["run", nextDir, missingManifest, path.join(tmp, "out-missing")], {
    HOME: duiHome,
    DUI_NEXT_BIN: "/nonexistent/next",
  }) as { status: string; surface: string; reason: string };
  assert.equal(missingReply.status, "unrenderable");
  assert.equal(missingReply.surface, "nextjs-web");
  assert.equal(missingReply.reason, "no-next-binary");

  console.log(
    JSON.stringify({
      status: "ok",
      radixControlsFound: controlFacts.length,
      nextVerifiedCells: nextReport.coverage.verified,
      nextAppRoutes: nextReport.nextAppRoutes.length,
      serverComponentBoundaries: nextReport.serverComponentBoundaries.length,
      missingNextReason: missingReply.reason,
    }),
  );
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
