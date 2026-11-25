//! Tag-based version resolution, update check, and source-build self-update (`vX.Y.Z`).

use crate::{Reply, failure};
use dui_core::version::{SemVerTag, latest_tag, parse_ls_remote_tags};
use serde_json::json;
use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub(crate) const DEFAULT_REPO_URL: &str = "https://github.com/de-novo/dui.git";

pub(crate) const VERSION: &str = env!("DUI_VERSION");

pub(crate) fn release_tag() -> Option<String> {
    SemVerTag::parse(VERSION).map(SemVerTag::tag_string)
}

fn next_release(tags: &[SemVerTag]) -> Option<SemVerTag> {
    let base = SemVerTag::parse(env!("CARGO_PKG_VERSION")).expect("workspace version");
    latest_tag(tags).filter(|&latest| latest > base || (latest == base && release_tag().is_none()))
}

pub(crate) fn repo_url(override_url: Option<&str>) -> String {
    override_url
        .map(str::to_owned)
        .or_else(|| {
            env::var("DUI_REPO_URL")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| DEFAULT_REPO_URL.to_owned())
}

fn fetch_remote_tags(repo: &str) -> Result<Vec<SemVerTag>, (u8, serde_json::Value)> {
    let output = Command::new("git")
        .args(["ls-remote", "--tags", "--refs", repo])
        .output()
        .map_err(|e| failure(3, &format!("failed to run git ls-remote: {e}")))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(failure(3, &format!("git ls-remote failed: {}", err.trim())));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_ls_remote_tags(&stdout))
}

pub(crate) fn check(repo: &str) -> Reply {
    let current = format!("v{VERSION}");
    let tags = fetch_remote_tags(repo)?;
    let latest = latest_tag(&tags);
    let newer = next_release(&tags);
    Ok(json!({
        "current": current,
        "latest": latest.map(|t| t.tag_string()),
        "updateAvailable": newer.is_some(),
        "nextTag": newer.map(|t| t.tag_string()),
        "repo": repo,
        "tags": tags.iter().map(|t| t.tag_string()).collect::<Vec<_>>(),
    }))
}

pub(crate) fn apply(repo: &str, requested_tag: Option<&str>, install_dir: Option<&Path>) -> Reply {
    let current = format!("v{VERSION}");
    let tags = fetch_remote_tags(repo)?;
    let target_tag = match requested_tag {
        Some(raw) => SemVerTag::parse(raw)
            .ok_or_else(|| failure(2, &format!("invalid release tag: {raw}")))?,
        None => match next_release(&tags) {
            Some(newer) => newer,
            None => {
                return Ok(json!({
                    "updated": false,
                    "current": current,
                    "latest": latest_tag(&tags).map(|t| t.tag_string()),
                    "updateAvailable": false,
                    "repo": repo,
                }));
            }
        },
    };
    let tag_str = target_tag.tag_string();

    let work = tempfile::tempdir().map_err(|e| failure(3, &e.to_string()))?;
    let src_dir = work.path().join("src");
    let build_dir = work.path().join("build");

    let clone = Command::new("git")
        .args(["clone", "--depth", "1", "--branch", &tag_str, repo])
        .arg(&src_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| failure(3, &format!("failed to invoke git clone: {e}")))?;
    if !clone.status.success() {
        let err = String::from_utf8_lossy(&clone.stderr);
        return Err(failure(
            3,
            &format!("git clone {tag_str} failed: {}", err.trim()),
        ));
    }

    for args in [
        vec!["ci", "--include=dev"],
        vec!["ci", "--prefix", "apps/canvas"],
    ] {
        let npm = Command::new("npm")
            .args(&args)
            .current_dir(&src_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| failure(3, &format!("failed to invoke npm: {e}")))?;
        if !npm.status.success() {
            let err = String::from_utf8_lossy(&npm.stderr);
            return Err(failure(
                3,
                &format!("npm {} failed: {}", args.join(" "), err.trim()),
            ));
        }
    }

    let cargo = Command::new("cargo")
        .args([
            "build",
            "--locked",
            "--release",
            "--bin",
            "dui",
            "--target-dir",
        ])
        .arg(&build_dir)
        .current_dir(&src_dir)
        .env("DUI_VERSION", &tag_str)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| failure(3, &format!("failed to invoke cargo build: {e}")))?;
    if !cargo.status.success() {
        let err = String::from_utf8_lossy(&cargo.stderr);
        return Err(failure(3, &format!("cargo build failed: {}", err.trim())));
    }

    let built = build_dir.join("release").join("dui");
    if !built.is_file() {
        return Err(failure(3, "built dui binary missing"));
    }

    let dest_path: PathBuf = match install_dir {
        Some(dir) => {
            fs::create_dir_all(dir).map_err(|e| failure(3, &e.to_string()))?;
            dir.join("dui")
        }
        None => env::current_exe().map_err(|e| failure(3, &e.to_string()))?,
    };
    let dest_parent = dest_path
        .parent()
        .ok_or_else(|| failure(3, "invalid install target"))?;
    let staged =
        tempfile::NamedTempFile::new_in(dest_parent).map_err(|e| failure(3, &e.to_string()))?;
    fs::copy(&built, staged.path()).map_err(|e| failure(3, &e.to_string()))?;
    fs::set_permissions(staged.path(), fs::Permissions::from_mode(0o755))
        .map_err(|e| failure(3, &e.to_string()))?;
    staged
        .persist(&dest_path)
        .map_err(|e| failure(3, &e.to_string()))?;

    Ok(json!({
        "updated": true,
        "from": current,
        "to": tag_str,
        "binary": dest_path.display().to_string(),
        "repo": repo,
    }))
}
