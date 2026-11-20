// Check the same source transform that the Vite host embeds, using the fixture's compiler.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  cpSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import {
  instrument,
  type InstrumentFact,
} from "../adapters/vite/instrument.mts";

const root = resolve(import.meta.dirname, "..");
const fixture = join(root, "fixtures/toggle");
const binary = resolve(process.argv[2] ?? join(root, "target/debug/dui"));
const stage = mkdtempSync(join(tmpdir(), "dui-capture-check-"));

try {
  cpSync(fixture, stage, {
    recursive: true,
    filter: (p) => !["node_modules", "dist"].includes(basename(p)),
  });
  symlinkSync(
    join(fixture, "node_modules"),
    join(stage, "node_modules"),
    "dir",
  );
  cpSync(
    join(import.meta.dirname, "fixtures/guards.tsx"),
    join(stage, "guards.tsx"),
  );
  function tool(name: string, ...args: string[]) {
    execFileSync(join(stage, "node_modules/.bin", name), args, {
      cwd: stage,
      stdio: "inherit",
    });
  }
  function artifacts(): Buffer[] {
    const dir = join(stage, "dist");
    return readdirSync(dir, { withFileTypes: true, recursive: true })
      .filter((entry) => entry.isFile())
      .map((entry) => readFileSync(join(entry.parentPath, entry.name)));
  }
  tool("vite", "build");
  const clean = artifacts();
  assert(
    clean.length > 0 && clean.every((file) => !file.includes("__dui")),
    "production bundle contains a probe or is empty",
  );

  const scan = execFileSync(binary, ["scan", stage, join(stage, "dui.json")], {
    encoding: "utf8",
  });
  const {
    facts,
    diagnostics,
  }: { facts: InstrumentFact[]; diagnostics: unknown[] } = JSON.parse(scan);
  assert.deepEqual(diagnostics, []);
  assert(
    facts.some((f) => f.kind === "slot") &&
      facts.some((f) => f.file === "guards.tsx" && f.kind === "branch"),
  );
  const require = createRequire(join(stage, "package.json"));
  const vite = await import(pathToFileURL(require.resolve("vite")).href);
  for (const file of new Set(facts.map((f) => f.file))) {
    const path = join(stage, file);
    writeFileSync(
      path,
      instrument(
        readFileSync(path, "utf8"),
        facts.filter((f) => f.file === file),
        vite,
      ),
    );
  }
  tool("tsc", "--project", "tsconfig.json", "--noEmit");
  tool("vite", "build");
  assert(
    artifacts().some((file) => file.includes("__duiProbe")),
    "development bundle lost its probes",
  );
} finally {
  rmSync(stage, { recursive: true, force: true });
}
