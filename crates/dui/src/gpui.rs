//! GPUI (`gpui-kit` / `gpui-pre`) native renderer adapter implementing the shared `Renderer` port.

use crate::{Reply, failure, ports::Renderer};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

pub(crate) static NATIVE_LAUNCHES: AtomicU64 = AtomicU64::new(0);

pub(crate) struct GpuiRenderer {
    binary: PathBuf,
}

impl GpuiRenderer {
    pub(crate) fn new(binary: PathBuf) -> Self {
        Self { binary }
    }
}

impl Renderer for GpuiRenderer {
    fn capture(
        &mut self,
        spec: &Value,
        input: &Value,
        width: u32,
        image: &Path,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let height = spec["height"].as_u64().unwrap_or(320) as u32;
        let payload = json!({
            "width": width,
            "height": height,
            "input": input,
            "imagePath": image.display().to_string(),
            "expect": spec["expect"].as_array().cloned().unwrap_or_default(),
        });

        let mut child = Command::new(&self.binary)
            .arg("--capture")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            writeln!(stdin, "{payload}")?;
        }
        let output = child.wait_with_output()?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("gpui fixture failed ({}): {}", output.status, err.trim()).into());
        }
        let stdout = String::from_utf8(output.stdout)?;
        let obs: Value = serde_json::from_str(stdout.trim())?;
        Ok(obs)
    }
}

pub(crate) fn select_toolchain() -> Option<String> {
    if let Ok(tc) = env::var("DUI_RUST_TOOLCHAIN")
        && !tc.trim().is_empty()
    {
        return Some(tc);
    }
    let active = Command::new("rustc").arg("--version").output().ok()?;
    let active_ver = String::from_utf8_lossy(&active.stdout);
    if rustc_minor_at_least(&active_ver, 96) {
        return None;
    }
    let list = Command::new("rustup")
        .args(["toolchain", "list"])
        .output()
        .ok()?;
    if !list.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&list.stdout);
    let mut best: Option<(u32, String)> = None;
    for line in stdout.lines() {
        let name = line.split_whitespace().next().unwrap_or("");
        if let Some(minor) = parse_toolchain_minor(name)
            && minor >= 96
            && best.as_ref().is_none_or(|(m, _)| minor > *m)
        {
            best = Some((minor, name.to_owned()));
        }
    }
    best.map(|(_, name)| name)
}

fn rustc_minor_at_least(version_line: &str, min_minor: u32) -> bool {
    let token = version_line.split_whitespace().nth(1).unwrap_or("");
    let mut parts = token.split('.');
    let major: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    major > 1 || (major == 1 && minor >= min_minor)
}

fn parse_toolchain_minor(name: &str) -> Option<u32> {
    let ver = name.split('-').next()?;
    let mut parts = ver.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    (major == 1).then_some(minor)
}

fn cargo_cmd(toolchain: Option<&str>) -> Command {
    let mut cmd = Command::new("cargo");
    if let Some(tc) = toolchain {
        cmd.env("RUSTUP_TOOLCHAIN", tc);
    }
    cmd
}

pub(crate) fn rustc_version(toolchain: Option<&str>) -> String {
    let mut cmd = Command::new("rustc");
    if let Some(tc) = toolchain {
        cmd.env("RUSTUP_TOOLCHAIN", tc);
    }
    cmd.arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn cargo_version(toolchain: Option<&str>) -> String {
    cargo_cmd(toolchain)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn package_name_from_manifest(project: &Path) -> Result<String, (u8, Value)> {
    let raw = fs::read_to_string(project.join("Cargo.toml"))
        .map_err(|e| failure(2, &format!("missing Cargo.toml: {e}")))?;
    let mut in_pkg = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_pkg = trimmed == "[package]";
            continue;
        }
        if in_pkg
            && let Some(rest) = trimmed.strip_prefix("name")
            && let Some(val) = rest.trim().strip_prefix('=')
        {
            let cleaned = val.trim().trim_matches('"');
            if !cleaned.is_empty() {
                return Ok(cleaned.to_owned());
            }
        }
    }
    Err(failure(2, "Cargo.toml missing [package] name"))
}

