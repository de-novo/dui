use std::{
    env,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .expect("read canvas dist dir")
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out);
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .expect("strip prefix")
                .to_string_lossy()
                .replace('\\', "/");
            out.push((format!("/{rel}"), path));
        }
    }
}

fn mime_for(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") || path.ends_with(".mjs") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else {
        "application/octet-stream"
    }
}

fn resolve_version_tag(root: &Path) -> String {
    if let Ok(explicit) = env::var("DUI_VERSION")
        && !explicit.trim().is_empty()
    {
        let trimmed = explicit.trim();
        return if trimmed.starts_with('v') {
            trimmed.to_owned()
        } else {
            format!("v{trimmed}")
        };
    }
    let exact = Command::new("git")
        .args(["describe", "--tags", "--exact-match", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|t| t.starts_with('v'));
    if let Some(tag) = exact {
        return tag;
    }
    let nearest = Command::new("git")
        .args([
            "describe",
            "--tags",
            "--match",
            "v[0-9]*.[0-9]*.[0-9]*",
            "--abbrev=0",
        ])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|t| t.starts_with('v'));
    nearest.unwrap_or_else(|| format!("v{}", env!("CARGO_PKG_VERSION")))
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    println!("cargo:rerun-if-env-changed=DUI_VERSION");
    println!("cargo:rustc-env=DUI_VERSION={}", resolve_version_tag(&root));

    for path in [
        "adapters",
        "tsconfig.json",
        "package.json",
        "package-lock.json",
        "node_modules/typescript/package.json",
        "apps/canvas/src",
        "apps/canvas/index.html",
        "apps/canvas/vite.config.ts",
        "apps/canvas/package.json",
        "apps/canvas/package-lock.json",
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
        .current_dir(&root)
        .args(["--project", "tsconfig.json", "--outDir"])
        .arg(&out_dir)
        .status()
        .expect("could not run TypeScript");
    assert!(status.success(), "TypeScript adapter build failed");

    // Build and embed the React + React Flow viewer (`apps/canvas`).
    let canvas_dir = root.join("apps/canvas");
    let vite_bin = canvas_dir.join("node_modules/vite/bin/vite.js");
    assert!(
        vite_bin.is_file(),
        "Install apps/canvas dependencies with npm ci --prefix apps/canvas"
    );
    let canvas_out = out_dir.join("canvas-dist");
    let canvas_status = Command::new("node")
        .arg(&vite_bin)
        .current_dir(&canvas_dir)
        .args(["build", "--emptyOutDir", "--outDir"])
        .arg(&canvas_out)
        .status()
        .expect("could not run Vite to build apps/canvas");
    assert!(canvas_status.success(), "apps/canvas Vite build failed");

    let mut files = Vec::new();
    collect_files(&canvas_out, &canvas_out, &mut files);

    let generated = out_dir.join("canvas_assets.rs");
    let mut file = File::create(&generated).expect("create canvas_assets.rs");
    writeln!(
        file,
        "pub fn asset(path: &str) -> Option<(&'static [u8], &'static str)> {{"
    )
    .expect("write header");
    writeln!(file, "    match path {{").expect("write match");
    for (url_path, disk_path) in &files {
        let mime = mime_for(url_path);
        let disk_str = disk_path.to_string_lossy();
        if url_path == "/index.html" {
            writeln!(
                file,
                "        \"/\" | \"/index.html\" => Some((include_bytes!({disk_str:?}), {mime:?})),"
            )
            .expect("write index arm");
        } else {
            writeln!(
                file,
                "        {url_path:?} => Some((include_bytes!({disk_str:?}), {mime:?})),"
            )
            .expect("write asset arm");
        }
    }
    writeln!(file, "        _ => None,").expect("write fallback");
    writeln!(file, "    }}").expect("write end match");
    writeln!(file, "}}").expect("write end fn");
}
