use crate::{Reply, archive, failure, replay::io};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

pub fn capture(project: &Path) -> Reply {
    capture_target(project, None)
}

pub(crate) enum WebTarget<'a> {
    Url(&'a str),
    BuildDir(&'a Path),
}

pub(crate) fn capture_target(project: &Path, target: Option<WebTarget<'_>>) -> Reply {
    let project = project.canonicalize().map_err(io)?;
    if target.is_none()
        && let Some((app, adapter)) = crate::native_map::target(&project)?
    {
        let inventory = adapter.discover(&app)?;
        let provenance = archive::provenance(&app)?;
        return archive::publish(provenance, |out| inventory.archive(out, &project));
    }
    let work = tempfile::tempdir().map_err(io)?;
    let manifest = work.path().join("map.json");
    let explicit = target.is_some();
    match target {
        None => discover(&project, &manifest)?,
        Some(target) => {
            let profile = match target {
                WebTarget::Url(url) if is_vite_project(&project) => url_with_router(&project, url)?,
                target => web_manifest(&project, target)?,
            };
            fs::write(&manifest, serde_json::to_vec(&profile).map_err(io)?).map_err(io)?;
        }
    }
    let result = archive::capture(
        &project,
        &manifest,
        None,
        archive::Capture::Run { force: explicit },
    );
    if let Ok(report) | Err((5, report)) = &result
        && let Some(line) = summary_line(report)
    {
        eprintln!("{line}");
    }
    result
}

/// One line for a person watching `dui map`: cells, wall time and the slowest phase.
fn summary_line(report: &Value) -> Option<String> {
    let timings = report.get("timings")?;
    let seconds = |ms: &Value| format!("{:.1} s", ms.as_u64().unwrap_or(0) as f64 / 1000.0);
    let cells = report["cells"].as_array().map_or(0, Vec::len);
    let mut line = format!(
        "dui map: {cells} cells ({} verified) in {}",
        report["coverage"]["verified"],
        seconds(&timings["wallMs"])
    );
    if let Some(phase) = timings["slowestPhase"].as_str() {
        line.push_str(&format!(
            "; slowest phase {phase} ({} over {} captures)",
            seconds(&timings["totals"][format!("{phase}Ms")]),
            timings["capturedCells"]
        ));
    }
    // Unverified cells by the kind of failure, most frequent first.
    let mut kinds: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for cell in report["cells"].as_array().into_iter().flatten() {
        if cell["status"] == "unverified"
            && let Some(reason) = cell["reason"].as_str()
        {
            let kind = reason.split(':').next().unwrap_or(reason);
            *kinds.entry(kind).or_default() += 1;
        }
    }
    if !kinds.is_empty() {
        let mut kinds: Vec<_> = kinds.into_iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let parts: Vec<String> = kinds.iter().map(|(k, n)| format!("{n} {k}")).collect();
        line.push_str(&format!("; unverified: {}", parts.join(", ")));
    }
    Some(line)
}

fn web_manifest(project: &Path, target: WebTarget<'_>) -> Reply {
    let mut profile = match fs::read(project.join("dui.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| failure(2, &format!("invalid dui.json: {e}")))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(io(e)),
    };
    let fields = profile
        .as_object_mut()
        .ok_or_else(|| failure(2, "dui.json must be an object"))?;
    match target {
        WebTarget::Url(url) => {
            let (_, path) =
                crate::universal_web::parse_loopback_url(url).map_err(|e| failure(2, &e))?;
            fields.insert("host".into(), json!("url"));
            fields.insert("url".into(), json!(url));
            fields.insert("path".into(), json!(path));
        }
        WebTarget::BuildDir(dir) => {
            let root = project
                .join(dir)
                .canonicalize()
                .map_err(|e| failure(2, &format!("invalid build directory: {e}")))?;
            let relative = root
                .strip_prefix(project)
                .ok()
                .filter(|_| root.is_dir())
                .ok_or_else(|| failure(2, "build directory must be inside the project"))?;
            let entry = fields.get("path").and_then(Value::as_str).unwrap_or("/");
            let fallback = fields
                .get("spaFallback")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let file = crate::universal_web::resolve_static_file(&root, entry, fallback);
            if !file.is_some_and(|path| {
                path.extension().is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm")
                })
            }) {
                return Err(failure(
                    2,
                    &format!(
                        "no built HTML page for {entry}; set path in dui.json to the build entry"
                    ),
                ));
            }
            fields.insert("host".into(), json!("static"));
            fields.insert("root".into(), json!(relative));
            fields.remove("url");
        }
    }
    fields.insert(
        "surface".into(),
        json!(dui_core::surface::SURFACE_UNIVERSAL_WEB),
    );
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
        // `"host": "url"` on a Vite project: read the router, then capture the running app.
        let profile = match non_vite["url"].as_str() {
            Some(url) if non_vite["host"] == "url" && is_vite_project(project) => {
                url_with_router(project, url)?
            }
            _ => non_vite,
        };
        let bytes = serde_json::to_vec_pretty(&profile).map_err(io)?;
        return fs::write(manifest, bytes).map_err(io);
    }
    let routed = vite_discovery(project)?;
    fs::write(manifest, serde_json::to_vec(&routed).map_err(io)?).map_err(io)
}

