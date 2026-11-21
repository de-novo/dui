//! Pure domain entities, content-hash identities, and algebraic rules shared by
//! the screen map, live captures, cache, diff, storage adapters, and review comments.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    num::NonZeroU32,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    EmptyField(&'static str),
    InvalidHash(&'static str),
    InvalidWidth,
    UnknownCellStatus(String),
    MissingReason(&'static str),
    UnknownEdgeSource(String),
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
        }
    }
}

impl std::error::Error for DomainError {}

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
    /// Matches the existing local archive directory identity: raw SHA-256 of the
    /// primary repository root path bytes.
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
        let value = value.into();
        if value.trim().is_empty() {
            Err(DomainError::EmptyField("run_id"))
        } else {
            Ok(Self(value))
        }
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

/// Positive viewport width in CSS pixels.
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

/// Pure algebraic cell verdict. `Unverified` and `Unrenderable` are always distinct.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum CellStatus {
    Verified,
    Unverified { reason: String },
    Unrenderable { reason: String },
}

impl CellStatus {
    pub fn from_wire(status: &str, reason: Option<String>) -> Result<Self, DomainError> {
        match status {
            "verified" => Ok(Self::Verified),
            "unverified" => {
                let reason = reason
                    .map(|r| r.trim().to_owned())
                    .filter(|r| !r.is_empty())
                    .ok_or(DomainError::MissingReason("unverified"))?;
                Ok(Self::Unverified { reason })
            }
            "unrenderable" => {
                let reason = reason
                    .map(|r| r.trim().to_owned())
                    .filter(|r| !r.is_empty())
                    .ok_or(DomainError::MissingReason("unrenderable"))?;
                Ok(Self::Unrenderable { reason })
            }
            other => Err(DomainError::UnknownCellStatus(other.to_owned())),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Unverified { .. } => "unverified",
            Self::Unrenderable { .. } => "unrenderable",
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Verified => None,
            Self::Unverified { reason } | Self::Unrenderable { reason } => Some(reason.as_str()),
        }
    }
}

/// Pure algebraic provenance for a transition edge.
/// Discovery or inference alone can never construct `Verified`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeSource {
    Extracted,
    Inferred,
    Verified,
}

