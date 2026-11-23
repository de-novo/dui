import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";

interface CargoDep {
  name: string;
  kind?: string | null;
}

interface CargoPackage {
  name: string;
  dependencies: CargoDep[];
}

interface CargoMetadata {
  packages: CargoPackage[];
}

const raw = execFileSync("cargo", ["metadata", "--locked", "--no-deps", "--format-version=1"], {
  encoding: "utf8",
});
const metadata = JSON.parse(raw) as CargoMetadata;
const workspaceNames = new Set(metadata.packages.map((p) => p.name));

// Architecture §6.1 allowed workspace dependency edges:
// - dui-core: zero workspace deps
// - dui-ports: only dui-core
// - dui-app: only dui-core, dui-ports (zero concrete adapters!)
// - concrete adapters (dui-storage, dui-lang-*, dui-host-*, dui-render-*): only dui-core, dui-ports
// - dui (composition root): allowed to wire everything
const allowedWorkspaceDeps: Record<string, Set<string>> = {
  "dui-core": new Set(),
  "dui-ports": new Set(["dui-core"]),
  "dui-app": new Set(["dui-core", "dui-ports"]),
  "dui-storage": new Set(["dui-core", "dui-ports"]),
};

for (const pkg of metadata.packages) {
  const wsDeps = pkg.dependencies
    .filter((d) => workspaceNames.has(d.name) && d.kind !== "dev")
    .map((d) => d.name);

  if (pkg.name === "dui") continue;

  const allowed =
    allowedWorkspaceDeps[pkg.name] ??
    (pkg.name.startsWith("dui-lang-") ||
    pkg.name.startsWith("dui-host-") ||
    pkg.name.startsWith("dui-render-")
      ? new Set(["dui-core", "dui-ports"])
      : null);

  assert.ok(allowed, `unexpected workspace crate '${pkg.name}' missing from dependency policy`);
  for (const dep of wsDeps) {
    assert.ok(
      allowed.has(dep),
      `architecture violation: crate '${pkg.name}' may not depend on workspace crate '${dep}' (allowed: ${[...allowed].join(", ") || "none"})`,
    );
  }
}

console.log(
  JSON.stringify({
    status: "ok",
    crates: metadata.packages.map((p) => p.name).sort(),
  }),
);
