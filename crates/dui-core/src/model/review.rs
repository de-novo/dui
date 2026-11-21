//! Review bounded context: mutable human/agent review comments with explicit lifecycle transitions.

use super::{
    CellId, CommentId, DomainError, EdgeId, NonEmptyText, ProjectId, RunId, ScreenId, StateId,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CommentTarget {
    Screen { screen_id: ScreenId },
    State { state_id: StateId },
    Cell { cell_id: CellId },
    Edge { edge_id: EdgeId },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommentStatus {
    Open,
    Resolved,
}

/// Aggregate root for a review comment attached to a screen, state, cell, or edge.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    id: CommentId,
    project_id: ProjectId,
    run_id: RunId,
    target: CommentTarget,
    author: NonEmptyText,
    body: NonEmptyText,
    status: CommentStatus,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl Comment {
    pub fn open(
        project_id: ProjectId,
        run_id: RunId,
        target: CommentTarget,
        author: impl Into<String>,
        body: impl Into<String>,
        created_at_ms: i64,
    ) -> Result<Self, DomainError> {
        let author = NonEmptyText::parse("author", author)?;
        let body = NonEmptyText::parse("body", body)?;
        let id = CommentId::derive(&json!([
            run_id,
            target,
            author.as_str(),
            created_at_ms,
            body.as_str()
        ]));
        Ok(Self {
            id,
            project_id,
            run_id,
            target,
            author,
            body,
            status: CommentStatus::Open,
            created_at_ms,
            updated_at_ms: created_at_ms,
        })
    }

    pub fn id(&self) -> &CommentId {
        &self.id
    }

    pub fn project_id(&self) -> &ProjectId {
        &self.project_id
    }

    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    pub fn target(&self) -> &CommentTarget {
        &self.target
    }

    pub fn author(&self) -> &str {
        self.author.as_str()
    }

    pub fn body(&self) -> &str {
        self.body.as_str()
    }

    pub fn status(&self) -> CommentStatus {
        self.status
    }

    pub fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    pub fn updated_at_ms(&self) -> i64 {
        self.updated_at_ms
    }

    pub fn edit(&mut self, body: impl Into<String>, now_ms: i64) -> Result<(), DomainError> {
        if now_ms < self.updated_at_ms {
            return Err(DomainError::TimestampRegression("updated_at_ms"));
        }
        self.body = NonEmptyText::parse("body", body)?;
        self.updated_at_ms = now_ms;
        Ok(())
    }

    pub fn resolve(&mut self, now_ms: i64) -> Result<(), DomainError> {
        if now_ms < self.updated_at_ms {
            return Err(DomainError::TimestampRegression("updated_at_ms"));
        }
        match self.status {
            CommentStatus::Open => {
                self.status = CommentStatus::Resolved;
                self.updated_at_ms = now_ms;
                Ok(())
            }
            CommentStatus::Resolved => Err(DomainError::InvalidStateTransition(
                "comment is already resolved",
            )),
        }
    }

    pub fn reopen(&mut self, now_ms: i64) -> Result<(), DomainError> {
        if now_ms < self.updated_at_ms {
            return Err(DomainError::TimestampRegression("updated_at_ms"));
        }
        match self.status {
            CommentStatus::Resolved => {
                self.status = CommentStatus::Open;
                self.updated_at_ms = now_ms;
                Ok(())
            }
            CommentStatus::Open => Err(DomainError::InvalidStateTransition(
                "comment is already open",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Project, Screen};

    #[test]
    fn comment_lifecycle_enforces_transitions_and_monotonic_timestamps() {
        let project = Project::new("/repo/demo", "demo").unwrap();
        let run_id = RunId::parse("run-1").unwrap();
        let target = CommentTarget::Screen {
            screen_id: Screen::derive_id("/"),
        };
        assert!(
            Comment::open(
                project.id().clone(),
                run_id.clone(),
                target.clone(),
                "",
                "hi",
                10
            )
            .is_err()
        );
        assert!(
            Comment::open(
                project.id().clone(),
                run_id.clone(),
                target.clone(),
                "dev",
                " ",
                10
            )
            .is_err()
        );

        let mut comment = Comment::open(
            project.id().clone(),
            run_id,
            target,
            "reviewer",
            "Check spacing",
            100,
        )
        .unwrap();
        assert_eq!(comment.status(), CommentStatus::Open);
        assert!(comment.reopen(101).is_err());
        comment.edit("Check mobile spacing", 150).unwrap();
        assert_eq!(comment.body(), "Check mobile spacing");
        assert!(comment.edit("Older", 120).is_err());
        comment.resolve(200).unwrap();
        assert_eq!(comment.status(), CommentStatus::Resolved);
        assert!(comment.resolve(201).is_err());
        comment.reopen(250).unwrap();
        assert_eq!(comment.status(), CommentStatus::Open);
    }
}
