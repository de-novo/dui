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
    if matches!(
        node.tag.as_str(),
        "button" | "a" | "input" | "select" | "textarea" | "checkbox" | "switch" | "radio" | "tab"
    ) {
        return true;
    }
    if matches!(
        node.role.as_deref(),
        Some("button" | "link" | "textbox" | "checkbox" | "switch" | "tab" | "menuitem")
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
    )
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

fn parse_css_rgba(raw: &str) -> Option<(f64, f64, f64, f64)> {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix("rgba(")
        .or_else(|| trimmed.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<&str> = inner
        .split([',', '/'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() < 3 {
        return None;
    }
    let r = parts[0].parse::<f64>().ok()?.clamp(0.0, 255.0);
    let g = parts[1].parse::<f64>().ok()?.clamp(0.0, 255.0);
    let b = parts[2].parse::<f64>().ok()?.clamp(0.0, 255.0);
    let a = if parts.len() >= 4 {
        parts[3].parse::<f64>().ok()?.clamp(0.0, 1.0)
    } else {
        1.0
    };
    Some((r, g, b, a))
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
        if !is_tui_row && vp_w > 0 && (r.x < -2 || r.x + r.width > vp_w + 2) {
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
            let is_native_check =
                node.tag == "input" && matches!(input_type.as_deref(), Some("checkbox" | "radio"));
            // Inline text links inside prose and native 13-16px checkboxes paired with labels are excluded.
            let enforce_target = !is_hidden_input
                && !is_native_check
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
            groups
                .entry((node.parent_id.as_deref(), node.tag.as_str()))
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
}
