use std::{env, path::Path, process::Command};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for path in [
        "adapters",
        "tsconfig.json",
        "package.json",
        "package-lock.json",
        "node_modules/typescript/package.json",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
    }
    let compiler = root.join("node_modules/typescript/bin/tsc");
    let version = Command::new("node")
        .arg(&compiler)
        .arg("--version")
        .output()
        .expect("Node is required to build the adapters");
    assert!(
        version.status.success()
            && String::from_utf8_lossy(&version.stdout).starts_with("Version 7."),
        "Install the project's TypeScript 7 build dependencies with npm ci"
    );
    let status = Command::new("node")
        .arg(compiler)
        .current_dir(root)
        .args(["--project", "tsconfig.json", "--outDir"])
        .arg(env::var_os("OUT_DIR").unwrap())
        .status()
        .expect("could not run TypeScript");
    assert!(status.success(), "TypeScript adapter build failed");
}
