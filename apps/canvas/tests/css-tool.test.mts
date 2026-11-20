import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("only build-time lightningcss and its declared binaries qualify for the MPL exception", async () => {
  const { packages } = JSON.parse(
    await readFile(new URL("../package-lock.json", import.meta.url), "utf8"),
  );
  const tool = packages["node_modules/lightningcss"];
  const allowed = new Set([
    "lightningcss",
    ...Object.keys(tool?.optionalDependencies ?? {}),
  ]);
  for (const [path, entry] of Object.entries(packages) as [
    string,
    { license?: string; dev?: boolean },
  ][]) {
    if (!entry.license?.includes("MPL")) continue;
    const name = path.split("node_modules/").at(-1)!;
    assert.ok(
      allowed.has(name) && entry.license === "MPL-2.0" && entry.dev === true,
      path,
    );
    for (const kind of [
      "dependencies",
      "devDependencies",
      "optionalDependencies",
      "peerDependencies",
    ]) {
      assert.ok(
        !(name in (packages[""][kind] ?? {})),
        `Expected transitive tool: ${name}`,
      );
    }
  }
});
