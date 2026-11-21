//! Thin storage port called by the application shell. Concrete backends
//! (file archives, SQLite via rusqlite/Drizzle, or remote APIs) implement `Store`
//! outside `dui-core`.

use crate::model::{
    BlobId, CacheEntry, CacheKey, Cell, Comment, CommentId, Commit, CommitId, CoverageSummary,
    Edge, Evidence, EvidenceId, ImageBlob, Project, ProjectId, Run, RunId, Screen, State, Worktree,
    WorktreeId,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StoreError {
    NotFound(String),
    Conflict(String),
    Backend(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
            Self::Conflict(msg) => write!(f, "conflict: {msg}"),
            Self::Backend(msg) => write!(f, "storage backend error: {msg}"),
        }
    }
}

impl std::error::Error for StoreError {}

pub type StoreResult<T> = Result<T, StoreError>;

/// Immutable snapshot of one run and all entities it owns or references.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunBundle {
    pub project: Project,
    pub worktree: Worktree,
    pub commit: Commit,
    pub run: Run,
    pub screens: Vec<Screen>,
    pub states: Vec<State>,
    pub cells: Vec<Cell>,
    pub edges: Vec<Edge>,
    pub evidence: Vec<Evidence>,
    pub blobs: Vec<ImageBlob>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunFilter {
    pub project_id: Option<ProjectId>,
    pub worktree_id: Option<WorktreeId>,
    pub branch: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub run: Run,
    pub project_name: String,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub dirty: bool,
    pub coverage: CoverageSummary,
}

/// Persistence boundary for dui metadata, content-addressed artifacts, cache entries,
/// and review comments. Implementations live in adapter crates, never in `dui-core`.
pub trait Store {
    fn upsert_project(&mut self, project: &Project) -> StoreResult<()>;
    fn upsert_worktree(&mut self, worktree: &Worktree) -> StoreResult<()>;
    fn put_commit(&mut self, commit: &Commit) -> StoreResult<()>;
    fn put_blob(&mut self, blob: &ImageBlob, bytes: &[u8]) -> StoreResult<()>;
    fn get_blob(&self, id: &BlobId) -> StoreResult<Option<(ImageBlob, Vec<u8>)>>;
    fn put_evidence(&mut self, evidence: &Evidence) -> StoreResult<()>;
    fn get_evidence(&self, id: &EvidenceId) -> StoreResult<Option<Evidence>>;
    fn save_run(&mut self, bundle: &RunBundle) -> StoreResult<()>;
    fn load_run(&self, id: &RunId) -> StoreResult<Option<RunBundle>>;
    fn list_runs(&self, filter: &RunFilter) -> StoreResult<Vec<RunSummary>>;
    fn get_cache(&self, key: &CacheKey) -> StoreResult<Option<CacheEntry>>;
    fn put_cache(&mut self, entry: &CacheEntry) -> StoreResult<()>;
    fn upsert_comment(&mut self, comment: &Comment) -> StoreResult<()>;
    fn list_comments(&self, run_id: &RunId) -> StoreResult<Vec<Comment>>;
    fn unreferenced_blobs(&self) -> StoreResult<Vec<BlobId>>;
    fn delete_blob(&mut self, id: &BlobId) -> StoreResult<()>;
}

