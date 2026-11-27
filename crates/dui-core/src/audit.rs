//! Pure deterministic UI state coverage audit against a target goal (default 99%).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_GOAL_PERCENT: f64 = 99.0;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditCellInput {
    pub path: String,
    #[serde(default)]
    pub state: Option<String>,
    pub width: u32,
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditCommentInput {
    pub id: String,
    #[serde(default)]
    pub path: Option<String>,
    pub target_label: String,
    pub author: String,
    pub body: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditInput {
    #[serde(default = "default_goal")]
    pub goal_percent: f64,
    #[serde(default)]
    pub cells: Vec<AuditCellInput>,
    #[serde(default)]
    pub branch_outcomes: u64,
    #[serde(default)]
    pub observed_branch_outcomes: u64,
    #[serde(default)]
    pub runtime_only_branches: Vec<String>,
    #[serde(default)]
    pub comments: Vec<AuditCommentInput>,
}

fn default_goal() -> f64 {
    DEFAULT_GOAL_PERCENT
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditGap {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    pub width: u32,
    pub status: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditSummary {
    pub total_cells: usize,
    pub planned_cells: usize,
    pub captured_cells: usize,
    pub captured_renderable_cells: usize,
    pub not_captured_cells: usize,
    pub verified_cells: usize,
    pub renderable_cells: usize,
    pub unverified_cells: usize,
    pub unrenderable_cells: usize,
    pub verified_percent: f64,
    pub renderable_percent: f64,
    pub branch_outcomes: u64,
    pub observed_branch_outcomes: u64,
    pub missing_branch_outcomes: u64,
    pub branch_percent: f64,
    pub open_comments: usize,
    pub resolved_comments: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenAudit {
    pub path: String,
    pub total_cells: usize,
    pub captured_cells: usize,
    pub captured_renderable_cells: usize,
    pub not_captured_cells: usize,
    pub verified_cells: usize,
    pub renderable_cells: usize,
    pub unverified_cells: usize,
    pub unrenderable_cells: usize,
    pub verified_percent: f64,
    pub open_comments: usize,
    pub unresolved_comments: Vec<AuditCommentInput>,
    pub gaps: Vec<AuditGap>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditReport {
    pub goal_percent: f64,
    pub meets_goal: bool,
    pub summary: AuditSummary,
    pub runtime_only_branches: Vec<String>,
    pub gaps: Vec<AuditGap>,
    pub not_captured: Vec<AuditGap>,
    pub screens: Vec<ScreenAudit>,
}

fn pct(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        return 0.0;
    }
    ((numerator as f64 * 10_000.0) / denominator as f64).round() / 100.0
}

pub fn evaluate(input: &AuditInput) -> AuditReport {
    let goal = input.goal_percent.clamp(0.0, 100.0);

    let mut total_cells = 0usize;
    let mut captured_cells = 0usize;
    let mut captured_renderable_cells = 0usize;
    let mut not_captured_cells = 0usize;
    let mut verified_cells = 0usize;
    let mut unverified_cells = 0usize;
    let mut unrenderable_cells = 0usize;
    let mut gaps = Vec::new();
    let mut not_captured = Vec::new();
    let mut by_screen: BTreeMap<String, ScreenAudit> = BTreeMap::new();

    let mut sorted_cells = input.cells.clone();
    sorted_cells.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.width.cmp(&b.width))
            .then_with(|| a.state.cmp(&b.state))
            .then_with(|| a.locale.cmp(&b.locale))
    });

    for cell in sorted_cells {
        total_cells += 1;
        let screen = by_screen
            .entry(cell.path.clone())
            .or_insert_with(|| ScreenAudit {
                path: cell.path.clone(),
                total_cells: 0,
                captured_cells: 0,
                captured_renderable_cells: 0,
                not_captured_cells: 0,
                verified_cells: 0,
                renderable_cells: 0,
                unverified_cells: 0,
                unrenderable_cells: 0,
                verified_percent: 0.0,
                open_comments: 0,
                unresolved_comments: Vec::new(),
                gaps: Vec::new(),
            });
        screen.total_cells += 1;

        match cell.status.as_str() {
            "verified" => {
                verified_cells += 1;
                captured_cells += 1;
                captured_renderable_cells += 1;
                screen.verified_cells += 1;
                screen.captured_cells += 1;
                screen.captured_renderable_cells += 1;
                screen.renderable_cells += 1;
            }
            "unrenderable" => {
                unrenderable_cells += 1;
                screen.unrenderable_cells += 1;
                let gap = AuditGap {
                    path: cell.path,
                    state: cell.state,
                    width: cell.width,
                    status: "unrenderable".into(),
                    reason: cell
                        .reason
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| "unrenderable".into()),
                    locale: cell.locale,
                };
                screen.gaps.push(gap.clone());
                gaps.push(gap);
            }
            _ => {
                let reason = cell
                    .reason
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| "missing-verification-evidence".into());
                let gap = AuditGap {
                    path: cell.path,
                    state: cell.state,
                    width: cell.width,
                    status: if crate::model::capture::is_uncaptured_reason(&reason) {
                        "not-captured".into()
                    } else {
                        "unverified".into()
                    },
                    reason,
                    locale: cell.locale,
                };
                screen.renderable_cells += 1;
                if gap.status == "not-captured" {
                    not_captured_cells += 1;
                    screen.not_captured_cells += 1;
                    not_captured.push(gap);
                } else {
                    unverified_cells += 1;
                    captured_cells += 1;
                    captured_renderable_cells += 1;
                    screen.unverified_cells += 1;
                    screen.captured_cells += 1;
                    screen.captured_renderable_cells += 1;
                    screen.gaps.push(gap.clone());
                    gaps.push(gap);
                }
            }
        }
    }

    let mut open_comments = 0usize;
    let mut resolved_comments = 0usize;
    let mut sorted_comments = input.comments.clone();
    sorted_comments.sort_by(|a, b| a.id.cmp(&b.id));
    for comment in sorted_comments {
        if comment.status == "open" {
            open_comments += 1;
            if let Some(ref p) = comment.path
                && let Some(screen) = by_screen.get_mut(p)
            {
                screen.open_comments += 1;
                screen.unresolved_comments.push(comment);
            }
        } else {
            resolved_comments += 1;
        }
    }

    let renderable_cells = total_cells.saturating_sub(unrenderable_cells);
    let verified_percent = pct(verified_cells as u64, captured_renderable_cells as u64);
    let renderable_percent = pct(renderable_cells as u64, total_cells as u64);
    let observed_branches = input.observed_branch_outcomes.min(input.branch_outcomes);
    let missing_branch_outcomes = input.branch_outcomes.saturating_sub(observed_branches);
    let branch_percent = if input.branch_outcomes == 0 {
        100.0
    } else {
        pct(observed_branches, input.branch_outcomes)
    };

    let meets_goal = captured_renderable_cells > 0
        && unrenderable_cells == 0
        && verified_percent >= goal
        && (input.branch_outcomes == 0 || branch_percent >= goal);

    let mut screens: Vec<ScreenAudit> = by_screen.into_values().collect();
    for s in &mut screens {
        s.verified_percent = pct(s.verified_cells as u64, s.captured_renderable_cells as u64);
    }

    let mut runtime_only_branches = input.runtime_only_branches.clone();
    runtime_only_branches.sort();
    runtime_only_branches.dedup();

    AuditReport {
        goal_percent: goal,
        meets_goal,
        summary: AuditSummary {
            total_cells,
            planned_cells: total_cells,
            captured_cells,
            captured_renderable_cells,
            not_captured_cells,
            verified_cells,
            renderable_cells,
            unverified_cells,
            unrenderable_cells,
            verified_percent,
            renderable_percent,
            branch_outcomes: input.branch_outcomes,
            observed_branch_outcomes: observed_branches,
            missing_branch_outcomes,
            branch_percent,
            open_comments,
            resolved_comments,
        },
        runtime_only_branches,
        gaps,
        not_captured,
        screens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(path: &str, width: u32, status: &str, reason: Option<&str>) -> AuditCellInput {
        AuditCellInput {
            path: path.into(),
            state: Some("default".into()),
            width,
            status: status.into(),
            reason: reason.map(str::to_owned),
            locale: None,
        }
    }

    #[test]
    fn meets_99_percent_goal_only_when_verified_and_branch_coverage_pass() {
        let input = AuditInput {
            goal_percent: 99.0,
            cells: vec![
                cell("/", 375, "verified", None),
                cell("/", 1280, "verified", None),
            ],
            branch_outcomes: 100,
            observed_branch_outcomes: 99,
            runtime_only_branches: Vec::new(),
            comments: Vec::new(),
        };
        let report = evaluate(&input);
        assert!(report.meets_goal);
        assert_eq!(report.summary.verified_percent, 100.0);
        assert_eq!(report.summary.branch_percent, 99.0);
        assert!(report.gaps.is_empty());
    }

    #[test]
    fn reports_unverified_unrenderable_gaps_and_attaches_only_open_comments() {
        let input = AuditInput {
            goal_percent: 99.0,
            cells: vec![
                cell("/order", 375, "verified", None),
                cell("/order", 1280, "unverified", Some("missing-assertion")),
                cell("/checkout", 375, "unrenderable", Some("no-render-env")),
            ],
            branch_outcomes: 10,
            observed_branch_outcomes: 8,
            runtime_only_branches: vec!["err-branch".into()],
            comments: vec![
                AuditCommentInput {
                    id: "c1".into(),
                    path: Some("/order".into()),
                    target_label: "/order".into(),
                    author: "alice".into(),
                    body: "Check mobile spacing".into(),
                    status: "open".into(),
                },
                AuditCommentInput {
                    id: "c2".into(),
                    path: Some("/order".into()),
                    target_label: "/order".into(),
                    author: "bob".into(),
                    body: "Already fixed".into(),
                    status: "resolved".into(),
                },
            ],
        };
        let report = evaluate(&input);
        assert!(!report.meets_goal);
        assert_eq!(report.summary.total_cells, 3);
        assert_eq!(report.summary.verified_cells, 1);
        assert_eq!(report.summary.renderable_cells, 2);
        assert_eq!(report.summary.unverified_cells, 1);
        assert_eq!(report.summary.unrenderable_cells, 1);
        assert_eq!(report.summary.missing_branch_outcomes, 2);
        assert_eq!(report.summary.open_comments, 1);
        assert_eq!(report.summary.resolved_comments, 1);
        assert_eq!(report.gaps.len(), 2);

        let order = report
            .screens
            .iter()
            .find(|s| s.path == "/order")
            .expect("order screen");
        assert_eq!(order.open_comments, 1);
        assert_eq!(order.unresolved_comments.len(), 1);
        assert_eq!(order.unresolved_comments[0].body, "Check mobile spacing");
    }
    #[test]
    fn not_captured_cells_are_separate_from_unverified_after_capture() {
        let input = AuditInput {
            goal_percent: 99.0,
            cells: vec![
                cell("/", 375, "verified", None),
                cell("/", 1280, "unverified", Some("assert-failed")),
                cell("/", 768, "unverified", Some("not-rendered")),
            ],
            branch_outcomes: 0,
            observed_branch_outcomes: 0,
            runtime_only_branches: Vec::new(),
            comments: Vec::new(),
        };
        let report = evaluate(&input);
        assert_eq!(report.summary.planned_cells, 3);
        assert_eq!(report.summary.captured_cells, 2);
        assert_eq!(report.summary.captured_renderable_cells, 2);
        assert_eq!(report.summary.not_captured_cells, 1);
        assert_eq!(report.summary.renderable_cells, 3);
        assert_eq!(report.summary.unverified_cells, 1);
        assert_eq!(report.summary.verified_percent, 50.0);
        assert_eq!(report.gaps.len(), 1);
        assert_eq!(report.not_captured.len(), 1);
        assert_eq!(report.not_captured[0].status, "not-captured");
        assert_eq!(report.not_captured[0].reason, "not-rendered");
    }
}
