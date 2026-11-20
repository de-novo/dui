// Build a native Unix distribution with the project's locked dependencies.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

interface CargoPackage {
  name: string;
  version: string;
  source: string | null;
  manifest_path: string;
  license_file: string | null;
}

process.chdir(fileURLToPath(new URL("../", import.meta.url)));
const output = (command: string, args: string[]) =>
  execFileSync(command, args, { encoding: "utf8" }).trim();
const host = output("rustc", ["-vV"]).match(/^host: (\S+)$/m)?.[1];
if (!host) throw new Error("rustc did not report a host target");
const metadata: { packages: CargoPackage[]; target_directory: string } = JSON.parse(
  output("cargo", ["metadata", "--locked", "--format-version", "1", "--filter-platform", host]),
);
const cli = metadata.packages.find((pkg) => pkg.source === null && pkg.name === "dui");
if (!cli) throw new Error("dui package missing from Cargo workspace");

execFileSync("cargo", ["build", "--release", "--locked", "--bin", cli.name, "--target", host], {
  stdio: "inherit",
});
const name = `dui-${cli.version}-${host}`;
const filename = name + ".tar.gz";
const out = join(metadata.target_directory, "dist");
mkdirSync(out, { recursive: true });
const stage = mkdtempSync(join(out, ".package-"));
try {
  const dir = join(stage, name);
  mkdirSync(dir);
  copyFileSync(join(metadata.target_directory, host, "release", "dui"), join(dir, "dui"));
  copyFileSync("LICENSE", join(dir, "LICENSE"));
  copyFileSync("docs/binary-install.md", join(dir, "README.md"));

  const notices = join(dir, "licenses");
  mkdirSync(notices);
  for (const pkg of metadata.packages.filter((pkg) => pkg.source !== null)) {
    const source = dirname(pkg.manifest_path);
    const files = new Set(
      readdirSync(source, { withFileTypes: true })
        .filter(
          (entry) =>
            entry.isFile() &&
            /^(licen[sc]e|copying|copyright|notice|unlicense)([-._]|$)/i.test(entry.name),
        )
        .map((entry) => entry.name),
    );
    if (pkg.license_file) files.add(pkg.license_file);
    if (!files.size) throw new Error(`No license file found for ${pkg.name} ${pkg.version}`);
    const dest = join(notices, `${pkg.name}-${pkg.version}`);
    mkdirSync(dest);
    for (const file of files) copyFileSync(join(source, file), join(dest, basename(file)));
  }
  const rust = join(
    output("rustc", ["--print", "sysroot"]),
    "share/doc/rust/COPYRIGHT-library.html",
  );
  if (existsSync(rust)) copyFileSync(rust, join(notices, "rust-std.html"));

  const archive = join(stage, filename);
  execFileSync("tar", ["-czf", archive, "-C", stage, name], {
    env: { ...process.env, COPYFILE_DISABLE: "1" },
  });
  const digest = createHash("sha256").update(readFileSync(archive)).digest("hex");
  writeFileSync(archive + ".sha256", digest + "  " + filename + "\n");
  const installer = readFileSync("scripts/install.sh.in", "utf8").replace("@VERSION@", cli.version);
  writeFileSync(join(stage, "install.sh"), installer, { mode: 0o755 });
  renameSync(archive, join(out, filename));
  renameSync(archive + ".sha256", join(out, filename + ".sha256"));
  renameSync(join(stage, "install.sh"), join(out, "install.sh"));
  console.log(join(out, filename));
} finally {
  rmSync(stage, { recursive: true, force: true });
}
