// Validate the workspace, lockfiles and release notes. CI also supplies public tags.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

function packages(manifest: string): { name: string; version: string }[] {
  const metadata = JSON.parse(
    execFileSync(
      "cargo",
      ["metadata", "--no-deps", "--locked", "--format-version", "1", "--manifest-path", manifest],
      { encoding: "utf8" },
    ),
  );
  return metadata.packages.filter((pkg: { id: string }) =>
    metadata.workspace_members.includes(pkg.id),
  );
}

const workspace = packages("Cargo.toml");
const version = workspace.find((pkg) => pkg.name === "dui")!.version;
const stable = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
assert.match(version, stable);
const desktop = packages("apps/desktop/Cargo.toml");
for (const [file, members] of [
  ["Cargo.lock", workspace],
  ["apps/desktop/Cargo.lock", desktop],
] as const) {
  const blocks = readFileSync(file, "utf8").split("[[package]]").slice(1);
  for (const pkg of members) {
    assert.equal(pkg.version, version, pkg.name);
    const block = blocks.find(
      (entry) => entry.includes(`\nname = "${pkg.name}"\n`) && !entry.includes("\nsource = "),
    );
    assert.ok(block?.includes(`\nversion = "${version}"\n`), `${file}: ${pkg.name}`);
  }
}

const notes = ["CHANGELOG.md", "docs/ko/changelog.md", "docs/zh-CN/changelog.md"];
let previousHeadings: string[] | undefined;
const latest = process.argv.includes("--published")
  ? execFileSync("sh", [resolve("scripts/install"), "--print-ref"], { encoding: "utf8" }).trim()
  : undefined;
for (const file of notes) {
  const contents = readFileSync(file, "utf8");
  const headings = contents.match(/^## \[.*$/gm) ?? [];
  if (previousHeadings) assert.deepEqual(headings, previousHeadings, file);
  previousHeadings = headings;
  const pending = headings[0] === `## [Unreleased] — v${version}`;
  const released = headings[1]?.match(/^## \[(v[^\]]+)\] - \d{4}-\d{2}-\d{2}$/)?.[1];
  assert.ok(released && stable.test(released.slice(1)), `${file}: missing latest dated release`);
  assert.ok(
    pending || (headings[0] === "## [Unreleased]" && released === `v${version}`),
    `${file}: workspace version must match the pending or released version`,
  );
  assert.ok(
    contents
      .split("\n")
      .some(
        (line) => line.startsWith("[Unreleased]: ") && line.endsWith(`/compare/${released}...HEAD`),
      ),
    `${file}: Unreleased comparison`,
  );
  if (latest !== undefined) {
    assert.ok(
      latest === released || (pending && latest === `v${version}`),
      `${file}: latest install ref ${latest} disagrees with ${released}`,
    );
  }
}
console.log(
  JSON.stringify({ version, crates: [...workspace, ...desktop].map((pkg) => pkg.name), notes }),
);
