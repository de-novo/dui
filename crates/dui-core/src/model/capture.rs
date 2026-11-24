//! Capture bounded context: immutable run aggregate, screens, states, cells, edges,
//! runtime evidence, image blobs, and coverage folds.

use super::{
    BlobId, CacheOutcome, CellId, CommitId, DomainError, EdgeId, EvidenceId, NonEmptyText,
    ProjectId, RunId, ScreenId, StateId, Width, WorktreeId,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Pure algebraic cell verdict. `Unverified` and `Unrenderable` are always distinct
/// and require non-empty reasons.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum CellStatus {
    Verified,
    Unverified { reason: NonEmptyText },
    Unrenderable { reason: NonEmptyText },
}

impl CellStatus {
    pub fn verified() -> Self {
        Self::Verified
    }

    pub fn unverified(reason: impl Into<String>) -> Result<Self, DomainError> {
        let reason = NonEmptyText::parse("reason", reason)
            .map_err(|_| DomainError::MissingReason("unverified"))?;
        Ok(Self::Unverified { reason })
    }

    pub fn unrenderable(reason: impl Into<String>) -> Result<Self, DomainError> {
        let reason = NonEmptyText::parse("reason", reason)
            .map_err(|_| DomainError::MissingReason("unrenderable"))?;
        Ok(Self::Unrenderable { reason })
    }

    pub fn from_wire(status: &str, reason: Option<String>) -> Result<Self, DomainError> {
        match status {
            "verified" => Ok(Self::Verified),
            "unverified" => Self::unverified(reason.unwrap_or_default()),
            "unrenderable" => Self::unrenderable(reason.unwrap_or_default()),
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

    pub fn is_unrenderable(&self) -> bool {
        matches!(self, Self::Unrenderable { .. })
    }
}

/// Pure algebraic provenance for a transition edge.
/// Discovery alone yields `Extracted`; only replay confirmation produces `Verified`.
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

/// Immutable run header owned by `RunAggregate`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    id: RunId,
    project_id: ProjectId,
    worktree_id: WorktreeId,
    commit_id: CommitId,
    kind: RunKind,
    created_at_ms: i64,
    environment: RunEnvironment,
    source_files: BTreeSet<String>,
}

impl Run {
    #[expect(
        clippy::too_many_arguments,
        reason = "constructs an immutable run header with all provenance fields in one step"
    )]
    pub fn new(
        id: RunId,
        project_id: ProjectId,
        worktree_id: WorktreeId,
        commit_id: CommitId,
        kind: RunKind,
        created_at_ms: i64,
        environment: RunEnvironment,
        source_files: BTreeSet<String>,
    ) -> Self {
        Self {
            id,
            project_id,
            worktree_id,
            commit_id,
            kind,
            created_at_ms,
            environment,
            source_files,
        }
    }

    pub fn id(&self) -> &RunId {
        &self.id
    }

    pub fn project_id(&self) -> &ProjectId {
        &self.project_id
    }

    pub fn worktree_id(&self) -> &WorktreeId {
        &self.worktree_id
    }

    pub fn commit_id(&self) -> &CommitId {
        &self.commit_id
    }

    pub fn kind(&self) -> &RunKind {
        &self.kind
    }

    pub fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    pub fn environment(&self) -> &RunEnvironment {
        &self.environment
    }

    pub fn source_files(&self) -> &BTreeSet<String> {
        &self.source_files
    }
}

/// Location of a screen declaration, independent of any capture or runtime state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NativeSource {
    pub name: String,
    pub source: String,
    pub line: std::num::NonZeroU32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dynamic: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ScreenOrigin {
    Entry,
    Native {
        #[serde(flatten)]
        declaration: NativeSource,
    },
    Router {
        file: String,
    },
    Anchor {
        from: String,
        selector: Option<String>,
    },
}

/// Immutable value object representing a discovered screen route.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screen {
    id: ScreenId,
    path: NonEmptyText,
    origin: ScreenOrigin,
}

impl Screen {
    pub fn new(path: impl Into<String>, origin: ScreenOrigin) -> Result<Self, DomainError> {
        let path = NonEmptyText::parse("screen_path", path)?;
        let id = match &origin {
            ScreenOrigin::Native { .. } => ScreenId::derive(&json!([path.as_str(), &origin])),
            ScreenOrigin::Entry | ScreenOrigin::Router { .. } | ScreenOrigin::Anchor { .. } => {
                Self::derive_id(path.as_str())
            }
        };
        Ok(Self { id, path, origin })
    }

