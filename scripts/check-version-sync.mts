import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const SEMVER_RE = /^\d+\.\d+\.\d+$/;

function readCargoVersion(relPath: string): string {
  const text = fs.readFileSync(path.join(root, relPath), "utf8");
  const match = text.match(/^version\s*=\s*"([^"]+)"/m);
  assert.ok(match, `${relPath} is missing a version field`);
  return match[1];
}

function readLatestChangelogVersion(relPath: string): string {
  const text = fs.readFileSync(path.join(root, relPath), "utf8");
  const match = text.match(/^## \[v([^\]]+)\]/m);
  assert.ok(match, `${relPath} is missing a '## [vX.Y.Z]' release heading`);
  return match[1];
}

const workspaceVersion = readCargoVersion("Cargo.toml");
assert.match(
  workspaceVersion,
  SEMVER_RE,
  `Cargo.toml version '${workspaceVersion}' must be strict SemVer (X.Y.Z)`,
);

const desktopVersion = readCargoVersion("apps/desktop/Cargo.toml");
assert.equal(
  desktopVersion,
  workspaceVersion,
  `apps/desktop/Cargo.toml (${desktopVersion}) must match Cargo.toml (${workspaceVersion})`,
);

for (const changelog of ["CHANGELOG.md", "docs/ko/changelog.md", "docs/zh-CN/changelog.md"]) {
  const latest = readLatestChangelogVersion(changelog);
  assert.equal(
    latest,
    workspaceVersion,
    `${changelog} latest release (v${latest}) must match Cargo.toml (v${workspaceVersion})`,
  );
}

const cargoLock = fs.readFileSync(path.join(root, "Cargo.lock"), "utf8");
for (const pkg of ["dui", "dui-core"]) {
  const pkgRe = new RegExp(`name = "${pkg}"\\nversion = "([^"]+)"`);
  const match = cargoLock.match(pkgRe);
  assert.ok(match, `Cargo.lock missing package '${pkg}'`);
  assert.equal(
    match[1],
    workspaceVersion,
    `Cargo.lock ${pkg} (${match[1]}) must match Cargo.toml (${workspaceVersion})`,
  );
}

console.log(
  JSON.stringify({
    status: "ok",
    version: workspaceVersion,
    checked: [
      "Cargo.toml",
      "Cargo.lock",
      "apps/desktop/Cargo.toml",
      "CHANGELOG.md",
      "docs/ko/changelog.md",
      "docs/zh-CN/changelog.md",
    ],
  }),
);
