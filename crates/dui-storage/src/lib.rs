//! SQLite + content-addressed blob adapter implementing the five `dui-core::storage`
//! repository ports behind `~/.dui/dui.db` and `~/.dui/blobs/`.

use dui_core::{
    model::{
        BlobId, CacheEntry, CacheInputs, CacheKey, CacheOutcome, Cell, CellId, CellStatus, Comment,
        CommentId, Commit, CommitId, ContentHash, CoverageSummary, Edge, EdgeEndpoint, EdgeId,
        EdgeSource, Evidence, EvidenceId, ImageBlob, Project, ProjectId, Run, RunAggregate,
        RunEnvironment, RunId, RunKind, Screen, ScreenId, ScreenOrigin, State, StateId, Width,
        Worktree, WorktreeId,
    },
    storage::{
        ArtifactStore, CacheRepository, CommentRepository, RunFilter, RunRepository, RunSummary,
        StoreError, StoreResult, WorkspaceRepository,
    },
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::ErrorKind,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_initial",
        include_str!("../../../packages/db/migrations/0001_initial.sql"),
    ),
    (
        "0002_run_branch",
        include_str!("../../../packages/db/migrations/0002_run_branch.sql"),
    ),
];

pub struct SqliteStore {
    conn: Connection,
    blobs_dir: PathBuf,
}

fn db_err(error: impl std::fmt::Display) -> StoreError {
    StoreError::Backend(error.to_string())
}

fn now_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn png_dimensions(bytes: &[u8]) -> (u32, u32) {
    if bytes.len() >= 24 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n" {
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        (w, h)
    } else {
        (0, 0)
    }
}

