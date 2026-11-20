import assert from "node:assert/strict";
import { globSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import test from "node:test";

const root = resolve(import.meta.dirname, "../../..");
const locks = [
  "package-lock.json",
  ...globSync("{adapters,apps,fixtures,packages}/*/package-lock.json", { cwd: root }),
];

for (const file of locks) {
  test(`build-only license exceptions in ${file}`, async () => {
    const { packages } = JSON.parse(await readFile(resolve(root, file), "utf8"));
    const tool = packages["node_modules/lightningcss"];
    const cssPackages = new Set(["lightningcss", ...Object.keys(tool?.optionalDependencies ?? {})]);
    for (const [path, entry] of Object.entries(packages) as [
      string,
      { license?: string; name?: string; dev?: boolean; optional?: boolean },
    ][]) {
      const license = entry.license ?? "";
      if (!license.includes("MPL") && !license.includes("0BSD")) continue;
      const name = path.split("node_modules/").at(-1)!;
      assert.equal(entry.name ?? name, name, path);
      assert.equal(entry.dev, true, path);
      if (license === "0BSD") {
        assert.equal(name, "tslib", path);
        assert.equal(entry.optional, true, path);
      } else {
        assert.ok(license === "MPL-2.0" && cssPackages.has(name), path);
      }
      for (const kind of [
        "dependencies",
        "devDependencies",
        "optionalDependencies",
        "peerDependencies",
      ]) {
        assert.ok(!(name in (packages[""][kind] ?? {})), `Expected transitive tool: ${name}`);
      }
    }
  });
}