/// Deterministic in-memory `Store` for core and shell tests (zero I/O, `BTreeMap` only).
#[derive(Clone, Debug, Default)]
pub struct MemoryStore {
    projects: BTreeMap<ProjectId, Project>,
    worktrees: BTreeMap<WorktreeId, Worktree>,
    commits: BTreeMap<CommitId, Commit>,
    blobs: BTreeMap<BlobId, (ImageBlob, Vec<u8>)>,
    evidence: BTreeMap<EvidenceId, Evidence>,
    runs: BTreeMap<RunId, RunBundle>,
    cache: BTreeMap<CacheKey, CacheEntry>,
    comments: BTreeMap<CommentId, Comment>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Store for MemoryStore {
    fn upsert_project(&mut self, project: &Project) -> StoreResult<()> {
        self.projects.insert(project.id.clone(), project.clone());
        Ok(())
    }

    fn upsert_worktree(&mut self, worktree: &Worktree) -> StoreResult<()> {
        self.worktrees.insert(worktree.id.clone(), worktree.clone());
        Ok(())
    }

    fn put_commit(&mut self, commit: &Commit) -> StoreResult<()> {
        self.commits
            .entry(commit.id.clone())
            .or_insert_with(|| commit.clone());
        Ok(())
    }

    fn put_blob(&mut self, blob: &ImageBlob, bytes: &[u8]) -> StoreResult<()> {
        if BlobId::from_image_bytes(bytes) != blob.id {
            return Err(StoreError::Conflict(
                "blob bytes do not match content hash".into(),
            ));
        }
        self.blobs
            .entry(blob.id.clone())
            .or_insert_with(|| (blob.clone(), bytes.to_vec()));
        Ok(())
    }

    fn get_blob(&self, id: &BlobId) -> StoreResult<Option<(ImageBlob, Vec<u8>)>> {
        Ok(self.blobs.get(id).cloned())
    }

    fn put_evidence(&mut self, evidence: &Evidence) -> StoreResult<()> {
        self.evidence
            .entry(evidence.id.clone())
            .or_insert_with(|| evidence.clone());
        Ok(())
    }

    fn get_evidence(&self, id: &EvidenceId) -> StoreResult<Option<Evidence>> {
        Ok(self.evidence.get(id).cloned())
    }

    fn save_run(&mut self, bundle: &RunBundle) -> StoreResult<()> {
        if self.runs.contains_key(&bundle.run.id) {
            return Err(StoreError::Conflict(format!(
                "run {} already exists",
                bundle.run.id
            )));
        }
        self.upsert_project(&bundle.project)?;
        self.upsert_worktree(&bundle.worktree)?;
        self.put_commit(&bundle.commit)?;
        for item in &bundle.evidence {
            self.put_evidence(item)?;
        }
        self.runs.insert(bundle.run.id.clone(), bundle.clone());
        Ok(())
    }

    fn load_run(&self, id: &RunId) -> StoreResult<Option<RunBundle>> {
        Ok(self.runs.get(id).cloned())
    }

    fn list_runs(&self, filter: &RunFilter) -> StoreResult<Vec<RunSummary>> {
        let mut summaries: Vec<RunSummary> = self
            .runs
            .values()
            .filter(|b| {
                filter
                    .project_id
                    .as_ref()
                    .is_none_or(|p| &b.run.project_id == p)
                    && filter
                        .worktree_id
                        .as_ref()
                        .is_none_or(|w| &b.run.worktree_id == w)
                    && filter
                        .branch
                        .as_ref()
                        .is_none_or(|br| b.worktree.branch.as_deref() == Some(br.as_str()))
            })
            .map(|b| RunSummary {
                run: b.run.clone(),
                project_name: self.projects.get(&b.run.project_id).map_or_else(
                    || b.project.display_name.clone(),
                    |p| p.display_name.clone(),
                ),
                branch: b.worktree.branch.clone(),
                commit_sha: b.commit.git_sha.clone(),
                dirty: b.commit.dirty,
                coverage: CoverageSummary::from_cells(&b.cells),
            })
            .collect();
        summaries.sort_by(|a, b| {
            b.run
                .created_at_ms
                .cmp(&a.run.created_at_ms)
                .then_with(|| b.run.id.cmp(&a.run.id))
        });
        if let Some(limit) = filter.limit {
            summaries.truncate(limit);
        }
        Ok(summaries)
    }

    fn get_cache(&self, key: &CacheKey) -> StoreResult<Option<CacheEntry>> {
        Ok(self.cache.get(key).cloned())
    }

    fn put_cache(&mut self, entry: &CacheEntry) -> StoreResult<()> {
        self.cache.insert(entry.key.clone(), entry.clone());
        Ok(())
    }

    fn upsert_comment(&mut self, comment: &Comment) -> StoreResult<()> {
        self.comments.insert(comment.id.clone(), comment.clone());
        Ok(())
    }

    fn list_comments(&self, run_id: &RunId) -> StoreResult<Vec<Comment>> {
        let mut items: Vec<Comment> = self
            .comments
            .values()
            .filter(|c| &c.run_id == run_id)
            .cloned()
            .collect();
        items.sort_by(|a, b| {
            a.created_at_ms
                .cmp(&b.created_at_ms)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(items)
    }

    fn unreferenced_blobs(&self) -> StoreResult<Vec<BlobId>> {
        let mut referenced = BTreeSet::new();
        for bundle in self.runs.values() {
            for cell in &bundle.cells {
                if let Some(blob) = &cell.image_blob {
                    referenced.insert(blob.clone());
                }
            }
        }
        for entry in self.cache.values() {
            if let Some(blob) = &entry.image_blob {
                referenced.insert(blob.clone());
            }
        }
        Ok(self
            .blobs
            .keys()
            .filter(|id| !referenced.contains(*id))
            .cloned()
            .collect())
    }

    fn delete_blob(&mut self, id: &BlobId) -> StoreResult<()> {
        self.blobs.remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        CacheInputs, CellStatus, CommentStatus, CommentTarget, ContentHash, EdgeEndpoint,
        EdgeSource, RunEnvironment, RunKind, ScreenOrigin, Width,
    };

    #[test]
    fn memory_store_covers_runs_blobs_cache_comments_and_gc() {
        let mut store = MemoryStore::new();
        let project = Project {
            id: ProjectId::from_primary_root_bytes(b"/repo/demo"),
            display_name: "demo".into(),
            primary_root: "/repo/demo".into(),
        };
        let worktree = Worktree {
            id: Worktree::derive_id(&project.id, "/repo/demo", ""),
            project_id: project.id.clone(),
            path: "/repo/demo".into(),
            app_relative: String::new(),
            branch: Some("main".into()),
        };
        let commit = Commit {
            id: Commit::derive_id(&project.id, Some("abcdef1"), false, None),
            project_id: project.id.clone(),
            git_sha: Some("abcdef1".into()),
            dirty: false,
            tree_digest: None,
        };
        let screen = Screen {
            id: Screen::derive_id("/"),
            path: "/".into(),
            origin: ScreenOrigin::Entry,
        };
        let state = State {
            id: State::derive_id(&screen.id, "default", None, &BTreeMap::new()),
            screen_id: screen.id.clone(),
            scenario: "default".into(),
            locale: None,
            inputs: BTreeMap::new(),
            plan_index: 0,
        };
        let run_id = RunId::parse("0001-run").unwrap();
        let width = Width::new(1280).unwrap();
        let bytes = b"fake-png-bytes";
        let blob = ImageBlob {
            id: BlobId::from_image_bytes(bytes),
            media_type: "image/png".into(),
            byte_len: bytes.len() as u64,
            width_px: 1280,
            height_px: 720,
        };
        store.put_blob(&blob, bytes).unwrap();
        let orphan_bytes = b"orphan-png";
        let orphan = ImageBlob {
            id: BlobId::from_image_bytes(orphan_bytes),
            media_type: "image/png".into(),
            byte_len: orphan_bytes.len() as u64,
            width_px: 375,
            height_px: 667,
        };
        store.put_blob(&orphan, orphan_bytes).unwrap();

        let evidence = Evidence::new(
            Some("/".into()),
            Some(1280),
            Some(true),
            Some(true),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeMap::new(),
            vec![],
            vec![],
        );
        let cell = Cell {
            id: Cell::derive_id(&run_id, &screen.id, &state.id, width, None),
            run_id: run_id.clone(),
            screen_id: screen.id.clone(),
            state_id: state.id.clone(),
            width,
            locale: None,
            status: CellStatus::Verified,
            image_blob: Some(blob.id.clone()),
            evidence_id: Some(evidence.id.clone()),
            cache: None,
        };
        let edge = Edge {
            id: Edge::derive_id(
                &run_id,
                &EdgeEndpoint::Screen {
                    screen_id: screen.id.clone(),
                },
                &EdgeEndpoint::Screen {
                    screen_id: Screen::derive_id("/search"),
                },
                EdgeSource::Extracted,
                None,
            ),
            run_id: run_id.clone(),
            from: EdgeEndpoint::Screen {
                screen_id: screen.id.clone(),
            },
            to: EdgeEndpoint::Screen {
                screen_id: Screen::derive_id("/search"),
            },
            source: EdgeSource::Extracted,
            trigger: None,
            selector: Some("header a[href]".into()),
            observed_widths: BTreeSet::from([width]),
        };
        let bundle = RunBundle {
            project: project.clone(),
            worktree,
            commit,
            run: Run {
                id: run_id.clone(),
                project_id: project.id.clone(),
                worktree_id: Worktree::derive_id(&project.id, "/repo/demo", ""),
                commit_id: Commit::derive_id(&project.id, Some("abcdef1"), false, None),
                kind: RunKind::Capture,
                created_at_ms: 1_795_200_000_000,
                environment: RunEnvironment {
                    os: "macos".into(),
                    compiler: Some("7.0.2".into()),
                    renderer: Some("Chrome/157".into()),
                    dui_version: "0.0.2".into(),
                    versions: BTreeMap::new(),
                },
                source_files: BTreeSet::from(["src/App.tsx".into()]),
            },
            screens: vec![screen],
            states: vec![state.clone()],
            cells: vec![cell.clone()],
            edges: vec![edge],
            evidence: vec![evidence.clone()],
            blobs: vec![blob.clone()],
        };
        store.save_run(&bundle).unwrap();
        assert!(store.save_run(&bundle).is_err());
        assert_eq!(store.load_run(&run_id).unwrap(), Some(bundle));
        assert_eq!(store.list_runs(&RunFilter::default()).unwrap().len(), 1);
        assert_eq!(store.unreferenced_blobs().unwrap(), vec![orphan.id.clone()]);
        store.delete_blob(&orphan.id).unwrap();
        assert!(store.unreferenced_blobs().unwrap().is_empty());

        let inputs = CacheInputs {
            source_files: BTreeMap::new(),
            types_digest: ContentHash::from_bytes(b"t"),
            mocks_digest: ContentHash::from_bytes(b"m"),
            state_id: state.id,
            width,
            locale: None,
            dui_version: "0.0.2".into(),
            rule_pack_digest: ContentHash::from_bytes(b"r"),
            renderer_env_digest: ContentHash::from_bytes(b"e"),
        };
        let entry = CacheEntry {
            key: inputs.key(),
            inputs,
            status: CellStatus::Verified,
            image_blob: Some(blob.id),
            evidence_id: Some(evidence.id),
            created_at_ms: 100,
            last_hit_at_ms: 100,
        };
        store.put_cache(&entry).unwrap();
        assert_eq!(store.get_cache(&entry.key).unwrap(), Some(entry));

        let comment = Comment {
            id: Comment::derive_id(
                &run_id,
                &CommentTarget::Cell {
                    cell_id: cell.id.clone(),
                },
                "reviewer",
                200,
                "Check contrast",
            ),
            project_id: project.id,
            run_id: run_id.clone(),
            target: CommentTarget::Cell { cell_id: cell.id },
            author: "reviewer".into(),
            body: "Check contrast".into(),
            status: CommentStatus::Open,
            created_at_ms: 200,
            updated_at_ms: 200,
        };
        store.upsert_comment(&comment).unwrap();
        assert_eq!(store.list_comments(&run_id).unwrap(), vec![comment]);
    }
}
