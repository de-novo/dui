use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxSpacing {
    #[serde(default)]
    pub top: i32,
    #[serde(default)]
    pub right: i32,
    #[serde(default)]
    pub bottom: i32,
    #[serde(default)]
    pub left: i32,
}

impl BoxSpacing {
    pub const fn is_zero(&self) -> bool {
        self.top == 0 && self.right == 0 && self.bottom == 0 && self.left == 0
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxModel {
    #[serde(default, skip_serializing_if = "BoxSpacing::is_zero")]
    pub margin: BoxSpacing,
    #[serde(default, skip_serializing_if = "BoxSpacing::is_zero")]
    pub padding: BoxSpacing,
    #[serde(default, skip_serializing_if = "BoxSpacing::is_zero")]
    pub border: BoxSpacing,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_radius: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderNodeSnapshot {
    pub id: String,
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default)]
    pub text: String,
    pub rect: RenderRect,
    #[serde(default)]
    pub style: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub box_model: Option<BoxModel>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderChangeKind {
    Added,
    Removed,
    TextChanged,
    LayoutMoved,
    StyleChanged,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderElementChange {
    pub kind: RenderChangeKind,
    pub id: String,
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_rect: Option<RenderRect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_rect: Option<RenderRect>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub style_changes: BTreeMap<String, (String, String)>,
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderDiffSummary {
    pub added: usize,
    pub removed: usize,
    pub text_changed: usize,
    pub layout_moved: usize,
    pub style_changed: usize,
    pub visual_only: bool,
    pub changes: Vec<RenderElementChange>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffCellSnapshot {
    pub path: String,
    pub scenario: String,
    pub state: String,
    pub width: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_key: Option<String>,
    #[serde(default)]
    pub source_files: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub render_tree: Vec<RenderNodeSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellSideSummary {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_key: Option<String>,
}

impl From<&DiffCellSnapshot> for CellSideSummary {
    fn from(s: &DiffCellSnapshot) -> Self {
        Self {
            status: s.status.clone(),
            reason: s.reason.clone(),
            image: s.image.clone(),
            cache_key: s.cache_key.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CellDiffKind {
    Added,
    Removed,
    Changed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusChange {
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PixelBounds {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PixelDiffMetrics {
    pub diff_pixels: u64,
    pub total_pixels: u64,
    pub diff_ratio: f64,
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_bounds: Option<PixelBounds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay_image: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellDiffEntry {
    pub kind: CellDiffKind,
    pub path: String,
    pub scenario: String,
    pub state: String,
    pub width: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<CellSideSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<CellSideSummary>,
    pub changed_files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_change: Option<StatusChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_diff: Option<RenderDiffSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_diff: Option<PixelDiffMetrics>,
}

impl CellDiffEntry {
    /// Pixel diffs apply only when both captures rendered a verified cell.
    /// Unrenderable or unverified cells report status changes instead.
    pub fn supports_pixel_diff(&self) -> bool {
        if self.kind != CellDiffKind::Changed {
            return false;
        }
        match (&self.before, &self.after) {
            (Some(b), Some(a)) => {
                b.status == "verified"
                    && a.status == "verified"
                    && b.image.is_some()
                    && a.image.is_some()
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenDiffGroup {
    pub path: String,
    pub cells: Vec<CellDiffEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureDiffSummary {
    pub total_changed: usize,
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub screens_affected: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureDiff {
    pub summary: CaptureDiffSummary,
    pub screens: Vec<ScreenDiffGroup>,
}

type CellKey = (String, String, u32, Option<String>);

fn cell_key(cell: &DiffCellSnapshot) -> CellKey {
    (
        cell.path.clone(),
        cell.state.clone(),
        cell.width,
        cell.locale.clone(),
    )
}

fn diff_source_files(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut all_keys = BTreeSet::new();
    for k in before.keys() {
        all_keys.insert(k);
    }
    for k in after.keys() {
        all_keys.insert(k);
    }
    all_keys
        .into_iter()
        .filter(|k| before.get(*k) != after.get(*k))
        .cloned()
        .collect()
}

fn cells_unchanged(before: &DiffCellSnapshot, after: &DiffCellSnapshot) -> bool {
    if before.status != after.status || before.reason != after.reason {
        return false;
    }
    if before.image.is_some() || after.image.is_some() {
        return before.image == after.image;
    }
    match (&before.cache_key, &after.cache_key) {
        (Some(a), Some(b)) => a == b,
        _ => true,
    }
}

/// Compare two captures and list only added, removed, or changed cells grouped by screen,
/// then sorted deterministically by state, width, and locale.
pub fn diff_captures(before: &[DiffCellSnapshot], after: &[DiffCellSnapshot]) -> CaptureDiff {
    let mut before_map: BTreeMap<CellKey, &DiffCellSnapshot> = BTreeMap::new();
    for cell in before {
        before_map.insert(cell_key(cell), cell);
    }
    let mut after_map: BTreeMap<CellKey, &DiffCellSnapshot> = BTreeMap::new();
    for cell in after {
        after_map.insert(cell_key(cell), cell);
    }

    let mut all_keys = BTreeSet::new();
    for k in before_map.keys() {
        all_keys.insert(k.clone());
    }
    for k in after_map.keys() {
        all_keys.insert(k.clone());
    }

    let mut by_screen: BTreeMap<String, Vec<CellDiffEntry>> = BTreeMap::new();
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut changed = 0usize;
    let mut unchanged = 0usize;

    for key in all_keys {
        match (before_map.get(&key), after_map.get(&key)) {
            (None, Some(a)) => {
                added += 1;
                let changed_files = a.source_files.keys().cloned().collect();
                by_screen
                    .entry(a.path.clone())
                    .or_default()
                    .push(CellDiffEntry {
                        kind: CellDiffKind::Added,
                        path: a.path.clone(),
                        scenario: a.scenario.clone(),
                        state: a.state.clone(),
                        width: a.width,
                        locale: a.locale.clone(),
                        before: None,
                        after: Some(CellSideSummary::from(*a)),
                        changed_files,
                        status_change: None,
                        render_diff: None,
                        pixel_diff: None,
                    });
            }
            (Some(b), None) => {
                removed += 1;
                let changed_files = b.source_files.keys().cloned().collect();
                by_screen
                    .entry(b.path.clone())
                    .or_default()
                    .push(CellDiffEntry {
                        kind: CellDiffKind::Removed,
                        path: b.path.clone(),
                        scenario: b.scenario.clone(),
                        state: b.state.clone(),
                        width: b.width,
                        locale: b.locale.clone(),
                        before: Some(CellSideSummary::from(*b)),
                        after: None,
                        changed_files,
                        status_change: None,
                        render_diff: None,
                        pixel_diff: None,
                    });
            }
            (Some(b), Some(a)) => {
                if cells_unchanged(b, a) {
                    unchanged += 1;
                    continue;
                }
                changed += 1;
                let changed_files = diff_source_files(&b.source_files, &a.source_files);
                let status_change = if b.status != a.status || b.reason != a.reason {
                    Some(StatusChange {
                        from: b.status.clone(),
                        to: a.status.clone(),
                        from_reason: b.reason.clone(),
                        to_reason: a.reason.clone(),
                    })
                } else {
                    None
                };
                let render_diff = if !b.render_tree.is_empty() || !a.render_tree.is_empty() {
                    Some(diff_render_trees(
                        &b.render_tree,
                        &a.render_tree,
                        b.image != a.image,
                    ))
                } else {
                    None
                };
                by_screen
                    .entry(a.path.clone())
                    .or_default()
                    .push(CellDiffEntry {
                        kind: CellDiffKind::Changed,
                        path: a.path.clone(),
                        scenario: a.scenario.clone(),
                        state: a.state.clone(),
                        width: a.width,
                        locale: a.locale.clone(),
                        before: Some(CellSideSummary::from(*b)),
                        after: Some(CellSideSummary::from(*a)),
                        changed_files,
                        status_change,
                        render_diff,
                        pixel_diff: None,
                    });
            }
            (None, None) => {}
        }
    }

    let screens: Vec<ScreenDiffGroup> = by_screen
        .into_iter()
        .map(|(path, mut cells)| {
            cells.sort_by(|a, b| {
                a.state
                    .cmp(&b.state)
                    .then_with(|| a.width.cmp(&b.width))
                    .then_with(|| a.locale.cmp(&b.locale))
            });
            ScreenDiffGroup { path, cells }
        })
        .collect();

    let total_changed = added + removed + changed;
    let screens_affected = screens.len();
    CaptureDiff {
        summary: CaptureDiffSummary {
            total_changed,
            added,
            removed,
            changed,
            unchanged,
            screens_affected,
        },
        screens,
    }
}

/// Normalize GPUI / native AccessKit element arrays (`role`, `label`, `bounds`, `checked`, `visible`)
/// into the unified platform-neutral `RenderNodeSnapshot` tree so native desktop apps share the
/// exact same element-level diff and canvas overlay pipeline as Web, TUI, and Android.
pub fn accesskit_elements_to_render_tree(
    elements: &[serde_json::Value],
) -> Vec<RenderNodeSnapshot> {
    let mut nodes = Vec::new();
    let mut id_counts: BTreeMap<String, usize> = BTreeMap::new();
    for el in elements {
        if el.get("visible").and_then(serde_json::Value::as_bool) == Some(false) {
            continue;
        }
        let bounds = &el["bounds"];
        let x = bounds["x"].as_f64().unwrap_or(0.0).round() as i32;
        let y = bounds["y"].as_f64().unwrap_or(0.0).round() as i32;
        let width = bounds["width"].as_f64().unwrap_or(0.0).round() as i32;
        let height = bounds["height"].as_f64().unwrap_or(0.0).round() as i32;
        if width <= 0 || height <= 0 {
            continue;
        }
        let raw_role = el["role"].as_str().map(str::trim).filter(|s| !s.is_empty());
        let tag = match raw_role.map(|r| r.to_ascii_lowercase()).as_deref() {
            Some("button") => "button",
            Some("checkbox") => "checkbox",
            Some("label" | "statictext") => "label",
            Some("textinput" | "textbox") => "input",
            Some("heading") => "h1",
            Some("list") => "list",
            Some("listitem") => "li",
            Some("dialog") => "dialog",
            Some("tab") => "tab",
            Some("switch") => "switch",
            Some("link") => "a",
            Some("image" | "img") => "img",
            _ => "element",
        };
        let text = el["label"].as_str().map(str::trim).unwrap_or("").to_owned();
        if text.is_empty() && raw_role.is_none() {
            continue;
        }
        let count = id_counts.entry(tag.to_owned()).or_insert(0);
        let id = format!("{tag}#{count}");
        *count += 1;

        let mut attributes = BTreeMap::new();
        if let Some(path) = el["path"].as_str().filter(|s| !s.is_empty()) {
            attributes.insert("accesskitPath".into(), path.to_owned());
        }
        if let Some(checked) = el.get("checked").and_then(serde_json::Value::as_bool) {
            attributes.insert("checked".into(), checked.to_string());
        }
        if let Some(role) = raw_role {
            attributes.insert("accesskitRole".into(), role.to_owned());
        }

        nodes.push(RenderNodeSnapshot {
            id,
            tag: tag.into(),
            role: raw_role.map(|r| r.to_ascii_lowercase()),
            text,
            rect: RenderRect {
                x,
                y,
                width,
                height,
            },
            style: BTreeMap::new(),
            parent_id: None,
            box_model: None,
            attributes,
        });
    }
    nodes
}

/// Pure structural render-tree diff comparing rendered DOM/layout/style elements between two
/// captures. Uses a 2px layout tolerance so subpixel/antialiasing jitter never triggers noise.
pub fn diff_render_trees(
    before: &[RenderNodeSnapshot],
    after: &[RenderNodeSnapshot],
    images_differ: bool,
) -> RenderDiffSummary {
    const LAYOUT_TOLERANCE_PX: i32 = 2;

    let before_map: BTreeMap<&str, &RenderNodeSnapshot> =
        before.iter().map(|n| (n.id.as_str(), n)).collect();
    let after_map: BTreeMap<&str, &RenderNodeSnapshot> =
        after.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut keys: BTreeSet<&str> = BTreeSet::new();
    keys.extend(before_map.keys().copied());
    keys.extend(after_map.keys().copied());

    let mut added = 0usize;
    let mut removed = 0usize;
    let mut text_changed = 0usize;
    let mut layout_moved = 0usize;
    let mut style_changed = 0usize;
    let mut changes = Vec::new();

    for id in keys {
        match (before_map.get(id), after_map.get(id)) {
            (None, Some(a)) => {
                added += 1;
                let label = if a.text.is_empty() {
                    a.tag.clone()
                } else {
                    format!("{} \"{}\"", a.tag, a.text)
                };
                changes.push(RenderElementChange {
                    kind: RenderChangeKind::Added,
                    id: a.id.clone(),
                    tag: a.tag.clone(),
                    role: a.role.clone(),
                    before_text: None,
                    after_text: (!a.text.is_empty()).then(|| a.text.clone()),
                    before_rect: None,
                    after_rect: Some(a.rect),
                    style_changes: BTreeMap::new(),
                    summary: format!("+ {label}"),
                });
            }
            (Some(b), None) => {
                removed += 1;
                let label = if b.text.is_empty() {
                    b.tag.clone()
                } else {
                    format!("{} \"{}\"", b.tag, b.text)
                };
                changes.push(RenderElementChange {
                    kind: RenderChangeKind::Removed,
                    id: b.id.clone(),
                    tag: b.tag.clone(),
                    role: b.role.clone(),
                    before_text: (!b.text.is_empty()).then(|| b.text.clone()),
                    after_text: None,
                    before_rect: Some(b.rect),
                    after_rect: None,
                    style_changes: BTreeMap::new(),
                    summary: format!("- {label}"),
                });
            }
            (Some(b), Some(a)) => {
                let text_diff = b.text != a.text;
                let dx = a.rect.x - b.rect.x;
                let dy = a.rect.y - b.rect.y;
                let dw = a.rect.width - b.rect.width;
                let dh = a.rect.height - b.rect.height;
                let layout_diff = dx.abs() > LAYOUT_TOLERANCE_PX
                    || dy.abs() > LAYOUT_TOLERANCE_PX
                    || dw.abs() > LAYOUT_TOLERANCE_PX
                    || dh.abs() > LAYOUT_TOLERANCE_PX;
                let mut style_changes = BTreeMap::new();
                let mut style_keys: BTreeSet<&str> = BTreeSet::new();
                style_keys.extend(b.style.keys().map(String::as_str));
                style_keys.extend(a.style.keys().map(String::as_str));
                for prop in style_keys {
                    let bv = b.style.get(prop).map(String::as_str).unwrap_or("");
                    let av = a.style.get(prop).map(String::as_str).unwrap_or("");
                    if bv != av {
                        style_changes.insert(prop.to_owned(), (bv.to_owned(), av.to_owned()));
                    }
                }
                if !text_diff && !layout_diff && style_changes.is_empty() {
                    continue;
                }
                let (kind, summary) = if text_diff {
                    text_changed += 1;
                    (
                        RenderChangeKind::TextChanged,
                        format!("{} \"{}\" → \"{}\"", a.tag, b.text, a.text),
                    )
                } else if !style_changes.is_empty() {
                    style_changed += 1;
                    let props = style_changes.keys().cloned().collect::<Vec<_>>().join(", ");
                    (
                        RenderChangeKind::StyleChanged,
                        format!("{} style ({props})", a.tag),
                    )
                } else {
                    layout_moved += 1;
                    let mut deltas = Vec::new();
                    if dx.abs() > LAYOUT_TOLERANCE_PX {
                        deltas.push(format!("x{dx:+}"));
                    }
                    if dy.abs() > LAYOUT_TOLERANCE_PX {
                        deltas.push(format!("y{dy:+}"));
                    }
                    if dw.abs() > LAYOUT_TOLERANCE_PX {
                        deltas.push(format!("w{dw:+}"));
                    }
                    if dh.abs() > LAYOUT_TOLERANCE_PX {
                        deltas.push(format!("h{dh:+}"));
                    }
                    (
                        RenderChangeKind::LayoutMoved,
                        format!("{} {}", a.tag, deltas.join(" ")),
                    )
                };
                changes.push(RenderElementChange {
                    kind,
                    id: a.id.clone(),
                    tag: a.tag.clone(),
                    role: a.role.clone(),
                    before_text: text_diff.then(|| b.text.clone()),
                    after_text: text_diff.then(|| a.text.clone()),
                    before_rect: Some(b.rect),
                    after_rect: Some(a.rect),
                    style_changes,
                    summary,
                });
            }
            (None, None) => {}
        }
    }

    let visual_only = changes.is_empty() && images_differ;
    RenderDiffSummary {
        added,
        removed,
        text_changed,
        layout_moved,
        style_changed,
        visual_only,
        changes,
    }
}

/// Compute per-pixel RGBA difference metrics and a readable highlight overlay buffer.
/// Unchanged pixels are softly desaturated; changed pixels are alpha-blended with brand orange
/// (`#EF803F` at 58% opacity) over the `after` screenshot so underlying UI text stays legible.
pub fn compute_rgba_diff(
    before_w: u32,
    before_h: u32,
    before_rgba: &[u8],
    after_w: u32,
    after_h: u32,
    after_rgba: &[u8],
) -> (PixelDiffMetrics, Vec<u8>) {
    let width = before_w.max(after_w);
    let height = before_h.max(after_h);
    let total_pixels = u64::from(width) * u64::from(height);
    let mut overlay = vec![0u8; (total_pixels as usize).saturating_mul(4)];
    let mut diff_pixels = 0u64;
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;

    for y in 0..height {
        for x in 0..width {
            let b_px = pixel_at(before_w, before_h, before_rgba, x, y);
            let a_px = pixel_at(after_w, after_h, after_rgba, x, y);
            let dst = ((y as usize) * (width as usize) + (x as usize)) * 4;
            if b_px == a_px {
                let px = a_px.unwrap_or([255, 255, 255, 255]);
                let lum = ((u16::from(px[0]) * 77 + u16::from(px[1]) * 150 + u16::from(px[2]) * 29)
                    >> 8) as u8;
                let faded = ((u16::from(lum) + 255 * 2) / 3) as u8;
                overlay[dst] = faded;
                overlay[dst + 1] = faded;
                overlay[dst + 2] = faded;
                overlay[dst + 3] = 255;
            } else {
                diff_pixels += 1;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                let base = a_px.or(b_px).unwrap_or([24, 24, 27, 255]);
                overlay[dst] = ((u16::from(base[0]) * 42 + 239 * 58) / 100) as u8;
                overlay[dst + 1] = ((u16::from(base[1]) * 42 + 128 * 58) / 100) as u8;
                overlay[dst + 2] = ((u16::from(base[2]) * 42 + 63 * 58) / 100) as u8;
                overlay[dst + 3] = 255;
            }
        }
    }

    let changed_bounds = if diff_pixels > 0 {
        Some(PixelBounds {
            x: min_x,
            y: min_y,
            width: max_x - min_x + 1,
            height: max_y - min_y + 1,
        })
    } else {
        None
    };

    let diff_ratio = if total_pixels > 0 {
        (diff_pixels as f64 / total_pixels as f64 * 10000.0).round() / 10000.0
    } else {
        0.0
    };

    (
        PixelDiffMetrics {
            diff_pixels,
            total_pixels,
            diff_ratio,
            width,
            height,
            changed_bounds,
            overlay_image: None,
        },
        overlay,
    )
}

/// Compute the transitive closure of source files affected by `changed` using a forward
/// `module_graph` (`importer -> [imported...]`). Pure and deterministic.
pub fn affected_files(
    module_graph: &BTreeMap<String, Vec<String>>,
    changed: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut reverse: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (importer, deps) in module_graph {
        for dep in deps {
            reverse
                .entry(dep.as_str())
                .or_default()
                .push(importer.as_str());
        }
    }
    let mut affected: BTreeSet<String> = changed.clone();
    let mut queue: std::collections::VecDeque<String> = changed.iter().cloned().collect();
    while let Some(file) = queue.pop_front() {
        if let Some(importers) = reverse.get(file.as_str()) {
            for &imp in importers {
                if affected.insert(imp.to_owned()) {
                    queue.push_back(imp.to_owned());
                }
            }
        }
    }
    affected
}

fn pixel_at(w: u32, h: u32, buf: &[u8], x: u32, y: u32) -> Option<[u8; 4]> {
    if x >= w || y >= h {
        return None;
    }
    let idx = ((y as usize) * (w as usize) + (x as usize)) * 4;
    if idx + 3 < buf.len() {
        Some([buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(
        path: &str,
        state: &str,
        width: u32,
        status: &str,
        reason: Option<&str>,
        image: Option<&str>,
        files: &[(&str, &str)],
    ) -> DiffCellSnapshot {
        DiffCellSnapshot {
            path: path.to_owned(),
            scenario: "default".to_owned(),
            state: state.to_owned(),
            width,
            locale: None,
            status: status.to_owned(),
            reason: reason.map(str::to_owned),
            image: image.map(str::to_owned),
            cache_key: None,
            source_files: files
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            render_tree: Vec::new(),
        }
    }

    #[test]
    fn identical_captures_produce_empty_diff() {
        let before = vec![
            snap(
                "/catalog",
                "s1",
                375,
                "verified",
                None,
                Some("aaa-375.png"),
                &[("Catalog.tsx", "h1")],
            ),
            snap(
                "/settings",
                "s2",
                1280,
                "verified",
                None,
                Some("bbb-1280.png"),
                &[("Settings.tsx", "h2")],
            ),
        ];
        let diff = diff_captures(&before, &before);
        assert_eq!(diff.summary.total_changed, 0);
        assert_eq!(diff.summary.unchanged, 2);
        assert!(diff.screens.is_empty());
    }

    #[test]
    fn groups_changed_added_and_removed_cells_with_changed_files() {
        let before = vec![
            snap(
                "/catalog",
                "s1",
                375,
                "verified",
                None,
                Some("aaa-375.png"),
                &[("Catalog.tsx", "h1"), ("Shared.tsx", "s0")],
            ),
            snap(
                "/settings",
                "s2",
                375,
                "verified",
                None,
                Some("bbb-375.png"),
                &[("Settings.tsx", "h2"), ("Shared.tsx", "s0")],
            ),
            snap(
                "/legacy",
                "s3",
                375,
                "verified",
                None,
                Some("ccc-375.png"),
                &[("Legacy.tsx", "h3")],
            ),
        ];
        let after = vec![
            snap(
                "/catalog",
                "s1",
                375,
                "verified",
                None,
                Some("ddd-375.png"),
                &[("Catalog.tsx", "h1-updated"), ("Shared.tsx", "s0")],
            ),
            snap(
                "/settings",
                "s2",
                375,
                "verified",
                None,
                Some("bbb-375.png"),
                &[("Settings.tsx", "h2"), ("Shared.tsx", "s0")],
            ),
            snap(
                "/billing",
                "s4",
                1280,
                "verified",
                None,
                Some("eee-1280.png"),
                &[("Billing.tsx", "h4")],
            ),
        ];

        let diff = diff_captures(&before, &after);
        assert_eq!(diff.summary.added, 1);
        assert_eq!(diff.summary.removed, 1);
        assert_eq!(diff.summary.changed, 1);
        assert_eq!(diff.summary.unchanged, 1);
        assert_eq!(diff.summary.screens_affected, 3);
        let paths: Vec<&str> = diff.screens.iter().map(|s| s.path.as_str()).collect();
        assert_eq!(paths, vec!["/billing", "/catalog", "/legacy"]);
        let catalog = &diff.screens[1].cells[0];
        assert_eq!(catalog.kind, CellDiffKind::Changed);
        assert_eq!(catalog.changed_files, vec!["Catalog.tsx"]);
        assert!(catalog.supports_pixel_diff());
    }

    #[test]
    fn unrenderable_or_unverified_cells_report_status_change_without_pixel_diff() {
        let before = vec![snap(
            "/checkout",
            "s1",
            375,
            "verified",
            None,
            Some("aaa-375.png"),
            &[("Checkout.tsx", "h1")],
        )];
        let after = vec![snap(
            "/checkout",
            "s1",
            375,
            "unrenderable",
            Some("no-render-env"),
            None,
            &[("Checkout.tsx", "h1")],
        )];

        let diff = diff_captures(&before, &after);
        assert_eq!(diff.summary.changed, 1);
        let entry = &diff.screens[0].cells[0];
        assert!(!entry.supports_pixel_diff());
        let sc = entry.status_change.as_ref().expect("status change");
        assert_eq!(sc.from, "verified");
        assert_eq!(sc.to, "unrenderable");
        assert_eq!(sc.to_reason.as_deref(), Some("no-render-env"));
    }

    #[test]
    fn rgba_diff_computes_exact_changed_bounds_and_ratio() {
        let before = vec![
            10, 10, 10, 255, 10, 10, 10, 255, // row 0
            10, 10, 10, 255, 10, 10, 10, 255, // row 1
        ];
        let after = vec![
            10, 10, 10, 255, 200, 10, 10, 255, // pixel (1,0) changed
            10, 10, 10, 255, 10, 10, 10, 255, // row 1 unchanged
        ];
        let (metrics, overlay) = compute_rgba_diff(2, 2, &before, 2, 2, &after);
        assert_eq!(metrics.diff_pixels, 1);
        assert_eq!(metrics.total_pixels, 4);
        assert_eq!(metrics.diff_ratio, 0.25);
        assert_eq!(
            metrics.changed_bounds,
            Some(PixelBounds {
                x: 1,
                y: 0,
                width: 1,
                height: 1,
            })
        );
        // Pixel (1,0) alpha-blended with #EF803F (58% orange over 42% [200, 10, 10]).
        assert_eq!(&overlay[4..8], &[222, 78, 40, 255]);
    }

    #[test]
    fn affected_files_propagates_changes_transitively_without_touching_unrelated_screens() {
        let graph = BTreeMap::from([
            ("Checkout.tsx".into(), vec!["Button.tsx".into()]),
            (
                "App.tsx".into(),
                vec!["Checkout.tsx".into(), "About.tsx".into()],
            ),
            ("About.tsx".into(), vec!["Footer.tsx".into()]),
        ]);
        let changed = BTreeSet::from(["Button.tsx".into()]);
        let affected = affected_files(&graph, &changed);
        assert!(affected.contains("Button.tsx"));
        assert!(affected.contains("Checkout.tsx"));
        assert!(affected.contains("App.tsx"));
        assert!(!affected.contains("About.tsx"));
        assert!(!affected.contains("Footer.tsx"));
    }

    fn render_node(
        id: &str,
        tag: &str,
        text: &str,
        rect: RenderRect,
        style: BTreeMap<String, String>,
    ) -> RenderNodeSnapshot {
        RenderNodeSnapshot {
            id: id.into(),
            tag: tag.into(),
            role: None,
            text: text.into(),
            rect,
            style,
            parent_id: None,
            box_model: None,
            attributes: BTreeMap::new(),
        }
    }

    #[test]
    fn diff_render_trees_tracks_text_layout_style_added_removed_and_ignores_1px_jitter() {
        let before = vec![
            render_node(
                "h1#0",
                "h1",
                "Catalog",
                RenderRect {
                    x: 16,
                    y: 16,
                    width: 200,
                    height: 32,
                },
                BTreeMap::from([("color".into(), "rgb(0, 0, 0)".into())]),
            ),
            render_node(
                "button#0",
                "button",
                "Save",
                RenderRect {
                    x: 16,
                    y: 120,
                    width: 80,
                    height: 36,
                },
                BTreeMap::new(),
            ),
            render_node(
                "p#0",
                "p",
                "Stable copy",
                RenderRect {
                    x: 16,
                    y: 200,
                    width: 300,
                    height: 20,
                },
                BTreeMap::new(),
            ),
            render_node(
                "aside#0",
                "aside",
                "Old banner",
                RenderRect {
                    x: 0,
                    y: 0,
                    width: 375,
                    height: 40,
                },
                BTreeMap::new(),
            ),
        ];
        let after = vec![
            render_node(
                "h1#0",
                "h1",
                "Updated Catalog",
                RenderRect {
                    x: 16,
                    y: 16,
                    width: 240,
                    height: 32,
                },
                BTreeMap::from([("color".into(), "rgb(0, 0, 0)".into())]),
            ),
            render_node(
                "button#0",
                "button",
                "Save",
                RenderRect {
                    x: 16,
                    y: 144,
                    width: 80,
                    height: 36,
                },
                BTreeMap::new(),
            ),
            render_node(
                "p#0",
                "p",
                "Stable copy",
                // 1px subpixel jitter must be ignored.
                RenderRect {
                    x: 16,
                    y: 201,
                    width: 300,
                    height: 20,
                },
                BTreeMap::new(),
            ),
            render_node(
                "section#0",
                "section",
                "New promo",
                RenderRect {
                    x: 16,
                    y: 240,
                    width: 300,
                    height: 48,
                },
                BTreeMap::new(),
            ),
        ];
        let diff = diff_render_trees(&before, &after, true);
        assert_eq!(diff.text_changed, 1);
        assert_eq!(diff.layout_moved, 1);
        assert_eq!(diff.added, 1);
        assert_eq!(diff.removed, 1);
        assert_eq!(diff.style_changed, 0);
        assert!(!diff.visual_only);
        assert!(!diff.changes.iter().any(|c| c.id == "p#0"));

        // Identical render trees with differing PNG hashes flag visual_only=true.
        let visual_only = diff_render_trees(&before, &before, true);
        assert!(visual_only.visual_only);
        assert!(visual_only.changes.is_empty());
    }

    #[test]
    fn render_node_snapshot_supports_legacy_json_and_box_model_round_trip() {
        let legacy = r#"{"id":"h1#0","tag":"h1","text":"Title","rect":{"x":0,"y":0,"width":100,"height":24},"style":{}}"#;
        let parsed: RenderNodeSnapshot =
            serde_json::from_str(legacy).expect("legacy snapshot parses");
        assert_eq!(parsed.parent_id, None);
        assert_eq!(parsed.box_model, None);
        assert!(parsed.attributes.is_empty());

        let enriched = RenderNodeSnapshot {
            parent_id: Some("main#0".into()),
            box_model: Some(BoxModel {
                margin: BoxSpacing {
                    top: 0,
                    right: 0,
                    bottom: 16,
                    left: 0,
                },
                padding: BoxSpacing {
                    top: 8,
                    right: 12,
                    bottom: 8,
                    left: 12,
                },
                border: BoxSpacing::default(),
                border_radius: Some("8px".into()),
                gap: Some("12px".into()),
            }),
            attributes: BTreeMap::from([("data-testid".into(), "hero-title".into())]),
            ..parsed
        };
        let encoded = serde_json::to_string(&enriched).expect("serialize enriched node");
        assert!(encoded.contains("\"boxModel\""));
        assert!(encoded.contains("\"parentId\":\"main#0\""));
        assert!(!encoded.contains("\"border\""));
        let decoded: RenderNodeSnapshot =
            serde_json::from_str(&encoded).expect("deserialize enriched node");
        assert_eq!(decoded, enriched);
    }

    #[test]
    fn accesskit_elements_convert_to_render_tree_and_diff_cleanly() {
        let before_els = vec![
            serde_json::json!({
                "path": "[0, 1]",
                "role": "CheckBox",
                "label": "Primary alerts",
                "checked": false,
                "visible": true,
                "bounds": {"x": 16.0, "y": 48.0, "width": 160.0, "height": 24.0}
            }),
            serde_json::json!({
                "path": "[0, 2]",
                "role": "Button",
                "label": "Primary density: compact",
                "checked": null,
                "visible": true,
                "bounds": {"x": 16.0, "y": 84.0, "width": 180.0, "height": 32.0}
            }),
            serde_json::json!({
                "path": "[0, 3]",
                "role": "Label",
                "label": "Hidden",
                "visible": false,
                "bounds": {"x": 0.0, "y": 0.0, "width": 50.0, "height": 20.0}
            }),
        ];
        let after_els = vec![
            serde_json::json!({
                "path": "[0, 1]",
                "role": "CheckBox",
                "label": "Primary alerts",
                "checked": true,
                "visible": true,
                "bounds": {"x": 16.0, "y": 48.0, "width": 160.0, "height": 24.0}
            }),
            serde_json::json!({
                "path": "[0, 2]",
                "role": "Button",
                "label": "Primary density: expanded",
                "checked": null,
                "visible": true,
                "bounds": {"x": 16.0, "y": 84.0, "width": 200.0, "height": 32.0}
            }),
        ];

        let before = accesskit_elements_to_render_tree(&before_els);
        let after = accesskit_elements_to_render_tree(&after_els);
        assert_eq!(before.len(), 2);
        assert_eq!(before[0].tag, "checkbox");
        assert_eq!(before[1].tag, "button");
        assert_eq!(
            before[0].attributes.get("checked").map(String::as_str),
            Some("false")
        );

        let diff = diff_render_trees(&before, &after, true);
        assert_eq!(diff.text_changed, 1);
        assert_eq!(diff.changes[0].id, "button#0");
        assert_eq!(
            diff.changes[0].before_text.as_deref(),
            Some("Primary density: compact")
        );
        assert_eq!(
            diff.changes[0].after_text.as_deref(),
            Some("Primary density: expanded")
        );
    }
}