impl SqliteStore {
    pub fn open(dui_home: &Path) -> StoreResult<(Self, Vec<String>)> {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dui_home)
            .map_err(db_err)?;
        let blobs_dir = dui_home.join("blobs");
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&blobs_dir)
            .map_err(db_err)?;
        let db_path = dui_home.join("dui.db");
        let mut conn = Connection::open(&db_path).map_err(db_err)?;
        conn.execute_batch(
            "PRAGMA busy_timeout = 10000;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )
        .map_err(db_err)?;

        let applied = apply_migrations(&mut conn)?;
        Ok((Self { conn, blobs_dir }, applied))
    }

    pub fn blob_path(&self, id: &BlobId) -> PathBuf {
        self.blobs_dir.join(format!("{}.png", id.as_str()))
    }

    pub fn link_or_copy_blob(&self, id: &BlobId, target: &Path) -> StoreResult<()> {
        let source = self.blob_path(id);
        if target.exists() {
            let _ = fs::remove_file(target);
        }
        match fs::hard_link(&source, target) {
            Ok(()) => Ok(()),
            Err(_) => {
                fs::copy(&source, target).map_err(db_err)?;
                Ok(())
            }
        }
    }

    /// Finds the most recently hit cached inputs for the same logical cell slot
    /// (`state_id`, `width`, `locale`) so `evaluate_cache` can report exact changed inputs on a miss.
    pub fn find_previous_cache_inputs(
        &self,
        state_id: &StateId,
        width: Width,
        locale: Option<&str>,
    ) -> StoreResult<Option<CacheInputs>> {
        let mut stmt = self
            .conn
            .prepare("SELECT inputs_json FROM cache_entries ORDER BY last_hit_at_ms DESC")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_err)?;
        for row in rows {
            let raw = row.map_err(db_err)?;
            let Ok(inputs) = serde_json::from_str::<CacheInputs>(&raw) else {
                continue;
            };
            if inputs.state_id == *state_id
                && inputs.width == width
                && inputs.locale.as_deref() == locale
            {
                return Ok(Some(inputs));
            }
        }
        Ok(None)
    }

    /// Removes unreferenced blobs and, when `max_bytes` is specified, evicts oldest
    /// cache-only entries until total blob storage is within `max_bytes` while keeping
    /// every blob referenced by a listed run intact.
    pub fn prune(&mut self, max_bytes: Option<u64>) -> StoreResult<Value> {
        let mut removed_blobs: u64 = 0;
        let mut freed_bytes: u64 = 0;

        let unreferenced = self.unreferenced_blobs()?;
        for id in unreferenced {
            let size = fs::metadata(self.blob_path(&id))
                .map(|m| m.len())
                .unwrap_or(0);
            self.delete_blob(&id)?;
            removed_blobs += 1;
            freed_bytes += size;
        }

        // Also remove any stray files in `blobs_dir` not tracked in `image_blobs`.
        if let Ok(entries) = fs::read_dir(&self.blobs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("png") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let tracked: Option<i64> = self
                    .conn
                    .query_row(
                        "SELECT 1 FROM image_blobs WHERE id = ?1",
                        params![stem],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(db_err)?;
                if tracked.is_none() {
                    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    if fs::remove_file(&path).is_ok() {
                        removed_blobs += 1;
                        freed_bytes += size;
                    }
                }
            }
        }

        if let Some(limit) = max_bytes {
            let mut total = self.total_blob_bytes()?;
            if total > limit {
                let mut stmt = self
                    .conn
                    .prepare(
                        "SELECT cache_key, image_blob_id FROM cache_entries
                         WHERE image_blob_id IS NOT NULL
                           AND image_blob_id NOT IN (SELECT image_blob_id FROM cells WHERE image_blob_id IS NOT NULL)
                         ORDER BY last_hit_at_ms ASC",
                    )
                    .map_err(db_err)?;
                let candidates: Vec<(String, String)> = stmt
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                    .map_err(db_err)?
                    .collect::<Result<_, _>>()
                    .map_err(db_err)?;
                drop(stmt);

                for (cache_key, _blob_id) in candidates {
                    if total <= limit {
                        break;
                    }
                    self.conn
                        .execute(
                            "DELETE FROM cache_entries WHERE cache_key = ?1",
                            params![cache_key],
                        )
                        .map_err(db_err)?;
                    for id in self.unreferenced_blobs()? {
                        let size = fs::metadata(self.blob_path(&id))
                            .map(|m| m.len())
                            .unwrap_or(0);
                        self.delete_blob(&id)?;
                        removed_blobs += 1;
                        freed_bytes += size;
                        total = total.saturating_sub(size);
                    }
                }
            }
        }

        let remaining_blobs: u64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM image_blobs", [], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(db_err)? as u64;
        let remaining_bytes = self.total_blob_bytes()?;
        Ok(json!({
            "removedBlobs": removed_blobs,
            "freedBytes": freed_bytes,
            "remainingBlobs": remaining_blobs,
            "remainingBytes": remaining_bytes,
        }))
    }

    fn total_blob_bytes(&self) -> StoreResult<u64> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM image_blobs")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_err)?;
        let mut total = 0u64;
        for row in rows {
            let id = BlobId::parse(row.map_err(db_err)?).map_err(db_err)?;
            total += fs::metadata(self.blob_path(&id))
                .map(|m| m.len())
                .unwrap_or(0);
        }
        Ok(total)
    }

    pub fn has_run(&self, id: &RunId) -> StoreResult<bool> {
        let exists: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM runs WHERE id = ?1",
                params![id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_err)?;
        Ok(exists.is_some())
    }

    /// Imports one archived run directory (`snapshot.json`, `report.json`, optional `graph.json`)
    /// into SQLite and content-addressed blobs. Returns `true` if newly imported, `false` if already present.
    pub fn import_run_dir(&mut self, run_dir: &Path) -> StoreResult<bool> {
        let snapshot_bytes = fs::read(run_dir.join("snapshot.json")).map_err(db_err)?;
        let snapshot: Value = serde_json::from_slice(&snapshot_bytes).map_err(db_err)?;
        let run_id_raw = snapshot["id"]
            .as_str()
            .or_else(|| run_dir.file_name().and_then(|n| n.to_str()))
            .unwrap_or("run");
        let run_id = RunId::parse(run_id_raw).map_err(db_err)?;
        if self.has_run(&run_id)? {
            return Ok(false);
        }

        let report_bytes = fs::read(run_dir.join("report.json")).map_err(db_err)?;
        let report: Value = serde_json::from_slice(&report_bytes).map_err(db_err)?;
        let graph: Value = fs::read(run_dir.join("graph.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_else(|| json!({"nodes": [], "edges": []}));

        let project_root = snapshot["projectRoot"].as_str().unwrap_or("/project");
        let project_id = match snapshot["projectId"].as_str() {
            Some(raw) => ProjectId::parse(raw).map_err(db_err)?,
            None => ProjectId::from_primary_root_bytes(project_root.as_bytes()),
        };

        let project_record_name = run_dir
            .parent()
            .and_then(Path::parent)
            .map(|p| p.join("project.json"))
            .and_then(|p| fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v["name"].as_str().map(str::to_owned))
            .filter(|s| !s.trim().is_empty());

        let fallback_name = Path::new(project_root)
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("Unnamed project");
        let display_name = project_record_name
            .as_deref()
            .or_else(|| {
                snapshot["projectName"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
            })
            .unwrap_or(fallback_name);

        let project =
            Project::with_id(project_id.clone(), project_root, display_name).map_err(db_err)?;
        let worktree_path = snapshot["worktree"].as_str().unwrap_or(project_root);
        let app_relative = snapshot["app"].as_str().unwrap_or_default();
        let branch = snapshot["branch"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let worktree = Worktree::new(project_id.clone(), worktree_path, app_relative, branch)
            .map_err(db_err)?;
        let commit = Commit::new(
            project_id.clone(),
            snapshot["commit"].as_str().map(str::to_owned),
            snapshot["dirty"].as_bool().unwrap_or(false),
            None,
        );

        let kind = if let Some(live) = report.get("live").filter(|v| v.is_object()) {
            let source_run =
                RunId::parse(live["sourceRun"].as_str().unwrap_or("source")).map_err(db_err)?;
            let source_state =
                StateId::parse(live["sourceCell"]["state"].as_str().unwrap_or_default())
                    .unwrap_or_else(|_| StateId::derive(&live["sourceCell"]["state"]));
            let source_width = Width::new(live["sourceCell"]["width"].as_u64().unwrap_or(1) as u32)
                .map_err(db_err)?;
            RunKind::LiveSave {
                source_run,
                source_state,
                source_width,
                revision: live["revision"].as_u64().unwrap_or(0),
            }
        } else {
            RunKind::Capture
        };

        let mut versions = BTreeMap::new();
        if let Some(map) = report["versions"].as_object() {
            for (k, v) in map {
                if let Some(s) = v.as_str() {
                    versions.insert(k.clone(), s.to_owned());
                }
            }
        }
        let source_files = report["sourceFiles"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let run = Run::new(
            run_id.clone(),
            project_id,
            worktree.id().clone(),
            commit.id().clone(),
            kind,
            snapshot["timestampMs"].as_i64().unwrap_or(0),
            RunEnvironment {
                os: report["os"].as_str().unwrap_or_default().to_owned(),
                compiler: report["compiler"].as_str().map(str::to_owned),
                renderer: report["browser"]["product"].as_str().map(str::to_owned),
                dui_version: snapshot["duiVersion"]
                    .as_str()
                    .unwrap_or(env!("CARGO_PKG_VERSION"))
                    .to_owned(),
                versions,
            },
            source_files,
        );

        let mut native_screens = BTreeMap::new();
        let mut screens_by_path: BTreeMap<String, Screen> = BTreeMap::new();
        let mut source_paths = BTreeMap::new();
        for node in graph["nodes"].as_array().into_iter().flatten() {
            if node["kind"] == "screen"
                && let Some(source) = node.get("nativeSource")
            {
                let declaration = serde_json::from_value(source.clone()).map_err(db_err)?;
                let path = node["label"]
                    .as_str()
                    .ok_or_else(|| db_err("missing native route"))?;
                let screen =
                    Screen::new(path, ScreenOrigin::Native { declaration }).map_err(db_err)?;
                native_screens.insert(screen.id().clone(), screen);
            } else if node["kind"] == "screen" && graph["screenDiscovery"]["kind"] == "tsx-source" {
                let id = node["id"]
                    .as_str()
                    .ok_or_else(|| db_err("missing source screen id"))?;
                let path = node["label"]
                    .as_str()
                    .ok_or_else(|| db_err("missing source screen label"))?;
                let origin = if id.starts_with("route:") {
                    ScreenOrigin::Router {
                        file: node["location"]["file"]
                            .as_str()
                            .ok_or_else(|| db_err("missing route location"))?
                            .to_owned(),
                    }
                } else {
                    ScreenOrigin::Entry
                };
                source_paths.insert(id, path);
                screens_by_path.insert(path.to_owned(), Screen::new(path, origin).map_err(db_err)?);
            }
        }

        let mut states_by_id: BTreeMap<StateId, State> = BTreeMap::new();
        let mut cells = Vec::new();
        let mut seen_cell_ids = BTreeSet::new();
        let mut evidence_list: Vec<Evidence> = Vec::new();
        let mut blob_items: Vec<(ImageBlob, Vec<u8>)> = Vec::new();

        for raw in report["cells"].as_array().into_iter().flatten() {
            let path = raw["path"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("/");
            let screen = if !native_screens.is_empty()
                && let Some(raw_id) = raw["screenId"].as_str()
            {
                let id = ScreenId::parse(raw_id).map_err(db_err)?;
                native_screens
                    .get(&id)
                    .filter(|s| s.path() == path)
                    .ok_or_else(|| db_err("cell references an unknown native declaration"))?
                    .clone()
            } else if let Some(existing) = screens_by_path.get(path) {
                existing.clone()
            } else {
                let origin = match report["screenDiscovery"]["sources"].get(path) {
                    Some(Value::String(file)) => ScreenOrigin::Router { file: file.clone() },
                    Some(obj) if obj["kind"] == "anchor" => ScreenOrigin::Anchor {
                        from: obj["from"].as_str().unwrap_or("/").to_owned(),
                        selector: obj["selector"].as_str().map(str::to_owned),
                    },
                    _ => ScreenOrigin::Entry,
                };
                let created = Screen::new(path, origin).map_err(db_err)?;
                screens_by_path.insert(path.to_owned(), created.clone());
                created
            };

            let scenario = raw["scenario"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("default");
            let mut inputs = BTreeMap::new();
            if let Some(obj) = raw["input"].as_object() {
                for (k, v) in obj {
                    inputs.insert(k.clone(), v.clone());
                }
            }
            let plan_index = raw["planIndex"].as_u64().unwrap_or(0) as u32;
            let locale = raw["locale"].as_str().map(str::to_owned);
            let state = match raw["state"].as_str().and_then(|s| StateId::parse(s).ok()) {
                Some(id) => State::with_id(
                    id,
                    screen.id().clone(),
                    scenario,
                    locale.clone(),
                    inputs,
                    plan_index,
                )
                .map_err(db_err)?,
                None => State::new(
                    screen.id().clone(),
                    scenario,
                    locale.clone(),
                    inputs,
                    plan_index,
                )
                .map_err(db_err)?,
            };
            let state_id = state.id().clone();
            states_by_id.entry(state_id.clone()).or_insert(state);

            let width = Width::new(raw["width"].as_u64().unwrap_or(1) as u32).map_err(db_err)?;
            let status_raw = raw["status"].as_str().unwrap_or("unverified");
            let reason = raw["reason"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| (status_raw != "verified").then(|| "unknown".to_owned()));
            let status = CellStatus::from_wire(status_raw, reason).map_err(db_err)?;

            let image_blob = if status.is_unrenderable() {
                None
            } else if let Some(name) = raw["image"].as_str() {
                let candidate = run_dir.join(name);
                if let Ok(bytes) = fs::read(&candidate) {
                    let (w, h) = png_dimensions(&bytes);
                    let blob = ImageBlob::from_bytes("image/png", &bytes, w, h).map_err(db_err)?;
                    let id = blob.id().clone();
                    blob_items.push((blob, bytes));
                    Some(id)
                } else {
                    None
                }
            } else {
                None
            };

            let evidence_id = raw["observation"].as_object().map(|obs| {
                let slots = obs
                    .get("slots")
                    .and_then(Value::as_object)
                    .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();
                let hits = obs
                    .get("hits")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect();
                let links = obs
                    .get("links")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect();
                let responses = obs
                    .get("responses")
                    .and_then(Value::as_object)
                    .map(|m| {
                        m.iter()
                            .filter_map(|(k, v)| v.as_u64().map(|code| (k.clone(), code as u16)))
                            .collect()
                    })
                    .unwrap_or_default();
                let live_edits = report["live"]["edits"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let ev = Evidence::new(
                    obs.get("path").and_then(Value::as_str).map(str::to_owned),
                    obs.get("width").and_then(Value::as_u64).map(|w| w as u32),
                    obs.get("ready").and_then(Value::as_bool),
                    obs.get("settled").and_then(Value::as_bool),
                    slots,
                    BTreeMap::new(),
                    hits,
                    links,
                    responses,
                    raw["checks"].as_array().cloned().unwrap_or_default(),
                    live_edits,
                )
                .with_screen_evidence(obs.get("screenEvidence"));
                let id = ev.id().clone();
                evidence_list.push(ev);
                id
            });

            let cell = Cell::new(
                run_id.clone(),
                screen.id().clone(),
                state_id,
                width,
                locale,
                status,
                image_blob,
                evidence_id,
                None,
            )
            .map_err(db_err)?;
            if seen_cell_ids.insert(cell.id().clone()) {
                cells.push(cell);
            }
        }

        let mut edges = Vec::new();
        let mut source_pairs = BTreeSet::new();
        for raw in graph["edges"].as_array().into_iter().flatten() {
            if raw["kind"] != "linksTo" {
                continue;
            }
            let Some(from_path) = raw["source"].as_str().and_then(|id| {
                source_paths
                    .get(id)
                    .copied()
                    .or_else(|| id.strip_prefix("route:"))
            }) else {
                continue;
            };
            let Some(to_path) = raw["target"].as_str().and_then(|id| {
                source_paths
                    .get(id)
                    .copied()
                    .or_else(|| id.strip_prefix("route:"))
            }) else {
                continue;
            };
            if !screens_by_path.contains_key(from_path) || !screens_by_path.contains_key(to_path) {
                continue;
            }
            // SQLite stores one extracted relation per pair. The archive keeps each TSX trigger.
            if !source_paths.is_empty() && !source_pairs.insert((from_path, to_path)) {
                continue;
            }
            let mut observed_widths = BTreeSet::new();
            for w in raw["observedWidths"].as_array().into_iter().flatten() {
                if let Some(px) = w.as_u64().and_then(|n| Width::new(n as u32).ok()) {
                    observed_widths.insert(px);
                }
            }
            edges.push(Edge::discovered(
                run_id.clone(),
                EdgeEndpoint::Screen {
                    screen_id: Screen::derive_id(from_path),
                },
                EdgeEndpoint::Screen {
                    screen_id: Screen::derive_id(to_path),
                },
                raw["selector"].as_str().map(str::to_owned),
                observed_widths,
            ));
        }

        let mut screens: Vec<_> = screens_by_path.into_values().collect();
        screens.extend(native_screens.into_values());

        let aggregate = RunAggregate::publish(
            run,
            screens,
            states_by_id.into_values().collect(),
            cells,
            edges,
        )
        .map_err(db_err)?;

        self.upsert_project(&project)?;
        self.upsert_worktree(&worktree)?;
        self.put_commit(&commit)?;
        for (blob, bytes) in &blob_items {
            self.put_blob(blob, bytes)?;
        }
        for ev in &evidence_list {
            self.put_evidence(ev)?;
        }
        match self.save_run(&aggregate) {
            Ok(()) => Ok(true),
            Err(StoreError::Conflict(_)) => Ok(false),
            Err(err) => Err(err),
        }
    }

    /// Scans `~/.dui/projects/<project_id>/runs/<run_id>` and imports any runs not yet in SQLite.
    pub fn import_disk_archive(&mut self, projects_dir: &Path) -> StoreResult<usize> {
        let entries = match fs::read_dir(projects_dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(db_err(err)),
        };
        let mut imported = 0;
        let mut run_dirs = Vec::new();
        for project in entries {
            let project_dir = project.map_err(db_err)?.path();
            let runs_dir = project_dir.join("runs");
            if !runs_dir.is_dir() {
                continue;
            }
            for entry in fs::read_dir(&runs_dir).map_err(db_err)? {
                let entry = entry.map_err(db_err)?;
                let name = entry.file_name();
                if name.to_string_lossy().starts_with('.')
                    || !entry.file_type().map_err(db_err)?.is_dir()
                {
                    continue;
                }
                let path = entry.path();
                if path.join("snapshot.json").is_file() && path.join("report.json").is_file() {
                    run_dirs.push(path);
                }
            }
        }
        run_dirs.sort();
        for dir in run_dirs {
            if self.import_run_dir(&dir)? {
                imported += 1;
            }
        }
        Ok(imported)
    }
}

fn apply_migrations(conn: &mut Connection) -> StoreResult<Vec<String>> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_err)?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS dui_migrations (
           tag TEXT PRIMARY KEY NOT NULL,
           applied_at_ms INTEGER NOT NULL
         );",
    )
    .map_err(db_err)?;

    let mut applied = Vec::new();
    for &(tag, sql) in MIGRATIONS {
        let already: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM dui_migrations WHERE tag = ?1",
                params![tag],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_err)?;
        if already.is_some() {
            continue;
        }
        tx.execute_batch(sql).map_err(db_err)?;
        tx.execute(
            "INSERT INTO dui_migrations (tag, applied_at_ms) VALUES (?1, ?2)",
            params![tag, now_epoch_ms()],
        )
        .map_err(db_err)?;
        applied.push(tag.to_owned());
    }
    tx.commit().map_err(db_err)?;
    Ok(applied)
}

impl WorkspaceRepository for SqliteStore {
    fn upsert_project(&mut self, project: &Project) -> StoreResult<()> {
        self.conn
            .execute(
                "INSERT INTO projects (id, display_name, primary_root)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET display_name = excluded.display_name",
                params![
                    project.id().as_str(),
                    project.display_name(),
                    project.primary_root(),
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn get_project(&self, id: &ProjectId) -> StoreResult<Option<Project>> {
        let row: Option<(String, String, String)> = self
            .conn
            .query_row(
                "SELECT id, display_name, primary_root FROM projects WHERE id = ?1",
                params![id.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(db_err)?;
        row.map(|(id, name, root)| {
            let pid = ProjectId::parse(id).map_err(db_err)?;
            Project::with_id(pid, root, name).map_err(db_err)
        })
        .transpose()
    }

    fn upsert_worktree(&mut self, worktree: &Worktree) -> StoreResult<()> {
        self.conn
            .execute(
                "INSERT INTO worktrees (id, project_id, path, app_relative, branch)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET branch = excluded.branch",
                params![
                    worktree.id().as_str(),
                    worktree.project_id().as_str(),
                    worktree.path(),
                    worktree.app_relative(),
                    worktree.branch(),
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn get_worktree(&self, id: &WorktreeId) -> StoreResult<Option<Worktree>> {
        let row: Option<(String, String, String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT project_id, path, app_relative, branch FROM worktrees WHERE id = ?1",
                params![id.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(db_err)?;
        row.map(|(pid, path, app_rel, branch)| {
            Worktree::new(
                ProjectId::parse(pid).map_err(db_err)?,
                path,
                app_rel,
                branch,
            )
            .map_err(db_err)
        })
        .transpose()
    }

    fn put_commit(&mut self, commit: &Commit) -> StoreResult<()> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO commits (id, project_id, git_sha, dirty, tree_digest)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    commit.id().as_str(),
                    commit.project_id().as_str(),
                    commit.git_sha(),
                    i64::from(commit.is_dirty()),
                    commit.tree_digest().map(ContentHash::as_str),
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn get_commit(&self, id: &CommitId) -> StoreResult<Option<Commit>> {
        let row: Option<(String, Option<String>, i64, Option<String>)> = self
            .conn
            .query_row(
                "SELECT project_id, git_sha, dirty, tree_digest FROM commits WHERE id = ?1",
                params![id.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(db_err)?;
        row.map(|(pid, git_sha, dirty, tree_digest)| {
            let tree = tree_digest
                .map(ContentHash::parse)
                .transpose()
                .map_err(db_err)?;
            Ok(Commit::new(
                ProjectId::parse(pid).map_err(db_err)?,
                git_sha,
                dirty != 0,
                tree,
            ))
        })
        .transpose()
    }
}

impl ArtifactStore for SqliteStore {
    fn put_blob(&mut self, blob: &ImageBlob, bytes: &[u8]) -> StoreResult<()> {
        if BlobId::from_image_bytes(bytes) != *blob.id() {
            return Err(StoreError::Conflict(
                "blob bytes do not match content hash".into(),
            ));
        }
        let target = self.blob_path(blob.id());
        if !target.exists() {
            let staged = tempfile::NamedTempFile::new_in(&self.blobs_dir).map_err(db_err)?;
            fs::write(staged.path(), bytes).map_err(db_err)?;
            match staged.persist_noclobber(&target) {
                Ok(_) => {}
                Err(err) if err.error.kind() == ErrorKind::AlreadyExists => {}
                Err(err) => return Err(db_err(err)),
            }
        }
        self.conn
            .execute(
                "INSERT OR IGNORE INTO image_blobs (id, media_type, byte_len, width_px, height_px)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    blob.id().as_str(),
                    blob.media_type(),
                    blob.byte_len() as i64,
                    i64::from(blob.width_px()),
                    i64::from(blob.height_px()),
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn get_blob(&self, id: &BlobId) -> StoreResult<Option<(ImageBlob, Vec<u8>)>> {
        let row: Option<(String, i64, i64, i64)> = self
            .conn
            .query_row(
                "SELECT media_type, byte_len, width_px, height_px FROM image_blobs WHERE id = ?1",
                params![id.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(db_err)?;
        let Some((media_type, byte_len, width_px, height_px)) = row else {
            return Ok(None);
        };
        let bytes = fs::read(self.blob_path(id)).map_err(db_err)?;
        let blob = ImageBlob::metadata(
            id.clone(),
            media_type,
            byte_len as u64,
            width_px as u32,
            height_px as u32,
        )
        .map_err(db_err)?;
        Ok(Some((blob, bytes)))
    }

    fn put_evidence(&mut self, evidence: &Evidence) -> StoreResult<()> {
        let payload = serde_json::to_string(evidence).map_err(db_err)?;
        self.conn
            .execute(
                "INSERT OR IGNORE INTO evidence (id, payload_json) VALUES (?1, ?2)",
                params![evidence.id().as_str(), payload],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn get_evidence(&self, id: &EvidenceId) -> StoreResult<Option<Evidence>> {
        let row: Option<String> = self
            .conn
            .query_row(
                "SELECT payload_json FROM evidence WHERE id = ?1",
                params![id.as_str()],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_err)?;
        row.map(|json| serde_json::from_str(&json).map_err(db_err))
            .transpose()
    }

    fn unreferenced_blobs(&self) -> StoreResult<Vec<BlobId>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id FROM image_blobs
                 WHERE id NOT IN (SELECT image_blob_id FROM cells WHERE image_blob_id IS NOT NULL)
                   AND id NOT IN (SELECT image_blob_id FROM cache_entries WHERE image_blob_id IS NOT NULL)
                 ORDER BY id",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_err)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(BlobId::parse(row.map_err(db_err)?).map_err(db_err)?);
        }
        Ok(result)
    }

    fn delete_blob(&mut self, id: &BlobId) -> StoreResult<()> {
        self.conn
            .execute(
                "DELETE FROM image_blobs WHERE id = ?1",
                params![id.as_str()],
            )
            .map_err(db_err)?;
        match fs::remove_file(self.blob_path(id)) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
            Err(err) => Err(db_err(err)),
        }
    }
}

impl RunRepository for SqliteStore {
    fn save_run(&mut self, aggregate: &RunAggregate) -> StoreResult<()> {
        let run = aggregate.run();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_err)?;

        let exists: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM runs WHERE id = ?1",
                params![run.id().as_str()],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_err)?;
        if exists.is_some() {
            return Err(StoreError::Conflict(format!(
                "run {} already exists",
                run.id()
            )));
        }

        let kind_json = serde_json::to_string(run.kind()).map_err(db_err)?;
        let env_json = serde_json::to_string(run.environment()).map_err(db_err)?;
        let files_json = serde_json::to_string(run.source_files()).map_err(db_err)?;
        let run_branch: Option<String> = tx
            .query_row(
                "SELECT branch FROM worktrees WHERE id = ?1",
                params![run.worktree_id().as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_err)?
            .flatten();
        tx.execute(
            "INSERT INTO runs (id, project_id, worktree_id, commit_id, kind_json, created_at_ms, environment_json, source_files_json, branch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run.id().as_str(),
                run.project_id().as_str(),
                run.worktree_id().as_str(),
                run.commit_id().as_str(),
                kind_json,
                run.created_at_ms(),
                env_json,
                files_json,
                run_branch,
            ],
        )
        .map_err(db_err)?;

        for screen in aggregate.screens().values() {
            let origin_json = serde_json::to_string(screen.origin()).map_err(db_err)?;
            tx.execute(
                "INSERT OR IGNORE INTO screens (id, path, origin_json) VALUES (?1, ?2, ?3)",
                params![screen.id().as_str(), screen.path(), origin_json],
            )
            .map_err(db_err)?;
            tx.execute(
                "INSERT OR IGNORE INTO run_screens (run_id, screen_id) VALUES (?1, ?2)",
                params![run.id().as_str(), screen.id().as_str()],
            )
            .map_err(db_err)?;
        }

        for state in aggregate.states().values() {
            let inputs_json = serde_json::to_string(state.inputs()).map_err(db_err)?;
            tx.execute(
                "INSERT OR IGNORE INTO states (id, screen_id, scenario, locale, inputs_json, plan_index)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    state.id().as_str(),
                    state.screen_id().as_str(),
                    state.scenario(),
                    state.locale(),
                    inputs_json,
                    i64::from(state.plan_index()),
                ],
            )
            .map_err(db_err)?;
            tx.execute(
                "INSERT OR IGNORE INTO run_states (run_id, state_id) VALUES (?1, ?2)",
                params![run.id().as_str(), state.id().as_str()],
            )
            .map_err(db_err)?;
        }

        for cell in aggregate.cells() {
            let status_json = serde_json::to_string(cell.status()).map_err(db_err)?;
            let cache_json = cell
                .cache()
                .map(serde_json::to_string)
                .transpose()
                .map_err(db_err)?;
            tx.execute(
                "INSERT INTO cells (id, run_id, screen_id, state_id, width_px, locale, status_json, image_blob_id, evidence_id, cache_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    cell.id().as_str(),
                    cell.run_id().as_str(),
                    cell.screen_id().as_str(),
                    cell.state_id().as_str(),
                    i64::from(cell.width().pixels()),
                    cell.locale(),
                    status_json,
                    cell.image_blob().map(BlobId::as_str),
                    cell.evidence_id().map(EvidenceId::as_str),
                    cache_json,
                ],
            )
            .map_err(db_err)?;
        }

        for edge in aggregate.edges() {
            let from_json = serde_json::to_string(edge.from()).map_err(db_err)?;
            let to_json = serde_json::to_string(edge.to()).map_err(db_err)?;
            let source = match edge.source() {
                EdgeSource::Extracted => "extracted",
                EdgeSource::Inferred => "inferred",
                EdgeSource::Verified => "verified",
            };
            let widths_json = serde_json::to_string(edge.observed_widths()).map_err(db_err)?;
            tx.execute(
                "INSERT INTO edges (id, run_id, from_json, to_json, source, trigger, selector, observed_widths_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    edge.id().as_str(),
                    edge.run_id().as_str(),
                    from_json,
                    to_json,
                    source,
                    Option::<&str>::None,
                    edge.selector(),
                    widths_json,
                ],
            )
            .map_err(db_err)?;
        }

        tx.commit().map_err(db_err)?;
        Ok(())
    }

    fn load_run(&self, id: &RunId) -> StoreResult<Option<RunAggregate>> {
        let run_row: Option<(String, String, String, String, i64, String, String)> = self
            .conn
            .query_row(
                "SELECT project_id, worktree_id, commit_id, kind_json, created_at_ms, environment_json, source_files_json
                 FROM runs WHERE id = ?1",
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
                    ))
                },
            )
            .optional()
            .map_err(db_err)?;
        let Some((pid, wid, cid, kind_json, created_at_ms, env_json, files_json)) = run_row else {
            return Ok(None);
        };
        let run = Run::new(
            id.clone(),
            ProjectId::parse(pid).map_err(db_err)?,
            WorktreeId::parse(wid).map_err(db_err)?,
            CommitId::parse(cid).map_err(db_err)?,
            serde_json::from_str(&kind_json).map_err(db_err)?,
            created_at_ms,
            serde_json::from_str(&env_json).map_err(db_err)?,
            serde_json::from_str(&files_json).map_err(db_err)?,
        );

        let mut screens = Vec::new();
        {
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT s.path, s.origin_json FROM screens s
                     INNER JOIN run_screens rs ON rs.screen_id = s.id
                     WHERE rs.run_id = ?1 ORDER BY s.path",
                )
                .map_err(db_err)?;
            let rows = stmt
                .query_map(params![id.as_str()], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(db_err)?;
            for row in rows {
                let (path, origin_json) = row.map_err(db_err)?;
                let origin: ScreenOrigin = serde_json::from_str(&origin_json).map_err(db_err)?;
                screens.push(Screen::new(path, origin).map_err(db_err)?);
            }
        }

        let mut states = Vec::new();
        {
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT st.id, st.screen_id, st.scenario, st.locale, st.inputs_json, st.plan_index
                     FROM states st
                     INNER JOIN run_states rs ON rs.state_id = st.id
                     WHERE rs.run_id = ?1 ORDER BY st.plan_index, st.id",
                )
                .map_err(db_err)?;
            let rows = stmt
                .query_map(params![id.as_str()], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                    ))
                })
                .map_err(db_err)?;
            for row in rows {
                let (sid, screen_id, scenario, locale, inputs_json, plan_index) =
                    row.map_err(db_err)?;
                let inputs: BTreeMap<String, Value> =
                    serde_json::from_str(&inputs_json).map_err(db_err)?;
                states.push(
                    State::with_id(
                        StateId::parse(sid).map_err(db_err)?,
                        dui_core::model::ScreenId::parse(screen_id).map_err(db_err)?,
                        scenario,
                        locale,
                        inputs,
                        plan_index as u32,
                    )
                    .map_err(db_err)?,
                );
            }
        }

        let mut cells = Vec::new();
        {
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT screen_id, state_id, width_px, locale, status_json, image_blob_id, evidence_id, cache_json
                     FROM cells WHERE run_id = ?1 ORDER BY id",
                )
                .map_err(db_err)?;
            let rows = stmt
                .query_map(params![id.as_str()], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Option<String>>(5)?,
                        r.get::<_, Option<String>>(6)?,
                        r.get::<_, Option<String>>(7)?,
                    ))
                })
                .map_err(db_err)?;
            for row in rows {
                let (
                    screen_id,
                    state_id,
                    width_px,
                    locale,
                    status_json,
                    blob_id,
                    ev_id,
                    cache_json,
                ) = row.map_err(db_err)?;
                let status: CellStatus = serde_json::from_str(&status_json).map_err(db_err)?;
                let cache: Option<CacheOutcome> = cache_json
                    .map(|j| serde_json::from_str(&j))
                    .transpose()
                    .map_err(db_err)?;
                cells.push(
                    Cell::new(
                        id.clone(),
                        dui_core::model::ScreenId::parse(screen_id).map_err(db_err)?,
                        StateId::parse(state_id).map_err(db_err)?,
                        Width::new(width_px as u32).map_err(db_err)?,
                        locale,
                        status,
                        blob_id.map(BlobId::parse).transpose().map_err(db_err)?,
                        ev_id.map(EvidenceId::parse).transpose().map_err(db_err)?,
                        cache,
                    )
                    .map_err(db_err)?,
                );
            }
        }

        let mut edges = Vec::new();
        {
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT from_json, to_json, source, trigger, selector, observed_widths_json
                     FROM edges WHERE run_id = ?1 ORDER BY id",
                )
                .map_err(db_err)?;
            let rows = stmt
                .query_map(params![id.as_str()], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, String>(5)?,
                    ))
                })
                .map_err(db_err)?;
            for row in rows {
                let (from_json, to_json, source_raw, trigger, selector, widths_json) =
                    row.map_err(db_err)?;
                let from: EdgeEndpoint = serde_json::from_str(&from_json).map_err(db_err)?;
                let to: EdgeEndpoint = serde_json::from_str(&to_json).map_err(db_err)?;
                let source = EdgeSource::from_wire(&source_raw).map_err(db_err)?;
                let widths: BTreeSet<Width> = serde_json::from_str(&widths_json).map_err(db_err)?;
                let edge = match source {
                    EdgeSource::Extracted => {
                        Edge::discovered(id.clone(), from, to, selector, widths)
                    }
                    EdgeSource::Inferred => Edge::inferred(id.clone(), from, to, trigger),
                    EdgeSource::Verified => {
                        Edge::discovered(id.clone(), from, to, selector, widths.clone())
                            .confirm_by_replay(id.clone(), widths)
                            .map_err(db_err)?
                    }
                };
                edges.push(edge);
            }
        }

        RunAggregate::publish(run, screens, states, cells, edges)
            .map(Some)
            .map_err(db_err)
    }

    fn list_runs(&self, filter: &RunFilter) -> StoreResult<Vec<RunSummary>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT r.id, r.project_id, r.worktree_id, r.commit_id, r.kind_json,
                        r.created_at_ms, r.environment_json, r.source_files_json,
                        p.display_name, COALESCE(r.branch, w.branch), c.git_sha, c.dirty
                 FROM runs r
                 INNER JOIN projects p ON p.id = r.project_id
                 INNER JOIN worktrees w ON w.id = r.worktree_id
                 INNER JOIN commits c ON c.id = r.commit_id
                 WHERE (?1 IS NULL OR r.project_id = ?1)
                   AND (?2 IS NULL OR r.worktree_id = ?2)
                   AND (?3 IS NULL OR COALESCE(r.branch, w.branch) = ?3)
                 ORDER BY r.created_at_ms DESC, r.id DESC",
            )
            .map_err(db_err)?;

        let rows = stmt
            .query_map(
                params![
                    filter.project_id.as_ref().map(ProjectId::as_str),
                    filter.worktree_id.as_ref().map(WorktreeId::as_str),
                    filter.branch.as_deref(),
                ],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, String>(7)?,
                        r.get::<_, String>(8)?,
                        r.get::<_, Option<String>>(9)?,
                        r.get::<_, Option<String>>(10)?,
                        r.get::<_, i64>(11)?,
                    ))
                },
            )
            .map_err(db_err)?;

        let mut summaries = Vec::new();
        for row in rows {
            let (
                id,
                pid,
                wid,
                cid,
                kind_json,
                created_at_ms,
                env_json,
                files_json,
                project_name,
                branch,
                commit_sha,
                dirty,
            ) = row.map_err(db_err)?;
            let run_id = RunId::parse(id).map_err(db_err)?;
            let coverage = self.coverage_for_run(&run_id)?;
            let run = Run::new(
                run_id,
                ProjectId::parse(pid).map_err(db_err)?,
                WorktreeId::parse(wid).map_err(db_err)?,
                CommitId::parse(cid).map_err(db_err)?,
                serde_json::from_str(&kind_json).map_err(db_err)?,
                created_at_ms,
                serde_json::from_str(&env_json).map_err(db_err)?,
                serde_json::from_str(&files_json).map_err(db_err)?,
            );
            summaries.push(RunSummary {
                run,
                project_name,
                branch,
                commit_sha,
                dirty: dirty != 0,
                coverage,
            });
            if let Some(limit) = filter.limit
                && summaries.len() >= limit
            {
                break;
            }
        }
        Ok(summaries)
    }
}

