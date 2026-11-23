//! Pure deterministic design-defect & accessibility lint engine over platform-neutral
//! `RenderNodeSnapshot` trees (works identically across Web, Desktop, Mobile/Android, and TUI).

use crate::diff::{RenderNodeSnapshot, RenderRect};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesignDefectKind {
    HorizontalOverflow,
    ElementOverlap,
    SmallTouchTarget,
    UnlabeledInteractive,
    LowContrast,
    SpacingDrift,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesignSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignFinding {
    pub kind: DesignDefectKind,
    pub severity: DesignSeverity,
    pub node_id: String,
    pub tag: String,
    pub rect: RenderRect,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_node_id: Option<String>,
    pub summary: String,
    pub measured: String,
    pub expected: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignAuditSummary {
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub findings: Vec<DesignFinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedDesignFinding {
    pub path: String,
    pub width: u32,
    pub state: Option<String>,
    pub scenario: Option<String>,
    pub finding: DesignFinding,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupedDesignFinding {
    pub kind: DesignDefectKind,
    pub severity: DesignSeverity,
    pub severity_rank: u8,
    pub path: String,
    pub width: u32,
    pub node_id: String,
    pub tag: String,
    pub rect: RenderRect,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_node_id: Option<String>,
    pub summary: String,
    pub measured: String,
    pub expected: String,
    pub occurrences: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupedDesignAudit {
    pub total: usize,
    pub raw_total: usize,
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub findings: Vec<GroupedDesignFinding>,
}

/// Deduplicate identical design findings across multiple states/scenarios of the same
/// `(path, width, kind, node_id)` so repeated defects are reported once with an exact
/// `occurrences` count instead of spamming reviewers and AI agents.
pub fn deduplicate_findings(items: Vec<ScopedDesignFinding>) -> GroupedDesignAudit {
    let raw_total = items.len();
    let mut grouped: BTreeMap<(DesignDefectKind, String, u32, String), GroupedDesignFinding> =
        BTreeMap::new();
    for item in items {
        let key = (
            item.finding.kind,
            item.path.clone(),
            item.width,
            item.finding.node_id.clone(),
        );
        let entry = grouped.entry(key).or_insert_with(|| {
            let severity_rank = match item.finding.severity {
                DesignSeverity::Error => 0,
                DesignSeverity::Warning => 1,
                DesignSeverity::Info => 2,
            };
            GroupedDesignFinding {
                kind: item.finding.kind,
                severity: item.finding.severity,
                severity_rank,
                path: item.path.clone(),
                width: item.width,
                node_id: item.finding.node_id,
                tag: item.finding.tag,
                rect: item.finding.rect,
                related_node_id: item.finding.related_node_id,
                summary: item.finding.summary,
                measured: item.finding.measured,
                expected: item.finding.expected,
                occurrences: 0,
                states: Vec::new(),
                scenario: item.scenario.clone(),
            }
        });
        entry.occurrences += 1;
        if let Some(st) = item.state
            && entry.states.len() < 8
            && !entry.states.contains(&st)
        {
            entry.states.push(st);
        }
    }
    let mut findings: Vec<GroupedDesignFinding> = grouped.into_values().collect();
    findings.sort_by(|a, b| {
        a.severity_rank
            .cmp(&b.severity_rank)
            .then(b.occurrences.cmp(&a.occurrences))
            .then(a.path.cmp(&b.path))
            .then(a.width.cmp(&b.width))
            .then(a.node_id.cmp(&b.node_id))
    });
    let errors = findings
        .iter()
        .filter(|f| f.severity == DesignSeverity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == DesignSeverity::Warning)
        .count();
    let info = findings
        .iter()
        .filter(|f| f.severity == DesignSeverity::Info)
        .count();
    GroupedDesignAudit {
        total: findings.len(),
        raw_total,
        errors,
        warnings,
        info,
        findings,
    }
}

fn is_interactive(node: &RenderNodeSnapshot) -> bool {
    if node
        .attributes
        .get("aria-hidden")
        .is_some_and(|v| v == "true")
        || matches!(node.role.as_deref(), Some("presentation" | "none"))
    {
        return false;
    }
    if matches!(
        node.tag.as_str(),
        "button" | "a" | "input" | "select" | "textarea" | "checkbox" | "switch" | "radio" | "tab"
    ) {
        return true;
    }
    if matches!(
        node.role.as_deref(),
        Some(
            "button" | "link" | "textbox" | "checkbox" | "switch" | "tab" | "menuitem" | "combobox"
        )
    ) {
        return true;
    }
    node.attributes
        .get("clickable")
        .is_some_and(|v| v == "true")
}

fn is_container_tag(tag: &str) -> bool {
    matches!(
        tag,
        "main"
            | "section"
            | "article"
            | "aside"
            | "nav"
            | "header"
            | "footer"
            | "form"
            | "dialog"
            | "list"
            | "scroll"
            | "view"
            | "compose-view"
            | "div"
            | "ul"
            | "ol"
            | "li"
            | "table"
            | "thead"
            | "tbody"
            | "tfoot"
            | "tr"
            | "td"
            | "th"
            | "fieldset"
            | "figure"
            | "dl"
            | "dt"
            | "dd"
            | "details"
    )
}

fn is_horizontally_clipped(node: &RenderNodeSnapshot) -> bool {
    node.attributes
        .get("data-dui-clipped-x")
        .is_some_and(|v| v == "true")
}

fn is_disabled(node: &RenderNodeSnapshot) -> bool {
    node.attributes.contains_key("disabled")
        || node
            .attributes
            .get("aria-disabled")
            .is_some_and(|v| v == "true")
}

fn is_ancestor(parent_map: &BTreeMap<&str, &str>, ancestor_id: &str, node_id: &str) -> bool {
    let mut cur = node_id;
    for _ in 0..64 {
        let Some(parent) = parent_map.get(cur) else {
            return false;
        };
        if *parent == ancestor_id {
            return true;
        }
        cur = parent;
    }
    false
}

fn has_accessible_label(node: &RenderNodeSnapshot) -> bool {
    if !node.text.trim().is_empty() {
        return true;
    }
    for key in [
        "aria-label",
        "ariaLabel",
        "aria-labelledby",
        "title",
        "contentDesc",
        "placeholder",
        "alt",
    ] {
        if node
            .attributes
            .get(key)
            .is_some_and(|v| !v.trim().is_empty())
        {
            return true;
        }
    }
    false
}

fn rect_contains(outer: RenderRect, inner: RenderRect) -> bool {
    inner.x >= outer.x - 2
        && inner.y >= outer.y - 2
        && inner.x + inner.width <= outer.x + outer.width + 2
        && inner.y + inner.height <= outer.y + outer.height + 2
}

fn overlap_intersection(a: RenderRect, b: RenderRect) -> (i32, i32) {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y2 = (a.y + a.height).min(b.y + b.height);
    ((x2 - x1).max(0), (y2 - y1).max(0))
}

fn parse_alpha(raw: &str) -> Option<f64> {
    let s = raw.trim();
    if let Some(pct) = s.strip_suffix('%') {
        Some((pct.trim().parse::<f64>().ok()? / 100.0).clamp(0.0, 1.0))
    } else {
        Some(s.parse::<f64>().ok()?.clamp(0.0, 1.0))
    }
}

fn parse_unit_or_pct(raw: &str, pct_scale: f64) -> Option<f64> {
    let s = raw.trim();
    if let Some(pct) = s.strip_suffix('%') {
        Some(pct.trim().parse::<f64>().ok()? / 100.0 * pct_scale)
    } else {
        s.parse::<f64>().ok()
    }
}

fn parse_angle_rad(raw: &str) -> Option<f64> {
    let s = raw.trim();
    if let Some(v) = s.strip_suffix("deg") {
        Some(v.trim().parse::<f64>().ok()?.to_radians())
    } else if let Some(v) = s.strip_suffix("rad") {
        v.trim().parse::<f64>().ok()
    } else if let Some(v) = s.strip_suffix("turn") {
        Some(v.trim().parse::<f64>().ok()? * std::f64::consts::TAU)
    } else if let Some(v) = s.strip_suffix("grad") {
        Some(v.trim().parse::<f64>().ok()? * std::f64::consts::PI / 200.0)
    } else if s.eq_ignore_ascii_case("none") {
        Some(0.0)
    } else {
        Some(s.parse::<f64>().ok()?.to_radians())
    }
}

fn linear_to_srgb_255(c: f64) -> f64 {
    let clamped = c.clamp(0.0, 1.0);
    let gamma = if clamped <= 0.003_130_8 {
        12.92 * clamped
    } else {
        1.055 * clamped.powf(1.0 / 2.4) - 0.055
    };
    (gamma * 255.0).clamp(0.0, 255.0)
}

fn oklab_to_srgb(l: f64, a: f64, b: f64) -> (f64, f64, f64) {
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548_0 * b;

    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;

    let r_lin = 4.076_741_662_1 * l3 - 3.307_711_591_3 * m3 + 0.230_969_929_2 * s3;
    let g_lin = -1.268_438_004_6 * l3 + 2.609_757_401_1 * m3 - 0.341_319_396_5 * s3;
    let b_lin = -0.004_196_086_3 * l3 - 0.703_418_614_7 * m3 + 1.707_614_701_0 * s3;

    (
        linear_to_srgb_255(r_lin),
        linear_to_srgb_255(g_lin),
        linear_to_srgb_255(b_lin),
    )
}

fn parse_css_rgba(raw: &str) -> Option<(f64, f64, f64, f64)> {
    let trimmed = raw.trim();
    if let Some(inner) = trimmed
        .strip_prefix("rgba(")
        .or_else(|| trimmed.strip_prefix("rgb("))
        .and_then(|s| s.strip_suffix(')'))
    {
        let (rgb_part, alpha_part) = match inner.split_once('/') {
            Some((l, r)) => (l, Some(r)),
            None => (inner, None),
        };
        let parts: Vec<&str> = rgb_part
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        let r = parse_unit_or_pct(parts[0], 255.0)?.clamp(0.0, 255.0);
        let g = parse_unit_or_pct(parts[1], 255.0)?.clamp(0.0, 255.0);
        let b = parse_unit_or_pct(parts[2], 255.0)?.clamp(0.0, 255.0);
        let a = if let Some(ap) = alpha_part {
            parse_alpha(ap)?
        } else if parts.len() >= 4 {
            parse_alpha(parts[3])?
        } else {
            1.0
        };
        return Some((r, g, b, a));
    }

    if let Some(inner) = trimmed
        .strip_prefix("oklab(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let (coords, alpha_part) = match inner.split_once('/') {
            Some((l, r)) => (l, Some(r)),
            None => (inner, None),
        };
        let parts: Vec<&str> = coords.split_whitespace().collect();
        if parts.len() < 3 {
            return None;
        }
        let l = parse_unit_or_pct(parts[0], 1.0)?;
        let a = parse_unit_or_pct(parts[1], 0.4)?;
        let b = parse_unit_or_pct(parts[2], 0.4)?;
        let alpha = match alpha_part {
            Some(ap) => parse_alpha(ap)?,
            None => 1.0,
        };
        let (r, g, bl) = oklab_to_srgb(l, a, b);
        return Some((r, g, bl, alpha));
    }

    if let Some(inner) = trimmed
        .strip_prefix("oklch(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let (coords, alpha_part) = match inner.split_once('/') {
            Some((l, r)) => (l, Some(r)),
            None => (inner, None),
        };
        let parts: Vec<&str> = coords.split_whitespace().collect();
        if parts.len() < 3 {
            return None;
        }
        let l = parse_unit_or_pct(parts[0], 1.0)?;
        let c = parse_unit_or_pct(parts[1], 0.4)?;
        let h_rad = parse_angle_rad(parts[2])?;
        let alpha = match alpha_part {
            Some(ap) => parse_alpha(ap)?,
            None => 1.0,
        };
        let (r, g, bl) = oklab_to_srgb(l, c * h_rad.cos(), c * h_rad.sin());
        return Some((r, g, bl, alpha));
    }

    if let Some(inner) = trimmed
        .strip_prefix("color(srgb ")
        .and_then(|s| s.strip_suffix(')'))
    {
        let (coords, alpha_part) = match inner.split_once('/') {
            Some((l, r)) => (l, Some(r)),
            None => (inner, None),
        };
        let parts: Vec<&str> = coords.split_whitespace().collect();
        if parts.len() < 3 {
            return None;
        }
        let r = (parse_unit_or_pct(parts[0], 1.0)? * 255.0).clamp(0.0, 255.0);
        let g = (parse_unit_or_pct(parts[1], 1.0)? * 255.0).clamp(0.0, 255.0);
        let b = (parse_unit_or_pct(parts[2], 1.0)? * 255.0).clamp(0.0, 255.0);
        let alpha = match alpha_part {
            Some(ap) => parse_alpha(ap)?,
            None => 1.0,
        };
        return Some((r, g, b, alpha));
    }

    None
}

fn relative_luminance(r: f64, g: f64, b: f64) -> f64 {
    let channel = |v: f64| {
        let s = v / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

fn contrast_ratio(fg: (f64, f64, f64), bg: (f64, f64, f64)) -> f64 {
    let l1 = relative_luminance(fg.0, fg.1, fg.2);
    let l2 = relative_luminance(bg.0, bg.1, bg.2);
    let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

fn parse_px(raw: &str) -> Option<f64> {
    raw.trim().strip_suffix("px")?.trim().parse::<f64>().ok()
}

/// Lint a single rendered frame's `RenderNodeSnapshot` tree for visual, layout, and accessibility defects.
pub fn lint_render_tree(nodes: &[RenderNodeSnapshot], viewport_width: u32) -> DesignAuditSummary {
    let mut findings = Vec::new();
    let vp_w = viewport_width as i32;

    // 1. Per-node checks: horizontal overflow, small touch targets, unlabeled interactive controls, low contrast.
    for node in nodes {
        let r = node.rect;
        if r.width <= 0 || r.height <= 0 {
            continue;
        }
        // Skip terminal cell rows for web-pixel viewport overflow if viewport_width is column count (< 200)
        let is_tui_row = node.tag == "tui-row";
        if !is_tui_row
            && !is_horizontally_clipped(node)
            && vp_w > 0
            && (r.x < -2 || r.x + r.width > vp_w + 2)
        {
            findings.push(DesignFinding {
                kind: DesignDefectKind::HorizontalOverflow,
                severity: DesignSeverity::Error,
                node_id: node.id.clone(),
                tag: node.tag.clone(),
                rect: r,
                related_node_id: None,
                summary: format!(
                    "{} ({}) overflows {vp_w}px viewport (x={}..{})",
                    node.tag,
                    node.id,
                    r.x,
                    r.x + r.width
                ),
                measured: format!("{}..{}px", r.x, r.x + r.width),
                expected: format!("0..{vp_w}px"),
            });
        }

        if is_interactive(node) {
            let input_type = node.attributes.get("type").map(|s| s.to_ascii_lowercase());
            let is_hidden_input = node.tag == "input" && input_type.as_deref() == Some("hidden");
            let is_native_check = (node.tag == "input"
                && matches!(input_type.as_deref(), Some("checkbox" | "radio")))
                || matches!(node.role.as_deref(), Some("checkbox" | "radio"));
            let is_desktop_edge_rail =
                vp_w > 500 && ((r.height >= 400 && r.width >= 8) || (r.width >= 400 && r.height >= 8));
            // Inline text links inside prose, checkbox/radio indicators paired with labels,
            // and full-height desktop sidebar resize rails are excluded.
            let enforce_target = !is_hidden_input
                && !is_native_check
                && !is_desktop_edge_rail
                && (node.tag != "a" || node.style.get("display").is_some_and(|d| d != "inline"));
            let too_small = if vp_w > 0 && vp_w <= 500 {
                (r.width < 44 && r.height < 44) || r.width < 32 || r.height < 32
            } else {
                r.width < 20 || r.height < 20
            };
            let min_label = if vp_w > 0 && vp_w <= 500 {
                "44x44"
            } else {
                "20x20"
            };
            if enforce_target && too_small {
                findings.push(DesignFinding {
                    kind: DesignDefectKind::SmallTouchTarget,
                    severity: DesignSeverity::Warning,
                    node_id: node.id.clone(),
                    tag: node.tag.clone(),
                    rect: r,
                    related_node_id: None,
                    summary: format!(
                        "{} ({}) is {}x{}px (below {min_label}px target minimum)",
                        node.tag, node.id, r.width, r.height
                    ),
                    measured: format!("{}x{}px", r.width, r.height),
                    expected: format!(">={min_label}px"),
                });
            }

            if !is_hidden_input && !has_accessible_label(node) {
                findings.push(DesignFinding {
                    kind: DesignDefectKind::UnlabeledInteractive,
                    severity: DesignSeverity::Error,
                    node_id: node.id.clone(),
                    tag: node.tag.clone(),
                    rect: r,
                    related_node_id: None,
                    summary: format!(
                        "Interactive {} ({}) has no visible text or accessible label",
                        node.tag, node.id
                    ),
                    measured: "empty-label".into(),
                    expected: "non-empty text or aria-label".into(),
                });
            }
        }

        let has_readable_text = !node.text.trim().is_empty()
            && (!is_tui_row || node.text.chars().any(char::is_alphanumeric));
        if has_readable_text
            && !is_disabled(node)
            && let (Some(fg_raw), Some(bg_raw)) =
                (node.style.get("color"), node.style.get("backgroundColor"))
            && let (Some((fr, fg, fb, fa)), Some((br, bg, bb, ba))) =
                (parse_css_rgba(fg_raw), parse_css_rgba(bg_raw))
            && fa >= 0.85
            && ba >= 0.85
        {
            let ratio = contrast_ratio((fr, fg, fb), (br, bg, bb));
            let font_px = node
                .style
                .get("fontSize")
                .and_then(|s| parse_px(s))
                .unwrap_or(16.0);
            let bold = node
                .style
                .get("fontWeight")
                .and_then(|w| w.parse::<u32>().ok())
                .is_some_and(|w| w >= 700);
            let min_ratio = if font_px >= 24.0 || (bold && font_px >= 19.0) {
                3.0
            } else {
                4.5
            };
            if ratio < min_ratio {
                findings.push(DesignFinding {
                    kind: DesignDefectKind::LowContrast,
                    severity: DesignSeverity::Warning,
                    node_id: node.id.clone(),
                    tag: node.tag.clone(),
                    rect: r,
                    related_node_id: None,
                    summary: format!(
                        "{} ({}) text contrast {:.2}:1 is below WCAG AA {:.1}:1",
                        node.tag, node.id, ratio, min_ratio
                    ),
                    measured: format!("{ratio:.2}:1"),
                    expected: format!(">={min_ratio:.1}:1"),
                });
            }
        }
    }

    // 2. Pairwise unintended overlap between non-container leaf/interactive elements.
    let mut parent_map: BTreeMap<&str, &str> = BTreeMap::new();
    for node in nodes {
        if let Some(pid) = node.parent_id.as_deref() {
            parent_map.insert(node.id.as_str(), pid);
        }
    }
    let leaf_nodes: Vec<&RenderNodeSnapshot> = nodes
        .iter()
        .filter(|n| {
            n.rect.width > 0
                && n.rect.height > 0
                && !is_container_tag(&n.tag)
                && !matches!(n.tag.as_str(), "tui-row" | "svg" | "img" | "label")
        })
        .collect();
    for i in 0..leaf_nodes.len() {
        for j in (i + 1)..leaf_nodes.len() {
            let a = leaf_nodes[i];
            let b = leaf_nodes[j];
            if a.parent_id.as_deref() == Some(b.id.as_str())
                || b.parent_id.as_deref() == Some(a.id.as_str())
                || is_ancestor(&parent_map, a.id.as_str(), b.id.as_str())
                || is_ancestor(&parent_map, b.id.as_str(), a.id.as_str())
                || rect_contains(a.rect, b.rect)
                || rect_contains(b.rect, a.rect)
            {
                continue;
            }
            let (dx, dy) = overlap_intersection(a.rect, b.rect);
            let overlap_area = i64::from(dx) * i64::from(dy);
            let min_area = (i64::from(a.rect.width) * i64::from(a.rect.height))
                .min(i64::from(b.rect.width) * i64::from(b.rect.height))
                .max(1);
            if dx >= 8 && dy >= 8 && overlap_area * 4 >= min_area {
                findings.push(DesignFinding {
                    kind: DesignDefectKind::ElementOverlap,
                    severity: DesignSeverity::Error,
                    node_id: a.id.clone(),
                    tag: a.tag.clone(),
                    rect: a.rect,
                    related_node_id: Some(b.id.clone()),
                    summary: format!(
                        "{} ({}) overlaps {} ({}) by {}x{}px",
                        a.tag, a.id, b.tag, b.id, dx, dy
                    ),
                    measured: format!("{dx}x{dy}px overlap"),
                    expected: "0px overlap".into(),
                });
            }
        }
    }

    // 3. Peer spacing consistency check for vertical stacks of 4+ identically tagged peers.
    let mut groups: BTreeMap<(Option<&str>, &str), Vec<&RenderNodeSnapshot>> = BTreeMap::new();
    for node in nodes {
        if node.rect.width > 0
            && node.rect.height > 0
            && !is_container_tag(&node.tag)
            && node.tag != "tui-row"
        {
            let parent_group = node
                .attributes
                .get("data-dui-dom-parent")
                .map(String::as_str)
                .or(node.parent_id.as_deref());
            groups
                .entry((parent_group, node.tag.as_str()))
                .or_default()
                .push(node);
        }
    }
    for ((_, tag), mut peers) in groups {
        if peers.len() < 4 {
            continue;
        }
        peers.sort_by_key(|n| (n.rect.y, n.rect.x));
        // Only evaluate true vertical stacks with aligned left edge (within 4px)
        let first_x = peers[0].rect.x;
        if !peers.iter().all(|n| (n.rect.x - first_x).abs() <= 4) {
            continue;
        }
        let mut gaps: Vec<(i32, &RenderNodeSnapshot)> = Vec::new();
        for pair in peers.windows(2) {
            let gap = pair[1].rect.y - (pair[0].rect.y + pair[0].rect.height);
            if gap >= 0 {
                gaps.push((gap, pair[1]));
            }
        }
        if gaps.len() < 3 {
            continue;
        }
        let mut sorted_gaps: Vec<i32> = gaps.iter().map(|(g, _)| *g).collect();
        sorted_gaps.sort_unstable();
        let median = sorted_gaps[sorted_gaps.len() / 2];
        if median == 0 {
            continue;
        }
        for (gap, target) in gaps {
            if (gap - median).abs() >= 8 {
                findings.push(DesignFinding {
                    kind: DesignDefectKind::SpacingDrift,
                    severity: DesignSeverity::Info,
                    node_id: target.id.clone(),
                    tag: tag.to_owned(),
                    rect: target.rect,
                    related_node_id: None,
                    summary: format!(
                        "{tag} ({}) vertical spacing is {gap}px (peer median {median}px)",
                        target.id
                    ),
                    measured: format!("{gap}px"),
                    expected: format!("~{median}px"),
                });
            }
        }
    }

    let errors = findings
        .iter()
        .filter(|f| f.severity == DesignSeverity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == DesignSeverity::Warning)
        .count();
    let info = findings
        .iter()
        .filter(|f| f.severity == DesignSeverity::Info)
        .count();
    DesignAuditSummary {
        total: findings.len(),
        errors,
        warnings,
        info,
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(
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
            parent_id: Some("main#0".into()),
            box_model: None,
            attributes: BTreeMap::new(),
        }
    }

    #[test]
    fn clean_accessible_layout_produces_zero_findings() {
        let tree = vec![
            node(
                "h1#0",
                "h1",
                "Billing",
                RenderRect {
                    x: 16,
                    y: 16,
                    width: 320,
                    height: 36,
                },
                BTreeMap::from([
                    ("color".into(), "rgb(17, 24, 39)".into()),
                    ("backgroundColor".into(), "rgb(255, 255, 255)".into()),
                    ("fontSize".into(), "28px".into()),
                ]),
            ),
            node(
                "button#0",
                "button",
                "Save",
                RenderRect {
                    x: 16,
                    y: 72,
                    width: 120,
                    height: 44,
                },
                BTreeMap::from([
                    ("color".into(), "rgb(255, 255, 255)".into()),
                    ("backgroundColor".into(), "rgb(24, 24, 27)".into()),
                ]),
            ),
        ];
        let audit = lint_render_tree(&tree, 375);
        assert_eq!(
            audit.total, 0,
            "expected clean layout, got {:?}",
            audit.findings
        );
    }

    #[test]
    fn detects_all_six_design_and_accessibility_defects() {
        let tree = vec![
            // 1. Horizontal overflow (> 375px)
            node(
                "h1#0",
                "h1",
                "Wide Heading",
                RenderRect {
                    x: 16,
                    y: 16,
                    width: 420,
                    height: 32,
                },
                BTreeMap::new(),
            ),
            // 2. Small touch target + 3. Unlabeled interactive button
            node(
                "button#0",
                "button",
                "",
                RenderRect {
                    x: 16,
                    y: 80,
                    width: 20,
                    height: 20,
                },
                BTreeMap::new(),
            ),
            // 4. Overlapping peer button + 5. Low WCAG contrast
            node(
                "button#1",
                "button",
                "Low Contrast",
                RenderRect {
                    x: 24,
                    y: 84,
                    width: 120,
                    height: 48,
                },
                BTreeMap::from([
                    ("color".into(), "rgb(180, 180, 180)".into()),
                    ("backgroundColor".into(), "rgb(255, 255, 255)".into()),
                    ("fontSize".into(), "14px".into()),
                ]),
            ),
            // 6. Spacing drift across 3 aligned <p> items (gaps: 12px vs 30px)
            node(
                "p#0",
                "p",
                "Row 1",
                RenderRect {
                    x: 16,
                    y: 200,
                    width: 200,
                    height: 20,
                },
                BTreeMap::new(),
            ),
            node(
                "p#1",
                "p",
                "Row 2",
                RenderRect {
                    x: 16,
                    y: 232,
                    width: 200,
                    height: 20,
                },
                BTreeMap::new(),
            ),
            node(
                "p#2",
                "p",
                "Row 3",
                RenderRect {
                    x: 16,
                    y: 264,
                    width: 200,
                    height: 20,
                },
                BTreeMap::new(),
            ),
            node(
                "p#3",
                "p",
                "Row 4 Drifted",
                RenderRect {
                    x: 16,
                    y: 314,
                    width: 200,
                    height: 20,
                },
                BTreeMap::new(),
            ),
        ];

        let audit = lint_render_tree(&tree, 375);
        let has = |k: DesignDefectKind| audit.findings.iter().any(|f| f.kind == k);
        assert!(has(DesignDefectKind::HorizontalOverflow), "overflow");
        assert!(has(DesignDefectKind::SmallTouchTarget), "touch target");
        assert!(has(DesignDefectKind::UnlabeledInteractive), "unlabeled");
        assert!(has(DesignDefectKind::ElementOverlap), "overlap");
        assert!(has(DesignDefectKind::LowContrast), "low contrast");
        assert!(has(DesignDefectKind::SpacingDrift), "spacing drift");
        assert!(audit.errors >= 3);
        assert!(audit.warnings >= 2);
        assert!(audit.info >= 1);
    }

    #[test]
    fn deduplicates_identical_defects_across_states_with_occurrence_counts() {
        let finding = DesignFinding {
            kind: DesignDefectKind::SmallTouchTarget,
            severity: DesignSeverity::Warning,
            node_id: "button#0".into(),
            tag: "button".into(),
            rect: RenderRect {
                x: 16,
                y: 80,
                width: 20,
                height: 20,
            },
            related_node_id: None,
            summary: "button (button#0) is 20x20px".into(),
            measured: "20x20px".into(),
            expected: ">=44x44px".into(),
        };
        let grouped = deduplicate_findings(vec![
            ScopedDesignFinding {
                path: "/".into(),
                width: 375,
                state: Some("s0".into()),
                scenario: Some("default".into()),
                finding: finding.clone(),
            },
            ScopedDesignFinding {
                path: "/".into(),
                width: 375,
                state: Some("s1".into()),
                scenario: Some("default".into()),
                finding: finding.clone(),
            },
            ScopedDesignFinding {
                path: "/".into(),
                width: 375,
                state: Some("s2".into()),
                scenario: Some("default".into()),
                finding,
            },
        ]);
        assert_eq!(grouped.raw_total, 3);
        assert_eq!(grouped.total, 1);
        assert_eq!(grouped.warnings, 1);
        assert_eq!(grouped.findings[0].occurrences, 3);
        assert_eq!(grouped.findings[0].states, vec!["s0", "s1", "s2"]);
    }

    #[test]
    fn excludes_clipped_scroll_ancestors_disabled_contrast_and_cross_card_spacing() {
        let mut clipped = node(
            "td#0",
            "span",
            "Scrollable column",
            RenderRect {
                x: 340,
                y: 16,
                width: 220,
                height: 24,
            },
            BTreeMap::new(),
        );
        clipped
            .attributes
            .insert("data-dui-clipped-x".into(), "true".into());

        let mut disabled_btn = node(
            "button#disabled",
            "button",
            "Disabled",
            RenderRect {
                x: 16,
                y: 60,
                width: 120,
                height: 44,
            },
            BTreeMap::from([
                ("color".into(), "rgb(180, 180, 180)".into()),
                ("backgroundColor".into(), "rgb(255, 255, 255)".into()),
                ("fontSize".into(), "14px".into()),
            ]),
        );
        disabled_btn
            .attributes
            .insert("disabled".into(), "true".into());

        let mut hidden_icon = node(
            "button#hidden",
            "button",
            "",
            RenderRect {
                x: 150,
                y: 60,
                width: 16,
                height: 16,
            },
            BTreeMap::new(),
        );
        hidden_icon
            .attributes
            .insert("aria-hidden".into(), "true".into());

        // Grandparent -> intermediate div -> grandchild badge slightly overlapping grandparent edge
        let grandparent = node(
            "button#gp",
            "button",
            "Notifications",
            RenderRect {
                x: 16,
                y: 120,
                width: 120,
                height: 44,
            },
            BTreeMap::new(),
        );
        let mut mid = node(
            "div#mid",
            "div",
            "",
            RenderRect {
                x: 100,
                y: 112,
                width: 40,
                height: 30,
            },
            BTreeMap::new(),
        );
        mid.parent_id = Some("button#gp".into());
        let mut grandchild = node(
            "span#badge",
            "span",
            "3",
            RenderRect {
                x: 112,
                y: 112,
                width: 28,
                height: 28,
            },
            BTreeMap::new(),
        );
        grandchild.parent_id = Some("div#mid".into());

        // Two separate cards with <p> elements sharing captured parent_id "main#0" but distinct DOM parents
        let card_p = |id: &str, y: i32, dom_parent: &str| {
            let mut p = node(
                id,
                "p",
                "Card line",
                RenderRect {
                    x: 16,
                    y,
                    width: 200,
                    height: 20,
                },
                BTreeMap::new(),
            );
            p.attributes
                .insert("data-dui-dom-parent".into(), dom_parent.into());
            p
        };

        let tree = vec![
            clipped,
            disabled_btn,
            hidden_icon,
            grandparent,
            mid,
            grandchild,
            card_p("p#c1a", 200, "p1"),
            card_p("p#c1b", 224, "p1"),
            card_p("p#c2a", 360, "p2"),
            card_p("p#c2b", 384, "p2"),
        ];

        let audit = lint_render_tree(&tree, 375);
        assert_eq!(
            audit.total, 0,
            "expected zero false-positive findings, got {:?}",
            audit.findings
        );
    }

    #[test]
    fn evaluates_oklch_oklab_contrast_and_exempts_aria_checkbox_and_desktop_rail() {
        // Low-contrast oklch muted gray on dark slate background -> must flag LowContrast
        let low_oklch = node(
            "p#muted",
            "p",
            "Muted text",
            RenderRect {
                x: 16,
                y: 16,
                width: 200,
                height: 20,
            },
            BTreeMap::from([
                ("color".into(), "oklch(0.35 0.01 250)".into()),
                ("backgroundColor".into(), "oklch(0.2 0.02 250)".into()),
                ("fontSize".into(), "14px".into()),
            ]),
        );
        // High-contrast oklch white on dark slate -> must pass
        let high_oklch = node(
            "p#bright",
            "p",
            "Bright text",
            RenderRect {
                x: 16,
                y: 48,
                width: 200,
                height: 20,
            },
            BTreeMap::from([
                ("color".into(), "oklch(0.984 0.003 247.858)".into()),
                ("backgroundColor".into(), "oklab(0.18 -0.01 -0.03)".into()),
                ("fontSize".into(), "14px".into()),
            ]),
        );
        // WAI-ARIA role="checkbox" (16x16 Radix checkbox) with resolved aria-label -> must pass
        let mut aria_checkbox = node(
            "button#cb",
            "button",
            "",
            RenderRect {
                x: 16,
                y: 90,
                width: 16,
                height: 16,
            },
            BTreeMap::new(),
        );
        aria_checkbox.role = Some("checkbox".into());
        aria_checkbox
            .attributes
            .insert("aria-label".into(), "Recents".into());
        // Desktop full-height sidebar resize rail (16x800px) -> must pass SmallTouchTarget
        let mut sidebar_rail = node(
            "button#rail",
            "button",
            "",
            RenderRect {
                x: 240,
                y: 0,
                width: 16,
                height: 800,
            },
            BTreeMap::new(),
        );
        sidebar_rail
            .attributes
            .insert("aria-label".into(), "Toggle Sidebar".into());

        let audit = lint_render_tree(&[low_oklch, high_oklch, aria_checkbox, sidebar_rail], 1280);
        assert_eq!(
            audit.findings.len(),
            1,
            "expected only low_oklch LowContrast, got {:?}",
            audit.findings
        );
        assert_eq!(audit.findings[0].kind, DesignDefectKind::LowContrast);
        assert_eq!(audit.findings[0].node_id, "p#muted");
    }
}
