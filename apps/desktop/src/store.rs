//! Direct SQLite reader/writer, synchronous RenderImage PNG cache, audit summary, and capture diff
//! against `~/.dui/dui.db` and `~/.dui/blobs/` for the GPUI desktop viewer.

use dui_core::{
    diff::{RenderDiffSummary, RenderNodeSnapshot, compute_rgba_diff, diff_render_trees},
    model::{
        CellId, Comment, CommentId, CommentStatus, CommentTarget, EdgeEndpoint, EdgeId, ProjectId,
        RunId, Screen, ScreenId, ScreenOrigin, capture::NativeSource,
    },
};
use gpui_kit::RenderImage;
use image::{Frame, RgbaImage};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};
use smallvec::smallvec;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellItem {
    pub id: String,
    pub state_id: String,
    pub width_px: u32,
    pub locale: Option<String>,
    pub status_kind: String,
    pub reason: Option<String>,
    pub screen_evidence: Option<Value>,
    pub observed_path: Option<String>,
    pub http_errors: Vec<String>,
    pub blob_id: Option<String>,
    pub blob_path: Option<PathBuf>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
}

fn read_png_dimensions(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut file = fs::File::open(path).ok()?;
    let mut header = [0u8; 24];
    file.read_exact(&mut header).ok()?;
    if &header[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
    (width > 0 && height > 0).then_some((width, height))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeItem {
    pub from_path: String,
    pub to_path: String,
    pub source: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenItem {
    pub id: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_source: Option<NativeSource>,
    pub cells: Vec<CellItem>,
    pub outgoing: Vec<EdgeItem>,
}

impl ScreenItem {
    /// Declared native routes have source evidence even before the first capture.
    pub fn missing_preview(&self, compact: bool) -> String {
        if self.cells.is_empty()
            && let Some(origin) = &self.native_source
        {
            let path = if compact {
                origin.source.rsplit('/').next().unwrap_or(&origin.source)
            } else {
                &origin.source
            };
            return format!("{path}:{}", origin.line);
        }
        "No image".into()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItem {
    pub id: String,
    pub project_id: String,
    pub run_id: String,
    pub author: String,
    pub body: String,
    pub status: String,
    pub screen_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_id: Option<String>,
    pub target_label: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunItem {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub branch: Option<String>,
    pub project_path: Option<PathBuf>,
    pub run_dir: Option<PathBuf>,
    pub created_at_ms: i64,
    pub screens: Vec<ScreenItem>,
}

impl RunItem {
    pub fn widths(&self) -> Vec<u32> {
        let mut set = BTreeSet::new();
        for sc in &self.screens {
            for c in &sc.cells {
                if c.width_px > 0 {
                    set.insert(c.width_px);
                }
            }
        }
        if set.is_empty() && !self.screens.iter().any(|s| s.native_source.is_some()) {
            vec![1280]
        } else {
            set.into_iter().collect()
        }
    }

    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let mut total = 0;
        let mut verified = 0;
        let mut unverified = 0;
        let mut unrenderable = 0;
        for sc in &self.screens {
            for c in &sc.cells {
                total += 1;
                match c.status_kind.as_str() {
                    "verified" => verified += 1,
                    "unrenderable" => unrenderable += 1,
                    _ => unverified += 1,
                }
            }
        }
        (total, verified, unverified, unrenderable)
    }
}

#[derive(Clone, Debug)]
pub struct ProjectGroup {
    pub project_name: String,
    pub run_indices: Vec<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopWorkspace {
    pub dui_home: PathBuf,
    pub db_path: PathBuf,
    pub runs: Vec<RunItem>,
    pub comments: Vec<CommentItem>,
}

impl DesktopWorkspace {
    pub fn project_groups(&self) -> Vec<ProjectGroup> {
        let mut order: Vec<String> = Vec::new();
        let mut map: BTreeMap<String, ProjectGroup> = BTreeMap::new();
        for (idx, run) in self.runs.iter().enumerate() {
            if !map.contains_key(&run.project_id) {
                order.push(run.project_id.clone());
                map.insert(
                    run.project_id.clone(),
                    ProjectGroup {
                        project_name: run.project_name.clone(),
                        run_indices: Vec::new(),
                    },
                );
            }
            if let Some(group) = map.get_mut(&run.project_id) {
                group.run_indices.push(idx);
            }
        }
        order.into_iter().filter_map(|id| map.remove(&id)).collect()
    }

    pub fn open_comments_by_screen(&self, project_id: &str) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for c in &self.comments {
            if c.project_id == project_id
                && c.status == "open"
                && let Some(p) = c.screen_id.as_ref().or(c.screen_path.as_ref())
            {
                *counts.entry(p.clone()).or_insert(0) += 1;
            }
        }
        counts
    }
}

#[derive(Clone, Debug)]
pub struct DiffCellEntry {
    pub path: String,
    pub width_px: u32,
    pub kind: String,
    pub before_blob: Option<PathBuf>,
    pub after_blob: Option<PathBuf>,
    pub changed_pixels: Option<u64>,
    pub diff_percent: Option<f64>,
    pub overlay: Option<Arc<RenderImage>>,
    pub render_diff: Option<RenderDiffSummary>,
    pub capture_width: u32,
    pub capture_height: u32,
}

#[derive(Default)]
pub struct ImageCache {
    decoded: BTreeMap<PathBuf, Option<Arc<RenderImage>>>,
}

impl ImageCache {
    pub fn get_or_decode(&mut self, path: &Path) -> Option<Arc<RenderImage>> {
        if let Some(cached) = self.decoded.get(path) {
            return cached.clone();
        }
        let img = decode_png_to_render_image(path);
        self.decoded.insert(path.to_path_buf(), img.clone());
        img
    }
}

pub fn decode_png_bytes_to_render_image(bytes: &[u8]) -> Option<Arc<RenderImage>> {
    let dyn_img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    let mut rgba: RgbaImage = dyn_img.to_rgba8();
    // GPUI RenderImage expects premultiplied/BGRA channel order on macOS Metal / WGPU.
    for px in rgba.pixels_mut() {
        px.0.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new(smallvec![Frame::new(rgba)])))
}

pub fn decode_png_to_render_image(path: &Path) -> Option<Arc<RenderImage>> {
    let bytes = fs::read(path).ok()?;
    decode_png_bytes_to_render_image(&bytes)
}

fn rgba_to_render_image(width: u32, height: u32, mut rgba: Vec<u8>) -> Option<Arc<RenderImage>> {
    for chunk in rgba.as_chunks_mut::<4>().0 {
        chunk.swap(0, 2);
    }
    let buf = RgbaImage::from_raw(width, height, rgba)?;
    Some(Arc::new(RenderImage::new(smallvec![Frame::new(buf)])))
}

fn load_render_trees_for_run(
    run: &RunItem,
) -> BTreeMap<(String, u32, usize), Vec<RenderNodeSnapshot>> {
    let mut map = BTreeMap::new();
    let Some(dir) = run.run_dir.as_deref() else {
        return map;
    };
    let Ok(bytes) = fs::read(dir.join("report.json")) else {
        return map;
    };
    let Ok(report) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return map;
    };
    let Some(cells) = report["cells"].as_array() else {
        return map;
    };
    let mut counts: BTreeMap<(String, u32), usize> = BTreeMap::new();
    for cell in cells {
        let path = cell["path"]
            .as_str()
            .or_else(|| report["path"].as_str())
            .unwrap_or("/")
            .to_owned();
        let w = cell["width"].as_u64().unwrap_or(0) as u32;
        let ordinal = *counts.entry((path.clone(), w)).or_insert(0);
        *counts.get_mut(&(path.clone(), w)).unwrap() += 1;
        if let Some(arr) = cell["observation"]["renderTree"].as_array()
            && let Ok(tree) = serde_json::from_value::<Vec<RenderNodeSnapshot>>(
                serde_json::Value::Array(arr.clone()),
            )
            && !tree.is_empty()
        {
            map.insert((path, w, ordinal), tree);
        }
    }
    map
}

fn compute_entry_pixel_diff(entry: &mut DiffCellEntry) {
    if entry.overlay.is_some() || entry.kind != "changed" {
        return;
    }
    if let (Some(bp), Some(ap)) = (&entry.before_blob, &entry.after_blob)
        && let (Ok(b_bytes), Ok(a_bytes)) = (fs::read(bp), fs::read(ap))
        && let (Ok(b_img), Ok(a_img)) = (
            image::load_from_memory_with_format(&b_bytes, image::ImageFormat::Png),
            image::load_from_memory_with_format(&a_bytes, image::ImageFormat::Png),
        )
    {
        let br = b_img.to_rgba8();
        let ar = a_img.to_rgba8();
        let (bw, bh) = br.dimensions();
        let (aw, ah) = ar.dimensions();
        entry.capture_width = aw.max(bw).max(1);
        entry.capture_height = ah.max(bh).max(1);
        let (metrics, overlay_rgba) = compute_rgba_diff(bw, bh, br.as_raw(), aw, ah, ar.as_raw());
        entry.changed_pixels = Some(metrics.diff_pixels);
        entry.diff_percent = Some((metrics.diff_ratio * 10000.0).round() / 100.0);
        entry.overlay = rgba_to_render_image(metrics.width, metrics.height, overlay_rgba);
    }
}

/// Lazily decode PNGs and synthesize pixel-diff overlays only when the user opens Pixel mode.
pub fn hydrate_pixel_diffs(entries: &mut [DiffCellEntry]) {
    for entry in entries.iter_mut() {
        compute_entry_pixel_diff(entry);
    }
}

#[derive(Clone, Debug, Default)]
pub struct RunRenderTrees {
    pub trees_by_screen_width: BTreeMap<(String, u32), Vec<RenderNodeSnapshot>>,
}

pub fn load_run_render_trees(run: &RunItem) -> RunRenderTrees {
    let mut out = RunRenderTrees::default();
    let trees = load_render_trees_for_run(run);
    for ((path, width, _ordinal), tree) in trees {
        out.trees_by_screen_width
            .entry((path, width))
            .or_insert(tree);
    }
    out
}

pub fn compute_run_diff(
    before: &RunItem,
    after: &RunItem,
    include_pixels: bool,
) -> Vec<DiffCellEntry> {
    let before_trees = load_render_trees_for_run(before);
    let after_trees = load_render_trees_for_run(after);
    let mut before_map: BTreeMap<(String, u32, usize), &CellItem> = BTreeMap::new();
    for sc in &before.screens {
        let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
        for c in &sc.cells {
            let ordinal = *counts.entry(c.width_px).or_insert(0);
            *counts.get_mut(&c.width_px).unwrap() += 1;
            before_map.insert((sc.path.clone(), c.width_px, ordinal), c);
        }
    }
    let mut after_map: BTreeMap<(String, u32, usize), &CellItem> = BTreeMap::new();
    for sc in &after.screens {
        let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
        for c in &sc.cells {
            let ordinal = *counts.entry(c.width_px).or_insert(0);
            *counts.get_mut(&c.width_px).unwrap() += 1;
            after_map.insert((sc.path.clone(), c.width_px, ordinal), c);
        }
    }

    let mut keys: BTreeSet<(String, u32, usize)> = BTreeSet::new();
    keys.extend(before_map.keys().cloned());
    keys.extend(after_map.keys().cloned());

    let mut entries = Vec::new();
    for key in keys {
        let b = before_map.get(&key).copied();
        let a = after_map.get(&key).copied();
        match (b, a) {
            (None, Some(after_cell)) => entries.push(DiffCellEntry {
                path: key.0,
                width_px: key.1,
                kind: "added".into(),
                before_blob: None,
                after_blob: after_cell.blob_path.clone(),
                changed_pixels: None,
                diff_percent: None,
                overlay: None,
                render_diff: None,
                capture_width: key.1.max(1),
                capture_height: 800,
            }),
            (Some(before_cell), None) => entries.push(DiffCellEntry {
                path: key.0,
                width_px: key.1,
                kind: "removed".into(),
                before_blob: before_cell.blob_path.clone(),
                after_blob: None,
                changed_pixels: None,
                diff_percent: None,
                overlay: None,
                render_diff: None,
                capture_width: key.1.max(1),
                capture_height: 800,
            }),
            (Some(before_cell), Some(after_cell)) => {
                if before_cell.status_kind == after_cell.status_kind
                    && before_cell.blob_id == after_cell.blob_id
                {
                    continue;
                }
                let b_tree = before_trees.get(&key);
                let a_tree = after_trees.get(&key);
                let render_diff = if b_tree.is_some() || a_tree.is_some() {
                    Some(diff_render_trees(
                        b_tree.map(Vec::as_slice).unwrap_or(&[]),
                        a_tree.map(Vec::as_slice).unwrap_or(&[]),
                        before_cell.blob_id != after_cell.blob_id,
                    ))
                } else {
                    None
                };
                let has_render_changes = render_diff
                    .as_ref()
                    .is_some_and(|rd| !rd.changes.is_empty());
                let mut entry = DiffCellEntry {
                    path: key.0,
                    width_px: key.1,
                    kind: "changed".into(),
                    before_blob: before_cell.blob_path.clone(),
                    after_blob: after_cell.blob_path.clone(),
                    changed_pixels: None,
                    diff_percent: None,
                    overlay: None,
                    render_diff,
                    capture_width: key.1.max(1),
                    capture_height: 800,
                };
                if include_pixels || !has_render_changes {
                    compute_entry_pixel_diff(&mut entry);
                }
                entries.push(entry);
            }
            (None, None) => {}
        }
    }
    entries
}

pub fn default_dui_home() -> PathBuf {
    if let Ok(home) = std::env::var("DUI_HOME")
        && !home.trim().is_empty()
    {
        return PathBuf::from(home);
    }
    if let Ok(home) = std::env::var("HOME")
        && !home.trim().is_empty()
    {
        return PathBuf::from(home).join(".dui");
    }
    PathBuf::from(".dui")
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn parse_status(status_json: &str) -> (String, Option<String>) {
    let Ok(val) = serde_json::from_str::<Value>(status_json) else {
        return ("unverified".into(), None);
    };
    let kind = val["status"]
        .as_str()
        .or_else(|| val["kind"].as_str())
        .unwrap_or("unverified")
        .to_owned();
    let reason = val["reason"].as_str().map(str::to_owned);
    (kind, reason)
}

pub fn load_workspace(dui_home: &Path) -> Result<DesktopWorkspace, String> {
    let db_path = dui_home.join("dui.db");
    if !db_path.exists() {
        return Ok(DesktopWorkspace {
            dui_home: dui_home.to_path_buf(),
            db_path,
            runs: Vec::new(),
            comments: Vec::new(),
        });
    }
    let conn = Connection::open(&db_path).map_err(|e| format!("open dui.db: {e}"))?;

    let mut screen_paths: BTreeMap<String, String> = BTreeMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT id, path FROM screens")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        for row in rows.flatten() {
            screen_paths.insert(row.0, row.1);
        }
    }

    let has_run_branch: bool = conn
        .prepare("PRAGMA table_info(runs)")
        .ok()
        .and_then(|mut s| {
            s.query_map([], |row| row.get::<_, String>(1))
                .ok()
                .map(|rows| rows.flatten().any(|col| col == "branch"))
        })
        .unwrap_or(false);
    let branch_expr = if has_run_branch {
        "COALESCE(r.branch, w.branch)"
    } else {
        "w.branch"
    };
    let mut runs = Vec::new();
    {
        let query = format!(
            "SELECT r.id, r.project_id, p.display_name, {branch_expr}, r.created_at_ms, p.primary_root, w.path, w.app_relative
             FROM runs r
             JOIN projects p ON p.id = r.project_id
             LEFT JOIN worktrees w ON w.id = r.worktree_id
             ORDER BY r.created_at_ms DESC, r.id DESC
             LIMIT 40"
        );
        let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
        let run_rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in run_rows.flatten() {
            let (
                run_id,
                project_id,
                project_name,
                branch,
                created_at_ms,
                primary_root,
                wt_path,
                app_rel,
            ) = row;
            let app_rel_str = app_rel.as_deref().filter(|s| !s.is_empty()).unwrap_or(".");
            let candidate_wt = wt_path
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(|b| Path::new(b).join(app_rel_str));
            let candidate_root = Path::new(&primary_root).join(app_rel_str);
            let project_path = candidate_wt
                .filter(|p| p.exists())
                .or_else(|| candidate_root.exists().then_some(candidate_root));
            let candidate_run_dir = dui_home
                .join("projects")
                .join(&project_id)
                .join("runs")
                .join(&run_id);
            let run_dir = candidate_run_dir.is_dir().then_some(candidate_run_dir);
            let screens = load_run_screens(&conn, dui_home, &run_id, &screen_paths)?;
            runs.push(RunItem {
                id: run_id,
                project_id,
                project_name,
                branch,
                project_path,
                run_dir,
                created_at_ms,
                screens,
            });
        }
    }

    let comments = load_comments(&conn, &screen_paths)?;
    Ok(DesktopWorkspace {
        dui_home: dui_home.to_path_buf(),
        db_path,
        runs,
        comments,
    })
}

fn load_run_screens(
    conn: &Connection,
    dui_home: &Path,
    run_id: &str,
    screen_paths: &BTreeMap<String, String>,
) -> Result<Vec<ScreenItem>, String> {
    let mut screen_map: BTreeMap<String, ScreenItem> = BTreeMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT rs.screen_id, sc.path, sc.origin_json
                 FROM run_screens rs
                 JOIN screens sc ON sc.id = rs.screen_id
                 WHERE rs.run_id = ?1
                 ORDER BY sc.path ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![run_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (id, path, origin) = row.map_err(|e| e.to_string())?;
            let native_source = match serde_json::from_str::<ScreenOrigin>(&origin) {
                Ok(ScreenOrigin::Native { declaration }) => Some(declaration),
                _ => None,
            };
            screen_map.insert(
                id.clone(),
                ScreenItem {
                    id,
                    path,
                    native_source,
                    cells: Vec::new(),
                    outgoing: Vec::new(),
                },
            );
        }
    }

    {
        let mut stmt = conn
            .prepare(
                "SELECT c.id, c.screen_id, c.state_id, c.width_px, c.locale, c.status_json, c.image_blob_id, e.payload_json
                 FROM cells c
                 LEFT JOIN evidence e ON e.id = c.evidence_id
                 WHERE c.run_id = ?1
                 ORDER BY c.width_px ASC, c.id ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![run_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for (id, screen_id, state_id, width_px, locale, status_json, blob_id, evidence_json) in
            rows.flatten()
        {
            let (status_kind, reason) = parse_status(&status_json);
            let mut screen_evidence = None;
            let mut observed_path = None;
            let mut http_errors = Vec::new();
            if let Some(raw_ev) = evidence_json
                && let Ok(ev) = serde_json::from_str::<Value>(&raw_ev)
            {
                let obs = if ev["observation"].is_object() {
                    &ev["observation"]
                } else {
                    &ev
                };
                screen_evidence = obs.get("screenEvidence").cloned();
                observed_path = obs["path"].as_str().map(str::to_owned);
                if let Some(resps) = obs["responses"].as_object() {
                    for (url, code) in resps {
                        if let Some(status_code) = code.as_u64()
                            && status_code >= 400
                        {
                            http_errors.push(format!("HTTP {status_code} {url}"));
                        }
                    }
                }
            }
            let blob_path = blob_id.as_ref().and_then(|hash| {
                let candidate = dui_home.join("blobs").join(format!("{hash}.png"));
                candidate.is_file().then_some(candidate)
            });
            let entry = screen_map
                .entry(screen_id.clone())
                .or_insert_with(|| ScreenItem {
                    id: screen_id.clone(),
                    path: screen_paths
                        .get(&screen_id)
                        .cloned()
                        .unwrap_or_else(|| screen_id.clone()),
                    native_source: None,
                    cells: Vec::new(),
                    outgoing: Vec::new(),
                });
            let (image_width, image_height) = blob_path
                .as_deref()
                .and_then(read_png_dimensions)
                .map_or((None, None), |(w, h)| (Some(w), Some(h)));
            entry.cells.push(CellItem {
                id,
                state_id,
                width_px: width_px.max(0) as u32,
                locale,
                status_kind,
                reason,
                screen_evidence,
                observed_path,
                http_errors,
                blob_id,
                blob_path,
                image_width,
                image_height,
            });
        }
    }

    {
        let mut stmt = conn
            .prepare("SELECT from_json, to_json, source FROM edges WHERE run_id = ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![run_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for (from_json, to_json, source) in rows.flatten() {
            if let (Ok(from_ep), Ok(to_ep)) = (
                serde_json::from_str::<EdgeEndpoint>(&from_json),
                serde_json::from_str::<EdgeEndpoint>(&to_json),
            ) {
                let from_id = from_ep.screen_id().as_str().to_owned();
                let to_id = to_ep.screen_id().as_str().to_owned();
                let from_path = screen_paths
                    .get(&from_id)
                    .cloned()
                    .unwrap_or_else(|| from_id.clone());
                let to_path = screen_paths
                    .get(&to_id)
                    .cloned()
                    .unwrap_or_else(|| to_id.clone());
                if let Some(screen) = screen_map.get_mut(&from_id) {
                    screen.outgoing.push(EdgeItem {
                        from_path,
                        to_path,
                        source,
                    });
                }
            }
        }
    }

    let mut screens: Vec<ScreenItem> = screen_map.into_values().collect();
    screens.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(screens)
}

fn load_comments(
    conn: &Connection,
    screen_paths: &BTreeMap<String, String>,
) -> Result<Vec<CommentItem>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, project_id, run_id, target_json, author, body, status, created_at_ms, updated_at_ms
             FROM comments
             ORDER BY created_at_ms ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, i64>(8)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for (id, project_id, run_id, target_json, author, body, status, created_at_ms, updated_at_ms) in
        rows.flatten()
    {
        let target = serde_json::from_str::<CommentTarget>(&target_json).ok();
        let (screen_path, target_label) = describe_target(conn, target.as_ref(), screen_paths);
        let screen_id = target.as_ref().and_then(comment_screen_id);
        out.push(CommentItem {
            id,
            project_id,
            run_id,
            author,
            body,
            status,
            screen_path,
            screen_id,
            target_label,
            created_at_ms,
            updated_at_ms,
        });
    }
    Ok(out)
}

fn comment_screen_id(target: &CommentTarget) -> Option<String> {
    if let CommentTarget::Screen { screen_id } = target {
        Some(screen_id.as_str().to_owned())
    } else {
        None
    }
}

fn describe_target(
    conn: &Connection,
    target: Option<&CommentTarget>,
    screen_paths: &BTreeMap<String, String>,
) -> (Option<String>, String) {
    let Some(target) = target else {
        return (None, "target".into());
    };
    match target {
        CommentTarget::Screen { screen_id } => {
            let p = screen_paths.get(screen_id.as_str()).cloned();
            let label = p.clone().unwrap_or_else(|| screen_id.as_str().to_owned());
            (p, label)
        }
        CommentTarget::State { state_id } => {
            let path: Option<String> = conn
                .query_row(
                    "SELECT sc.path FROM states st JOIN screens sc ON sc.id = st.screen_id WHERE st.id = ?1",
                    params![state_id.as_str()],
                    |r| r.get(0),
                )
                .optional()
                .ok()
                .flatten();
            (path, state_id.as_str().to_owned())
        }
        CommentTarget::Cell { cell_id } => {
            let row: Option<(String, i64)> = conn
                .query_row(
                    "SELECT sc.path, c.width_px FROM cells c JOIN screens sc ON sc.id = c.screen_id WHERE c.id = ?1",
                    params![cell_id.as_str()],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .ok()
                .flatten();
            match row {
                Some((path, width)) => (Some(path), format!("{width} px")),
                None => (None, cell_id.as_str().to_owned()),
            }
        }
        CommentTarget::Edge { edge_id } => {
            let row: Option<(String, String)> = conn
                .query_row(
                    "SELECT from_json, to_json FROM edges WHERE id = ?1",
                    params![edge_id.as_str()],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .ok()
                .flatten();
            if let Some((fj, tj)) = row
                && let (Ok(from_ep), Ok(to_ep)) = (
                    serde_json::from_str::<EdgeEndpoint>(&fj),
                    serde_json::from_str::<EdgeEndpoint>(&tj),
                )
            {
                let from_p = screen_paths.get(from_ep.screen_id().as_str()).cloned();
                let to_p = screen_paths.get(to_ep.screen_id().as_str()).cloned();
                let label = format!(
                    "{} -> {}",
                    from_p.as_deref().unwrap_or("?"),
                    to_p.as_deref().unwrap_or("?")
                );
                return (from_p, label);
            }
            (None, edge_id.as_str().to_owned())
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NewCommentAnchor {
    Screen {
        id: String,
        path: String,
    },
    Cell {
        path: String,
        cell_id: String,
        width_px: u32,
    },
    Transition {
        from_path: String,
        to_path: String,
    },
}

pub fn add_comment(
    dui_home: &Path,
    project_id: &str,
    run_id: &str,
    anchor: NewCommentAnchor,
    author: &str,
    body: &str,
) -> Result<CommentItem, String> {
    let db_path = dui_home.join("dui.db");
    let conn = Connection::open(&db_path).map_err(|e| format!("open dui.db: {e}"))?;
    let pid = ProjectId::parse(project_id).map_err(|e| e.to_string())?;
    let rid = RunId::parse(run_id).map_err(|e| e.to_string())?;

    let (target, screen_path, target_label) = match anchor {
        NewCommentAnchor::Screen { id, path } => {
            let screen_id = ScreenId::parse(&id).map_err(|e| e.to_string())?;
            (
                CommentTarget::Screen { screen_id },
                Some(path.clone()),
                path,
            )
        }
        NewCommentAnchor::Cell {
            path,
            cell_id,
            width_px,
        } => {
            let cid = CellId::parse(&cell_id).map_err(|e| e.to_string())?;
            (
                CommentTarget::Cell { cell_id: cid },
                Some(path),
                format!("{width_px} px"),
            )
        }
        NewCommentAnchor::Transition { from_path, to_path } => {
            let from_id = Screen::derive_id(&from_path);
            let to_id = Screen::derive_id(&to_path);
            conn.execute(
                "INSERT OR IGNORE INTO screens (id, path, origin_json) VALUES (?1, ?2, ?3)",
                params![from_id.as_str(), &from_path, r#"{"kind":"entry"}"#],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT OR IGNORE INTO screens (id, path, origin_json) VALUES (?1, ?2, ?3)",
                params![to_id.as_str(), &to_path, r#"{"kind":"entry"}"#],
            )
            .map_err(|e| e.to_string())?;
            let edge_id = EdgeId::derive(&json!([&rid, &from_id, &to_id]));
            let from_json = serde_json::to_string(&EdgeEndpoint::Screen { screen_id: from_id })
                .map_err(|e| e.to_string())?;
            let to_json = serde_json::to_string(&EdgeEndpoint::Screen { screen_id: to_id })
                .map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT OR IGNORE INTO edges (id, run_id, from_json, to_json, source, trigger, selector, observed_widths_json)
                 VALUES (?1, ?2, ?3, ?4, 'extracted', NULL, NULL, '[]')",
                params![edge_id.as_str(), rid.as_str(), from_json, to_json],
            )
            .map_err(|e| e.to_string())?;
            let label = format!("{from_path} -> {to_path}");
            (CommentTarget::Edge { edge_id }, Some(from_path), label)
        }
    };

    let comment =
        Comment::open(pid, rid, target, author, body, now_ms()).map_err(|e| e.to_string())?;
    let target_json = serde_json::to_string(comment.target()).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO comments (id, project_id, run_id, target_json, author, body, status, created_at_ms, updated_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET body = excluded.body, status = excluded.status, updated_at_ms = excluded.updated_at_ms",
        params![
            comment.id().as_str(),
            comment.project_id().as_str(),
            comment.run_id().as_str(),
            target_json,
            comment.author(),
            comment.body(),
            comment.created_at_ms(),
            comment.updated_at_ms(),
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(CommentItem {
        id: comment.id().as_str().to_owned(),
        project_id: comment.project_id().as_str().to_owned(),
        run_id: comment.run_id().as_str().to_owned(),
        author: comment.author().to_owned(),
        body: comment.body().to_owned(),
        status: "open".into(),
        screen_path,
        screen_id: comment_screen_id(comment.target()),
        target_label,
        created_at_ms: comment.created_at_ms(),
        updated_at_ms: comment.updated_at_ms(),
    })
}

pub fn toggle_comment_status(dui_home: &Path, comment_id: &str) -> Result<String, String> {
    let db_path = dui_home.join("dui.db");
    let conn = Connection::open(&db_path).map_err(|e| format!("open dui.db: {e}"))?;
    let id = CommentId::parse(comment_id).map_err(|e| e.to_string())?;
    let row: (String, String, String, String, String, String, i64, i64) = conn
        .query_row(
            "SELECT project_id, run_id, target_json, author, body, status, created_at_ms, updated_at_ms
             FROM comments WHERE id = ?1",
            params![id.as_str()],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;

    let mut comment = Comment::open(
        ProjectId::parse(row.0).map_err(|e| e.to_string())?,
        RunId::parse(row.1).map_err(|e| e.to_string())?,
        serde_json::from_str(&row.2).map_err(|e| e.to_string())?,
        row.3,
        row.4,
        row.6,
    )
    .map_err(|e| e.to_string())?;
    if row.5 == "resolved" {
        comment.resolve(row.7).map_err(|e| e.to_string())?;
    }
    let next_ts = now_ms().max(row.7 + 1);
    let next_status = match comment.status() {
        CommentStatus::Open => {
            comment.resolve(next_ts).map_err(|e| e.to_string())?;
            "resolved"
        }
        CommentStatus::Resolved => {
            comment.reopen(next_ts).map_err(|e| e.to_string())?;
            "open"
        }
    };
    conn.execute(
        "UPDATE comments SET status = ?1, updated_at_ms = ?2 WHERE id = ?3",
        params![next_status, comment.updated_at_ms(), id.as_str()],
    )
    .map_err(|e| e.to_string())?;
    Ok(next_status.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const SCHEMA: &str = include_str!("../../../packages/db/migrations/0001_initial.sql");

    #[test]
    fn loads_runs_screens_and_roundtrips_comments_in_sqlite() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("dui.db");
        let conn = Connection::open(&db_path).expect("open db");
        conn.execute_batch(SCHEMA).expect("apply schema");

        let project_id = dui_core::model::Project::new("/repo/demo", "demo-app")
            .expect("project")
            .id()
            .as_str()
            .to_owned();
        let screen_id = Screen::derive_id("/order");
        conn.execute(
            "INSERT INTO projects (id, display_name, primary_root) VALUES (?1, 'demo-app', '/repo/demo')",
            params![&project_id],
        )
        .expect("insert project");
        conn.execute(
            "INSERT INTO worktrees (id, project_id, path, app_relative, branch) VALUES ('wt-1', ?1, '/repo/demo', '.', 'main')",
            params![&project_id],
        )
        .expect("insert worktree");
        conn.execute(
            "INSERT INTO commits (id, project_id, git_sha, dirty, tree_digest) VALUES ('cm-1', ?1, 'abc123', 0, NULL)",
            params![&project_id],
        )
        .expect("insert commit");
        conn.execute(
            "INSERT INTO runs (id, project_id, worktree_id, commit_id, kind_json, created_at_ms, environment_json, source_files_json)
             VALUES ('run-1', ?1, 'wt-1', 'cm-1', '{\"kind\":\"map\"}', 1000, '{}', '[]')",
            params![&project_id],
        )
        .expect("insert run");
        conn.execute(
            "INSERT INTO screens (id, path, origin_json) VALUES (?1, '/order', '{\"kind\":\"entry\"}')",
            params![screen_id.as_str()],
        )
        .expect("insert screen");
        conn.execute(
            "INSERT INTO run_screens (run_id, screen_id) VALUES ('run-1', ?1)",
            params![screen_id.as_str()],
        )
        .expect("insert run_screen");
        conn.execute(
            "INSERT INTO states (id, screen_id, scenario, locale, inputs_json, plan_index) VALUES ('st-1', ?1, 'default', NULL, '{}', 0)",
            params![screen_id.as_str()],
        )
        .expect("insert state");
        conn.execute(
            "INSERT INTO cells (id, run_id, screen_id, state_id, width_px, locale, status_json, image_blob_id, evidence_id, cache_json)
             VALUES ('cell-1', 'run-1', ?1, 'st-1', 375, NULL, '{\"status\":\"verified\"}', NULL, NULL, NULL)",
            params![screen_id.as_str()],
        )
        .expect("insert cell");
        let native_origin =
            json!({"kind":"native", "name":"profile", "source":"lib/router.dart", "line":17});
        let native_screen = Screen::new(
            "/profile",
            serde_json::from_value(native_origin.clone()).unwrap(),
        )
        .unwrap();
        let native_id = native_screen.id().as_str();
        conn.execute(
            "INSERT INTO screens (id, path, origin_json) VALUES (?1, '/profile', ?2)",
            params![native_id, native_origin.to_string()],
        )
        .expect("insert native screen");
        conn.execute(
            "INSERT INTO run_screens (run_id, screen_id) VALUES ('run-1', ?1)",
            params![native_id],
        )
        .expect("link native screen");
        drop(conn);

        assert_eq!(
            parse_status(r#"{"status":"verified"}"#),
            ("verified".into(), None)
        );
        assert_eq!(
            parse_status(r#"{"status":"unverified","reason":"assert-failed"}"#),
            ("unverified".into(), Some("assert-failed".into()))
        );
        assert_eq!(
            parse_status(r#"{"status":"unrenderable","reason":"no-render-env"}"#),
            ("unrenderable".into(), Some("no-render-env".into()))
        );

        let added = add_comment(
            dir.path(),
            &project_id,
            "run-1",
            NewCommentAnchor::Screen {
                id: screen_id.as_str().to_owned(),
                path: "/order".into(),
            },
            "reviewer",
            "Tighten button spacing on mobile",
        )
        .expect("add comment");
        assert_eq!(added.status, "open");

        let ws = load_workspace(dir.path()).expect("load workspace");
        assert_eq!(ws.runs.len(), 1);
        assert_eq!(ws.project_groups().len(), 1);
        let screens = &ws.runs[0].screens;
        assert_eq!(screens.len(), 2);
        assert_eq!(screens[0].cells.len(), 1);
        assert_eq!(screens[0].missing_preview(true), "No image");
        assert!(screens[1].cells.is_empty());
        assert_eq!(screens[1].missing_preview(true), "router.dart:17");
        assert_eq!(screens[1].missing_preview(false), "lib/router.dart:17");
        assert_eq!(ws.comments.len(), 1);
        assert_eq!(ws.comments[0].body, "Tighten button spacing on mobile");

        let native_comment = add_comment(
            dir.path(),
            &project_id,
            "run-1",
            NewCommentAnchor::Screen {
                id: native_id.to_owned(),
                path: "/profile".into(),
            },
            "reviewer",
            "Check the declared route",
        )
        .expect("comment on source route");
        assert_eq!(native_comment.screen_id.as_deref(), Some(native_id));
        let reloaded = load_workspace(dir.path()).unwrap();
        assert_eq!(
            reloaded.open_comments_by_screen(&project_id).get(native_id),
            Some(&1)
        );
        let next = toggle_comment_status(dir.path(), &added.id).expect("resolve");
        assert_eq!(next, "resolved");
        let reopened = toggle_comment_status(dir.path(), &added.id).expect("reopen");
        assert_eq!(reopened, "open");
    }

    #[test]
    fn lazy_desktop_diff_skips_pixel_overlay_until_hydrated() {
        let dir = tempfile::tempdir().unwrap();
        let before_dir = dir.path().join("before");
        let after_dir = dir.path().join("after");
        fs::create_dir_all(&before_dir).unwrap();
        fs::create_dir_all(&after_dir).unwrap();

        let make_png = |path: &Path, rgba: [u8; 4]| {
            let mut img = RgbaImage::new(2, 2);
            for px in img.pixels_mut() {
                *px = image::Rgba(rgba);
            }
            img.save_with_format(path, image::ImageFormat::Png).unwrap();
        };
        let b_png = before_dir.join("a.png");
        let a_png = after_dir.join("b.png");
        make_png(&b_png, [12, 12, 12, 255]);
        make_png(&a_png, [240, 240, 240, 255]);

        fs::write(
            before_dir.join("report.json"),
            json!({"path":"/","cells":[{"path":"/","width":1280,"observation":{"renderTree":[{"id":"h1#0","tag":"h1","text":"Old","rect":{"x":0,"y":0,"width":80,"height":24},"style":{}}]}}]}).to_string(),
        )
        .unwrap();
        fs::write(
            after_dir.join("report.json"),
            json!({"path":"/","cells":[{"path":"/","width":1280,"observation":{"renderTree":[{"id":"h1#0","tag":"h1","text":"New","rect":{"x":0,"y":0,"width":80,"height":24},"style":{}}]}}]}).to_string(),
        )
        .unwrap();

        let make_run = |id: &str, run_dir: PathBuf, blob_id: &str, blob_path: PathBuf| RunItem {
            id: id.into(),
            project_id: "p1".into(),
            project_name: "demo".into(),
            branch: Some("main".into()),
            created_at_ms: 1,
            project_path: None,
            run_dir: Some(run_dir),
            screens: vec![ScreenItem {
                id: "s1".into(),
                path: "/".into(),
                native_source: None,
                cells: vec![CellItem {
                    id: "c1".into(),
                    state_id: "st0".into(),
                    width_px: 1280,
                    locale: None,
                    status_kind: "verified".into(),
                    reason: None,
                    screen_evidence: None,
                    observed_path: Some("/".into()),
                    http_errors: Vec::new(),
                    blob_id: Some(blob_id.into()),
                    blob_path: Some(blob_path),
                    image_width: Some(2),
                    image_height: Some(2),
                }],
                outgoing: Vec::new(),
            }],
        };

        let before = make_run("r1", before_dir, "hash-a", b_png);
        let after = make_run("r2", after_dir, "hash-b", a_png);

        let mut entries = compute_run_diff(&before, &after, false);
        assert_eq!(entries.len(), 1);
        assert!(
            entries[0]
                .render_diff
                .as_ref()
                .is_some_and(|rd| rd.text_changed == 1)
        );
        assert!(entries[0].overlay.is_none());

        hydrate_pixel_diffs(&mut entries);
        assert!(entries[0].overlay.is_some());
        assert_eq!(entries[0].changed_pixels, Some(4));
    }

    #[test]
    fn compute_run_design_audit_loads_box_model_trees_and_detects_defects() {
        let dir = tempfile::tempdir().expect("tempdir");
        let run_dir = dir.path().join("run-design");
        fs::create_dir_all(&run_dir).expect("mkdir");
        fs::write(
            run_dir.join("report.json"),
            json!({
                "path": "/",
                "cells": [{
                    "path": "/",
                    "width": 375,
                    "observation": {
                        "renderTree": [
                            {
                                "id": "h1#0",
                                "tag": "h1",
                                "text": "Wide Heading",
                                "rect": {"x": 16, "y": 16, "width": 440, "height": 32},
                                "style": {"fontSize": "24px"},
                                "boxModel": {
                                    "margin": {"top": 8, "right": 0, "bottom": 12, "left": 0},
                                    "padding": {"top": 4, "right": 8, "bottom": 4, "left": 8},
                                    "gap": "8px"
                                },
                                "attributes": {"data-testid": "hero"}
                            },
                            {
                                "id": "button#0",
                                "tag": "button",
                                "text": "",
                                "rect": {"x": 16, "y": 72, "width": 20, "height": 20},
                                "style": {}
                            }
                        ]
                    }
                }]
            })
            .to_string(),
        )
        .expect("write report");

        let run = RunItem {
            id: "r1".into(),
            project_id: "p1".into(),
            project_name: "demo".into(),
            branch: Some("main".into()),
            project_path: None,
            run_dir: Some(run_dir),
            created_at_ms: 1,
            screens: vec![],
        };

        let loaded = load_run_render_trees(&run);
        let nodes = loaded
            .trees_by_screen_width
            .get(&("/".into(), 375))
            .expect("render tree loaded");
        assert_eq!(nodes.len(), 2);
        assert_eq!(
            nodes[0].box_model.as_ref().map(|b| b.padding.right),
            Some(8)
        );
        assert_eq!(
            nodes[0].attributes.get("data-testid").map(String::as_str),
            Some("hero")
        );
    }
}