impl SqliteStore {
    fn coverage_for_run(&self, run_id: &RunId) -> StoreResult<CoverageSummary> {
        let mut stmt = self
            .conn
            .prepare("SELECT status_json FROM cells WHERE run_id = ?1")
            .map_err(db_err)?;
        let rows = stmt
            .query_map(params![run_id.as_str()], |r| r.get::<_, String>(0))
            .map_err(db_err)?;
        let mut summary = CoverageSummary::default();
        for row in rows {
            let status: CellStatus = serde_json::from_str(&row.map_err(db_err)?).map_err(db_err)?;
            summary.total += 1;
            summary.planned += 1;
            match status {
                CellStatus::Verified => {
                    summary.verified += 1;
                    summary.captured += 1;
                    summary.captured_renderable += 1;
                    summary.renderable += 1;
                }
                CellStatus::Unverified { ref reason } => {
                    summary.renderable += 1;
                    if reason.as_str() == "not-rendered" {
                        summary.not_captured += 1;
                    } else {
                        summary.unverified += 1;
                        summary.captured += 1;
                        summary.captured_renderable += 1;
                    }
                }
                CellStatus::Unrenderable { .. } => {
                    summary.unrenderable += 1;
                }
            }
        }
        Ok(summary)
    }
}

struct CacheRow {
    inputs_json: String,
    status_json: String,
    blob_id: Option<String>,
    ev_id: Option<String>,
    created_at_ms: i64,
    last_hit_at_ms: i64,
}