    pub fn derive_id(path: &str) -> ScreenId {
        ScreenId::derive(&json!(path))
    }

    pub fn id(&self) -> &ScreenId {
        &self.id
    }

    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    pub fn origin(&self) -> &ScreenOrigin {
        &self.origin
    }
}

/// Immutable value object representing one deterministic state combination on a screen.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    id: StateId,
    screen_id: ScreenId,
    scenario: NonEmptyText,
    locale: Option<NonEmptyText>,
    inputs: BTreeMap<String, Value>,
    plan_index: u32,
}

impl State {
    pub fn new(
        screen_id: ScreenId,
        scenario: impl Into<String>,
        locale: Option<String>,
        inputs: BTreeMap<String, Value>,
        plan_index: u32,
    ) -> Result<Self, DomainError> {
        let scenario = NonEmptyText::parse("scenario", scenario)?;
        let locale = locale
            .map(|l| NonEmptyText::parse("locale", l))
            .transpose()?;
        let id = StateId::derive(&json!([
            screen_id,
            scenario.as_str(),
            locale.as_ref().map(NonEmptyText::as_str),
            inputs
        ]));
        Ok(Self {
            id,
            screen_id,
            scenario,
            locale,
            inputs,
            plan_index,
        })
    }

    pub fn with_id(
        id: StateId,
        screen_id: ScreenId,
        scenario: impl Into<String>,
        locale: Option<String>,
        inputs: BTreeMap<String, Value>,
        plan_index: u32,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            id,
            screen_id,
            scenario: NonEmptyText::parse("scenario", scenario)?,
            locale: locale
                .map(|l| NonEmptyText::parse("locale", l))
                .transpose()?,
            inputs,
            plan_index,
        })
    }

    pub fn id(&self) -> &StateId {
        &self.id
    }

    pub fn screen_id(&self) -> &ScreenId {
        &self.screen_id
    }

    pub fn scenario(&self) -> &str {
        self.scenario.as_str()
    }

    pub fn locale(&self) -> Option<&str> {
        self.locale.as_ref().map(NonEmptyText::as_str)
    }

    pub fn inputs(&self) -> &BTreeMap<String, Value> {
        &self.inputs
    }

    pub fn plan_index(&self) -> u32 {
        self.plan_index
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

/// Entity owned by a `RunAggregate` recording one `screen × state × width × locale` outcome.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    id: CellId,
    run_id: RunId,
    screen_id: ScreenId,
    state_id: StateId,
    width: Width,
    locale: Option<NonEmptyText>,
    status: CellStatus,
    image_blob: Option<BlobId>,
    evidence_id: Option<EvidenceId>,
    cache: Option<CacheOutcome>,
}

impl Cell {
    #[expect(
        clippy::too_many_arguments,
        reason = "enforces all cell invariants and derives the canonical CellId in one constructor"
    )]
    pub fn new(
        run_id: RunId,
        screen_id: ScreenId,
        state_id: StateId,
        width: Width,
        locale: Option<String>,
        status: CellStatus,
        image_blob: Option<BlobId>,
        evidence_id: Option<EvidenceId>,
        cache: Option<CacheOutcome>,
    ) -> Result<Self, DomainError> {
        if status.is_unrenderable() && image_blob.is_some() {
            return Err(DomainError::UnrenderableHasImage);
        }
        let locale = locale
            .map(|l| NonEmptyText::parse("locale", l))
            .transpose()?;
        let id = CellId::derive(&json!([
            run_id,
            screen_id,
            state_id,
            width,
            locale.as_ref().map(NonEmptyText::as_str)
        ]));
        Ok(Self {
            id,
            run_id,
            screen_id,
            state_id,
            width,
            locale,
            status,
            image_blob,
            evidence_id,
            cache,
        })
    }

    pub fn id(&self) -> &CellId {
        &self.id
    }

    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    pub fn screen_id(&self) -> &ScreenId {
        &self.screen_id
    }

    pub fn state_id(&self) -> &StateId {
        &self.state_id
    }

    pub fn width(&self) -> Width {
        self.width
    }

    pub fn locale(&self) -> Option<&str> {
        self.locale.as_ref().map(NonEmptyText::as_str)
    }

    pub fn status(&self) -> &CellStatus {
        &self.status
    }

    pub fn image_blob(&self) -> Option<&BlobId> {
        self.image_blob.as_ref()
    }

    pub fn evidence_id(&self) -> Option<&EvidenceId> {
        self.evidence_id.as_ref()
    }

    pub fn cache(&self) -> Option<&CacheOutcome> {
        self.cache.as_ref()
    }

    pub fn diff_key(&self) -> CellKey {
        CellKey {
            screen_id: self.screen_id.clone(),
            state_id: self.state_id.clone(),
            width: self.width,
            locale: self.locale().map(str::to_owned),
        }
    }
}

