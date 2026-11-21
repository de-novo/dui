//! Review comments anchored to screens, states, widths (cells), and transitions (`v0.0.5`).

use crate::{Reply, archive, failure, replay::read, sqlite_store::SqliteStore};
use dui_core::{
    model::{
        CellId, Comment, CommentId, CommentStatus, CommentTarget, EdgeId, ProjectId, RunId, Screen,
        StateId,
    },
    storage::CommentRepository,
};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn open_store() -> Result<SqliteStore, (u8, Value)> {
    let home = archive::home_dir()?;
    let (store, _) = SqliteStore::open(&home).map_err(|e| failure(3, &e.to_string()))?;
    Ok(store)
}

fn serialize_comment(store: &SqliteStore, comment: &Comment) -> Value {
    let status = match comment.status() {
        CommentStatus::Open => "open",
        CommentStatus::Resolved => "resolved",
    };
    json!({
        "id": comment.id().as_str(),
        "projectId": comment.project_id().as_str(),
        "runId": comment.run_id().as_str(),
        "target": comment.target(),
        "location": store.describe_comment_target(comment.target()),
        "author": comment.author(),
        "body": comment.body(),
        "status": status,
        "createdAtMs": comment.created_at_ms(),
        "updatedAtMs": comment.updated_at_ms(),
    })
}

fn resolve_run_identity(
    store: &mut SqliteStore,
    args: &Value,
) -> Result<(ProjectId, RunId), (u8, Value)> {
    if let Some(run_dir) = args["run"].as_str().filter(|s| !s.is_empty()) {
        let dir = Path::new(run_dir);
        let _ = store.import_run_dir(dir);
        let snap = read(&dir.join("snapshot.json"))?;
        let pid = snap["projectId"]
            .as_str()
            .ok_or_else(|| failure(2, "snapshot missing projectId"))?;
        let rid = snap["id"]
            .as_str()
            .ok_or_else(|| failure(2, "snapshot missing id"))?;
        let project_id = ProjectId::parse(pid).map_err(|e| failure(2, &e.to_string()))?;
        let run_id = RunId::parse(rid).map_err(|e| failure(2, &e.to_string()))?;
        return Ok((project_id, run_id));
    }
    let pid = args["projectId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| failure(2, "missing run or projectId"))?;
    let rid = args["runId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| failure(2, "missing run or runId"))?;
    let project_id = ProjectId::parse(pid).map_err(|e| failure(2, &e.to_string()))?;
    let run_id = RunId::parse(rid).map_err(|e| failure(2, &e.to_string()))?;
    Ok((project_id, run_id))
}

fn build_target(
    store: &SqliteStore,
    run_id: &RunId,
    target_val: &Value,
) -> Result<CommentTarget, (u8, Value)> {
    let kind = target_val["kind"]
        .as_str()
        .unwrap_or("screen")
        .to_ascii_lowercase();
    match kind.as_str() {
        "screen" => {
            let path = target_val["path"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| failure(2, "screen target requires path"))?;
            let screen_id = Screen::derive_id(path);
            let _ = store.ensure_screen_record(&screen_id, path);
            Ok(CommentTarget::Screen { screen_id })
        }
        "state" => {
            let state_raw = target_val["state"]
                .as_str()
                .or_else(|| target_val["stateId"].as_str())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| failure(2, "state target requires state"))?;
            let state_id =
                StateId::parse(state_raw).unwrap_or_else(|_| StateId::derive(&json!(state_raw)));
            if let Some(path) = target_val["path"].as_str().filter(|s| !s.is_empty()) {
                let _ = store.ensure_screen_record(&Screen::derive_id(path), path);
            }
            Ok(CommentTarget::State { state_id })
        }
        "cell" | "width" => {
            if let Some(raw_cell_id) = target_val["cellId"].as_str().filter(|s| !s.is_empty()) {
                let cell_id = CellId::parse(raw_cell_id).map_err(|e| failure(2, &e.to_string()))?;
                return Ok(CommentTarget::Cell { cell_id });
            }
            let path = target_val["path"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| failure(2, "cell target requires path"))?;
            let width = target_val["width"]
                .as_u64()
                .and_then(|w| u32::try_from(w).ok())
                .filter(|&w| w > 0)
                .ok_or_else(|| failure(2, "cell target requires positive width"))?;
            let state = target_val["state"]
                .as_str()
                .or_else(|| target_val["stateId"].as_str())
                .filter(|s| !s.is_empty());
            let screen_id = Screen::derive_id(path);
            let _ = store.ensure_screen_record(&screen_id, path);
            let cell_id = match store
                .find_cell_id(run_id, path, state, width)
                .map_err(|e| failure(3, &e.to_string()))?
            {
                Some(found) => found,
                None => CellId::derive(&json!([run_id, screen_id, state.unwrap_or(""), width])),
            };
            Ok(CommentTarget::Cell { cell_id })
        }
        "transition" | "edge" => {
            if let Some(raw_edge_id) = target_val["edgeId"].as_str().filter(|s| !s.is_empty()) {
                let edge_id = EdgeId::parse(raw_edge_id).map_err(|e| failure(2, &e.to_string()))?;
                return Ok(CommentTarget::Edge { edge_id });
            }
            let from = target_val["path"]
                .as_str()
                .or_else(|| target_val["from"].as_str())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| failure(2, "transition target requires path/from"))?;
            let to = target_val["to"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| failure(2, "transition target requires to"))?;
            let from_id = Screen::derive_id(from);
            let to_id = Screen::derive_id(to);
            let _ = store.ensure_screen_record(&from_id, from);
            let _ = store.ensure_screen_record(&to_id, to);
            let edge_id = EdgeId::derive(&json!([run_id, from_id, to_id]));
            let _ = store.ensure_edge_record(&edge_id, run_id, &from_id, &to_id);
            Ok(CommentTarget::Edge { edge_id })
        }
        _ => Err(failure(
            2,
            "target kind must be screen, state, cell, width, or transition",
        )),
    }
}

