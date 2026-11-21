//! Direct SQLite reader/writer against `~/.dui/dui.db` and `~/.dui/blobs/` for the GPUI desktop viewer.

use dui_core::model::{
    Comment, CommentId, CommentStatus, CommentTarget, EdgeEndpoint, ProjectId, RunId, Screen,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
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
    pub created_at_ms: i64,
    pub screens: Vec<ScreenItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopWorkspace {
    pub dui_home: PathBuf,
    pub db_path: PathBuf,
    pub runs: Vec<RunItem>,
    pub comments: Vec<CommentItem>,
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
    let kind = val["kind"].as_str().unwrap_or("unverified").to_owned();
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
                "SELECT r.id, r.project_id, p.display_name, w.branch, r.created_at_ms
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
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in run_rows.flatten() {
            let (run_id, project_id, project_name, branch, created_at_ms) = row;
            let screens = load_run_screens(&conn, dui_home, &run_id, &screen_paths)?;
            runs.push(RunItem {
                id: run_id,
                project_id,
                project_name,
                branch,
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
                "SELECT id, screen_id, state_id, width_px, locale, status_json, image_blob_id
                 FROM cells
                 WHERE run_id = ?1
                 ORDER BY width_px ASC, id ASC",
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
                ))
            })
            .map_err(|e| e.to_string())?;
        for (id, screen_id, state_id, width_px, locale, status_json, blob_id) in rows.flatten() {
            let (status_kind, reason) = parse_status(&status_json);
            let blob_path = blob_id.and_then(|hash| {
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

pub fn add_screen_comment(
    dui_home: &Path,
    project_id: &str,
    run_id: &str,
    screen_path: &str,
    author: &str,
    body: &str,
) -> Result<CommentItem, String> {
    let db_path = dui_home.join("dui.db");
    let conn = Connection::open(&db_path).map_err(|e| format!("open dui.db: {e}"))?;
    let pid = ProjectId::parse(project_id).map_err(|e| e.to_string())?;
    let rid = RunId::parse(run_id).map_err(|e| e.to_string())?;
    let screen_id = Screen::derive_id(screen_path);
    conn.execute(
        "INSERT OR IGNORE INTO screens (id, path, origin_json) VALUES (?1, ?2, ?3)",
        params![screen_id.as_str(), screen_path, r#"{"kind":"entry"}"#],
    )
    .map_err(|e| e.to_string())?;

    let target = CommentTarget::Screen { screen_id };
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
        screen_path: Some(screen_path.to_owned()),
        target_label: screen_path.to_owned(),
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
             VALUES ('cell-1', 'run-1', ?1, 'st-1', 375, NULL, '{\"kind\":\"verified\"}', NULL, NULL, NULL)",
            params![screen_id.as_str()],
        )
        .expect("insert cell");
        drop(conn);

        let added = add_screen_comment(
            dir.path(),
            &project_id,
            "run-1",
            "/order",
            "reviewer",
            "Tighten button spacing on mobile",
        )
        .expect("add comment");
        assert_eq!(added.status, "open");

        let ws = load_workspace(dir.path()).expect("load workspace");
        assert_eq!(ws.runs.len(), 1);
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
