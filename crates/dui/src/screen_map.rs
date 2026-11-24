use crate::{Reply, archive, failure, replay::io};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

pub fn capture(project: &Path) -> Reply {
    capture_target(project, None)
}

pub fn capture_target(project: &Path, url: Option<&str>) -> Reply {
    let project = project.canonicalize().map_err(io)?;
    if url.is_none()
        && let Some((app, adapter)) = crate::native_map::target(&project)?
    {
        let inventory = adapter.discover(&app)?;
        return archive::publish(archive::provenance(&app)?, |out| inventory.archive(out));
    }
    let work = tempfile::tempdir().map_err(io)?;
    let manifest = work.path().join("map.json");
    if let Some(url) = url {
        let profile = url_manifest(&project, url)?;
        fs::write(&manifest, serde_json::to_vec(&profile).map_err(io)?).map_err(io)?;
    } else {
        discover(&project, &manifest)?;
    }
    archive::capture(
        &project,
        &manifest,
        None,
        archive::Capture::Run {
            force: url.is_some(),
        },
    )
}

fn url_manifest(project: &Path, url: &str) -> Reply {
    let (_, path) = crate::universal_web::parse_loopback_url(url).map_err(|e| failure(2, &e))?;
    let mut profile = match fs::read(project.join("dui.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| failure(2, &format!("invalid dui.json: {e}")))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(io(e)),
    };
    let fields = profile
        .as_object_mut()
        .ok_or_else(|| failure(2, "dui.json must be an object"))?;
    fields.insert("host".into(), json!("url"));
    fields.insert(
        "surface".into(),
        json!(dui_core::surface::SURFACE_UNIVERSAL_WEB),
    );
    fields.insert("url".into(), json!(url));
    fields.insert("path".into(), json!(path));
    fields.remove("rendererUrl");
    apply_discovery_defaults(&mut profile);
    // The generated manifest lives outside the project. Keep profile resources
    // relative to the user's dui.json when passing them to the existing renderer.
    for key in ["storageState", "mockPreload", "localesDir"] {
        if let Some(rel) = profile[key].as_str() {
            profile[key] = json!(project.join(rel));
        }
    }
    if let Some(packs) = profile["rulePacks"].as_array_mut() {
        for pack in packs {
            if let Some(rel) = pack.as_str() {
                *pack = json!(project.join(rel));
            }
        }
    }
    Ok(profile)
}

fn package_has_dep(pkg: &Value, name: &str) -> bool {
    pkg["dependencies"].get(name).is_some() || pkg["devDependencies"].get(name).is_some()
}

fn has_config_prefix(project: &Path, prefix: &str) -> bool {
    ["ts", "mts", "cts", "js", "mjs", "cjs"]
        .iter()
        .any(|ext| project.join(format!("{prefix}.{ext}")).is_file())
}

/// Detect non-Vite web hosts (Next.js App Router, plain static HTML, or explicit `host` in `dui.json`)
/// so `dui map .` and `dui scan .` work zero-config across any web stack without requiring `vite`.
fn detect_non_vite_manifest(project: &Path) -> Option<Value> {
    let profile: Value = fs::read(project.join("dui.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| json!({}));

    let explicit_host = profile["host"].as_str();
    if let Some(host) = explicit_host {
        if host != "vite" {
            let mut merged = profile.clone();
            if merged.get("widths").is_none() {
                merged["widths"] = json!([375, 1280]);
            }
            if merged.get("routeSelector").is_none()
                && matches!(host, "next" | "nextjs" | "static" | "url" | "command")
            {
                merged["routeSelector"] = json!("a[href]");
            }
            return Some(merged);
        }
        return None;
    }

    let app_root = profile["appRoot"]
        .as_str()
        .map(|rel| project.join(rel))
        .unwrap_or_else(|| project.to_path_buf());
    let pkg: Value = fs::read(app_root.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);

    let has_vite = package_has_dep(&pkg, "vite") || has_config_prefix(&app_root, "vite.config");
    let has_next = package_has_dep(&pkg, "next") || has_config_prefix(&app_root, "next.config");
    let has_static_html =
        app_root.join("index.html").is_file() || app_root.join("public/index.html").is_file();

    let detected_host = if has_next && !has_vite {
        Some("next")
    } else if !has_vite && has_static_html && pkg.is_null() {
        Some("static")
    } else {
        None
    }?;

    let mut manifest = if profile.is_object() {
        profile
    } else {
        json!({})
    };
    manifest["host"] = json!(detected_host);
    apply_discovery_defaults(&mut manifest);
    Some(manifest)
}

fn apply_discovery_defaults(manifest: &mut Value) {
    if manifest.get("widths").is_none() {
        manifest["widths"] = json!([375, 1280]);
    }
    if manifest.get("routeSelector").is_none() {
        manifest["routeSelector"] = json!("a[href]");
    }
    if manifest.get("discoverOverlays").is_none() {
        manifest["discoverOverlays"] = json!(true);
    }
}

pub(crate) fn discover(project: &Path, manifest: &Path) -> Result<(), (u8, serde_json::Value)> {
    if let Some(non_vite) = detect_non_vite_manifest(project) {
        let bytes = serde_json::to_vec_pretty(&non_vite).map_err(io)?;
        return fs::write(manifest, bytes).map_err(io);
    }
    let work = tempfile::tempdir().map_err(io)?;
    let discover = work.path().join("discover.mjs");
    fs::write(
        &discover,
        include_str!(concat!(env!("OUT_DIR"), "/vite/discover.mjs")),
    )
    .map_err(io)?;
    fs::write(
        work.path().join("parser.mjs"),
        include_str!(concat!(env!("OUT_DIR"), "/vite/parser.mjs")),
    )
    .map_err(io)?;
    let result = Command::new("node")
        .arg(discover)
        .arg(project)
        .arg(include_str!("../../../rules/web.json"))
        .output()
        .map_err(io)?;
    if !result.status.success() {
        return Err(failure(3, &String::from_utf8_lossy(&result.stderr)));
    }
    fs::write(manifest, result.stdout).map_err(io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_nextjs_and_static_projects_without_requiring_vite() {
        let next_dir = tempfile::tempdir().expect("tempdir");
        fs::write(
            next_dir.path().join("package.json"),
            r#"{"name":"demo-next","dependencies":{"next":"16.4.6","react":"19.3.1"}}"#,
        )
        .expect("write package.json");
        let next_manifest = detect_non_vite_manifest(next_dir.path()).expect("detected next");
        assert_eq!(next_manifest["host"], "next");
        assert_eq!(next_manifest["routeSelector"], "a[href]");

        let static_dir = tempfile::tempdir().expect("tempdir");
        fs::write(
            static_dir.path().join("index.html"),
            "<!doctype html><h1>Hello</h1>",
        )
        .expect("write index.html");
        let static_manifest = detect_non_vite_manifest(static_dir.path()).expect("detected static");
        assert_eq!(static_manifest["host"], "static");

        let vite_dir = tempfile::tempdir().expect("tempdir");
        fs::write(
            vite_dir.path().join("package.json"),
            r#"{"name":"demo-vite","devDependencies":{"vite":"8.0.0"}}"#,
        )
        .expect("write package.json");
        assert!(detect_non_vite_manifest(vite_dir.path()).is_none());
    }
}
