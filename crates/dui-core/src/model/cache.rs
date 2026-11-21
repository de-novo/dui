//! Build cache bounded context: canonical cell input hashing, hit/miss evaluation,
//! and mutable cache pointers referencing immutable artifacts.

use super::{BlobId, CacheKey, CellStatus, ContentHash, DomainError, EvidenceId, StateId, Width};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

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

/// Value object holding every canonical input digest that determines a cell's rendered output.
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

/// Pure domain service: decides cache reuse and names every changed input on a miss.
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

/// Aggregate root for a cached cell entry pointing to immutable artifacts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheEntry {
    key: CacheKey,
    inputs: CacheInputs,
    status: CellStatus,
    image_blob: Option<BlobId>,
    evidence_id: Option<EvidenceId>,
    created_at_ms: i64,
    last_hit_at_ms: i64,
}

impl CacheEntry {
    pub fn new(
        inputs: CacheInputs,
        status: CellStatus,
        image_blob: Option<BlobId>,
        evidence_id: Option<EvidenceId>,
        created_at_ms: i64,
    ) -> Result<Self, DomainError> {
        if status.is_unrenderable() && image_blob.is_some() {
            return Err(DomainError::UnrenderableHasImage);
        }
        let key = inputs.key();
        Ok(Self {
            key,
            inputs,
            status,
            image_blob,
            evidence_id,
            created_at_ms,
            last_hit_at_ms: created_at_ms,
        })
    }

    pub fn key(&self) -> &CacheKey {
        &self.key
    }

    pub fn inputs(&self) -> &CacheInputs {
        &self.inputs
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

    pub fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    pub fn last_hit_at_ms(&self) -> i64 {
        self.last_hit_at_ms
    }

    pub fn record_hit(&mut self, now_ms: i64) -> Result<(), DomainError> {
        if now_ms < self.last_hit_at_ms {
            return Err(DomainError::TimestampRegression("last_hit_at_ms"));
        }
        self.last_hit_at_ms = now_ms;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Screen, State};

    fn sample_inputs(width: u32, locale: Option<&str>) -> CacheInputs {
        let screen_id = Screen::derive_id("/");
        let state = State::new(
            screen_id,
            "default",
            locale.map(str::to_owned),
            BTreeMap::new(),
            0,
        )
        .unwrap();
        CacheInputs {
            source_files: BTreeMap::from([(
                "src/App.tsx".into(),
                ContentHash::from_bytes(b"export default function App() {}"),
            )]),
            types_digest: ContentHash::from_bytes(b"types-v1"),
            mocks_digest: ContentHash::from_bytes(b"mocks-v1"),
            state_id: state.id().clone(),
            width: Width::new(width).unwrap(),
            locale: locale.map(str::to_owned),
            dui_version: "0.0.2".into(),
            rule_pack_digest: ContentHash::from_bytes(b"react-v1"),
            renderer_env_digest: ContentHash::from_bytes(b"chrome-157-macos"),
        }
    }

    #[test]
    fn cache_evaluation_and_hit_tracking_enforce_rules() {
        let base = sample_inputs(1280, Some("en"));
        assert_eq!(
            evaluate_cache(&base, Some(&base), false),
            CacheOutcome::Hit { key: base.key() }
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

        let mut entry = CacheEntry::new(base, CellStatus::verified(), None, None, 100).unwrap();
        entry.record_hit(250).unwrap();
        assert_eq!(entry.last_hit_at_ms(), 250);
        assert_eq!(
            entry.record_hit(200),
            Err(DomainError::TimestampRegression("last_hit_at_ms"))
        );
    }
}