/// Content-addressed immutable value object holding runtime observation evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    id: EvidenceId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    screen_evidence: Option<Value>,
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
}

impl Evidence {
    #[expect(
        clippy::too_many_arguments,
        reason = "constructs an immutable evidence value object with its canonical digest in one step"
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
            screen_evidence: None,
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

    pub fn with_screen_evidence(mut self, evidence: Option<&Value>) -> Self {
        if let Some(value) = evidence.filter(|value| value.is_object()) {
            self.screen_evidence = Some(value.clone());
            let mut payload = json!(&self);
            if let Value::Object(fields) = &mut payload {
                fields.remove("id");
            }
            self.id = EvidenceId::derive(&payload);
        }
        self
    }

    pub fn screen_evidence(&self) -> Option<&Value> {
        self.screen_evidence.as_ref()
    }

    pub fn id(&self) -> &EvidenceId {
        &self.id
    }

    pub fn observed_path(&self) -> Option<&str> {
        self.observed_path.as_deref()
    }

    pub fn observed_width(&self) -> Option<u32> {
        self.observed_width
    }

    pub fn ready(&self) -> Option<bool> {
        self.ready
    }

    pub fn settled(&self) -> Option<bool> {
        self.settled
    }

    pub fn slots(&self) -> &BTreeMap<String, Value> {
        &self.slots
    }

    pub fn responses(&self) -> &BTreeMap<String, u16> {
        &self.responses
    }

    pub fn assertions(&self) -> &[Value] {
        &self.assertions
    }

    pub fn hits(&self) -> &BTreeSet<String> {
        &self.hits
    }

    pub fn links(&self) -> &BTreeSet<String> {
        &self.links
    }
}

/// Content-addressed immutable value object describing a rendered image stored on disk.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageBlob {
    id: BlobId,
    media_type: NonEmptyText,
    byte_len: u64,
    width_px: u32,
    height_px: u32,
}

impl ImageBlob {
    pub fn from_bytes(
        media_type: impl Into<String>,
        bytes: &[u8],
        width_px: u32,
        height_px: u32,
    ) -> Result<Self, DomainError> {
        Self::metadata(
            BlobId::from_image_bytes(bytes),
            media_type,
            bytes.len() as u64,
            width_px,
            height_px,
        )
    }

    pub fn metadata(
        id: BlobId,
        media_type: impl Into<String>,
        byte_len: u64,
        width_px: u32,
        height_px: u32,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            id,
            media_type: NonEmptyText::parse("media_type", media_type)?,
            byte_len,
            width_px,
            height_px,
        })
    }

    pub fn id(&self) -> &BlobId {
        &self.id
    }

    pub fn media_type(&self) -> &str {
        self.media_type.as_str()
    }

    pub fn byte_len(&self) -> u64 {
        self.byte_len
    }

    pub fn width_px(&self) -> u32 {
        self.width_px
    }

    pub fn height_px(&self) -> u32 {
        self.height_px
    }
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

impl EdgeEndpoint {
    pub fn screen_id(&self) -> &ScreenId {
        match self {
            Self::Screen { screen_id } | Self::State { screen_id, .. } => screen_id,
        }
    }

    pub fn state_id(&self) -> Option<&StateId> {
        match self {
            Self::Screen { .. } => None,
            Self::State { state_id, .. } => Some(state_id),
        }
    }
}

/// Transition edge owned by a `RunAggregate`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    id: EdgeId,
    run_id: RunId,
    from: EdgeEndpoint,
    to: EdgeEndpoint,
    source: EdgeSource,
    trigger: Option<String>,
    selector: Option<String>,
    observed_widths: BTreeSet<Width>,
}

