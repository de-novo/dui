//! Pure DDD domain model organized into four bounded contexts:
//! - `workspace`: `Project` & `Worktree` aggregate roots and `Commit` value object
//! - `capture`: `RunAggregate` root owning `Cell`s & `Edge`s over `Screen`, `State`, `Width`, `Evidence`, `ImageBlob`
//! - `cache`: `CacheEntry` aggregate root, `CacheInputs`, `CacheOutcome`, and `evaluate_cache`
//! - `review`: `Comment` aggregate root with lifecycle transitions

pub mod cache;
pub mod capture;
pub mod review;
pub mod workspace;

pub use cache::{CacheEntry, CacheInputKind, CacheInputs, CacheOutcome, evaluate_cache};
pub use capture::{
    Cell, CellKey, CellStatus, CoverageSummary, Edge, EdgeEndpoint, EdgeSource, Evidence,
    ImageBlob, Run, RunAggregate, RunEnvironment, RunKind, Screen, ScreenOrigin, State,
};
pub use review::{Comment, CommentStatus, CommentTarget};
pub use workspace::{Commit, Project, Worktree};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fmt, num::NonZeroU32};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    EmptyField(&'static str),
    InvalidHash(&'static str),
    InvalidWidth,
    UnknownCellStatus(String),
    MissingReason(&'static str),
    UnknownEdgeSource(String),
    UnrenderableHasImage,
    ReplayRequiresWidths,
    AggregateRunMismatch(&'static str),
    MissingReference(&'static str),
    StateScreenMismatch,
    DuplicateCellId,
    InvalidStateTransition(&'static str),
    TimestampRegression(&'static str),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(f, "{field} must not be empty"),
            Self::InvalidHash(field) => {
                write!(f, "{field} must be a 64-char lowercase hex SHA-256")
            }
            Self::InvalidWidth => write!(f, "width must be a positive integer"),
            Self::UnknownCellStatus(status) => write!(f, "unknown cell status: {status}"),
            Self::MissingReason(status) => write!(f, "{status} cell requires a non-empty reason"),
            Self::UnknownEdgeSource(source) => write!(f, "unknown edge source: {source}"),
            Self::UnrenderableHasImage => write!(f, "unrenderable cell cannot carry an image blob"),
            Self::ReplayRequiresWidths => {
                write!(
                    f,
                    "replay verification requires at least one confirmed width"
                )
            }
            Self::AggregateRunMismatch(entity) => {
                write!(f, "{entity} run_id does not match the owning run")
            }
            Self::MissingReference(entity) => {
                write!(f, "run aggregate is missing referenced {entity}")
            }
            Self::StateScreenMismatch => write!(f, "cell state belongs to a different screen"),
            Self::DuplicateCellId => write!(f, "run aggregate contains a duplicate cell id"),
            Self::InvalidStateTransition(msg) => write!(f, "invalid state transition: {msg}"),
            Self::TimestampRegression(field) => write!(f, "{field} must not move backwards"),
        }
    }
}

impl std::error::Error for DomainError {}

/// Value object enforcing a trimmed, non-empty UTF-8 string.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct NonEmptyText(String);

impl NonEmptyText {
    pub fn parse(field: &'static str, value: impl Into<String>) -> Result<Self, DomainError> {
        let trimmed = value.into().trim().to_owned();
        if trimmed.is_empty() {
            Err(DomainError::EmptyField(field))
        } else {
            Ok(Self(trimmed))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NonEmptyText {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse("text", value)
    }
}

impl From<NonEmptyText> for String {
    fn from(value: NonEmptyText) -> Self {
        value.0
    }
}

impl fmt::Display for NonEmptyText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Validated lowercase 64-character SHA-256 hex digest.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentHash(String);

impl ContentHash {
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(value))
        } else {
            Err(DomainError::InvalidHash("content_hash"))
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(bytes)))
    }

    pub fn from_canonical(tag: &str, payload: &Value) -> Self {
        let encoded = json!([tag, payload]).to_string();
        Self::from_bytes(encoded.as_bytes())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ContentHash {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<ContentHash> for String {
    fn from(hash: ContentHash) -> Self {
        hash.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

macro_rules! hash_id {
    ($name:ident, $tag:literal) => {
        #[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(ContentHash);

        impl $name {
            pub fn from_hash(hash: ContentHash) -> Self {
                Self(hash)
            }

            pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
                ContentHash::parse(value).map(Self)
            }

            pub fn derive(payload: &Value) -> Self {
                Self(ContentHash::from_canonical($tag, payload))
            }

            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            pub fn hash(&self) -> &ContentHash {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

hash_id!(ProjectId, "dui:project:v1");
hash_id!(WorktreeId, "dui:worktree:v1");
hash_id!(CommitId, "dui:commit:v1");
hash_id!(ScreenId, "dui:screen:v1");
hash_id!(StateId, "dui:state:v1");
hash_id!(CellId, "dui:cell:v1");
hash_id!(EvidenceId, "dui:evidence:v1");
hash_id!(BlobId, "dui:blob:v1");
hash_id!(EdgeId, "dui:edge:v1");
hash_id!(CommentId, "dui:comment:v1");
hash_id!(CacheKey, "dui:cache:v1");

impl ProjectId {
    /// Matches the local archive directory identity: raw SHA-256 of primary repo root bytes.
    pub fn from_primary_root_bytes(path_bytes: &[u8]) -> Self {
        Self(ContentHash::from_bytes(path_bytes))
    }
}

impl BlobId {
    /// Content-addressed image identity is the raw SHA-256 of the image bytes.
    pub fn from_image_bytes(bytes: &[u8]) -> Self {
        Self(ContentHash::from_bytes(bytes))
    }
}

/// Non-empty run identifier (portable across existing `~/.dui` runs and SQLite/Postgres).
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct RunId(String);

impl RunId {
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        NonEmptyText::parse("run_id", value).map(|t| Self(t.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RunId {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<RunId> for String {
    fn from(id: RunId) -> Self {
        id.0
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Positive viewport width in CSS pixels (Value Object).
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Width(NonZeroU32);

impl Width {
    pub fn new(pixels: u32) -> Result<Self, DomainError> {
        NonZeroU32::new(pixels)
            .map(Self)
            .ok_or(DomainError::InvalidWidth)
    }

    pub fn pixels(self) -> u32 {
        self.0.get()
    }
}

impl fmt::Display for Width {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