impl EdgeSource {
    pub fn from_wire(source: &str) -> Result<Self, DomainError> {
        match source {
            "extracted" => Ok(Self::Extracted),
            "inferred" => Ok(Self::Inferred),
            "verified" => Ok(Self::Verified),
            other => Err(DomainError::UnknownEdgeSource(other.to_owned())),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheInputKind {
    SourceFiles,
    Types,
    Mocks,
    State,
    Width,
    Locale,
    DuiVersion,
    RulePack,
    RendererEnvironment,
    Forced,
    Missing,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "kebab-case")]
pub enum CacheOutcome {
    Hit {
        key: CacheKey,
    },
    Miss {
        key: CacheKey,
        changed: BTreeSet<CacheInputKind>,
    },
}

/// Canonical cell inputs used to derive a deterministic cache key and explain misses.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInputs {
    pub source_files: BTreeMap<String, ContentHash>,
    pub types_digest: ContentHash,
    pub mocks_digest: ContentHash,
    pub state_id: StateId,
    pub width: Width,
    pub locale: Option<String>,
    pub dui_version: String,
    pub rule_pack_digest: ContentHash,
    pub renderer_env_digest: ContentHash,
}

impl CacheInputs {
    pub fn key(&self) -> CacheKey {
        CacheKey::derive(&json!({
            "sourceFiles": self.source_files,
            "typesDigest": self.types_digest,
            "mocksDigest": self.mocks_digest,
            "stateId": self.state_id,
            "width": self.width,
            "locale": self.locale,
            "duiVersion": self.dui_version,
            "rulePackDigest": self.rule_pack_digest,
            "rendererEnvDigest": self.renderer_env_digest,
        }))
    }
}

/// Pure cache decision: reuses a cached cell only when every input digest matches.
pub fn evaluate_cache(
    current: &CacheInputs,
    previous: Option<&CacheInputs>,
    force: bool,
) -> CacheOutcome {
    let key = current.key();
    if force {
        return CacheOutcome::Miss {
            key,
            changed: BTreeSet::from([CacheInputKind::Forced]),
        };
    }
    let Some(prev) = previous else {
        return CacheOutcome::Miss {
            key,
            changed: BTreeSet::from([CacheInputKind::Missing]),
        };
    };
    if prev.key() == key {
        return CacheOutcome::Hit { key };
    }
    let mut changed = BTreeSet::new();
    if current.source_files != prev.source_files {
        changed.insert(CacheInputKind::SourceFiles);
    }
    if current.types_digest != prev.types_digest {
        changed.insert(CacheInputKind::Types);
    }
    if current.mocks_digest != prev.mocks_digest {
        changed.insert(CacheInputKind::Mocks);
    }
    if current.state_id != prev.state_id {
        changed.insert(CacheInputKind::State);
    }
    if current.width != prev.width {
        changed.insert(CacheInputKind::Width);
    }
    if current.locale != prev.locale {
        changed.insert(CacheInputKind::Locale);
    }
    if current.dui_version != prev.dui_version {
        changed.insert(CacheInputKind::DuiVersion);
    }
    if current.rule_pack_digest != prev.rule_pack_digest {
        changed.insert(CacheInputKind::RulePack);
    }
    if current.renderer_env_digest != prev.renderer_env_digest {
        changed.insert(CacheInputKind::RendererEnvironment);
    }
    if changed.is_empty() {
        changed.insert(CacheInputKind::Missing);
    }
    CacheOutcome::Miss { key, changed }
}

/// 1. Project — shared identity across all branches, worktrees, and runs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: ProjectId,
    pub display_name: String,
    pub primary_root: String,
}

/// 2. Worktree — one checkout of a project; `branch` may change on checkout.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub id: WorktreeId,
    pub project_id: ProjectId,
    pub path: String,
    pub app_relative: String,
    pub branch: Option<String>,
}

impl Worktree {
    pub fn derive_id(project_id: &ProjectId, path: &str, app_relative: &str) -> WorktreeId {
        WorktreeId::derive(&json!([project_id, path, app_relative]))
    }
}

/// 3. Commit — immutable source revision (clean Git commit or dirty working tree).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub id: CommitId,
    pub project_id: ProjectId,
    pub git_sha: Option<String>,
    pub dirty: bool,
    pub tree_digest: Option<ContentHash>,
}