impl Edge {
    pub fn discovered(
        run_id: RunId,
        from: EdgeEndpoint,
        to: EdgeEndpoint,
        selector: Option<String>,
        observed_widths: BTreeSet<Width>,
    ) -> Self {
        let id = EdgeId::derive(&json!([
            run_id,
            from,
            to,
            EdgeSource::Extracted,
            Value::Null
        ]));
        Self {
            id,
            run_id,
            from,
            to,
            source: EdgeSource::Extracted,
            trigger: None,
            selector,
            observed_widths,
        }
    }

    pub fn inferred(
        run_id: RunId,
        from: EdgeEndpoint,
        to: EdgeEndpoint,
        trigger: Option<String>,
    ) -> Self {
        let id = EdgeId::derive(&json!([run_id, from, to, EdgeSource::Inferred, trigger]));
        Self {
            id,
            run_id,
            from,
            to,
            source: EdgeSource::Inferred,
            trigger,
            selector: None,
            observed_widths: BTreeSet::new(),
        }
    }

    /// Promotes a discovered or inferred edge to `EdgeSource::Verified` after replay confirms it.
    pub fn confirm_by_replay(
        &self,
        run_id: RunId,
        confirmed_widths: BTreeSet<Width>,
    ) -> Result<Self, DomainError> {
        if confirmed_widths.is_empty() {
            return Err(DomainError::ReplayRequiresWidths);
        }
        let id = EdgeId::derive(&json!([
            run_id,
            self.from,
            self.to,
            EdgeSource::Verified,
            self.trigger
        ]));
        Ok(Self {
            id,
            run_id,
            from: self.from.clone(),
            to: self.to.clone(),
            source: EdgeSource::Verified,
            trigger: self.trigger.clone(),
            selector: self.selector.clone(),
            observed_widths: confirmed_widths,
        })
    }

    pub fn id(&self) -> &EdgeId {
        &self.id
    }

    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    pub fn from(&self) -> &EdgeEndpoint {
        &self.from
    }

    pub fn to(&self) -> &EdgeEndpoint {
        &self.to
    }

    pub fn source(&self) -> EdgeSource {
        self.source
    }

    pub fn selector(&self) -> Option<&str> {
        self.selector.as_deref()
    }

    pub fn observed_widths(&self) -> &BTreeSet<Width> {
        &self.observed_widths
    }
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
            match cell.status() {
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

/// Aggregate root for one published capture, replay, or live-saved run.
/// Guarantees referential consistency across all owned cells, edges, screens, and states.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAggregate {
    run: Run,
    screens: BTreeMap<ScreenId, Screen>,
    states: BTreeMap<StateId, State>,
    cells: Vec<Cell>,
    edges: Vec<Edge>,
}

impl RunAggregate {
    pub fn publish(
        run: Run,
        screens: Vec<Screen>,
        states: Vec<State>,
        cells: Vec<Cell>,
        edges: Vec<Edge>,
    ) -> Result<Self, DomainError> {
        let screen_map: BTreeMap<ScreenId, Screen> =
            screens.into_iter().map(|s| (s.id().clone(), s)).collect();
        let state_map: BTreeMap<StateId, State> =
            states.into_iter().map(|s| (s.id().clone(), s)).collect();
        for state in state_map.values() {
            if !screen_map.contains_key(state.screen_id()) {
                return Err(DomainError::MissingReference("screen"));
            }
        }
        let mut seen_cells = BTreeSet::new();
        for cell in &cells {
            if cell.run_id() != run.id() {
                return Err(DomainError::AggregateRunMismatch("cell"));
            }
            if !screen_map.contains_key(cell.screen_id()) {
                return Err(DomainError::MissingReference("screen"));
            }
            let Some(state) = state_map.get(cell.state_id()) else {
                return Err(DomainError::MissingReference("state"));
            };
            if state.screen_id() != cell.screen_id() {
                return Err(DomainError::StateScreenMismatch);
            }
            if !seen_cells.insert(cell.id().clone()) {
                return Err(DomainError::DuplicateCellId);
            }
        }
        for edge in &edges {
            if edge.run_id() != run.id() {
                return Err(DomainError::AggregateRunMismatch("edge"));
            }
            for endpoint in [edge.from(), edge.to()] {
                if !screen_map.contains_key(endpoint.screen_id()) {
                    return Err(DomainError::MissingReference("edge screen"));
                }
                if let Some(state_id) = endpoint.state_id()
                    && !state_map.contains_key(state_id)
                {
                    return Err(DomainError::MissingReference("edge state"));
                }
            }
        }
        Ok(Self {
            run,
            screens: screen_map,
            states: state_map,
            cells,
            edges,
        })
    }