pub(crate) fn lockfile_versions(project: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(raw) = fs::read_to_string(project.join("Cargo.lock")) else {
        return out;
    };
    let mut name: Option<String> = None;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            name = None;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("name = ") {
            name = Some(rest.trim_matches('"').to_owned());
        } else if let Some(rest) = trimmed.strip_prefix("version = ")
            && let Some(pkg) = name.as_deref()
            && matches!(
                pkg,
                "gpui-kit" | "gpui-pre" | "gpui-base" | "gpui-component"
            )
        {
            out.insert(pkg.to_owned(), rest.trim_matches('"').to_owned());
        }
    }
    out
}

pub(crate) fn build_fixture(
    project: &Path,
    toolchain: Option<&str>,
) -> Result<PathBuf, (u8, Value)> {
    crate::host::HOST_BUILDS.fetch_add(1, Ordering::Relaxed);
    let bin_name = package_name_from_manifest(project)?;
    let bin_path = project.join("target").join("debug").join(&bin_name);
    let manifest_path = project.join("Cargo.toml");
    let status = cargo_cmd(toolchain)
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| failure(3, &format!("failed to invoke cargo: {e}")))?;
    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        return Err(failure(
            3,
            &format!("gpui fixture build failed: {}", stderr.trim()),
        ));
    }
    if !bin_path.is_file() {
        return Err(failure(
            3,
            &format!("expected gpui binary at {}", bin_path.display()),
        ));
    }
    Ok(bin_path)
}

