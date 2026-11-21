//! Context-scoped repository and artifact ports behind which concrete storage
//! adapters (file archives, SQLite via rusqlite/Drizzle, or remote APIs) sit.

use crate::model::{
    BlobId, CacheEntry, CacheKey, Comment, CommentId, Commit, CommitId, CoverageSummary, Evidence,
    EvidenceId, ImageBlob, Project, ProjectId, Run, RunAggregate, RunId, Worktree, WorktreeId,
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

/// Repository for the Workspace bounded context (`Project`, `Worktree`, `Commit`).
pub trait WorkspaceRepository {
    fn upsert_project(&mut self, project: &Project) -> StoreResult<()>;
    fn get_project(&self, id: &ProjectId) -> StoreResult<Option<Project>>;
    fn upsert_worktree(&mut self, worktree: &Worktree) -> StoreResult<()>;
    fn get_worktree(&self, id: &WorktreeId) -> StoreResult<Option<Worktree>>;
    fn put_commit(&mut self, commit: &Commit) -> StoreResult<()>;
    fn get_commit(&self, id: &CommitId) -> StoreResult<Option<Commit>>;
}

/// Repository for the Capture bounded context (`RunAggregate` root).
pub trait RunRepository {
    fn save_run(&mut self, aggregate: &RunAggregate) -> StoreResult<()>;
    fn load_run(&self, id: &RunId) -> StoreResult<Option<RunAggregate>>;
    fn list_runs(&self, filter: &RunFilter) -> StoreResult<Vec<RunSummary>>;
}

/// Content-addressed artifact store for immutable `ImageBlob`s and `Evidence`.
pub trait ArtifactStore {
    fn put_blob(&mut self, blob: &ImageBlob, bytes: &[u8]) -> StoreResult<()>;
    fn get_blob(&self, id: &BlobId) -> StoreResult<Option<(ImageBlob, Vec<u8>)>>;
    fn put_evidence(&mut self, evidence: &Evidence) -> StoreResult<()>;
    fn get_evidence(&self, id: &EvidenceId) -> StoreResult<Option<Evidence>>;
    fn unreferenced_blobs(&self) -> StoreResult<Vec<BlobId>>;
    fn delete_blob(&mut self, id: &BlobId) -> StoreResult<()>;
}

/// Repository for the Build Cache bounded context (`CacheEntry` aggregate root).
pub trait CacheRepository {
    fn get_cache(&self, key: &CacheKey) -> StoreResult<Option<CacheEntry>>;
    fn put_cache(&mut self, entry: &CacheEntry) -> StoreResult<()>;
}

/// Repository for the Review bounded context (`Comment` aggregate root).
pub trait CommentRepository {
    fn upsert_comment(&mut self, comment: &Comment) -> StoreResult<()>;
    fn get_comment(&self, id: &CommentId) -> StoreResult<Option<Comment>>;
    fn list_comments(&self, run_id: &RunId) -> StoreResult<Vec<Comment>>;
}

/// Convenience supertrait for full storage adapters implementing all five ports.
pub trait Store:
    WorkspaceRepository + RunRepository + ArtifactStore + CacheRepository + CommentRepository
{
}

impl<T> Store for T where
    T: WorkspaceRepository + RunRepository + ArtifactStore + CacheRepository + CommentRepository
{
}

/// Deterministic in-memory implementation of all five ports for pure tests (`BTreeMap` only).
#[derive(Clone, Debug, Default)]
pub struct MemoryStore {
    projects: BTreeMap<ProjectId, Project>,
    worktrees: BTreeMap<WorktreeId, Worktree>,
    commits: BTreeMap<CommitId, Commit>,
    blobs: BTreeMap<BlobId, (ImageBlob, Vec<u8>)>,
    evidence: BTreeMap<EvidenceId, Evidence>,
    runs: BTreeMap<RunId, RunAggregate>,
    cache: BTreeMap<CacheKey, CacheEntry>,
    comments: BTreeMap<CommentId, Comment>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl WorkspaceRepository for MemoryStore {
    fn upsert_project(&mut self, project: &Project) -> StoreResult<()> {
        self.projects.insert(project.id().clone(), project.clone());
        Ok(())
    }

    fn get_project(&self, id: &ProjectId) -> StoreResult<Option<Project>> {
        Ok(self.projects.get(id).cloned())
    }

    fn upsert_worktree(&mut self, worktree: &Worktree) -> StoreResult<()> {
        self.worktrees
            .insert(worktree.id().clone(), worktree.clone());
        Ok(())
    }

    fn get_worktree(&self, id: &WorktreeId) -> StoreResult<Option<Worktree>> {
        Ok(self.worktrees.get(id).cloned())
    }

    fn put_commit(&mut self, commit: &Commit) -> StoreResult<()> {
        self.commits
            .entry(commit.id().clone())
            .or_insert_with(|| commit.clone());
        Ok(())
    }

    fn get_commit(&self, id: &CommitId) -> StoreResult<Option<Commit>> {
        Ok(self.commits.get(id).cloned())
    }
}

impl RunRepository for MemoryStore {
    fn save_run(&mut self, aggregate: &RunAggregate) -> StoreResult<()> {
        let run = aggregate.run();
        if self.runs.contains_key(run.id()) {
            return Err(StoreError::Conflict(format!(
                "run {} already exists",
                run.id()
            )));
        }
        if !self.projects.contains_key(run.project_id()) {
            return Err(StoreError::NotFound(format!(
                "project {}",
                run.project_id()
            )));
        }
        if !self.worktrees.contains_key(run.worktree_id()) {
            return Err(StoreError::NotFound(format!(
                "worktree {}",
                run.worktree_id()
            )));
        }
        if !self.commits.contains_key(run.commit_id()) {
            return Err(StoreError::NotFound(format!("commit {}", run.commit_id())));
        }
        self.runs.insert(run.id().clone(), aggregate.clone());
        Ok(())
    }

    fn load_run(&self, id: &RunId) -> StoreResult<Option<RunAggregate>> {
        Ok(self.runs.get(id).cloned())
    }

    fn list_runs(&self, filter: &RunFilter) -> StoreResult<Vec<RunSummary>> {
        let mut summaries: Vec<RunSummary> = self
            .runs
            .values()
            .filter(|agg| {
                let run = agg.run();
                let wt = self.worktrees.get(run.worktree_id());
                filter
                    .project_id
                    .as_ref()
                    .is_none_or(|p| run.project_id() == p)
                    && filter
                        .worktree_id
                        .as_ref()
                        .is_none_or(|w| run.worktree_id() == w)
                    && filter
                        .branch
                        .as_ref()
                        .is_none_or(|br| wt.and_then(Worktree::branch) == Some(br.as_str()))
            })
            .map(|agg| {
                let run = agg.run();
                let project_name = self
                    .projects
                    .get(run.project_id())
                    .map_or_else(String::new, |p| p.display_name().to_owned());
                let branch = self
                    .worktrees
                    .get(run.worktree_id())
                    .and_then(Worktree::branch)
                    .map(str::to_owned);
                let commit = self.commits.get(run.commit_id());
                RunSummary {
                    run: run.clone(),
                    project_name,
                    branch,
                    commit_sha: commit.and_then(Commit::git_sha).map(str::to_owned),
                    dirty: commit.is_some_and(Commit::is_dirty),
                    coverage: agg.coverage(),
                }
            })
            .collect();
        summaries.sort_by(|a, b| {
            b.run
                .created_at_ms()
                .cmp(&a.run.created_at_ms())
                .then_with(|| b.run.id().cmp(a.run.id()))
        });
        if let Some(limit) = filter.limit {
            summaries.truncate(limit);
        }
        Ok(summaries)
    }
}

impl ArtifactStore for MemoryStore {
    fn put_blob(&mut self, blob: &ImageBlob, bytes: &[u8]) -> StoreResult<()> {
        if BlobId::from_image_bytes(bytes) != *blob.id() {
            return Err(StoreError::Conflict(
                "blob bytes do not match content hash".into(),
            ));
        }
        self.blobs
            .entry(blob.id().clone())
            .or_insert_with(|| (blob.clone(), bytes.to_vec()));
        Ok(())
    }

    fn get_blob(&self, id: &BlobId) -> StoreResult<Option<(ImageBlob, Vec<u8>)>> {
        Ok(self.blobs.get(id).cloned())
    }

    fn put_evidence(&mut self, evidence: &Evidence) -> StoreResult<()> {
        self.evidence
            .entry(evidence.id().clone())
            .or_insert_with(|| evidence.clone());
        Ok(())
    }

    fn get_evidence(&self, id: &EvidenceId) -> StoreResult<Option<Evidence>> {
        Ok(self.evidence.get(id).cloned())
    }

    fn unreferenced_blobs(&self) -> StoreResult<Vec<BlobId>> {
        let mut referenced = BTreeSet::new();
        for agg in self.runs.values() {
            for cell in agg.cells() {
                if let Some(blob) = cell.image_blob() {
                    referenced.insert(blob.clone());
                }
            }
        }
        for entry in self.cache.values() {
            if let Some(blob) = entry.image_blob() {
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

impl CacheRepository for MemoryStore {
    fn get_cache(&self, key: &CacheKey) -> StoreResult<Option<CacheEntry>> {
        Ok(self.cache.get(key).cloned())
    }

    fn put_cache(&mut self, entry: &CacheEntry) -> StoreResult<()> {
        self.cache.insert(entry.key().clone(), entry.clone());
        Ok(())
    }
}

impl CommentRepository for MemoryStore {
    fn upsert_comment(&mut self, comment: &Comment) -> StoreResult<()> {
        self.comments.insert(comment.id().clone(), comment.clone());
        Ok(())
    }

    fn get_comment(&self, id: &CommentId) -> StoreResult<Option<Comment>> {
        Ok(self.comments.get(id).cloned())
    }

    fn list_comments(&self, run_id: &RunId) -> StoreResult<Vec<Comment>> {
        let mut items: Vec<Comment> = self
            .comments
            .values()
            .filter(|c| c.run_id() == run_id)
            .cloned()
            .collect();
        items.sort_by(|a, b| {
            a.created_at_ms()
                .cmp(&b.created_at_ms())
                .then_with(|| a.id().cmp(b.id()))
        });
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        CacheInputs, Cell, CellStatus, CommentTarget, ContentHash, Edge, EdgeEndpoint,
        RunEnvironment, RunKind, Screen, ScreenOrigin, State, Width,
    };

    #[test]
    fn split_repositories_enforce_context_boundaries_and_gc() {
        let mut store = MemoryStore::new();
        let project = Project::new("/repo/demo", "demo").unwrap();
        let worktree =
            Worktree::new(project.id().clone(), "/repo/demo", "", Some("main".into())).unwrap();
        let commit = Commit::new(project.id().clone(), Some("abcdef1".into()), false, None);
        store.upsert_project(&project).unwrap();
        store.upsert_worktree(&worktree).unwrap();
        store.put_commit(&commit).unwrap();

        let screen = Screen::new("/", ScreenOrigin::Entry).unwrap();
        let search = Screen::new(
            "/search",
            ScreenOrigin::Anchor {
                from: "/".into(),
                selector: Some("header a[href]".into()),
            },
        )
        .unwrap();
        let state = State::new(screen.id().clone(), "default", None, BTreeMap::new(), 0).unwrap();
        let run_id = RunId::parse("0001-run").unwrap();
        let width = Width::new(1280).unwrap();

        let png = b"fake-png-bytes";
        let blob = ImageBlob::from_bytes("image/png", png, 1280, 720).unwrap();
        store.put_blob(&blob, png).unwrap();
        let orphan_png = b"orphan-png";
        let orphan = ImageBlob::from_bytes("image/png", orphan_png, 375, 667).unwrap();
        store.put_blob(&orphan, orphan_png).unwrap();

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
        store.put_evidence(&evidence).unwrap();

        let cell = Cell::new(
            run_id.clone(),
            screen.id().clone(),
            state.id().clone(),
            width,
            None,
            CellStatus::verified(),
            Some(blob.id().clone()),
            Some(evidence.id().clone()),
            None,
        )
        .unwrap();
        let edge = Edge::discovered(
            run_id.clone(),
            EdgeEndpoint::Screen {
                screen_id: screen.id().clone(),
            },
            EdgeEndpoint::Screen {
                screen_id: search.id().clone(),
            },
            Some("header a[href]".into()),
            BTreeSet::from([width]),
        );
        let run = Run::new(
            run_id.clone(),
            project.id().clone(),
            worktree.id().clone(),
            commit.id().clone(),
            RunKind::Capture,
            1_795_200_000_000,
            RunEnvironment {
                os: "macos".into(),
                compiler: Some("7.0.2".into()),
                renderer: Some("Chrome/157".into()),
                dui_version: "0.0.2".into(),
                versions: BTreeMap::new(),
            },
            BTreeSet::from(["src/App.tsx".into()]),
        );
        let aggregate = RunAggregate::publish(
            run,
            vec![screen, search],
            vec![state.clone()],
            vec![cell.clone()],
            vec![edge],
        )
        .unwrap();

        store.save_run(&aggregate).unwrap();
        assert!(store.save_run(&aggregate).is_err());
        assert_eq!(store.load_run(&run_id).unwrap(), Some(aggregate));
        assert_eq!(store.list_runs(&RunFilter::default()).unwrap().len(), 1);
        assert_eq!(
            store.unreferenced_blobs().unwrap(),
            vec![orphan.id().clone()]
        );
        store.delete_blob(orphan.id()).unwrap();
        assert!(store.unreferenced_blobs().unwrap().is_empty());

        let inputs = CacheInputs {
            source_files: BTreeMap::new(),
            types_digest: ContentHash::from_bytes(b"t"),
            mocks_digest: ContentHash::from_bytes(b"m"),
            state_id: state.id().clone(),
            width,
            locale: None,
            dui_version: "0.0.2".into(),
            rule_pack_digest: ContentHash::from_bytes(b"r"),
            renderer_env_digest: ContentHash::from_bytes(b"e"),
        };
        let entry = CacheEntry::new(
            inputs,
            CellStatus::verified(),
            Some(blob.id().clone()),
            Some(evidence.id().clone()),
            100,
        )
        .unwrap();
        store.put_cache(&entry).unwrap();
        assert_eq!(store.get_cache(entry.key()).unwrap(), Some(entry));

        let mut comment = Comment::open(
            project.id().clone(),
            run_id.clone(),
            CommentTarget::Cell {
                cell_id: cell.id().clone(),
            },
            "reviewer",
            "Check contrast",
            200,
        )
        .unwrap();
        store.upsert_comment(&comment).unwrap();
        comment.resolve(300).unwrap();
        store.upsert_comment(&comment).unwrap();
        assert_eq!(store.list_comments(&run_id).unwrap(), vec![comment]);
    }
}