pub fn add(args: &Value) -> Reply {
    let mut store = open_store()?;
    let (project_id, run_id) = resolve_run_identity(&mut store, args)?;
    let body = args["body"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| failure(2, "comment body is required"))?;
    let author = args["author"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("reviewer");
    let target = build_target(&store, &run_id, &args["target"])?;
    let comment = Comment::open(project_id, run_id, target, author, body, now_ms())
        .map_err(|e| failure(2, &e.to_string()))?;
    store
        .upsert_comment(&comment)
        .map_err(|e| failure(3, &e.to_string()))?;
    Ok(json!({ "comment": serialize_comment(&store, &comment) }))
}

pub fn list(args: &Value) -> Reply {
    let mut store = open_store()?;
    let mut project_filter = args["projectId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| ProjectId::parse(s).map_err(|e| failure(2, &e.to_string())))
        .transpose()?;
    let run_filter = args["runId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| RunId::parse(s).map_err(|e| failure(2, &e.to_string())))
        .transpose()?;

    if project_filter.is_none()
        && let Some(run_dir) = args["run"].as_str().filter(|s| !s.is_empty())
    {
        let dir = Path::new(run_dir);
        let _ = store.import_run_dir(dir);
        let snap = read(&dir.join("snapshot.json"))?;
        if let Some(pid) = snap["projectId"].as_str() {
            project_filter = Some(ProjectId::parse(pid).map_err(|e| failure(2, &e.to_string()))?);
        }
    }

    let raw_comments = match (&run_filter, &project_filter) {
        (Some(rid), _) => store
            .list_comments(rid)
            .map_err(|e| failure(3, &e.to_string()))?,
        (None, Some(pid)) => store
            .list_comments_by_project(pid)
            .map_err(|e| failure(3, &e.to_string()))?,
        (None, None) => store
            .list_all_comments()
            .map_err(|e| failure(3, &e.to_string()))?,
    };

    let status_filter = args["status"].as_str().filter(|s| !s.is_empty());
    let path_filter = args["path"].as_str().filter(|s| !s.is_empty());
    let width_filter = args["width"].as_u64();

    let mut items = Vec::new();
    let mut open_count = 0usize;
    let mut resolved_count = 0usize;
    for c in &raw_comments {
        let serialized = serialize_comment(&store, c);
        if let Some(st) = status_filter
            && serialized["status"].as_str() != Some(st)
        {
            continue;
        }
        if let Some(p) = path_filter
            && serialized["location"]["path"].as_str() != Some(p)
        {
            continue;
        }
        if let Some(w) = width_filter
            && serialized["location"]["width"].as_u64() != Some(w)
        {
            continue;
        }
        if serialized["status"] == "open" {
            open_count += 1;
        } else {
            resolved_count += 1;
        }
        items.push(serialized);
    }

    Ok(json!({
        "summary": {
            "total": items.len(),
            "open": open_count,
            "resolved": resolved_count,
        },
        "comments": items,
    }))
}

pub fn resolve(args: &Value) -> Reply {
    let mut store = open_store()?;
    let id_str = args["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| failure(2, "missing comment id"))?;
    let id = CommentId::parse(id_str).map_err(|e| failure(2, &e.to_string()))?;
    let mut comment = store
        .get_comment(&id)
        .map_err(|e| failure(3, &e.to_string()))?
        .ok_or_else(|| failure(2, "comment-not-found"))?;
    let ts = now_ms().max(comment.updated_at_ms() + 1);
    comment
        .resolve(ts)
        .map_err(|e| failure(2, &e.to_string()))?;
    store
        .upsert_comment(&comment)
        .map_err(|e| failure(3, &e.to_string()))?;
    Ok(json!({ "comment": serialize_comment(&store, &comment) }))
}

pub fn reopen(args: &Value) -> Reply {
    let mut store = open_store()?;
    let id_str = args["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| failure(2, "missing comment id"))?;
    let id = CommentId::parse(id_str).map_err(|e| failure(2, &e.to_string()))?;
    let mut comment = store
        .get_comment(&id)
        .map_err(|e| failure(3, &e.to_string()))?
        .ok_or_else(|| failure(2, "comment-not-found"))?;
    let ts = now_ms().max(comment.updated_at_ms() + 1);
    comment.reopen(ts).map_err(|e| failure(2, &e.to_string()))?;
    store
        .upsert_comment(&comment)
        .map_err(|e| failure(3, &e.to_string()))?;
    Ok(json!({ "comment": serialize_comment(&store, &comment) }))
}
