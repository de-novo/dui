//! Capture storage shared by the CLI and MCP server.
use crate::{Reply, analysis, failure, host, replay};
use dui_core::model::ProjectId;
use replay::{Error, io, read};
use serde_json::{Value, json};
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
    let project_id = ProjectId::from_primary_root_bytes(repo.as_os_str().as_bytes());
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
    let home = root()?;
    let archive = home.join("projects");
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
    let (mut store, _) = crate::sqlite_store::SqliteStore::open(&home).map_err(io)?;
    store.import_run_dir(&out).map_err(io)?;
    report["snapshot"] = metadata;
    report["output"] = json!(out);
    if partial {
        Err((5, report))
    } else {
        Ok(report)
    }
}

pub(crate) fn history() -> Reply {
    use dui_core::storage::{RunFilter, RunRepository, WorkspaceRepository};

    let home = root()?;
    let archive = home.join("projects");
    fs::create_dir_all(&archive).map_err(io)?;
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
            if let Ok(meta) = read(&entry.path().join("snapshot.json"))
                && let Some(root_path) = meta["projectRoot"].as_str()
            {
                let id = ProjectId::from_primary_root_bytes(root_path.as_bytes());
                if directory.file_name().and_then(|n| n.to_str()) == Some(id.as_str()) {
                    let _ = project_name(&directory, Path::new(root_path))?;
                }
            }
        }
    }

    let (mut store, _) = crate::sqlite_store::SqliteStore::open(&home).map_err(io)?;
    store.import_disk_archive(&archive).map_err(io)?;

    let summaries = store.list_runs(&RunFilter::default()).map_err(io)?;
    let mut runs = Vec::with_capacity(summaries.len());
    for summary in summaries {
        let run = &summary.run;
        let project = store
            .get_project(run.project_id())
            .map_err(io)?
            .ok_or_else(|| failure(3, "missing project in sqlite store"))?;
        let worktree = store
            .get_worktree(run.worktree_id())
            .map_err(io)?
            .ok_or_else(|| failure(3, "missing worktree in sqlite store"))?;
        let out = archive
            .join(run.project_id().as_str())
            .join("runs")
            .join(run.id().as_str());
        let cells = if let Ok(report) = read(&out.join("report.json"))
            && let Some(items) = report["cells"].as_array()
        {
            items
                .iter()
                .map(|c| {
                    json!({
                        "path": c["path"],
                        "state": c["state"],
                        "scenario": c["scenario"],
                        "width": c["width"],
                        "status": c["status"],
                        "image": c["image"],
                        "reason": c["reason"]
                    })
                })
                .collect::<Vec<_>>()
        } else if let Some(agg) = store.load_run(run.id()).map_err(io)? {
            agg.cells()
                .iter()
                .map(|c| {
                    let path = agg.screens().get(c.screen_id()).map_or("/", |s| s.path());
                    let scenario = agg
                        .states()
                        .get(c.state_id())
                        .map_or("default", |s| s.scenario());
                    json!({
                        "path": path,
                        "state": c.state_id().as_str(),
                        "scenario": scenario,
                        "width": c.width().pixels(),
                        "status": c.status().kind(),
                        "image": c.image_blob().map(|b| format!("{}.png", b.as_str())),
                        "reason": c.status().reason()
                    })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        runs.push(json!({
            "projectId": run.project_id().as_str(),
            "projectName": project.display_name(),
            "projectRoot": project.primary_root(),
            "app": worktree.app_relative(),
            "worktree": worktree.path(),
            "branch": summary.branch,
            "commit": summary.commit_sha,
            "dirty": summary.dirty,
            "id": run.id().as_str(),
            "timestampMs": run.created_at_ms(),
            "output": out,
            "cells": cells
        }));
    }

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
pub(crate) struct ImportedRun {
    pub project: dui_core::model::Project,
    pub worktree: dui_core::model::Worktree,
    pub commit: dui_core::model::Commit,
    pub aggregate: dui_core::model::RunAggregate,
    pub evidence: Vec<dui_core::model::Evidence>,
}

#[cfg(test)]
pub(crate) fn import_run(
    snapshot: &Value,
    report: &Value,
    graph: &Value,
) -> Result<ImportedRun, dui_core::model::DomainError> {
    use dui_core::model::{
        BlobId, Cell, CellStatus, Commit, Edge, EdgeEndpoint, Evidence, Project, Run, RunAggregate,
        RunEnvironment, RunId, RunKind, Screen, ScreenOrigin, State, StateId, Width, Worktree,
    };
    use std::collections::{BTreeMap, BTreeSet};

    let project_root = snapshot["projectRoot"].as_str().unwrap_or("/project");
    let project_id = match snapshot["projectId"].as_str() {
        Some(raw) => ProjectId::parse(raw)?,
        None => ProjectId::from_primary_root_bytes(project_root.as_bytes()),
    };
    let project = Project::with_id(
        project_id.clone(),
        project_root,
        snapshot["projectName"]
            .as_str()
            .unwrap_or("Unnamed project"),
    )?;
    let worktree_path = snapshot["worktree"].as_str().unwrap_or(project_root);
    let app_relative = snapshot["app"].as_str().unwrap_or_default();
    let worktree = Worktree::new(
        project_id.clone(),
        worktree_path,
        app_relative,
        snapshot["branch"].as_str().map(str::to_owned),
    )?;
    let commit = Commit::new(
        project_id.clone(),
        snapshot["commit"].as_str().map(str::to_owned),
        snapshot["dirty"].as_bool().unwrap_or(false),
        None,
    );
    let run_id = RunId::parse(snapshot["id"].as_str().unwrap_or("run"))?;
    let kind = if let Some(live) = report.get("live").filter(|v| v.is_object()) {
        RunKind::LiveSave {
            source_run: RunId::parse(live["sourceRun"].as_str().unwrap_or("source"))?,
            source_state: StateId::parse(live["sourceCell"]["state"].as_str().unwrap_or_default())
                .unwrap_or_else(|_| StateId::derive(&live["sourceCell"]["state"])),
            source_width: Width::new(live["sourceCell"]["width"].as_u64().unwrap_or(1) as u32)?,
            revision: live["revision"].as_u64().unwrap_or(0),
        }
    } else {
        RunKind::Capture
    };
    let mut versions = BTreeMap::new();
    if let Some(map) = report["versions"].as_object() {
        for (k, v) in map {
            if let Some(s) = v.as_str() {
                versions.insert(k.clone(), s.to_owned());
            }
        }
    }
    let source_files = report["sourceFiles"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let run = Run::new(
        run_id.clone(),
        project_id,
        worktree.id().clone(),
        commit.id().clone(),
        kind,
        snapshot["timestampMs"].as_i64().unwrap_or(0),
        RunEnvironment {
            os: report["os"].as_str().unwrap_or_default().to_owned(),
            compiler: report["compiler"].as_str().map(str::to_owned),
            renderer: report["browser"]["product"].as_str().map(str::to_owned),
            dui_version: env!("CARGO_PKG_VERSION").to_owned(),
            versions,
        },
        source_files,
    );

    let mut screens_by_path: BTreeMap<String, Screen> = BTreeMap::new();
    let mut states_by_id = BTreeMap::new();
    let mut cells = Vec::new();
    let mut evidence_by_id = BTreeMap::new();

    for raw in report["cells"].as_array().into_iter().flatten() {
        let path = raw["path"].as_str().unwrap_or("/");
        let screen = if let Some(existing) = screens_by_path.get(path) {
            existing.clone()
        } else {
            let origin = match report["screenDiscovery"]["sources"].get(path) {
                Some(Value::String(file)) => ScreenOrigin::Router { file: file.clone() },
                Some(obj) if obj["kind"] == "anchor" => ScreenOrigin::Anchor {
                    from: obj["from"].as_str().unwrap_or("/").to_owned(),
                    selector: obj["selector"].as_str().map(str::to_owned),
                },
                _ => ScreenOrigin::Entry,
            };
            let created = Screen::new(path, origin)?;
            screens_by_path.insert(path.to_owned(), created.clone());
            created
        };
        let scenario = raw["scenario"].as_str().unwrap_or("default");
        let mut inputs = BTreeMap::new();
        if let Some(obj) = raw["input"].as_object() {
            for (k, v) in obj {
                inputs.insert(k.clone(), v.clone());
            }
        }
        let plan_index = raw["planIndex"].as_u64().unwrap_or(0) as u32;
        let state = match raw["state"].as_str().and_then(|s| StateId::parse(s).ok()) {
            Some(id) => {
                State::with_id(id, screen.id().clone(), scenario, None, inputs, plan_index)?
            }
            None => State::new(screen.id().clone(), scenario, None, inputs, plan_index)?,
        };
        let state_id = state.id().clone();
        states_by_id.entry(state_id.clone()).or_insert(state);

        let width = Width::new(raw["width"].as_u64().unwrap_or(0) as u32)?;
        let status = CellStatus::from_wire(
            raw["status"].as_str().unwrap_or("unverified"),
            raw["reason"].as_str().map(str::to_owned),
        )?;
        let image_blob = raw["image"].as_str().map(|name| {
            raw["imageSha256"]
                .as_str()
                .and_then(|s| BlobId::parse(s).ok())
                .or_else(|| name.split('-').next().and_then(|s| BlobId::parse(s).ok()))
                .unwrap_or_else(|| BlobId::from_image_bytes(name.as_bytes()))
        });
        let evidence_id = raw["observation"].as_object().map(|obs| {
            let slots = obs
                .get("slots")
                .and_then(Value::as_object)
                .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();
            let hits = obs
                .get("hits")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            let links = obs
                .get("links")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            let ev = Evidence::new(
                obs.get("path").and_then(Value::as_str).map(str::to_owned),
                obs.get("width").and_then(Value::as_u64).map(|w| w as u32),
                obs.get("ready").and_then(Value::as_bool),
                obs.get("settled").and_then(Value::as_bool),
                slots,
                BTreeMap::new(),
                hits,
                links,
                BTreeMap::new(),
                raw["checks"].as_array().cloned().unwrap_or_default(),
                vec![],
            );
            let id = ev.id().clone();
            evidence_by_id.entry(id.clone()).or_insert(ev);
            id
        });
        cells.push(Cell::new(
            run_id.clone(),
            screen.id().clone(),
            state_id,
            width,
            None,
            status,
            image_blob,
            evidence_id,
            None,
        )?);
    }

    let mut edges = Vec::new();
    for raw in graph["edges"].as_array().into_iter().flatten() {
        if raw["kind"] != "linksTo" {
            continue;
        }
        let Some(from_path) = raw["source"]
            .as_str()
            .and_then(|s| s.strip_prefix("route:"))
        else {
            continue;
        };
        let Some(to_path) = raw["target"]
            .as_str()
            .and_then(|s| s.strip_prefix("route:"))
        else {
            continue;
        };
        let mut observed_widths = BTreeSet::new();
        for w in raw["observedWidths"].as_array().into_iter().flatten() {
            if let Some(px) = w.as_u64().and_then(|n| Width::new(n as u32).ok()) {
                observed_widths.insert(px);
            }
        }
        edges.push(Edge::discovered(
            run_id.clone(),
            EdgeEndpoint::Screen {
                screen_id: Screen::derive_id(from_path),
            },
            EdgeEndpoint::Screen {
                screen_id: Screen::derive_id(to_path),
            },
            raw["selector"].as_str().map(str::to_owned),
            observed_widths,
        ));
    }

    let aggregate = RunAggregate::publish(
        run,
        screens_by_path.into_values().collect(),
        states_by_id.into_values().collect(),
        cells,
        edges,
    )?;
    Ok(ImportedRun {
        project,
        worktree,
        commit,
        aggregate,
        evidence: evidence_by_id.into_values().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dui_core::storage::{
        ArtifactStore, MemoryStore, RunFilter, RunRepository, WorkspaceRepository,
    };

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

        let mut store = MemoryStore::new();
        let mut snap_main = baseline.clone();
        snap_main["id"] = json!("0001-main");
        snap_main["projectName"] = json!("demo");
        snap_main["timestampMs"] = json!(1000);
        let mut snap_feat = feature.clone();
        snap_feat["id"] = json!("0002-feat");
        snap_feat["projectName"] = json!("demo");
        snap_feat["timestampMs"] = json!(2000);
        let report = json!({"os":"macos","cells":[{"path":"/","scenario":"default","state":"s0","width":1280,"planIndex":0,"status":"verified","image":"a.png"}]});
        let graph = json!({"nodes":[],"edges":[]});
        let b1 = import_run(&snap_main, &report, &graph).unwrap();
        let b2 = import_run(&snap_feat, &report, &graph).unwrap();
        assert_eq!(b1.project.id(), b2.project.id());
        assert_ne!(b1.worktree.id(), b2.worktree.id());
        for imported in [&b1, &b2] {
            store.upsert_project(&imported.project).unwrap();
            store.upsert_worktree(&imported.worktree).unwrap();
            store.put_commit(&imported.commit).unwrap();
            for ev in &imported.evidence {
                store.put_evidence(ev).unwrap();
            }
            store.save_run(&imported.aggregate).unwrap();
        }
        let listed = store
            .list_runs(&RunFilter {
                project_id: Some(b1.project.id().clone()),
                branch: Some("layout/new".into()),
                ..RunFilter::default()
            })
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].run.id().as_str(), "0002-feat");
    }
}
