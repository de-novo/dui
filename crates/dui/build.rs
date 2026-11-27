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

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn build_version(root: &Path) -> String {
    let base = env!("CARGO_PKG_VERSION");
    let expected = format!("v{base}");
    let requested = env::var("DUI_VERSION").unwrap_or_default();
    // An archive nested inside another checkout must not inherit its tags.
    let checkout = root.join(".git").exists();
    if checkout {
        let mut paths = vec![
            "HEAD".to_owned(),
            "refs/tags".to_owned(),
            "packed-refs".to_owned(),
        ];
        if let Some(branch) = git(root, &["symbolic-ref", "-q", "HEAD"]) {
            paths.push(branch);
        }
        for name in paths {
            if let Some(path) = git(
                root,
                &["rev-parse", "--path-format=absolute", "--git-path", &name],
            ) {
                let path = Path::new(&path);
                // A packed branch has no loose ref yet. Its next commit creates one.
                let watch = path.ancestors().find(|candidate| candidate.exists());
                if let Some(watch) = watch
                    && (watch == path || name.starts_with("refs/"))
                {
                    println!("cargo:rerun-if-changed={}", watch.display());
                }
            }
        }
        let tags = git(root, &["tag", "--points-at", "HEAD"]).expect("read Git tags at HEAD");
        let mut release = false;
        for tag in tags.lines().filter(|tag| {
            tag.starts_with('v') && dui_core::version::SemVerTag::parse(tag).is_some()
        }) {
            assert_eq!(
                tag, expected,
                "release tag must match the Cargo workspace version"
            );
            release = true;
        }
        if release {
            assert!(
                requested.is_empty() || requested == expected || requested == base,
                "DUI_VERSION must match the release tag"
            );
            return base.to_owned();
        }
    }
    assert!(
        requested.is_empty(),
        "DUI_VERSION requires a matching stable tag at HEAD"
    );
    if checkout {
        let sha = git(root, &["rev-parse", "--short=12", "HEAD"]).expect("read build revision");
        format!("{base}-dev+{sha}")
    } else {
        format!("{base}-dev")
    }
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    println!("cargo:rerun-if-env-changed=DUI_VERSION");
    println!("cargo:rustc-env=DUI_VERSION={}", build_version(&root));

    for path in [
        "adapters",
        "tsconfig.json",
        "package.json",
        "package-lock.json",
        "node_modules/typescript/package.json",
        "apps/canvas/src",
        "schemas/source-model.v1.schema.json",
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