/// A Vite project: `vite` in the app's package.json or a `vite.config.*` file.
fn is_vite_project(project: &Path) -> bool {
    let profile: Value = fs::read(project.join("dui.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| json!({}));
    let app_root = profile["appRoot"]
        .as_str()
        .map_or_else(|| project.to_path_buf(), |rel| project.join(rel));
    let pkg: Value = fs::read(app_root.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    package_has_dep(&pkg, "vite") || has_config_prefix(&app_root, "vite.config")
}

/// Map a running Vite app at `url` with the routes its router declares. Link crawl still runs
/// on top. Without router routes this is the plain URL map.
fn url_with_router(project: &Path, url: &str) -> Reply {
    let mut profile = web_manifest(project, WebTarget::Url(url))?;
    let Ok(routed) = vite_discovery(project) else {
        return Ok(profile);
    };
    if routed["screenDiscovery"]["mode"] != "router" {
        return Ok(profile);
    }
    let requested = profile["path"].as_str().unwrap_or("/").to_owned();
    for key in [
        "path",
        "readySelector",
        "hasScreenAssertion",
        "cookies",
        "scenarios",
        "screenDiscovery",
        "linkDefaults",
        "routeSelector",
    ] {
        match routed.get(key) {
            Some(value) => profile[key] = value.clone(),
            None => {
                if let Some(fields) = profile.as_object_mut() {
                    fields.remove(key);
                }
            }
        }
    }
    // A deep URL is captured too, as one more screen.
    let routed_paths: Vec<&str> = std::iter::once(&routed["path"])
        .chain(
            routed["scenarios"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|s| &s["path"]),
        )
        .filter_map(Value::as_str)
        .collect();
    if requested != "/" && !routed_paths.contains(&requested.as_str()) {
        let screen = json!({
            "path": requested,
            "readySelector": routed["readySelector"],
            "hasScreenAssertion": routed["hasScreenAssertion"],
        });
        if let Some(scenarios) = profile["scenarios"].as_array_mut() {
            scenarios.push(screen);
        } else {
            profile["scenarios"] = json!([screen]);
        }
        profile["screenDiscovery"]["sources"][&requested] = json!({"kind": "entry"});
    }
    Ok(profile)
}

/// Run the Vite route discovery and return its capture manifest.
fn vite_discovery(project: &Path) -> Reply {
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
    serde_json::from_slice(&result.stdout).map_err(|e| failure(3, &e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_map_uses_the_project_entry_and_checks_the_resolved_html() {
        let dir = tempfile::tempdir().expect("tempdir");
        let project = dir.path().canonicalize().expect("project");
        let build = project.join("output");
        fs::create_dir_all(build.join("start")).expect("build");
        fs::write(build.join("start/index.html"), "<h1>Start</h1>").expect("entry");
        fs::write(project.join("dui.json"), r#"{"host":"url","url":"http://localhost:80","rendererUrl":"old","path":"/start","widths":[412]}"#).expect("profile");
        let manifest =
            web_manifest(&project, WebTarget::BuildDir(Path::new("output"))).expect("map");
        assert_eq!(manifest["host"], "static");
        assert_eq!(manifest["root"], "output");
        assert_eq!(manifest["path"], "/start");
        assert_eq!(manifest["widths"], json!([412]));
        assert!(manifest.get("rendererUrl").is_none());
        assert!(manifest.get("url").is_none());
        fs::remove_file(build.join("start/index.html")).expect("remove entry");
        assert_eq!(
            web_manifest(&project, WebTarget::BuildDir(&build))
                .expect_err("missing HTML")
                .0,
            2
        );
        let outside = tempfile::tempdir().expect("outside");
        assert_eq!(
            web_manifest(&project, WebTarget::BuildDir(outside.path()))
                .expect_err("outside project")
                .0,
            2
        );
        std::os::unix::fs::symlink(outside.path(), build.join("loop")).expect("link");
        assert!(crate::universal_web::collect_static_source_files(&project, "output").is_empty());
    }

    #[test]
    fn the_summary_groups_unverified_cells_by_reason() {
        let cell = |status: &str, reason: Option<&str>| json!({"status": status, "reason": reason});
        let report = json!({
            "coverage": {"verified": 1},
            "timings": {"wallMs": 12_300, "slowestPhase": "settle", "capturedCells": 5,
                        "totals": {"settleMs": 9_000}},
            "cells": [
                cell("verified", None),
                cell("unverified", Some("not-ready: h1 + unsettled: li#3 added")),
                cell("unverified", Some("not-ready: h1")),
                cell("unverified", Some("redirected: /a → /b")),
                cell("unverified", Some("api-error: 401 /api/me")),
            ],
        });
        assert_eq!(
            summary_line(&report).expect("line"),
            "dui map: 5 cells (1 verified) in 12.3 s; slowest phase settle (9.0 s over 5 captures); \
             unverified: 2 not-ready, 1 api-error, 1 redirected"
        );
    }

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
