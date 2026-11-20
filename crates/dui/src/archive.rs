//! Capture storage shared by the CLI and MCP server.
use crate::{Reply, analysis, failure, host, replay};
use replay::{Error, io, read};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, symlink},
    },
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) enum Capture<'a> {
    Run,
    Replay(&'a Path, Option<u16>),
    Exercise(u16),
}

fn root() -> Result<PathBuf, Error> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| failure(3, "HOME must be an absolute path"))?;
    let dir = home.join(".dui");
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .map_err(io)?;
    dir.canonicalize().map_err(io)
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .trim_end_matches('\n')
            .to_owned()
    })
}

pub(crate) fn provenance(project: &Path) -> Result<Value, Error> {
    let app = project
        .canonicalize()
        .map_err(|_| failure(2, "project directory does not exist"))?;
    if !app.is_dir() {
        return Err(failure(2, "project is not a directory"));
    }
    let worktree = git(&app, &["rev-parse", "--show-toplevel"]).map(PathBuf::from);
    let mut repo = app.clone();
    let mut branch = None;
    let mut commit = None;
    let mut dirty = None;
    if let Some(dir) = &worktree {
        let trees = git(dir, &["worktree", "list", "--porcelain", "-z"])
            .ok_or_else(|| failure(3, "cannot identify primary worktree"))?;
        repo = trees
            .split('\0')
            .next()
            .and_then(|s| s.strip_prefix("worktree "))
            .map(PathBuf::from)
            .ok_or_else(|| failure(3, "invalid worktree list"))?;
        branch = git(dir, &["symbolic-ref", "--quiet", "--short", "HEAD"]);
        commit = git(dir, &["rev-parse", "--verify", "HEAD"]);
        dirty =
            git(dir, &["status", "--porcelain", "--untracked-files=normal"]).map(|s| !s.is_empty());
    }
    let project_id = format!("{:x}", Sha256::digest(repo.as_os_str().as_bytes()));
    let relative = app
        .strip_prefix(worktree.as_ref().unwrap_or(&app))
        .map_err(io)?;
    Ok(
        json!({"projectId":project_id,"projectRoot":repo,"app":relative,"worktree":worktree,
        "branch":branch,"commit":commit,"dirty":dirty}),
    )
}

