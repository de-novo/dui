//! Port traits and boundary contracts for `dui` (`TypeOracle`, `Host`, `Renderer`,
//! `SurfaceAdapter` / `SurfaceRegistry`, and `AppStore`).
//!
//! `dui-ports` depends only on `dui-core` and holds zero concrete adapter implementations.

use dui_core::{
    model::{
        BlobId, CacheInputs, CellId, Comment, CommentTarget, EdgeId, ProjectId, RunId, ScreenId,
        StateId, Width,
    },
    storage::{
        ArtifactStore, CacheRepository, CommentRepository, RunRepository, StoreResult,
        WorkspaceRepository,
    },
    surface::{SurfaceDescriptor, UnrenderableReason, resolve_surface_id},
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub type Reply = Result<Value, (u8, Value)>;

pub fn failure(code: u8, reason: &str) -> (u8, Value) {
    (code, json!({ "status": "error", "reason": reason }))
}

pub trait TypeOracle {
    fn facts(&self, project: &Path, spec: &Path) -> Reply;
}

pub struct CaptureBatchJob<'a> {
    pub spec: &'a Value,
    pub input: &'a Value,
    pub width: u32,
    pub image: &'a Path,
}

pub trait Renderer {
    fn capture(
        &mut self,
        spec: &Value,
        input: &Value,
        width: u32,
        image: &Path,
    ) -> Result<Value, Box<dyn std::error::Error>>;

    fn capture_batch(&mut self, jobs: &[CaptureBatchJob<'_>]) -> Vec<Result<Value, String>> {
        jobs.iter()
            .map(|job| {
                self.capture(job.spec, job.input, job.width, job.image)
                    .map_err(|e| e.to_string())
            })
            .collect()
    }
}

pub trait Host {
    type Session;
    fn start(
        &self,
        project: &Path,
        manifest: &Path,
        facts: &Value,
    ) -> Result<Self::Session, (u8, Value)>;
}

/// Level 1 universal oracle requiring no compiler toolchain.
pub struct NoopOracle;

impl TypeOracle for NoopOracle {
    fn facts(&self, _project: &Path, spec: &Path) -> Reply {
        let bytes = fs::read(spec).map_err(|e| failure(2, &e.to_string()))?;
        let manifest: Value =
            serde_json::from_slice(&bytes).map_err(|e| failure(2, &e.to_string()))?;
        let path = manifest.get("path").and_then(Value::as_str).unwrap_or("/");
        Ok(json!({
            "status": "ok",
            "tier": "universal",
            "compiler": "none",
            "path": path,
            "diagnostics": [],
            "facts": [],
            "autoMocks": {},
            "files": []
        }))
    }
}

pub trait SurfaceAdapter {
    fn descriptor(&self) -> SurfaceDescriptor;
    fn oracle(&self) -> &dyn TypeOracle {
        &NoopOracle
    }
    fn check_env(&self) -> Result<(), UnrenderableReason> {
        Ok(())
    }
    fn run(&self, project: &Path, spec: &Path, out: &Path, force: bool) -> Reply;
}

pub fn unrenderable_reply(surface_id: &str, reason: &UnrenderableReason) -> Value {
    json!({
        "status": "unrenderable",
        "surface": surface_id,
        "reason": reason.code,
        "detail": reason.detail,
        "verified": 0,
        "attempted": 0
    })
}

pub struct SurfaceRegistry {
    adapters: Vec<Box<dyn SurfaceAdapter>>,
}

impl Default for SurfaceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SurfaceRegistry {
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    pub fn register(&mut self, adapter: Box<dyn SurfaceAdapter>) {
        self.adapters.push(adapter);
    }

    pub fn descriptors(&self) -> Vec<SurfaceDescriptor> {
        self.adapters.iter().map(|a| a.descriptor()).collect()
    }

    pub fn find(&self, surface_id: &str) -> Option<&dyn SurfaceAdapter> {
        self.adapters
            .iter()
            .find(|a| a.descriptor().id == surface_id)
            .map(Box::as_ref)
    }

    pub fn dispatch(&self, project: &Path, spec: &Path, out: &Path, force: bool) -> Reply {
        let manifest: Value = match fs::read(spec)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        {
            Some(v) => v,
            None => Value::Null,
        };
        let surface_id = resolve_surface_id(&manifest);
        let Some(adapter) = self.find(surface_id) else {
            let reason = UnrenderableReason::new(
                "unsupported-surface",
                format!("no surface adapter registered for `{surface_id}`"),
            );
            return Ok(unrenderable_reply(surface_id, &reason));
        };
        if let Err(reason) = adapter.check_env() {
            return Ok(unrenderable_reply(surface_id, &reason));
        }
        adapter.run(project, spec, out, force)
    }
}

/// Application-level storage port extending the five `dui-core::storage` repositories with
/// run-archive import and blob-materialization operations needed by `dui-app` use cases.
pub trait AppStore:
    WorkspaceRepository + RunRepository + ArtifactStore + CacheRepository + CommentRepository
{
    fn blob_path(&self, id: &BlobId) -> PathBuf;
    fn link_or_copy_blob(&self, id: &BlobId, target: &Path) -> StoreResult<()>;
    fn find_previous_cache_inputs(
        &self,
        state_id: &StateId,
        width: Width,
        locale: Option<&str>,
    ) -> StoreResult<Option<CacheInputs>>;
    fn import_run_dir(&mut self, run_dir: &Path) -> StoreResult<bool>;
    fn ensure_screen_record(&self, screen_id: &ScreenId, path: &str) -> StoreResult<()>;
    fn ensure_edge_record(
        &self,
        edge_id: &EdgeId,
        run_id: &RunId,
        from_screen_id: &ScreenId,
        to_screen_id: &ScreenId,
    ) -> StoreResult<()>;
    fn find_cell_id(
        &self,
        run_id: &RunId,
        path: &str,
        state: Option<&str>,
        width: u32,
    ) -> StoreResult<Option<CellId>>;
    fn describe_comment_target(&self, target: &CommentTarget) -> Value;
    fn list_comments_by_project(&self, project_id: &ProjectId) -> StoreResult<Vec<Comment>>;
    fn list_all_comments(&self) -> StoreResult<Vec<Comment>>;
}