fn state_hash(path: &str, name: &str, input: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(json!(["gpui", path, name, input]).to_string().as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) fn is_gpui_manifest(spec: &Value) -> bool {
    matches!(spec["runtime"].as_str(), Some("gpui-kit" | "gpui"))
}

pub(crate) fn run(project: &Path, manifest: &Path, out: &Path) -> Reply {
    let spec: Value =
        serde_json::from_slice(&fs::read(manifest).map_err(|_| failure(2, "missing spec"))?)
            .map_err(|_| failure(2, "invalid spec"))?;
    let path = spec["path"].as_str().unwrap_or("/").to_owned();
    let widths: Vec<u32> = spec["widths"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_u64().and_then(|n| u32::try_from(n).ok()))
        .filter(|&w| w > 0)
        .collect();
    if widths.is_empty() {
        return Err(failure(2, "positive widths required"));
    }
    let states = spec["states"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| failure(2, "states array required for gpui-kit fixture"))?;

    fs::create_dir_all(out).map_err(|e| failure(3, &e.to_string()))?;
    fs::write(
        out.join("manifest.json"),
        serde_json::to_vec_pretty(&spec).map_err(|e| failure(3, &e.to_string()))?,
    )
    .map_err(|e| failure(3, &e.to_string()))?;

    let toolchain = select_toolchain();
    let binary = build_fixture(project, toolchain.as_deref())?;
    let versions = lockfile_versions(project);
    let rustc = rustc_version(toolchain.as_deref());
    let cargo = cargo_version(toolchain.as_deref());

    let mut renderer = GpuiRenderer::new(binary);
    let mut cells = Vec::new();
    let mut verified = 0usize;
    let mut unrenderable = 0usize;
    let mut render_path = String::new();
    let mut element_tree_available = false;

    for (index, state_def) in states.iter().enumerate() {
        let name = state_def["name"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("state-{index}"));
        let input = &state_def["input"];
        let state_id = state_hash(&path, &name, input);

        let mut cell_spec = spec.clone();
        cell_spec["expect"] = state_def["expect"].clone();

        for &width in &widths {
            let staged =
                tempfile::NamedTempFile::new_in(out).map_err(|e| failure(3, &e.to_string()))?;
            let obs = renderer
                .capture(&cell_spec, input, width, staged.path())
                .map_err(|e| failure(3, &e.to_string()))?;

            if render_path.is_empty()
                && let Some(rp) = obs["renderPath"].as_str()
            {
                render_path = rp.to_owned();
            }
            if obs["elementTreeAvailable"] == json!(true) {
                element_tree_available = true;
            }

            let status = obs["status"].as_str().unwrap_or("unverified");
            let reason = obs.get("reason").cloned().unwrap_or(Value::Null);

            if status == "unrenderable" {
                unrenderable += 1;
                cells.push(json!({
                    "scenario": name,
                    "path": path,
                    "width": width,
                    "input": input,
                    "state": state_id,
                    "planIndex": index,
                    "status": "unrenderable",
                    "reason": reason,
                    "elementTreeAvailable": obs["elementTreeAvailable"],
                    "observation": obs,
                }));
                continue;
            }

            let bytes = fs::read(staged.path()).map_err(|e| failure(3, &e.to_string()))?;
            let image_name = format!("{:x}-{width}.png", Sha256::digest(&bytes));
            staged
                .persist(out.join(&image_name))
                .map_err(|e| failure(3, &e.to_string()))?;

            if status == "verified" {
                verified += 1;
            }
            cells.push(json!({
                "scenario": name,
                "path": path,
                "width": width,
                "input": input,
                "state": state_id,
                "planIndex": index,
                "image": image_name,
                "status": status,
                "reason": reason,
                "elementTreeAvailable": obs["elementTreeAvailable"],
                "observation": obs,
            }));
        }
    }

    let renderable = cells.len() - unrenderable;
    let report = json!({
        "runtime": "gpui-kit",
        "os": env::consts::OS,
        "arch": env::consts::ARCH,
        "toolchain": {
            "rustc": rustc,
            "cargo": cargo,
        },
        "versions": versions,
        "renderPath": if render_path.is_empty() { "unknown" } else { &render_path },
        "elementTreeAvailable": element_tree_available,
        "path": path,
        "widths": widths,
        "coverage": {
            "verified": verified,
            "renderable": renderable,
            "attempted": renderable,
            "unrenderable": unrenderable,
            "branchOutcomes": 0,
            "observedBranchOutcomes": 0,
        },
        "cells": cells,
    });

    let base_graph = json!({
        "nodes": [{
            "id": format!("route:{path}"),
            "kind": "screen",
            "path": path,
            "label": path,
        }],
        "edges": [],
    });
    let graph = crate::graph_view::with_cells(base_graph, &report)?;
    crate::graph_view::write(out, &graph)?;
    fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| failure(3, &e.to_string()))?,
    )
    .map_err(|e| failure(3, &e.to_string()))?;
    fs::write(
        out.join("graph.json"),
        serde_json::to_vec_pretty(&graph).map_err(|e| failure(3, &e.to_string()))?,
    )
    .map_err(|e| failure(3, &e.to_string()))?;

    if verified == renderable && unrenderable == 0 {
        Ok(report)
    } else {
        Err((5, report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_gpui_manifests_and_toolchain_versions() {
        assert!(is_gpui_manifest(&json!({"runtime": "gpui-kit"})));
        assert!(is_gpui_manifest(&json!({"runtime": "gpui"})));
        assert!(!is_gpui_manifest(&json!({"tsconfig": "tsconfig.json"})));

        assert!(rustc_minor_at_least(
            "rustc 1.100.0 (0ef1bc386 2026-11-09)",
            96
        ));
        assert!(!rustc_minor_at_least(
            "rustc 1.93.1 (01f6ddf75 2026-02-11)",
            96
        ));
        assert_eq!(
            parse_toolchain_minor("1.100.0-aarch64-apple-darwin"),
            Some(100)
        );
        assert_eq!(parse_toolchain_minor("stable-aarch64-apple-darwin"), None);
    }

    #[test]
    fn reads_gpui_kit_and_gpui_pre_versions_from_fixture_lockfile() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gpui-kit");
        let versions = lockfile_versions(&fixture);
        assert_eq!(versions.get("gpui-kit").map(String::as_str), Some("0.10.0"));
        assert!(
            versions
                .get("gpui-pre")
                .is_some_and(|v| v.starts_with("0.3.")),
            "expected gpui-pre 0.3.x in lockfile, got {versions:?}"
        );
    }
}
