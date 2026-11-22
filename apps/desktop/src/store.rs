//! Direct SQLite reader/writer, synchronous RenderImage PNG cache, audit summary, and capture diff
//! against `~/.dui/dui.db` and `~/.dui/blobs/` for the GPUI desktop viewer.

use dui_core::{
    diff::compute_rgba_diff,
    model::{
        CellId, Comment, CommentId, CommentStatus, CommentTarget, EdgeEndpoint, EdgeId, ProjectId,
        RunId, Screen,
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
    pub observed_path: Option<String>,
    pub http_errors: Vec<String>,
    pub blob_id: Option<String>,
    pub blob_path: Option<PathBuf>,
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
    pub cells: Vec<CellItem>,
    pub outgoing: Vec<EdgeItem>,
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
        if set.is_empty() {
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

    pub fn open_comments_by_path(&self, project_id: &str) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for c in &self.comments {
            if c.project_id == project_id
                && c.status == "open"
                && let Some(ref p) = c.screen_path
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

pub fn compute_run_diff(before: &RunItem, after: &RunItem) -> Vec<DiffCellEntry> {
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
            }),
            (Some(before_cell), Some(after_cell)) => {
                if before_cell.status_kind == after_cell.status_kind
                    && before_cell.blob_id == after_cell.blob_id
                {
                    continue;
                }
                let mut changed_pixels = None;
                let mut diff_percent = None;
                let mut overlay = None;
                if let (Some(bp), Some(ap)) = (&before_cell.blob_path, &after_cell.blob_path)
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
                    let (metrics, overlay_rgba) =
                        compute_rgba_diff(bw, bh, br.as_raw(), aw, ah, ar.as_raw());
                    changed_pixels = Some(metrics.diff_pixels);
                    diff_percent = Some((metrics.diff_ratio * 10000.0).round() / 100.0);
                    overlay = rgba_to_render_image(metrics.width, metrics.height, overlay_rgba);
                }
                entries.push(DiffCellEntry {
                    path: key.0,
                    width_px: key.1,
                    kind: "changed".into(),
                    before_blob: before_cell.blob_path.clone(),
                    after_blob: after_cell.blob_path.clone(),
                    changed_pixels,
                    diff_percent,
                    overlay,
                });
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

    let mut runs = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT r.id, r.project_id, p.display_name, w.branch, r.created_at_ms, p.primary_root, w.path, w.app_relative
                 FROM runs r
                 JOIN projects p ON p.id = r.project_id
                 LEFT JOIN worktrees w ON w.id = r.worktree_id
                 ORDER BY r.created_at_ms DESC, r.id DESC
                 LIMIT 40",
            )
            .map_err(|e| e.to_string())?;
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
                "SELECT rs.screen_id, sc.path
                 FROM run_screens rs
                 JOIN screens sc ON sc.id = rs.screen_id
                 WHERE rs.run_id = ?1
                 ORDER BY sc.path ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![run_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        for (id, path) in rows.flatten() {
            screen_map.insert(
                id.clone(),
                ScreenItem {
                    id,
                    path,
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
                    cells: Vec::new(),
                    outgoing: Vec::new(),
                });
            entry.cells.push(CellItem {
                id,
                state_id,
                width_px: width_px.max(0) as u32,
                locale,
                status_kind,
                reason,
                observed_path,
                http_errors,
                blob_id,
                blob_path,
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
        let (screen_path, target_label) = describe_target(conn, &target_json, screen_paths);
        out.push(CommentItem {
            id,
            project_id,
            run_id,
            author,
            body,
            status,
            screen_path,
            target_label,
            created_at_ms,
            updated_at_ms,
        });
    }
    Ok(out)
}

fn describe_target(
    conn: &Connection,
    target_json: &str,
    screen_paths: &BTreeMap<String, String>,
) -> (Option<String>, String) {
    let Ok(target) = serde_json::from_str::<CommentTarget>(target_json) else {
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
    Screen(String),
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
        NewCommentAnchor::Screen(path) => {
            let screen_id = Screen::derive_id(&path);
            conn.execute(
                "INSERT OR IGNORE INTO screens (id, path, origin_json) VALUES (?1, ?2, ?3)",
                params![screen_id.as_str(), &path, r#"{"kind":"entry"}"#],
            )
            .map_err(|e| e.to_string())?;
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
            NewCommentAnchor::Screen("/order".into()),
            "reviewer",
            "Tighten button spacing on mobile",
        )
        .expect("add comment");
        assert_eq!(added.status, "open");

        let ws = load_workspace(dir.path()).expect("load workspace");
        assert_eq!(ws.runs.len(), 1);
        assert_eq!(ws.project_groups().len(), 1);
        assert_eq!(ws.runs[0].screens.len(), 1);
        assert_eq!(ws.runs[0].screens[0].cells.len(), 1);
        assert_eq!(ws.comments.len(), 1);
        assert_eq!(ws.comments[0].body, "Tighten button spacing on mobile");

        let next = toggle_comment_status(dir.path(), &added.id).expect("resolve");
        assert_eq!(next, "resolved");
        let reopened = toggle_comment_status(dir.path(), &added.id).expect("reopen");
        assert_eq!(reopened, "open");
    }
}
