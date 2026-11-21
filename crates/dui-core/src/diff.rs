use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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

/// Compute per-pixel RGBA difference metrics and a visual highlight overlay buffer.
/// Unchanged pixels are muted grayscale; changed pixels are marked in high-contrast orange (`#EF803F`).
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
                overlay[dst] = 239;
                overlay[dst + 1] = 128;
                overlay[dst + 2] = 63;
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
        // Pixel (1,0) highlighted in #EF803F.
        assert_eq!(&overlay[4..8], &[239, 128, 63, 255]);
    }
}
