//! Workspace bounded context: repository identity, worktree checkouts, and source commits.

use super::{CommitId, ContentHash, DomainError, NonEmptyText, ProjectId, WorktreeId};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Aggregate root representing one logical project across all branches, worktrees, and runs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    id: ProjectId,
    display_name: NonEmptyText,
    primary_root: NonEmptyText,
}

impl Project {
    pub fn new(
        primary_root: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let primary_root = NonEmptyText::parse("primary_root", primary_root)?;
        let id = ProjectId::from_primary_root_bytes(primary_root.as_str().as_bytes());
        Self::with_id(id, primary_root.as_str(), display_name)
    }

    pub fn with_id(
        id: ProjectId,
        primary_root: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            id,
            display_name: NonEmptyText::parse("display_name", display_name)?,
            primary_root: NonEmptyText::parse("primary_root", primary_root)?,
        })
    }

    pub fn id(&self) -> &ProjectId {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        self.display_name.as_str()
    }

    pub fn primary_root(&self) -> &str {
        self.primary_root.as_str()
    }

    pub fn rename(&mut self, display_name: impl Into<String>) -> Result<(), DomainError> {
        self.display_name = NonEmptyText::parse("display_name", display_name)?;
        Ok(())
    }
}

/// Aggregate root for one local checkout of a `Project`. `branch` changes on checkout.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    id: WorktreeId,
    project_id: ProjectId,
    path: NonEmptyText,
    app_relative: String,
    branch: Option<NonEmptyText>,
}

impl Worktree {
    pub fn new(
        project_id: ProjectId,
        path: impl Into<String>,
        app_relative: impl Into<String>,
        branch: Option<String>,
    ) -> Result<Self, DomainError> {
        let path = NonEmptyText::parse("worktree_path", path)?;
        let app_relative = app_relative.into();
        let branch = branch
            .map(|b| NonEmptyText::parse("branch", b))
            .transpose()?;
        let id = WorktreeId::derive(&json!([project_id, path.as_str(), app_relative]));
        Ok(Self {
            id,
            project_id,
            path,
            app_relative,
            branch,
        })
    }

    pub fn id(&self) -> &WorktreeId {
        &self.id
    }

    pub fn project_id(&self) -> &ProjectId {
        &self.project_id
    }

    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    pub fn app_relative(&self) -> &str {
        &self.app_relative
    }

    pub fn branch(&self) -> Option<&str> {
        self.branch.as_ref().map(NonEmptyText::as_str)
    }

    pub fn checkout(&mut self, branch: Option<String>) -> Result<(), DomainError> {
        self.branch = branch
            .map(|b| NonEmptyText::parse("branch", b))
            .transpose()?;
        Ok(())
    }
}

/// Immutable value object identifying a clean commit or dirty working tree snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    id: CommitId,
    project_id: ProjectId,
    git_sha: Option<String>,
    dirty: bool,
    tree_digest: Option<ContentHash>,
}

impl Commit {
    pub fn new(
        project_id: ProjectId,
        git_sha: Option<String>,
        dirty: bool,
        tree_digest: Option<ContentHash>,
    ) -> Self {
        let git_sha = git_sha
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        let id = CommitId::derive(&json!([project_id, git_sha, dirty, tree_digest]));
        Self {
            id,
            project_id,
            git_sha,
            dirty,
            tree_digest,
        }
    }

    pub fn id(&self) -> &CommitId {
        &self.id
    }

    pub fn project_id(&self) -> &ProjectId {
        &self.project_id
    }

    pub fn git_sha(&self) -> Option<&str> {
        self.git_sha.as_deref()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn tree_digest(&self) -> Option<&ContentHash> {
        self.tree_digest.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_and_worktree_enforce_invariants_and_transitions() {
        assert!(Project::new("", "demo").is_err());
        assert!(Project::new("/repo/demo", "   ").is_err());

        let mut project = Project::new("/repo/demo", "demo").unwrap();
        assert_eq!(project.display_name(), "demo");
        project.rename("renamed-demo").unwrap();
        assert_eq!(project.display_name(), "renamed-demo");
        assert!(project.rename(" ").is_err());

        let mut wt =
            Worktree::new(project.id().clone(), "/repo/demo", "", Some("main".into())).unwrap();
        assert_eq!(wt.branch(), Some("main"));
        wt.checkout(Some("feat/domain".into())).unwrap();
        assert_eq!(wt.branch(), Some("feat/domain"));
        wt.checkout(None).unwrap();
        assert_eq!(wt.branch(), None);
    }
}