impl Commit {
    pub fn derive_id(
        project_id: &ProjectId,
        git_sha: Option<&str>,
        dirty: bool,
        tree_digest: Option<&ContentHash>,
    ) -> CommitId {
        CommitId::derive(&json!([project_id, git_sha, dirty, tree_digest]))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RunKind {
    Capture,
    Replay,
    LiveSave {
        source_run: RunId,
        source_state: StateId,
        source_width: Width,
        revision: u64,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEnvironment {
    pub os: String,
    pub compiler: Option<String>,
    pub renderer: Option<String>,
    pub dui_version: String,
    pub versions: BTreeMap<String, String>,
}

/// 4. Run — one immutable capture, replay, or saved live edit belonging to a commit/worktree.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: RunId,
    pub project_id: ProjectId,
    pub worktree_id: WorktreeId,
    pub commit_id: CommitId,
    pub kind: RunKind,
    pub created_at_ms: i64,
    pub environment: RunEnvironment,
    pub source_files: BTreeSet<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ScreenOrigin {
    Entry,
    Router {
        file: String,
    },
    Anchor {
        from: String,
        selector: Option<String>,
    },
}

/// 5. Screen — a discovered route or root view in the application.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screen {
    pub id: ScreenId,
    pub path: String,
    pub origin: ScreenOrigin,
}

impl Screen {
    pub fn derive_id(path: &str) -> ScreenId {
        ScreenId::derive(&json!(path))
    }
}

/// 6. State — one deterministic combination of scenario, locale, and state-slot inputs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub id: StateId,
    pub screen_id: ScreenId,
    pub scenario: String,
    pub locale: Option<String>,
    pub inputs: BTreeMap<String, Value>,
    pub plan_index: u32,
}

impl State {
    pub fn derive_id(
        screen_id: &ScreenId,
        scenario: &str,
        locale: Option<&str>,
        inputs: &BTreeMap<String, Value>,
    ) -> StateId {
        StateId::derive(&json!([screen_id, scenario, locale, inputs]))
    }
}

/// Cross-run comparison key for diffing two captures (#11).
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellKey {
    pub screen_id: ScreenId,
    pub state_id: StateId,
    pub width: Width,
    pub locale: Option<String>,
}

/// 8. Cell — one rendered outcome (`screen × state × width × locale`) owned by a `Run`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub id: CellId,
    pub run_id: RunId,
    pub screen_id: ScreenId,
    pub state_id: StateId,
    pub width: Width,
    pub locale: Option<String>,
    pub status: CellStatus,
    pub image_blob: Option<BlobId>,
    pub evidence_id: Option<EvidenceId>,
    pub cache: Option<CacheOutcome>,
}

impl Cell {
    pub fn derive_id(
        run_id: &RunId,
        screen_id: &ScreenId,
        state_id: &StateId,
        width: Width,
        locale: Option<&str>,
    ) -> CellId {
        CellId::derive(&json!([run_id, screen_id, state_id, width, locale]))
    }

    pub fn diff_key(&self) -> CellKey {
        CellKey {
            screen_id: self.screen_id.clone(),
            state_id: self.state_id.clone(),
            width: self.width,
            locale: self.locale.clone(),
        }
    }
}

/// 9. Evidence — immutable, content-addressed runtime observation supporting a cell verdict.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub id: EvidenceId,
    pub observed_path: Option<String>,
    pub observed_width: Option<u32>,
    pub ready: Option<bool>,
    pub settled: Option<bool>,
    pub slots: BTreeMap<String, Value>,
    pub instances: BTreeMap<String, BTreeMap<String, Value>>,
    pub hits: BTreeSet<String>,
    pub links: BTreeSet<String>,
    pub responses: BTreeMap<String, u16>,
    pub assertions: Vec<Value>,
    pub live_edits: Vec<Value>,
}

impl Evidence {
    #[expect(
        clippy::too_many_arguments,
        reason = "constructs an immutable evidence record with its canonical digest in one step"
    )]
    pub fn new(
        observed_path: Option<String>,
        observed_width: Option<u32>,
        ready: Option<bool>,
        settled: Option<bool>,
        slots: BTreeMap<String, Value>,
        instances: BTreeMap<String, BTreeMap<String, Value>>,
        hits: BTreeSet<String>,
        links: BTreeSet<String>,
        responses: BTreeMap<String, u16>,
        assertions: Vec<Value>,
        live_edits: Vec<Value>,
    ) -> Self {
        let id = EvidenceId::derive(&json!({
            "observedPath": observed_path,
            "observedWidth": observed_width,
            "ready": ready,
            "settled": settled,
            "slots": slots,
            "instances": instances,
            "hits": hits,
            "links": links,
            "responses": responses,
            "assertions": assertions,
            "liveEdits": live_edits,
        }));
        Self {
            id,
            observed_path,
            observed_width,
            ready,
            settled,
            slots,
            instances,
            hits,
            links,
            responses,
            assertions,
            live_edits,
        }
    }
}

/// 10. ImageBlob — metadata for a content-addressed rendered image stored on disk.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageBlob {
    pub id: BlobId,
    pub media_type: String,
    pub byte_len: u64,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum EdgeEndpoint {
    Screen {
        screen_id: ScreenId,
    },
    State {
        screen_id: ScreenId,
        state_id: StateId,
    },
}

