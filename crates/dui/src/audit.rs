//! Coverage audit shell (`dui audit` / MCP `audit` / viewer `/api/audit`).

use crate::{Reply, archive, failure, replay::read, sqlite_store::SqliteStore};
use dui_core::{
    audit::{AuditCellInput, AuditCommentInput, AuditInput, DEFAULT_GOAL_PERCENT, evaluate},
    model::{CommentStatus, ProjectId},
};
use serde_json::{Value, json};
use std::path::Path;

pub fn audit_run(run_dir: &Path, goal_percent: Option<f64>) -> Reply {
    if !run_dir.is_dir() {
        return Err(failure(2, "run directory does not exist"));
    }
    let report = read(&run_dir.join("report.json"))?;
    let snapshot = read(&run_dir.join("snapshot.json")).ok();
    let default_path = report["path"].as_str().unwrap_or("/").to_owned();

    let mut cells = Vec::new();
    if let Some(arr) = report["cells"].as_array() {
        for item in arr {
            let path = item["path"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(&default_path)
                .to_owned();
            let width = item["width"].as_u64().unwrap_or(0) as u32;
            let status = item["status"].as_str().unwrap_or("unverified").to_owned();
            let reason = item["reason"].as_str().map(str::to_owned);
            let state = item["state"].as_str().map(str::to_owned);
            let locale = item["locale"].as_str().map(str::to_owned);
            cells.push(AuditCellInput {
                path,
                state,
                width,
                status,
                reason,
                locale,
            });
        }
    }

    let branch_outcomes = report["coverage"]["branchOutcomes"].as_u64().unwrap_or(0);
    let observed_branch_outcomes = report["coverage"]["observedBranchOutcomes"]
        .as_u64()
        .unwrap_or(0);
    let runtime_only_branches = report["runtimeOnlyInstrumentedBranches"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut comments = Vec::new();
    if let Some(pid_str) = snapshot.as_ref().and_then(|s| s["projectId"].as_str())
        && let Ok(pid) = ProjectId::parse(pid_str)
        && let Ok(home) = archive::home_dir()
        && let Ok((mut store, _)) = SqliteStore::open(&home)
    {
        let _ = store.import_run_dir(run_dir);
        if let Ok(found) = store.list_comments_by_project(&pid) {
            for c in found {
                let loc = store.describe_comment_target(c.target());
                let path = loc["path"].as_str().map(str::to_owned);
                let target_label = if let Some(to) = loc["to"].as_str() {
                    format!("{} -> {to}", path.as_deref().unwrap_or("?"))
                } else if let Some(w) = loc["width"].as_u64() {
                    format!("{w} px")
                } else {
                    path.clone().unwrap_or_else(|| "target".into())
                };
                comments.push(AuditCommentInput {
                    id: c.id().as_str().to_owned(),
                    path,
                    target_label,
                    author: c.author().to_owned(),
                    body: c.body().to_owned(),
                    status: match c.status() {
                        CommentStatus::Open => "open".into(),
                        CommentStatus::Resolved => "resolved".into(),
                    },
                });
            }
        }
    }

    let input = AuditInput {
        goal_percent: goal_percent.unwrap_or(DEFAULT_GOAL_PERCENT),
        cells,
        branch_outcomes,
        observed_branch_outcomes,
        runtime_only_branches,
        comments,
    };
    let audit = evaluate(&input);
    let mut out = serde_json::to_value(audit).map_err(|e| failure(3, &e.to_string()))?;
    if let Some(obj) = out.as_object_mut() {
        obj.insert("run".into(), json!(run_dir.display().to_string()));
        if let Some(id) = snapshot.as_ref().and_then(|s| s["id"].as_str()) {
            obj.insert("runId".into(), json!(id));
        }
        if let Some(pid) = snapshot.as_ref().and_then(|s| s["projectId"].as_str()) {
            obj.insert("projectId".into(), json!(pid));
        }
    }
    Ok(out)
}

pub fn command(args: &Value) -> Reply {
    let run = args["run"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| failure(2, "missing run directory"))?;
    let goal = args["goal"].as_f64();
    audit_run(Path::new(run), goal)
}
