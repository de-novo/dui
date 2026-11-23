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
  fields?: Array<{ name: string; type: string }>;
}

interface ScanReport {
  versions: Record<string, string>;
  facts: ScanFact[];
}

interface ToolchainReport {
  toolchains: Array<{ toolchain: string; binary: string; available: boolean; status: string }>;
}

function runDui(args: string[], homeDir?: string): unknown {
  const stdout = execFileSync(duiBin, args, {
    encoding: "utf8",
    env: homeDir ? { ...process.env, HOME: homeDir } : process.env,
  });
  return JSON.parse(stdout) as unknown;
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dui-compiler-oracles-"));
const duiHome = path.join(tmp, ".dui-home");

try {
  // 1. Official toolchain probe covers all 6 languages
  const tc = runDui(["toolchains"]) as ToolchainReport;
  assert.equal(tc.toolchains.length, 6);
  const rust = tc.toolchains.find((t) => t.toolchain === "rustc");
  assert.equal(rust?.available, true);
  assert.equal(rust?.status, "supported");

  // 2. Ink (TS/React TUI) + React Native / Expo Router via official TypeScript 7 compiler
  const projDir = path.join(tmp, "ts-multi-surface");
  const nm = path.join(projDir, "node_modules");
  fs.mkdirSync(path.join(projDir, "src"), { recursive: true });
  fs.mkdirSync(path.join(nm, "react"), { recursive: true });
  fs.mkdirSync(path.join(nm, "ink"), { recursive: true });
  fs.mkdirSync(path.join(nm, "react-native"), { recursive: true });
  fs.mkdirSync(path.join(nm, "expo-router"), { recursive: true });
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
    path.join(nm, "ink", "package.json"),
    JSON.stringify({ name: "ink", version: "5.2.0", types: "index.d.ts" }),
  );
  fs.writeFileSync(
    path.join(nm, "ink", "index.d.ts"),
    `export function useInput(handler: (input: string) => void): void;
export function useFocus(): { isFocused: boolean };`,
  );
  fs.writeFileSync(
    path.join(nm, "react-native", "package.json"),
    JSON.stringify({ name: "react-native", version: "0.88.0", types: "index.d.ts" }),
  );
  fs.writeFileSync(
    path.join(nm, "react-native", "index.d.ts"),
    `export function Modal(props: { visible?: boolean; children?: any }): any;
export function Switch(props: { value?: boolean }): any;
export function useColorScheme(): "light" | "dark";`,
  );
  fs.writeFileSync(
    path.join(nm, "expo-router", "package.json"),
    JSON.stringify({ name: "expo-router", version: "5.1.0", types: "index.d.ts" }),
  );
  fs.writeFileSync(
    path.join(nm, "expo-router", "index.d.ts"),
    `export function useLocalSearchParams<T extends Record<string, string>>(): T;`,
  );

  fs.writeFileSync(
    path.join(projDir, "package.json"),
    JSON.stringify({ name: "multi-surface-fixture", private: true }),
  );
  fs.writeFileSync(
    path.join(projDir, "tsconfig.json"),
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
    path.join(projDir, "src", "Screens.tsx"),
    `import { useState } from "react";
import { useInput } from "ink";
import { Modal, Switch, useColorScheme } from "react-native";
import { useLocalSearchParams } from "expo-router";

type TuiPane = "overview" | "logs";

export function InkDashboard() {
  const [pane, setPane] = useState<TuiPane>("overview");
  useInput((ch) => {
    if (ch === "l") setPane("logs");
  });
  return pane === "overview" ? <div /> : <section />;
}

export function MobileCheckout() {
  const [open] = useState<boolean>(false);
  const [enabled] = useState<boolean>(true);
  const params = useLocalSearchParams<{ plan: "free" | "pro" }>();
  const scheme = useColorScheme();
  return (
    <Modal visible={open}>
      <Switch value={enabled} />
      <span>{params.plan}{scheme}</span>
    </Modal>
  );
}
`,
  );

  const manifest = path.join(projDir, "dui.json");
  fs.writeFileSync(
    manifest,
    JSON.stringify({ tsconfig: "tsconfig.json", path: "/", widths: [390], readySelector: "body" }),
  );

  const scan = runDui(["scan", projDir, manifest], duiHome) as ScanReport;
  assert.equal(scan.versions.ink, "5.2.0");
  assert.equal(scan.versions["expo-router"], "5.1.0");
  const modalFact = scan.facts.find(
    (f) => f.kind === "control" && f.package === "react-native" && f.label === "Modal",
  );
  const switchFact = scan.facts.find(
    (f) => f.kind === "control" && f.package === "react-native" && f.label === "Switch",
  );
  const searchFact = scan.facts.find((f) => f.kind === "searchParam" && f.label === "plan");
  assert.ok(
    modalFact?.fields?.some((fld) => fld.name === "visible"),
    "Modal visible prop extracted",
  );
  assert.ok(
    switchFact?.fields?.some((fld) => fld.name === "value"),
    "Switch value prop extracted",
  );
  assert.ok(searchFact, "expo-router useLocalSearchParams plan extracted");

  console.log(
    JSON.stringify({
      status: "ok",
      toolchainsProbed: tc.toolchains.length,
      inkVersion: scan.versions.ink,
      expoRouterVersion: scan.versions["expo-router"],
      extractedFacts: scan.facts.length,
    }),
  );
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