/// 11. Edge (transition) — directed link or action between screens or states.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub id: EdgeId,
    pub run_id: RunId,
    pub from: EdgeEndpoint,
    pub to: EdgeEndpoint,
    pub source: EdgeSource,
    pub trigger: Option<String>,
    pub selector: Option<String>,
    pub observed_widths: BTreeSet<Width>,
}

impl Edge {
    pub fn derive_id(
        run_id: &RunId,
        from: &EdgeEndpoint,
        to: &EdgeEndpoint,
        source: EdgeSource,
        trigger: Option<&str>,
    ) -> EdgeId {
        EdgeId::derive(&json!([run_id, from, to, source, trigger]))
    }
}

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

/// 12. Comment — mutable human or agent review note attached to a map entity (v0.0.5).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: CommentId,
    pub project_id: ProjectId,
    pub run_id: RunId,
    pub target: CommentTarget,
    pub author: String,
    pub body: String,
    pub status: CommentStatus,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl Comment {
    pub fn derive_id(
        run_id: &RunId,
        target: &CommentTarget,
        author: &str,
        created_at_ms: i64,
        initial_body: &str,
    ) -> CommentId {
        CommentId::derive(&json!([
            run_id,
            target,
            author,
            created_at_ms,
            initial_body
        ]))
    }
}

/// 13. CacheEntry — mutable pointer from a canonical cell input key to immutable artifacts (#8).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheEntry {
    pub key: CacheKey,
    pub inputs: CacheInputs,
    pub status: CellStatus,
    pub image_blob: Option<BlobId>,
    pub evidence_id: Option<EvidenceId>,
    pub created_at_ms: i64,
    pub last_hit_at_ms: i64,
}

/// Pure deterministic coverage fold over a run's cells.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageSummary {
    pub verified: usize,
    pub unverified: usize,
    pub unrenderable: usize,
    pub renderable: usize,
    pub total: usize,
}

