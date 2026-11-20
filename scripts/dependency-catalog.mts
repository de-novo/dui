import { execFileSync } from "node:child_process";
import { existsSync, globSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { isDeepStrictEqual } from "node:util";

type Versions = Record<string, string>;
const groups = [
  "dependencies",
  "devDependencies",
  "optionalDependencies",
  "peerDependencies",
] as const;
type Manifest = Partial<Record<(typeof groups)[number] | "overrides", Versions>>;

process.chdir(join(import.meta.dirname, ".."));
const catalog: Versions = JSON.parse(readFileSync("dependency-catalog.json", "utf8"));
const write = process.argv[2] === "--write";
const unused = new Set(Object.keys(catalog));
const errors: string[] = [];

for (const [name, version] of Object.entries(catalog)) {
  if (!/^\d+\.\d+\.\d+(?:-[\w.-]+)?(?:\+[\w.-]+)?$/.test(version)) {
    throw new Error(`dependency-catalog.json: expected an exact version for ${name}`);
  }
}

function lockMatches(file: string, manifest: Manifest): boolean {
  if (!existsSync(file)) return false;
  const { packages } = JSON.parse(readFileSync(file, "utf8")) as {
    packages: Record<string, Manifest & { version?: string }>;
  };
  for (const group of groups) {
    const versions = manifest[group] ?? {};
    if (!isDeepStrictEqual(packages[""]?.[group] ?? {}, versions)) return false;
    for (const [name, version] of Object.entries(versions)) {
      if (packages[`node_modules/${name}`]?.version !== version) return false;
    }
  }
  for (const [path, pkg] of Object.entries(packages)) {
    const name = path.split("node_modules/").at(-1)!;
    const override = manifest.overrides?.[name];
    if (override && pkg.version !== override) return false;
  }
  return true;
}

const manifests = [
  "package.json",
  ...globSync("{adapters,apps,fixtures,packages}/*/package.json").sort(),
];
for (const file of manifests) {
  const manifest: Manifest = JSON.parse(readFileSync(file, "utf8"));
  let changed = false;
  for (const group of [...groups, "overrides"] as const) {
    const versions = manifest[group];
    if (!versions) continue;
    for (const [name, current] of Object.entries(versions)) {
      const version = catalog[name];
      if (!version) throw new Error(`${file}: add ${name} to dependency-catalog.json`);
      unused.delete(name);
      if (current === version) continue;
      if (!write) errors.push(`${file}: ${group}.${name} is ${current}, expected ${version}`);
      versions[name] = version;
      changed = true;
    }
  }
  if (write && changed) writeFileSync(file, JSON.stringify(manifest, null, 2) + "\n");
  const lock = join(dirname(file), "package-lock.json");
  if (lockMatches(lock, manifest)) continue;
  if (!write) {
    errors.push(`${lock}: run npm run deps:sync`);
    continue;
  }
  const npm = process.env.npm_execpath;
  if (!npm) throw new Error("Run npm run deps:sync to update lockfiles");
  execFileSync(
    process.execPath,
    [npm, "install", "--package-lock-only", "--ignore-scripts", "--no-audit", "--no-fund"],
    {
      cwd: dirname(file),
      stdio: "inherit",
    },
  );
  if (!lockMatches(lock, manifest)) throw new Error(`${lock}: version mismatch after npm install`);
}
if (unused.size) errors.push(`Unused dependency catalog entries: ${[...unused].join(", ")}`);
if (errors.length) throw new Error(errors.join("\n"));
console.log(`Dependency catalog matches ${manifests.length} npm manifests and lockfiles.`);