impl CacheRepository for SqliteStore {
    fn get_cache(&self, key: &CacheKey) -> StoreResult<Option<CacheEntry>> {
        let row = self
            .conn
            .query_row(
                "SELECT inputs_json, status_json, image_blob_id, evidence_id, created_at_ms, last_hit_at_ms
                 FROM cache_entries WHERE cache_key = ?1",
                params![key.as_str()],
                |r| {
                    Ok(CacheRow {
                        inputs_json: r.get(0)?,
                        status_json: r.get(1)?,
                        blob_id: r.get(2)?,
                        ev_id: r.get(3)?,
                        created_at_ms: r.get(4)?,
                        last_hit_at_ms: r.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(db_err)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let inputs: CacheInputs = serde_json::from_str(&row.inputs_json).map_err(db_err)?;
        let status: CellStatus = serde_json::from_str(&row.status_json).map_err(db_err)?;
        let mut entry = CacheEntry::new(
            inputs,
            status,
            row.blob_id.map(BlobId::parse).transpose().map_err(db_err)?,
            row.ev_id
                .map(EvidenceId::parse)
                .transpose()
                .map_err(db_err)?,
            row.created_at_ms,
        )
        .map_err(db_err)?;
        if row.last_hit_at_ms > row.created_at_ms {
            entry.record_hit(row.last_hit_at_ms).map_err(db_err)?;
        }
        Ok(Some(entry))
    }

    fn put_cache(&mut self, entry: &CacheEntry) -> StoreResult<()> {
        let inputs_json = serde_json::to_string(entry.inputs()).map_err(db_err)?;
        let status_json = serde_json::to_string(entry.status()).map_err(db_err)?;
        self.conn
            .execute(
                "INSERT INTO cache_entries (cache_key, inputs_json, status_json, image_blob_id, evidence_id, created_at_ms, last_hit_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(cache_key) DO UPDATE SET
                   status_json = excluded.status_json,
                   image_blob_id = excluded.image_blob_id,
                   evidence_id = excluded.evidence_id,
                   last_hit_at_ms = excluded.last_hit_at_ms",
                params![
                    entry.key().as_str(),
                    inputs_json,
                    status_json,
                    entry.image_blob().map(BlobId::as_str),
                    entry.evidence_id().map(EvidenceId::as_str),
                    entry.created_at_ms(),
                    entry.last_hit_at_ms(),
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }
}

struct CommentRow {
    pid: String,
    rid: String,
    target_json: String,
    author: String,
    body: String,
    status: String,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl CommentRepository for SqliteStore {
    fn upsert_comment(&mut self, comment: &Comment) -> StoreResult<()> {
        let target_json = serde_json::to_string(comment.target()).map_err(db_err)?;
        let status = match comment.status() {
            dui_core::model::CommentStatus::Open => "open",
            dui_core::model::CommentStatus::Resolved => "resolved",
        };
        self.conn
            .execute(
                "INSERT INTO comments (id, project_id, run_id, target_json, author, body, status, created_at_ms, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                   body = excluded.body,
                   status = excluded.status,
                   updated_at_ms = excluded.updated_at_ms",
                params![
                    comment.id().as_str(),
                    comment.project_id().as_str(),
                    comment.run_id().as_str(),
                    target_json,
                    comment.author(),
                    comment.body(),
                    status,
                    comment.created_at_ms(),
                    comment.updated_at_ms(),
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn get_comment(&self, id: &CommentId) -> StoreResult<Option<Comment>> {
        let row = self
            .conn
            .query_row(
                "SELECT project_id, run_id, target_json, author, body, status, created_at_ms, updated_at_ms
                 FROM comments WHERE id = ?1",
                params![id.as_str()],
                |r| {
                    Ok(CommentRow {
                        pid: r.get(0)?,
                        rid: r.get(1)?,
                        target_json: r.get(2)?,
                        author: r.get(3)?,
                        body: r.get(4)?,
                        status: r.get(5)?,
                        created_at_ms: r.get(6)?,
                        updated_at_ms: r.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(db_err)?;
        row.map(decode_comment).transpose()
    }

    fn list_comments(&self, run_id: &RunId) -> StoreResult<Vec<Comment>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT project_id, run_id, target_json, author, body, status, created_at_ms, updated_at_ms
                 FROM comments WHERE run_id = ?1 ORDER BY created_at_ms ASC, id ASC",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map(params![run_id.as_str()], |r| {
                Ok(CommentRow {
                    pid: r.get(0)?,
                    rid: r.get(1)?,
                    target_json: r.get(2)?,
                    author: r.get(3)?,
                    body: r.get(4)?,
                    status: r.get(5)?,
                    created_at_ms: r.get(6)?,
                    updated_at_ms: r.get(7)?,
                })
            })
            .map_err(db_err)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(decode_comment(row.map_err(db_err)?)?);
        }
        Ok(out)
    }
}

impl SqliteStore {
    pub fn list_comments_by_project(&self, project_id: &ProjectId) -> StoreResult<Vec<Comment>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT project_id, run_id, target_json, author, body, status, created_at_ms, updated_at_ms
                 FROM comments WHERE project_id = ?1 ORDER BY created_at_ms ASC, id ASC",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map(params![project_id.as_str()], |r| {
                Ok(CommentRow {
                    pid: r.get(0)?,
                    rid: r.get(1)?,
                    target_json: r.get(2)?,
                    author: r.get(3)?,
                    body: r.get(4)?,
                    status: r.get(5)?,
                    created_at_ms: r.get(6)?,
                    updated_at_ms: r.get(7)?,
                })
            })
            .map_err(db_err)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(decode_comment(row.map_err(db_err)?)?);
        }
        Ok(out)
    }

    pub fn list_all_comments(&self) -> StoreResult<Vec<Comment>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT project_id, run_id, target_json, author, body, status, created_at_ms, updated_at_ms
                 FROM comments ORDER BY created_at_ms ASC, id ASC",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(CommentRow {
                    pid: r.get(0)?,
                    rid: r.get(1)?,
                    target_json: r.get(2)?,
                    author: r.get(3)?,
                    body: r.get(4)?,
                    status: r.get(5)?,
                    created_at_ms: r.get(6)?,
                    updated_at_ms: r.get(7)?,
                })
            })
            .map_err(db_err)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(decode_comment(row.map_err(db_err)?)?);
        }
        Ok(out)
    }