    pub fn run(&self) -> &Run {
        &self.run
    }

    pub fn screens(&self) -> &BTreeMap<ScreenId, Screen> {
        &self.screens
    }

    pub fn states(&self) -> &BTreeMap<StateId, State> {
        &self.states
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn coverage(&self) -> CoverageSummary {
        CoverageSummary::from_cells(&self.cells)
    }

    pub fn cells_for_screen(&self, screen_id: &ScreenId) -> Vec<&Cell> {
        self.cells
            .iter()
            .filter(|c| c.screen_id() == screen_id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Commit, Project, Worktree};

    #[test]
    fn cell_status_and_unrenderable_image_invariants_are_enforced() {
        assert_eq!(
            CellStatus::from_wire("verified", None).unwrap(),
            CellStatus::verified()
        );
        assert!(CellStatus::unverified("   ").is_err());
        assert!(CellStatus::unrenderable("").is_err());
        assert!(CellStatus::from_wire("unknown", None).is_err());

        let screen = Screen::new("/", ScreenOrigin::Entry).unwrap();
        let state = State::new(screen.id().clone(), "default", None, BTreeMap::new(), 0).unwrap();
        let res = Cell::new(
            RunId::parse("run-1").unwrap(),
            screen.id().clone(),
            state.id().clone(),
            Width::new(375).unwrap(),
            None,
            CellStatus::unrenderable("no-render-env").unwrap(),
            Some(BlobId::from_image_bytes(b"png")),
            None,
            None,
        );
        assert_eq!(res, Err(DomainError::UnrenderableHasImage));
    }

    #[test]
    fn edge_requires_replay_confirmation_to_become_verified() {
        let run_id = RunId::parse("run-1").unwrap();
        let s1 = Screen::derive_id("/graph");
        let s2 = Screen::derive_id("/");
        let w = Width::new(1280).unwrap();
        let discovered = Edge::discovered(
            run_id.clone(),
            EdgeEndpoint::Screen { screen_id: s1 },
            EdgeEndpoint::Screen { screen_id: s2 },
            Some("header a[href]".into()),
            BTreeSet::from([w]),
        );
        assert_eq!(discovered.source(), EdgeSource::Extracted);
        assert_eq!(
            discovered.confirm_by_replay(run_id.clone(), BTreeSet::new()),
            Err(DomainError::ReplayRequiresWidths)
        );
        let verified = discovered
            .confirm_by_replay(RunId::parse("run-2").unwrap(), BTreeSet::from([w]))
            .unwrap();
        assert_eq!(verified.source(), EdgeSource::Verified);
    }

    #[test]
    fn run_aggregate_rejects_mismatched_run_ids_and_dangling_references() {
        let project = Project::new("/repo/demo", "demo").unwrap();
        let wt = Worktree::new(project.id().clone(), "/repo/demo", "", None).unwrap();
        let commit = Commit::new(project.id().clone(), Some("abc".into()), false, None);
        let run_id = RunId::parse("run-1").unwrap();
        let run = Run::new(
            run_id.clone(),
            project.id().clone(),
            wt.id().clone(),
            commit.id().clone(),
            RunKind::Capture,
            100,
            RunEnvironment::default(),
            BTreeSet::new(),
        );
        let s1 = Screen::new("/", ScreenOrigin::Entry).unwrap();
        let s2 = Screen::new("/other", ScreenOrigin::Entry).unwrap();
        let st1 = State::new(s1.id().clone(), "default", None, BTreeMap::new(), 0).unwrap();
        let bad_cell = Cell::new(
            RunId::parse("other-run").unwrap(),
            s1.id().clone(),
            st1.id().clone(),
            Width::new(375).unwrap(),
            None,
            CellStatus::verified(),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            RunAggregate::publish(
                run.clone(),
                vec![s1.clone()],
                vec![st1.clone()],
                vec![bad_cell],
                vec![],
            ),
            Err(DomainError::AggregateRunMismatch("cell"))
        );

        let wrong_screen_cell = Cell::new(
            run_id.clone(),
            s2.id().clone(),
            st1.id().clone(),
            Width::new(375).unwrap(),
            None,
            CellStatus::verified(),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            RunAggregate::publish(
                run,
                vec![s1, s2],
                vec![st1],
                vec![wrong_screen_cell],
                vec![],
            ),
            Err(DomainError::StateScreenMismatch)
        );
    }
}