// A project owns one display name. Concurrent first captures use the same winner.
fn project_name(directory: &Path, repo: &Path) -> Result<String, Error> {
    let record = directory.join("project.json");
    let load = || -> Result<String, Error> {
        read(&record)?["name"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(|| failure(3, "invalid archived project name"))
    };
    if record.try_exists().map_err(io)? {
        return load();
    }
    let manifest = read(&repo.join("package.json")).ok();
    let name = manifest
        .as_ref()
        .and_then(|m| m["name"].as_str())
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .or_else(|| repo.file_name().and_then(|n| n.to_str()))
        .unwrap_or("Unnamed project");
    let staged = tempfile::NamedTempFile::new_in(directory).map_err(io)?;
    serde_json::to_writer(staged.as_file(), &json!({ "name": name })).map_err(io)?;
    match staged.persist_noclobber(&record) {
        Ok(_) => Ok(name.to_owned()),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => load(),
        Err(e) => Err(io(e)),
    }
}

// A legacy OUTPUT is a live pointer. Completed runs stay in the home archive.
fn link_to(source: &Path, dest: &Path) -> Result<(), Error> {
    let parent = dest
        .parent()
        .ok_or_else(|| failure(2, "invalid output path"))?;
    let stage = tempfile::tempdir_in(parent).map_err(io)?;
    let link = stage.path().join("link");
    symlink(source, &link).map_err(io)?;
    fs::rename(link, dest).map_err(io)
}

fn expose(out: &Path, dest: &Path, archive: &Path) -> Result<(), Error> {
    let parent = dest
        .parent()
        .ok_or_else(|| failure(2, "invalid output path"))?;
    fs::create_dir_all(parent).map_err(io)?;
    if dest.is_symlink() && fs::read_link(dest).map_err(io)?.starts_with(archive) || !dest.exists()
    {
        return link_to(out, dest);
    }
    // Older versions created real directories. Preserve unrelated files.
    for entry in fs::read_dir(out).map_err(io)? {
        let entry = entry.map_err(io)?;
        link_to(&entry.path(), &dest.join(entry.file_name()))?;
    }
    Ok(())
}

pub(crate) fn capture(
    project: &Path,
    manifest: &Path,
    alias: Option<&Path>,
    action: Capture<'_>,
) -> Reply {
    let metadata = provenance(project)?;
    let home = root()?;
    let archive = home.join("projects");
    let alias = alias.map(std::path::absolute).transpose().map_err(io)?;
    if let Some(dest) = &alias {
        let parent = dest
            .parent()
            .ok_or_else(|| failure(2, "invalid output path"))?;
        fs::create_dir_all(parent).map_err(io)?;
        let parent = parent.canonicalize().map_err(io)?;
        let name = dest
            .file_name()
            .ok_or_else(|| failure(2, "invalid output path"))?;
        let resolved = parent.join(name);
        if resolved.starts_with(&home)
            || home.starts_with(&resolved)
            || project.canonicalize().map_err(io)?.starts_with(&resolved)
        {
            return Err(failure(2, "use a separate output directory or omit OUTPUT"));
        }
        if matches!(action, Capture::Replay(..)) && dest.symlink_metadata().is_ok() {
            return Err(failure(2, "use a new output directory"));
        }
        if dest.is_symlink() && !fs::read_link(dest).map_err(io)?.starts_with(&archive) {
            return Err(failure(2, "output symlink does not belong to dui"));
        }
        if dest.exists() && !dest.is_dir() {
            return Err(failure(2, "output must be a directory"));
        }
    }
    let result = publish(metadata, |staged| match action {
        Capture::Run => host::run(project, manifest, staged),
        Capture::Replay(plan, port) => replay::run(project, manifest, plan, staged, port),
        Capture::Exercise(port) => analysis::exercise(project, manifest, staged, port),
    });
    let report = match &result {
        Ok(body) | Err((5, body)) => body,
        Err(_) => return result,
    };
    if let Some(dest) = alias {
        let out = Path::new(
            report["output"]
                .as_str()
                .ok_or_else(|| failure(3, "missing archive path"))?,
        );
        expose(out, &dest, &archive).map_err(|(_, err)| (3, json!({"error":err,"output":out})))?;
    }
    result
}

pub(crate) fn publish(mut metadata: Value, write: impl FnOnce(&Path) -> Reply) -> Reply {
    let archive = root()?.join("projects");
    let id = metadata["projectId"]
        .as_str()
        .ok_or_else(|| failure(3, "missing project id"))?;
    let directory = archive.join(id);
    let runs = directory.join("runs");
    fs::create_dir_all(&runs).map_err(io)?;
    let stage = tempfile::Builder::new()
        .prefix(".pending-")
        .tempdir_in(&runs)
        .map_err(io)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io)?
        .as_millis();
    let suffix = stage
        .path()
        .file_name()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix(".pending-"))
        .ok_or_else(|| failure(3, "invalid temporary directory"))?;
    let snapshot = format!("{stamp:016}-{suffix}");
    let out = runs.join(&snapshot);
    let staged = stage.path().join("artifacts");
    let result = write(&staged);
    let (mut report, partial) = match result {
        Ok(report) => (report, false),
        Err((5, report)) => (report, true),
        Err(e) => return Err(e),
    };
    let repo = Path::new(
        metadata["projectRoot"]
            .as_str()
            .ok_or_else(|| failure(3, "missing project root"))?,
    );
    metadata["projectName"] = json!(project_name(&directory, repo)?);
    metadata["id"] = json!(snapshot);
    metadata["timestampMs"] = json!(stamp as u64);
    analysis::store(&staged.join("snapshot.json"), &metadata)?;
    fs::rename(&staged, &out).map_err(io)?;
    report["snapshot"] = metadata;
    report["output"] = json!(out);
    if partial {
        Err((5, report))
    } else {
        Ok(report)
    }
}