    pub fn ensure_screen_record(&self, screen_id: &ScreenId, path: &str) -> StoreResult<()> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO screens (id, path, origin_json) VALUES (?1, ?2, ?3)",
                params![screen_id.as_str(), path, r#"{"kind":"entry"}"#],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn ensure_edge_record(
        &self,
        edge_id: &EdgeId,
        run_id: &RunId,
        from_screen_id: &ScreenId,
        to_screen_id: &ScreenId,
    ) -> StoreResult<()> {
        let from_json = serde_json::to_string(&EdgeEndpoint::Screen {
            screen_id: from_screen_id.clone(),
        })
        .map_err(db_err)?;
        let to_json = serde_json::to_string(&EdgeEndpoint::Screen {
            screen_id: to_screen_id.clone(),
        })
        .map_err(db_err)?;
        self.conn
            .execute(
                "INSERT OR IGNORE INTO edges (id, run_id, from_json, to_json, source, trigger, selector, observed_widths_json)
                 VALUES (?1, ?2, ?3, ?4, 'extracted', NULL, NULL, '[]')",
                params![edge_id.as_str(), run_id.as_str(), from_json, to_json],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn find_cell_id(
        &self,
        run_id: &RunId,
        path: &str,
        state: Option<&str>,
        width: u32,
    ) -> StoreResult<Option<CellId>> {
        let screen_id = Screen::derive_id(path);
        let id: Option<String> = match state {
            Some(st) => self
                .conn
                .query_row(
                    "SELECT id FROM cells WHERE run_id = ?1 AND screen_id = ?2 AND state_id = ?3 AND width_px = ?4 LIMIT 1",
                    params![run_id.as_str(), screen_id.as_str(), st, i64::from(width)],
                    |r| r.get(0),
                )
                .optional()
                .map_err(db_err)?,
            None => self
                .conn
                .query_row(
                    "SELECT id FROM cells WHERE run_id = ?1 AND screen_id = ?2 AND width_px = ?3 LIMIT 1",
                    params![run_id.as_str(), screen_id.as_str(), i64::from(width)],
                    |r| r.get(0),
                )
                .optional()
                .map_err(db_err)?,
        };
        id.map(|raw| CellId::parse(raw).map_err(db_err)).transpose()
    }

    pub fn describe_comment_target(&self, target: &dui_core::model::CommentTarget) -> Value {
        use dui_core::model::CommentTarget;
        match target {
            CommentTarget::Screen { screen_id } => {
                let path: Option<String> = self
                    .conn
                    .query_row(
                        "SELECT path FROM screens WHERE id = ?1",
                        params![screen_id.as_str()],
                        |r| r.get(0),
                    )
                    .optional()
                    .ok()
                    .flatten();
                json!({
                    "kind": "screen",
                    "screenId": screen_id.as_str(),
                    "path": path,
                })
            }
            CommentTarget::State { state_id } => {
                let row: Option<(String, String)> = self
                    .conn
                    .query_row(
                        "SELECT st.screen_id, sc.path FROM states st LEFT JOIN screens sc ON sc.id = st.screen_id WHERE st.id = ?1",
                        params![state_id.as_str()],
                        |r| Ok((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())),
                    )
                    .optional()
                    .ok()
                    .flatten();
                json!({
                    "kind": "state",
                    "stateId": state_id.as_str(),
                    "screenId": row.as_ref().map(|(s, _)| s),
                    "path": row.as_ref().map(|(_, p)| p).filter(|p| !p.is_empty()),
                })
            }
            CommentTarget::Cell { cell_id } => {
                let row: Option<(String, String, i64, Option<String>)> = self
                    .conn
                    .query_row(
                        "SELECT c.screen_id, c.state_id, c.width_px, sc.path FROM cells c LEFT JOIN screens sc ON sc.id = c.screen_id WHERE c.id = ?1",
                        params![cell_id.as_str()],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                    )
                    .optional()
                    .ok()
                    .flatten();
                json!({
                    "kind": "cell",
                    "cellId": cell_id.as_str(),
                    "screenId": row.as_ref().map(|(s, _, _, _)| s),
                    "stateId": row.as_ref().map(|(_, st, _, _)| st),
                    "width": row.as_ref().map(|(_, _, w, _)| *w),
                    "path": row.as_ref().and_then(|(_, _, _, p)| p.as_ref()),
                })
            }
            CommentTarget::Edge { edge_id } => {
                let row: Option<(String, String)> = self
                    .conn
                    .query_row(
                        "SELECT from_json, to_json FROM edges WHERE id = ?1",
                        params![edge_id.as_str()],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()
                    .ok()
                    .flatten();
                let mut from_path = None;
                let mut to_path = None;
                if let Some((fj, tj)) = row {
                    if let Ok(ep) = serde_json::from_str::<EdgeEndpoint>(&fj) {
                        from_path = self
                            .conn
                            .query_row(
                                "SELECT path FROM screens WHERE id = ?1",
                                params![ep.screen_id().as_str()],
                                |r| r.get::<_, String>(0),
                            )
                            .optional()
                            .ok()
                            .flatten();
                    }
                    if let Ok(ep) = serde_json::from_str::<EdgeEndpoint>(&tj) {
                        to_path = self
                            .conn
                            .query_row(
                                "SELECT path FROM screens WHERE id = ?1",
                                params![ep.screen_id().as_str()],
                                |r| r.get::<_, String>(0),
                            )
                            .optional()
                            .ok()
                            .flatten();
                    }
                }
                json!({
                    "kind": "transition",
                    "edgeId": edge_id.as_str(),
                    "path": from_path,
                    "to": to_path,
                })
            }
        }
    }
}

impl dui_ports::AppStore for SqliteStore {
    fn blob_path(&self, id: &BlobId) -> PathBuf {
        SqliteStore::blob_path(self, id)
    }

    fn link_or_copy_blob(&self, id: &BlobId, target: &Path) -> StoreResult<()> {
        SqliteStore::link_or_copy_blob(self, id, target)
    }

    fn find_previous_cache_inputs(
        &self,
        state_id: &StateId,
        width: Width,
        locale: Option<&str>,
    ) -> StoreResult<Option<CacheInputs>> {
        SqliteStore::find_previous_cache_inputs(self, state_id, width, locale)
    }

    fn import_run_dir(&mut self, run_dir: &Path) -> StoreResult<bool> {
        SqliteStore::import_run_dir(self, run_dir)
    }

    fn ensure_screen_record(&self, screen_id: &ScreenId, path: &str) -> StoreResult<()> {
        SqliteStore::ensure_screen_record(self, screen_id, path)
    }

    fn ensure_edge_record(
        &self,
        edge_id: &EdgeId,
        run_id: &RunId,
        from_screen_id: &ScreenId,
        to_screen_id: &ScreenId,
    ) -> StoreResult<()> {
        SqliteStore::ensure_edge_record(self, edge_id, run_id, from_screen_id, to_screen_id)
    }

    fn find_cell_id(
        &self,
        run_id: &RunId,
        path: &str,
        state: Option<&str>,
        width: u32,
    ) -> StoreResult<Option<CellId>> {
        SqliteStore::find_cell_id(self, run_id, path, state, width)
    }

    fn describe_comment_target(&self, target: &dui_core::model::CommentTarget) -> Value {
        SqliteStore::describe_comment_target(self, target)
    }

    fn list_comments_by_project(&self, project_id: &ProjectId) -> StoreResult<Vec<Comment>> {
        SqliteStore::list_comments_by_project(self, project_id)
    }

    fn list_all_comments(&self) -> StoreResult<Vec<Comment>> {
        SqliteStore::list_all_comments(self)
    }
}

fn decode_comment(row: CommentRow) -> StoreResult<Comment> {
    let mut comment = Comment::open(
        ProjectId::parse(row.pid).map_err(db_err)?,
        RunId::parse(row.rid).map_err(db_err)?,
        serde_json::from_str(&row.target_json).map_err(db_err)?,
        row.author,
        row.body,
        row.created_at_ms,
    )
    .map_err(db_err)?;
    if row.status == "resolved" {
        comment.resolve(row.updated_at_ms).map_err(db_err)?;
    } else if row.updated_at_ms > row.created_at_ms {
        comment.resolve(row.updated_at_ms).map_err(db_err)?;
        comment.reopen(row.updated_at_ms).map_err(db_err)?;
    }
    Ok(comment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn write_sample_disk_run(
        projects_dir: &Path,
        project_root: &str,
        project_name: &str,
        branch: Option<&str>,
        run_id: &str,
        ts: i64,
    ) -> PathBuf {
        let pid = ProjectId::from_primary_root_bytes(project_root.as_bytes());
        let project_dir = projects_dir.join(pid.as_str());
        let run_dir = project_dir.join("runs").join(run_id);
        fs::create_dir_all(&run_dir).unwrap();
        fs::write(
            project_dir.join("project.json"),
            json!({"name": project_name}).to_string(),
        )
        .unwrap();
        let mut png = vec![
            0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0, 0, 13,
        ];
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&1280u32.to_be_bytes());
        png.extend_from_slice(&720u32.to_be_bytes());
        png.extend_from_slice(run_id.as_bytes());
        fs::write(run_dir.join("screen.png"), &png).unwrap();
        fs::write(
            run_dir.join("snapshot.json"),
            json!({
                "id": run_id,
                "projectId": pid.as_str(),
                "projectName": project_name,
                "projectRoot": project_root,
                "worktree": project_root,
                "app": "",
                "branch": branch,
                "commit": "abcdef123456",
                "dirty": false,
                "timestampMs": ts
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            run_dir.join("report.json"),
            json!({
                "os": "macos",
                "compiler": "7.0.2",
                "cells": [{
                    "path": "/",
                    "scenario": "default",
                    "state": "state-0",
                    "width": 1280,
                    "planIndex": 0,
                    "status": "verified",
                    "image": "screen.png",
                    "observation": {"path": "/", "width": 1280, "ready": true, "settled": true, "slots": {}, "hits": []}
                }]
            })
            .to_string(),
        )
        .unwrap();
        run_dir
    }

    #[test]
    fn fresh_install_migrates_once_and_second_start_applies_none() {
        let tmp = tempfile::tempdir().unwrap();
        let (_first, applied_first) = SqliteStore::open(tmp.path()).unwrap();
        assert_eq!(applied_first, vec!["0001_initial", "0002_run_branch"]);
        let (_second, applied_second) = SqliteStore::open(tmp.path()).unwrap();
        assert!(applied_second.is_empty());
    }

    #[test]
    fn imports_disk_history_and_restores_losslessly_after_database_deletion() {
        let tmp = tempfile::tempdir().unwrap();
        let projects = tmp.path().join("projects");
        write_sample_disk_run(
            &projects,
            "/repo/alpha",
            "alpha",
            Some("main"),
            "0001-a",
            1000,
        );
        write_sample_disk_run(
            &projects,
            "/repo/alpha",
            "alpha",
            Some("feat/ui"),
            "0002-b",
            2000,
        );

        let (mut store, _) = SqliteStore::open(tmp.path()).unwrap();
        assert_eq!(store.import_disk_archive(&projects).unwrap(), 2);
        assert_eq!(store.import_disk_archive(&projects).unwrap(), 0);
        let before = store.list_runs(&RunFilter::default()).unwrap();
        assert_eq!(before.len(), 2);
        assert_eq!(before[0].project_name, "alpha");
        assert_eq!(before[0].branch.as_deref(), Some("feat/ui"));
        drop(store);

        // Delete the SQLite database files and verify full restoration from disk files.
        for file in ["dui.db", "dui.db-wal", "dui.db-shm"] {
            let _ = fs::remove_file(tmp.path().join(file));
        }
        let (mut restored, applied) = SqliteStore::open(tmp.path()).unwrap();
        assert_eq!(applied, vec!["0001_initial", "0002_run_branch"]);
        assert_eq!(restored.import_disk_archive(&projects).unwrap(), 2);
        let after = restored.list_runs(&RunFilter::default()).unwrap();
        assert_eq!(after, before);
    }

    #[test]
    fn concurrent_runs_do_not_corrupt_sqlite_database() {
        let tmp = tempfile::tempdir().unwrap();
        let projects = tmp.path().join("projects");
        let dir_a = write_sample_disk_run(
            &projects,
            "/repo/conc",
            "conc",
            Some("main"),
            "0001-t1",
            1000,
        );
        let dir_b = write_sample_disk_run(
            &projects,
            "/repo/conc",
            "conc",
            Some("feat"),
            "0002-t2",
            2000,
        );
        let home_a = tmp.path().to_path_buf();
        let home_b = tmp.path().to_path_buf();

        // Pre-initialize schema once, then write concurrently from two threads.
        let _ = SqliteStore::open(tmp.path()).unwrap();

        let t1 = thread::spawn(move || {
            let (mut s, _) = SqliteStore::open(&home_a).unwrap();
            s.import_run_dir(&dir_a).unwrap()
        });
        let t2 = thread::spawn(move || {
            let (mut s, _) = SqliteStore::open(&home_b).unwrap();
            s.import_run_dir(&dir_b).unwrap()
        });
        assert!(t1.join().unwrap());
        assert!(t2.join().unwrap());

        let (store, _) = SqliteStore::open(tmp.path()).unwrap();
        let runs = store.list_runs(&RunFilter::default()).unwrap();
        assert_eq!(runs.len(), 2);
        assert!(
            store
                .load_run(&RunId::parse("0001-t1").unwrap())
                .unwrap()
                .is_some()
        );
        assert!(
            store
                .load_run(&RunId::parse("0002-t2").unwrap())
                .unwrap()
                .is_some()
        );
    }
}
