// Verify Priority 2 declarative rule packs (#63) without framework conditionals in dui-core:
// 1. Extract `store` (Zustand 5, Redux Toolkit 2, Jotai 3), `machine` (XState 5), `form` (React Hook Form 7),
//    `schema` (valibot 1), and `route`/`search` (React Router 8) facts via official TypeScript 7 compiler.
// 2. Enforce latest-stable-major policy by rejecting an older major (e.g. zustand 4.x) as `unsupported` (exit 4).
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const template = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-p2-rules-check-"));

function writeFakePkg(
  nodeModules: string,
  name: string,
  version: string,
  dts: string,
  strictExports = false,
) {
  const dir = join(nodeModules, ...name.split("/"));
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "package.json"),
    JSON.stringify({
      name,
      version,
      types: "index.d.ts",
      main: "index.js",
      // Regression guard (#90): strict `exports` hiding `./package.json` (like valibot 1.x).
      ...(strictExports
        ? { exports: { ".": { types: "./index.d.ts", default: "./index.js" } } }
        : {}),
    }),
  );
  writeFileSync(join(dir, "index.d.ts"), dts);
  writeFileSync(join(dir, "index.js"), "export {};\n");
}

function runDui(args: string[], envExtra: Record<string, string>) {
  const res = spawnSync(binary, args, {
    env: { ...process.env, ...envExtra },
    encoding: "utf8",
  });
  const text = (res.stdout || res.stderr).trim();
  assert(text.length > 0, `empty output for ${args.join(" ")}`);
  return { status: res.status ?? 1, body: JSON.parse(text) };
}

try {
  const home = join(stage, "home");
  const env = { HOME: home };
  const appDir = join(stage, "p2-app");
  cpSync(template, appDir, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });

  const nm = join(appDir, "node_modules");
  mkdirSync(nm, { recursive: true });
  for (const dep of ["typescript", "react", "react-dom", "vite", "@types"]) {
    const src = join(template, "node_modules", dep);
    symlinkSync(src, join(nm, dep), "dir");
  }

  writeFakePkg(nm, "zustand", "5.1.0", "export declare function create<T>(init: () => T): T;\n");
  writeFakePkg(
    nm,
    "@reduxjs/toolkit",
    "2.15.0",
    "export declare function createSlice<S>(cfg: { name: string; initialState: S }): { initialState: S };\n",
  );
  writeFakePkg(nm, "jotai", "3.0.2", "export declare function atom<T>(initial: T): { init: T };\n");
  writeFakePkg(
    nm,
    "xstate",
    "5.33.4",
    "export declare function createMachine<S extends string>(cfg: { initial: S }): { value: S };\n",
  );
  writeFakePkg(
    nm,
    "react-hook-form",
    "7.92.0",
    "export declare function useForm<T>(): { formState: { isSubmitting: boolean }; values: T };\n",
  );
  writeFakePkg(
    nm,
    "valibot",
    "1.5.0",
    "export declare function picklist<const T extends readonly string[]>(opts: T): { options: T };\n",
    true,
  );
  writeFakePkg(
    nm,
    "react-router",
    "8.5.2",
    "export declare function createBrowserRouter(routes: { path: string }[]): unknown;\nexport declare function useSearchParams(): [URLSearchParams, (next: URLSearchParams) => void];\n",
  );

  writeFileSync(
    join(appDir, "main.tsx"),
    [
      'import { create } from "zustand";',
      'import { createSlice } from "@reduxjs/toolkit";',
      'import { atom } from "jotai";',
      'import { createMachine } from "xstate";',
      'import { useForm } from "react-hook-form";',
      'import { picklist } from "valibot";',
      'import { createBrowserRouter, useSearchParams } from "react-router";',
      "",
      'export const useCartStore = create(() => ({ open: false as boolean, mode: "grid" as "grid" | "list" }));',
      'export const filterSlice = createSlice({ name: "filter", initialState: { status: "all" as "all" | "active" } });',
      'export const themeAtom = atom<"light" | "dark">("light");',
      'export const checkoutMachine = createMachine<"idle" | "submitting" | "done">({ initial: "idle" });',
      'export const statusSchema = picklist(["draft", "published"] as const);',
      'export const router = createBrowserRouter([{ path: "/checkout" }]);',
      "",
      "export function CheckoutForm() {",
      "  const form = useForm<{ email: string }>();",
      "  const [params] = useSearchParams();",
      '  const step = params.get("step");',
      '  return <div>{form.formState.isSubmitting ? "Saving" : step === "confirm" ? "Confirm" : "Edit"}</div>;',
      "}",
      "",
    ].join("\n"),
  );

  const manifest = join(appDir, "dui.json");
  writeFileSync(
    manifest,
    JSON.stringify(
      {
        tsconfig: "tsconfig.json",
        path: "/",
        widths: [375],
      },
      null,
      2,
    ),
  );

  const scan = runDui(["scan", appDir, manifest], env);
  assert.equal(scan.status, 0, JSON.stringify(scan.body));
  const facts = scan.body.facts as { kind: string; package?: string; label?: string }[];

  const storePkgs = new Set(facts.filter((f) => f.kind === "store").map((f) => f.package));
  assert(storePkgs.has("zustand"), `missing zustand store fact: ${JSON.stringify(facts)}`);
  assert(storePkgs.has("@reduxjs/toolkit"), "missing redux toolkit store fact");
  assert(storePkgs.has("jotai"), "missing jotai store fact");

  assert(
    facts.some((f) => f.kind === "machine" && f.package === "xstate"),
    "missing xstate machine fact",
  );
  assert(
    facts.some((f) => f.kind === "form" && f.package === "react-hook-form"),
    "missing react-hook-form fact",
  );
  assert(
    facts.some((f) => f.kind === "schema" && f.label === "picklist"),
    "missing valibot schema fact",
  );
  assert(
    facts.some((f) => f.kind === "route" && f.label === "createBrowserRouter"),
    "missing react-router route fact",
  );
  assert(
    facts.some((f) => f.kind === "searchParam" && f.label === "step"),
    "missing react-router URLSearchParams searchParam fact",
  );

  // Older major rejection check: zustand 4.5.0 must return exit 4 (`unsupported`).
  writeFakePkg(nm, "zustand", "4.5.0", "export declare function create<T>(init: () => T): T;\n");
  const oldMajor = runDui(["scan", appDir, manifest], env);
  assert.equal(oldMajor.status, 4, `expected unsupported exit 4, got ${JSON.stringify(oldMajor)}`);
  assert.equal(oldMajor.body.status, "unsupported");

  console.log(
    JSON.stringify({
      ok: true,
      stores: [...storePkgs].sort(),
      machine: true,
      form: true,
      schema: true,
      router: true,
      olderMajorRejected: true,
    }),
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
