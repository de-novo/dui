//! Capture-to-capture diff with changed source file attribution and PNG pixel-diff overlays.

use dui_core::{
    diff::{
        DiffCellSnapshot, RenderNodeSnapshot, affected_files, compute_rgba_diff, diff_captures,
    },
    model::{CacheKey, ProjectId},
    storage::RunFilter,
};
use dui_ports::{AppStore, Reply, failure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
    process::Command,
};

fn read_json_file(path: &Path) -> Result<Value, (u8, Value)> {
    let bytes = fs::read(path).map_err(|e| failure(2, &format!("{}: {e}", path.display())))?;
    serde_json::from_slice(&bytes).map_err(|e| failure(2, &format!("{}: {e}", path.display())))
}

fn optional_json_file(path: &Path) -> Option<Value> {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

fn load_cell_snapshots<S: AppStore>(
    report: &Value,
    store: Option<&S>,
) -> Result<Vec<DiffCellSnapshot>, (u8, Value)> {
    let report_hashes: BTreeMap<String, String> = report["sourceFileHashes"]
        .as_object()
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default();

    let cells = report["cells"]
        .as_array()
        .ok_or_else(|| failure(2, "report.json missing cells array"))?;

    let mut out = Vec::with_capacity(cells.len());
    for cell in cells {
        let path = cell["path"]
            .as_str()
            .or_else(|| report["path"].as_str())
            .unwrap_or("/")
            .to_owned();
        let scenario = cell["scenario"].as_str().unwrap_or("default").to_owned();
        let state = cell["state"].as_str().unwrap_or("").to_owned();
        let width = cell["width"].as_u64().unwrap_or(0) as u32;
        let locale = cell["locale"].as_str().map(str::to_owned);
        let status = cell["status"].as_str().unwrap_or("unverified").to_owned();
        let reason = cell["reason"].as_str().map(str::to_owned);
        let image = cell["image"].as_str().map(str::to_owned);
        let cache_key = cell["cache"]["key"].as_str().map(str::to_owned);

        let mut source_files = BTreeMap::new();
        if let Some(list) = cell["sourceFiles"].as_array() {
            for file in list.iter().filter_map(Value::as_str) {
                if let Some(hash) = report_hashes.get(file) {
                    source_files.insert(file.to_owned(), hash.clone());
                }
            }
        }
        if source_files.is_empty()
            && let Some(key_str) = cache_key.as_deref()
            && let Some(db) = store
            && let Ok(key) = CacheKey::parse(key_str)
            && let Ok(Some(entry)) = db.get_cache(&key)
        {
            for (file, digest) in &entry.inputs().source_files {
                source_files.insert(file.clone(), digest.as_str().to_owned());
            }
        }
        if source_files.is_empty() && !report_hashes.is_empty() {
            source_files = report_hashes.clone();
        }

        let render_tree: Vec<RenderNodeSnapshot> = cell["observation"]["renderTree"]
            .as_array()
            .and_then(|arr| serde_json::from_value(Value::Array(arr.clone())).ok())
            .unwrap_or_default();

        out.push(DiffCellSnapshot {
            path,
            scenario,
            state,
            width,
            locale,
            status,
            screen_status: cell["observation"]["screenEvidence"]["screenStatus"]
                .as_str()
                .map(str::to_owned),
            reason,
            image,
            cache_key,
            source_files,
            render_tree,
        });
    }
    Ok(out)
}

/// Share of pixels that differ between two PNGs (any channel off by more than `tolerance`).
/// Rows present in only one image count as differing.
pub fn png_mismatch(a: &[u8], b: &[u8], tolerance: u8) -> Result<f64, String> {
    let (aw, ah, ap) = decode_png_rgba_bytes(a)?;
    let (bw, bh, bp) = decode_png_rgba_bytes(b)?;
    if aw != bw {
        return Ok(1.0);
    }
    let rows = ah.min(bh) as usize;
    let row_bytes = aw as usize * 4;
    let mut differing = (ah.max(bh) as usize - rows) * aw as usize;
    for (pa, pb) in ap[..rows * row_bytes]
        .chunks_exact(4)
        .zip(bp[..rows * row_bytes].chunks_exact(4))
    {
        if pa.iter().zip(pb).any(|(x, y)| x.abs_diff(*y) > tolerance) {
            differing += 1;
        }
    }
    let total = (ah.max(bh) as usize * aw as usize).max(1);
    Ok(differing as f64 / total as f64)
}

fn decode_png_rgba(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    decode_png_rgba_bytes(&fs::read(path).map_err(|e| e.to_string())?)
}

pub(crate) fn decode_png_rgba_bytes(png_bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let buf_size = reader
        .output_buffer_size()
        .ok_or_else(|| "png buffer size overflow".to_owned())?;
    let mut raw = vec![0u8; buf_size];
    let info = reader.next_frame(&mut raw).map_err(|e| e.to_string())?;
    let bytes = &raw[..info.buffer_size()];
    let pixel_count = (info.width as usize).saturating_mul(info.height as usize);
    let rgba = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for chunk in bytes.chunks_exact(3) {
                out.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for chunk in bytes.chunks_exact(2) {
                out.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for &g in bytes {
                out.extend_from_slice(&[g, g, g, 255]);
            }
            out
        }
        png::ColorType::Indexed => {
            return Err("unexpected indexed png after EXPAND".to_owned());
        }
    };
    Ok((info.width, info.height, rgba))
}

fn write_png_rgba(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    Ok(())
}

fn overlay_filename(before_img: &str, after_img: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(before_img.as_bytes());
    hasher.update(b":");
    hasher.update(after_img.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("{}-diff.png", &digest[..16])
}

pub fn compare<S: AppStore>(
    store: Option<&S>,
    before: &Path,
    after: &Path,
    overlay_dir: Option<&Path>,
) -> Reply {
    compare_with_options(store, before, after, overlay_dir, true)
}

pub fn compare_with_options<S: AppStore>(
    store: Option<&S>,
    before: &Path,
    after: &Path,
    overlay_dir: Option<&Path>,
    include_pixels: bool,
) -> Reply {
    compare_archives(
        store,
        before,
        after,
        Mode::Visual {
            overlay_dir,
            include_pixels,
        },
    )
}

#[derive(Clone, Copy)]
pub enum Mode<'a> {
    Visual {
        overlay_dir: Option<&'a Path>,
        include_pixels: bool,
    },
    RegressionsOnly,
}

struct Capture {
    dir: PathBuf,
    snapshot: Option<Value>,
    report: Value,
}

impl Capture {
    fn load(dir: &Path) -> Result<Self, (u8, Value)> {
        Ok(Self {
            dir: dir.to_owned(),
            snapshot: optional_json_file(&dir.join("snapshot.json")),
            report: read_json_file(&dir.join("report.json"))?,
        })
    }

    fn provenance(&self) -> Value {
        json!({"path":self.dir,"snapshot":self.snapshot})
    }
}

pub fn compare_archives<S: AppStore>(
    store: Option<&S>,
    before: &Path,
    after: &Path,
    mode: Mode<'_>,
) -> Reply {
    compare_loaded(store, Capture::load(before)?, Capture::load(after)?, mode)
}

fn compare_loaded<S: AppStore>(
    store: Option<&S>,
    before: Capture,
    after: Capture,
    mode: Mode<'_>,
) -> Reply {
    let verification = dui_core::diff::regressions::compare(&before.report, &after.report)
        .map_err(|e| failure(2, e))?;
    let mut report = json!({"before":before.provenance(),"after":after.provenance(),"verification":verification});
    let Mode::Visual {
        overlay_dir,
        include_pixels,
    } = mode
    else {
        return Ok(report);
    };
    let before_cells = load_cell_snapshots(&before.report, store)?;
    let after_cells = load_cell_snapshots(&after.report, store)?;
    let mut diff = diff_captures(&before_cells, &after_cells);

    for screen in &mut diff.screens {
        for cell in &mut screen.cells {
            if !cell.supports_pixel_diff() {
                cell.pixel_diff = None;
                continue;
            }
            // In lazy mode, skip eager PNG decoding whenever render-level diff already
            // identified concrete element changes (still decode for visual-only or legacy cells).
            let has_render_changes = cell
                .render_diff
                .as_ref()
                .is_some_and(|rd| !rd.changes.is_empty());
            if !include_pixels && has_render_changes {
                cell.pixel_diff = None;
                continue;
            }
            let Some(before_img) = cell.before.as_ref().and_then(|s| s.image.as_deref()) else {
                continue;
            };
            let Some(after_img) = cell.after.as_ref().and_then(|s| s.image.as_deref()) else {
                continue;
            };
            let before_path = before.dir.join(before_img);
            let after_path = after.dir.join(after_img);
            let Ok((bw, bh, brgba)) = decode_png_rgba(&before_path) else {
                continue;
            };
            let Ok((aw, ah, argba)) = decode_png_rgba(&after_path) else {
                continue;
            };
            let (mut metrics, overlay_rgba) = compute_rgba_diff(bw, bh, &brgba, aw, ah, &argba);
            if let Some(dir) = overlay_dir {
                let name = overlay_filename(before_img, after_img);
                let out_path = dir.join(&name);
                if write_png_rgba(&out_path, metrics.width, metrics.height, &overlay_rgba).is_ok() {
                    metrics.overlay_image = Some(name);
                }
            }
            cell.pixel_diff = Some(metrics);
        }
    }

    report["summary"] = json!(diff.summary);
    report["screens"] = json!(diff.screens);
    Ok(report)
}

pub fn compare_branches_with_store<S: AppStore>(
    store: &S,
    home: &Path,
    project_id_str: &str,
    base_branch: &str,
    resolved_head: &str,
    overlay_dir: Option<&Path>,
) -> Reply {
    compare_branch_runs(
        store,
        home,
        project_id_str,
        (base_branch, resolved_head),
        None,
        Mode::Visual {
            overlay_dir,
            include_pixels: true,
        },
    )
}

pub fn compare_branch_runs<S: AppStore>(
    store: &S,
    home: &Path,
    project_id_str: &str,
    branches: (&str, &str),
    app: Option<&str>,
    mode: Mode<'_>,
) -> Reply {
    let (base_branch, resolved_head) = branches;
    let pid = ProjectId::parse(project_id_str).map_err(|e| failure(2, &e.to_string()))?;

    let find_latest_run = |branch: &str| -> Result<Capture, (u8, Value)> {
        let runs = store
            .list_runs(&RunFilter {
                project_id: Some(pid.clone()),
                branch: Some(branch.to_owned()),
                ..RunFilter::default()
            })
            .map_err(|e| failure(3, &e.to_string()))?;
        for summary in runs {
            let dir = home
                .join("projects")
                .join(summary.run.project_id().as_str())
                .join("runs")
                .join(summary.run.id().as_str());
            let capture = Capture::load(&dir)?;
            if capture.report.get("recheck").is_some()
                || capture.report.get("live").is_some()
                || app.is_some_and(|a| {
                    capture.snapshot.as_ref().and_then(|s| s["app"].as_str()) != Some(a)
                })
            {
                continue;
            }
            return Ok(capture);
        }
        Err(failure(
            2,
            &format!("no matching archived capture found for branch '{branch}' in project"),
        ))
    };

    let mut reply = compare_loaded(
        Some(store),
        find_latest_run(base_branch)?,
        find_latest_run(resolved_head)?,
        mode,
    )?;
    if let Some(obj) = reply.as_object_mut() {
        obj.insert("beforeBranch".into(), json!(base_branch));
        obj.insert("afterBranch".into(), json!(resolved_head));
    }
    Ok(reply)
}

pub fn changed_since_summary(project: &Path, base_ref: &str, module_graph_val: &Value) -> Value {
    let app_canon = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let repo_root = Command::new("git")
        .args(["--no-optional-locks", "-C"])
        .arg(&app_canon)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or_else(|| app_canon.clone());
    let app_prefix = app_canon
        .strip_prefix(&repo_root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .filter(|s| !s.is_empty());

    let mut changed = BTreeSet::new();
    let normalize = |raw: &str| -> Option<String> {
        let clean = raw.trim().replace('\\', "/");
        if clean.is_empty() {
            return None;
        }
        if let Some(ref prefix) = app_prefix {
            clean.strip_prefix(&format!("{prefix}/")).map(str::to_owned)
        } else {
            Some(clean)
        }
    };

    if let Ok(out) = Command::new("git")
        .args(["--no-optional-locks", "-C"])
        .arg(&repo_root)
        .args(["diff", "--name-only", &format!("{base_ref}...HEAD")])
        .output()
        && out.status.success()
    {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if let Some(rel) = normalize(line) {
                changed.insert(rel);
            }
        }
    }
    if let Ok(out) = Command::new("git")
        .args(["--no-optional-locks", "-C"])
        .arg(&repo_root)
        .args(["status", "--porcelain"])
        .output()
        && out.status.success()
    {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if line.len() > 3
                && let Some(rel) = normalize(&line[3..])
            {
                changed.insert(rel);
            }
        }
    }

    let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Some(obj) = module_graph_val.as_object() {
        for (k, v) in obj {
            let deps = v
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|d| d.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            graph.insert(k.clone(), deps);
        }
    }
    let affected = affected_files(&graph, &changed);
    json!({
        "baseRef": base_ref,
        "gitChangedFiles": changed.into_iter().collect::<Vec<_>>(),
        "affectedFiles": affected.into_iter().collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_solid_png(path: &Path, rgba: [u8; 4]) {
        let pixels = vec![
            rgba[0], rgba[1], rgba[2], rgba[3], rgba[0], rgba[1], rgba[2], rgba[3], rgba[0],
            rgba[1], rgba[2], rgba[3], rgba[0], rgba[1], rgba[2], rgba[3],
        ];
        write_png_rgba(path, 2, 2, &pixels).expect("write test png");
    }

    #[test]
    fn lazy_compare_skips_pixel_diff_when_render_diff_finds_changes_and_hydrates_on_request() {
        let tmp = tempfile::tempdir().unwrap();
        let before = tmp.path().join("before");
        let after = tmp.path().join("after");
        fs::create_dir_all(&before).unwrap();
        fs::create_dir_all(&after).unwrap();

        write_solid_png(&before.join("a.png"), [10, 10, 10, 255]);
        write_solid_png(&after.join("b.png"), [240, 240, 240, 255]);

        let tree_before = json!([{"id":"h1#0","tag":"h1","text":"Old","rect":{"x":0,"y":0,"width":80,"height":24},"style":{}}]);
        let tree_after = json!([{"id":"h1#0","tag":"h1","text":"New","rect":{"x":0,"y":0,"width":80,"height":24},"style":{}}]);

        fs::write(
            before.join("report.json"),
            json!({
                "path": "/",
                "cells": [
                    {"path":"/","scenario":"default","state":"s0","width":375,"status":"verified","image":"a.png","observation":{"renderTree":tree_before}},
                    {"path":"/canvas","scenario":"case-1","state":"s1","width":375,"status":"verified","image":"a.png","observation":{"renderTree":tree_before}}
                ]
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            after.join("report.json"),
            json!({
                "path": "/",
                "cells": [
                    {"path":"/","scenario":"default","state":"s0","width":375,"status":"verified","image":"b.png","observation":{"renderTree":tree_after}},
                    {"path":"/canvas","scenario":"case-1","state":"s1","width":375,"status":"verified","image":"b.png","observation":{"renderTree":tree_before}}
                ]
            })
            .to_string(),
        )
        .unwrap();

        // Lazy mode skips pixelDiff for `/` (renderDiff found h1 text change),
        // but still computes pixelDiff automatically for visual-only `/canvas`.
        let no_store: Option<&DummyStore> = None;
        let lazy =
            compare_with_options(no_store, &before, &after, None, false).expect("lazy compare");
        let screens = lazy["screens"].as_array().unwrap();
        let home_cell = &screens.iter().find(|s| s["path"] == "/").unwrap()["cells"][0];
        assert_eq!(home_cell["renderDiff"]["textChanged"], 1);
        assert!(home_cell.get("pixelDiff").is_none());

        let canvas_cell = &screens.iter().find(|s| s["path"] == "/canvas").unwrap()["cells"][0];
        assert_eq!(canvas_cell["renderDiff"]["visualOnly"], true);
        assert_eq!(canvas_cell["pixelDiff"]["diffPixels"], 4);

        // Eager pixel mode hydrates pixelDiff on `/` as well.
        let eager =
            compare_with_options(no_store, &before, &after, None, true).expect("eager compare");
        let home_eager = &eager["screens"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["path"] == "/")
            .unwrap()["cells"][0];
        assert_eq!(home_eager["pixelDiff"]["diffPixels"], 4);
    }

    struct DummyStore;
    impl dui_core::storage::WorkspaceRepository for DummyStore {
        fn upsert_project(
            &mut self,
            _: &dui_core::model::Project,
        ) -> dui_core::storage::StoreResult<()> {
            unimplemented!()
        }
        fn get_project(
            &self,
            _: &dui_core::model::ProjectId,
        ) -> dui_core::storage::StoreResult<Option<dui_core::model::Project>> {
            unimplemented!()
        }
        fn upsert_worktree(
            &mut self,
            _: &dui_core::model::Worktree,
        ) -> dui_core::storage::StoreResult<()> {
            unimplemented!()
        }
        fn get_worktree(
            &self,
            _: &dui_core::model::WorktreeId,
        ) -> dui_core::storage::StoreResult<Option<dui_core::model::Worktree>> {
            unimplemented!()
        }
        fn put_commit(
            &mut self,
            _: &dui_core::model::Commit,
        ) -> dui_core::storage::StoreResult<()> {
            unimplemented!()
        }
        fn get_commit(
            &self,
            _: &dui_core::model::CommitId,
        ) -> dui_core::storage::StoreResult<Option<dui_core::model::Commit>> {
            unimplemented!()
        }
    }
    impl dui_core::storage::RunRepository for DummyStore {
        fn save_run(
            &mut self,
            _: &dui_core::model::RunAggregate,
        ) -> dui_core::storage::StoreResult<()> {
            unimplemented!()
        }
        fn load_run(
            &self,
            _: &dui_core::model::RunId,
        ) -> dui_core::storage::StoreResult<Option<dui_core::model::RunAggregate>> {
            unimplemented!()
        }
        fn list_runs(
            &self,
            _: &RunFilter,
        ) -> dui_core::storage::StoreResult<Vec<dui_core::storage::RunSummary>> {
            unimplemented!()
        }
    }
    impl dui_core::storage::ArtifactStore for DummyStore {
        fn put_blob(
            &mut self,
            _: &dui_core::model::ImageBlob,
            _: &[u8],
        ) -> dui_core::storage::StoreResult<()> {
            unimplemented!()
        }
        fn get_blob(
            &self,
            _: &dui_core::model::BlobId,
        ) -> dui_core::storage::StoreResult<Option<(dui_core::model::ImageBlob, Vec<u8>)>> {
            unimplemented!()
        }
        fn put_evidence(
            &mut self,
            _: &dui_core::model::Evidence,
        ) -> dui_core::storage::StoreResult<()> {
            unimplemented!()
        }
        fn get_evidence(
            &self,
            _: &dui_core::model::EvidenceId,
        ) -> dui_core::storage::StoreResult<Option<dui_core::model::Evidence>> {
            unimplemented!()
        }
        fn unreferenced_blobs(
            &self,
        ) -> dui_core::storage::StoreResult<Vec<dui_core::model::BlobId>> {
            unimplemented!()
        }
        fn delete_blob(
            &mut self,
            _: &dui_core::model::BlobId,
        ) -> dui_core::storage::StoreResult<()> {
            unimplemented!()
        }
    }
    impl dui_core::storage::CacheRepository for DummyStore {
        fn put_cache(
            &mut self,
            _: &dui_core::model::CacheEntry,
        ) -> dui_core::storage::StoreResult<()> {
            unimplemented!()
        }
        fn get_cache(
            &self,
            _: &CacheKey,
        ) -> dui_core::storage::StoreResult<Option<dui_core::model::CacheEntry>> {
            Ok(None)
        }
    }
    impl dui_core::storage::CommentRepository for DummyStore {
        fn upsert_comment(
            &mut self,
            _: &dui_core::model::Comment,
        ) -> dui_core::storage::StoreResult<()> {
            unimplemented!()
        }
        fn get_comment(
            &self,
            _: &dui_core::model::CommentId,
        ) -> dui_core::storage::StoreResult<Option<dui_core::model::Comment>> {
            unimplemented!()
        }
        fn list_comments(
            &self,
            _: &dui_core::model::RunId,
        ) -> dui_core::storage::StoreResult<Vec<dui_core::model::Comment>> {
            unimplemented!()
        }
    }
    impl AppStore for DummyStore {
        fn blob_path(&self, _: &dui_core::model::BlobId) -> PathBuf {
            unimplemented!()
        }
        fn link_or_copy_blob(
            &self,
            _: &dui_core::model::BlobId,
            _: &Path,
        ) -> dui_core::storage::StoreResult<()> {
            unimplemented!()
        }
        fn find_previous_cache_inputs(
            &self,
            _: &dui_core::model::StateId,
            _: dui_core::model::Width,
            _: Option<&str>,
        ) -> dui_core::storage::StoreResult<Option<dui_core::model::CacheInputs>> {
            unimplemented!()
        }
        fn import_run_dir(&mut self, _: &Path) -> dui_core::storage::StoreResult<bool> {
            unimplemented!()
        }
        fn ensure_screen_record(
            &self,
            _: &dui_core::model::ScreenId,
            _: &str,
        ) -> dui_core::storage::StoreResult<()> {
            unimplemented!()
        }
        fn ensure_edge_record(
            &self,
            _: &dui_core::model::EdgeId,
            _: &dui_core::model::RunId,
            _: &dui_core::model::ScreenId,
            _: &dui_core::model::ScreenId,
        ) -> dui_core::storage::StoreResult<()> {
            unimplemented!()
        }
        fn find_cell_id(
            &self,
            _: &dui_core::model::RunId,
            _: &str,
            _: Option<&str>,
            _: u32,
        ) -> dui_core::storage::StoreResult<Option<dui_core::model::CellId>> {
            unimplemented!()
        }
        fn describe_comment_target(&self, _: &dui_core::model::CommentTarget) -> Value {
            unimplemented!()
        }
        fn list_comments_by_project(
            &self,
            _: &ProjectId,
        ) -> dui_core::storage::StoreResult<Vec<dui_core::model::Comment>> {
            unimplemented!()
        }
        fn list_all_comments(
            &self,
        ) -> dui_core::storage::StoreResult<Vec<dui_core::model::Comment>> {
            unimplemented!()
        }
    }
}