pub(crate) fn history() -> Reply {
    let home = root()?;
    let archive = home.join("projects");
    fs::create_dir_all(&archive).map_err(io)?;
    let mut runs = Vec::new();
    for project in fs::read_dir(&archive).map_err(io)? {
        let directory = project.map_err(io)?.path();
        let path = directory.join("runs");
        if !path.is_dir() {
            continue;
        }
        for entry in fs::read_dir(path).map_err(io)? {
            let entry = entry.map_err(io)?;
            if entry.file_name().as_bytes().starts_with(b".")
                || !entry.file_type().map_err(io)?.is_dir()
            {
                continue;
            }
            let out = entry.path();
            let mut meta = read(&out.join("snapshot.json"))?;
            if let Some(root) = meta["projectRoot"].as_str() {
                let id = format!("{:x}", Sha256::digest(root.as_bytes()));
                if directory.file_name().and_then(|n| n.to_str()) == Some(id.as_str()) {
                    meta["projectName"] = json!(project_name(&directory, Path::new(root))?);
                }
            }
            let report = read(&out.join("report.json"))?;
            let cells = report["cells"]
                .as_array()
                .ok_or_else(|| failure(3, "invalid archived report"))?;
            meta["cells"] = json!(cells.iter().map(|c| json!({
                "path":c["path"],"state":c["state"],"scenario":c["scenario"],"width":c["width"],
                "status":c["status"],"image":c["image"],"reason":c["reason"]
            })).collect::<Vec<_>>());
            meta["output"] = json!(out);
            runs.push(meta);
        }
    }
    runs.sort_by(|a, b| b["id"].as_str().cmp(&a["id"].as_str()));
    let data = serde_json::to_string(&runs)
        .map_err(io)?
        .replace('<', "\\u003c");
    let html = include_str!("history.html")
        .replace(
            "/*VIEW*/",
            include_str!(concat!(env!("OUT_DIR"), "/browser/history.js")),
        )
        .replace("/*RUNS*/", &data);
    let index = home.join("history.html");
    use std::io::Write;
    let mut stage = tempfile::NamedTempFile::new_in(&home).map_err(io)?;
    stage.write_all(html.as_bytes()).map_err(io)?;
    stage.persist(&index).map_err(io)?;
    for run in &mut runs {
        if let Some(o) = run.as_object_mut() {
            o.remove("cells");
        }
    }
    Ok(json!({"gallery":index,"snapshots":runs}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_name_survives_manifest_and_branch_changes() {
        let repo = tempfile::tempdir().unwrap();
        let archive = tempfile::tempdir().unwrap();
        let manifest = repo.path().join("package.json");
        fs::write(&manifest, r#"{"name":"@team/editor"}"#).unwrap();
        assert_eq!(
            project_name(archive.path(), repo.path()).unwrap(),
            "@team/editor"
        );
        fs::write(&manifest, r#"{"name":"another-branch"}"#).unwrap();
        assert_eq!(
            project_name(archive.path(), repo.path()).unwrap(),
            "@team/editor"
        );
    }

    #[test]
    fn captures_share_project_identity_across_worktrees_and_branch_switches() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        fs::create_dir(&main).unwrap();
        let no_git = provenance(&main).unwrap();
        assert!(no_git["worktree"].is_null());
        git(&main, &["init", "-b", "main"]).unwrap();
        git(
            &main,
            &[
                "-c",
                "user.name=dui test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                "baseline",
            ],
        )
        .unwrap();
        let linked = tmp.path().join("feature");
        git(
            &main,
            &[
                "worktree",
                "add",
                "-b",
                "layout/new",
                linked.to_str().unwrap(),
            ],
        )
        .unwrap();
        let baseline = provenance(&main).unwrap();
        fs::write(linked.join("local.txt"), "uncommitted").unwrap();
        let feature = provenance(&linked).unwrap();
        assert_eq!(feature["projectId"], baseline["projectId"]);
        assert_ne!(feature["worktree"], baseline["worktree"]);
        assert_eq!(feature["commit"], baseline["commit"]);
        assert_eq!(feature["branch"], "layout/new");
        assert_eq!(feature["dirty"], true);
        git(&linked, &["checkout", "--detach"]).unwrap();
        let detached = provenance(&linked).unwrap();
        assert!(detached["branch"].is_null());
        assert_eq!(detached["projectId"], baseline["projectId"]);
    }
}