impl CoverageSummary {
    pub fn from_cells(cells: &[Cell]) -> Self {
        let mut summary = Self::default();
        for cell in cells {
            summary.total += 1;
            match cell.status {
                CellStatus::Verified => {
                    summary.verified += 1;
                    summary.renderable += 1;
                }
                CellStatus::Unverified { .. } => {
                    summary.unverified += 1;
                    summary.renderable += 1;
                }
                CellStatus::Unrenderable { .. } => {
                    summary.unrenderable += 1;
                }
            }
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_inputs(width: u32, locale: Option<&str>) -> CacheInputs {
        CacheInputs {
            source_files: BTreeMap::from([(
                "src/App.tsx".into(),
                ContentHash::from_bytes(b"export default function App() {}"),
            )]),
            types_digest: ContentHash::from_bytes(b"types-v1"),
            mocks_digest: ContentHash::from_bytes(b"mocks-v1"),
            state_id: State::derive_id(
                &Screen::derive_id("/"),
                "default",
                locale,
                &BTreeMap::new(),
            ),
            width: Width::new(width).unwrap(),
            locale: locale.map(str::to_owned),
            dui_version: "0.0.2".into(),
            rule_pack_digest: ContentHash::from_bytes(b"react-v1"),
            renderer_env_digest: ContentHash::from_bytes(b"chrome-157-macos"),
        }
    }

    #[test]
    fn cell_status_keeps_unverified_and_unrenderable_separate_with_reasons() {
        assert_eq!(
            CellStatus::from_wire("verified", None).unwrap(),
            CellStatus::Verified
        );
        assert_eq!(
            CellStatus::from_wire("unverified", Some("assert-failed".into())).unwrap(),
            CellStatus::Unverified {
                reason: "assert-failed".into()
            }
        );
        assert_eq!(
            CellStatus::from_wire("unrenderable", Some("no-render-env".into())).unwrap(),
            CellStatus::Unrenderable {
                reason: "no-render-env".into()
            }
        );
        assert!(CellStatus::from_wire("unverified", None).is_err());
        assert!(CellStatus::from_wire("unrenderable", Some(" ".into())).is_err());
        assert!(CellStatus::from_wire("unknown", None).is_err());
    }

    #[test]
    fn edge_source_rejects_unknown_variants_and_separates_extracted_from_verified() {
        assert_eq!(
            EdgeSource::from_wire("extracted").unwrap(),
            EdgeSource::Extracted
        );
        assert_eq!(
            EdgeSource::from_wire("inferred").unwrap(),
            EdgeSource::Inferred
        );
        assert_eq!(
            EdgeSource::from_wire("verified").unwrap(),
            EdgeSource::Verified
        );
        assert_ne!(EdgeSource::Extracted, EdgeSource::Verified);
        assert!(EdgeSource::from_wire("discovered").is_err());
    }

    #[test]
    fn cache_evaluation_reports_hits_and_exact_changed_inputs() {
        let base = sample_inputs(1280, Some("en"));
        assert_eq!(
            evaluate_cache(&base, Some(&base), false),
            CacheOutcome::Hit { key: base.key() }
        );
        assert_eq!(
            evaluate_cache(&base, Some(&base), true),
            CacheOutcome::Miss {
                key: base.key(),
                changed: BTreeSet::from([CacheInputKind::Forced]),
            }
        );
        let mut edited = base.clone();
        edited
            .source_files
            .insert("src/App.tsx".into(), ContentHash::from_bytes(b"changed"));
        edited.renderer_env_digest = ContentHash::from_bytes(b"chrome-158-macos");
        assert_eq!(
            evaluate_cache(&edited, Some(&base), false),
            CacheOutcome::Miss {
                key: edited.key(),
                changed: BTreeSet::from([
                    CacheInputKind::SourceFiles,
                    CacheInputKind::RendererEnvironment,
                ]),
            }
        );
    }

    #[test]
    fn deterministic_ids_and_coverage_fold_match_inputs() {
        let screen = Screen {
            id: Screen::derive_id("/orders"),
            path: "/orders".into(),
            origin: ScreenOrigin::Router {
                file: "src/routes/orders.tsx".into(),
            },
        };
        let state = State {
            id: State::derive_id(&screen.id, "default", None, &BTreeMap::new()),
            screen_id: screen.id.clone(),
            scenario: "default".into(),
            locale: None,
            inputs: BTreeMap::new(),
            plan_index: 0,
        };
        let run_id = RunId::parse("run-1").unwrap();
        let w1 = Width::new(375).unwrap();
        let w2 = Width::new(1280).unwrap();
        let cells = vec![
            Cell {
                id: Cell::derive_id(&run_id, &screen.id, &state.id, w1, None),
                run_id: run_id.clone(),
                screen_id: screen.id.clone(),
                state_id: state.id.clone(),
                width: w1,
                locale: None,
                status: CellStatus::Verified,
                image_blob: Some(BlobId::from_image_bytes(b"png-1")),
                evidence_id: None,
                cache: None,
            },
            Cell {
                id: Cell::derive_id(&run_id, &screen.id, &state.id, w2, None),
                run_id: run_id.clone(),
                screen_id: screen.id.clone(),
                state_id: state.id.clone(),
                width: w2,
                locale: None,
                status: CellStatus::Unrenderable {
                    reason: "no-render-env".into(),
                },
                image_blob: None,
                evidence_id: None,
                cache: None,
            },
        ];
        assert_ne!(cells[0].id, cells[1].id);
        assert_eq!(
            CoverageSummary::from_cells(&cells),
            CoverageSummary {
                verified: 1,
                unverified: 0,
                unrenderable: 1,
                renderable: 1,
                total: 2,
            }
        );
    }
}
