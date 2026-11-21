//! Pure transition replay rules and edge provenance verdicts (`extracted` / `inferred` / `verified`).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeProvenance {
    Extracted,
    Inferred,
    Verified,
}

impl EdgeProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Extracted => "extracted",
            Self::Inferred => "inferred",
            Self::Verified => "verified",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionAction {
    pub kind: String,
    pub selector: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionSpec {
    pub from: String,
    pub to: String,
    pub action: TransitionAction,
    pub origin: EdgeProvenance,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expect_selector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expect_text: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionObservation {
    pub width: u32,
    pub started_at: String,
    pub landed_at: String,
    pub action_executed: bool,
    pub destination_ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_text_matched: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionVerdict {
    pub from: String,
    pub to: String,
    pub action: TransitionAction,
    pub origin: EdgeProvenance,
    pub provenance: EdgeProvenance,
    pub replay_verified: bool,
    pub confirmed_widths: Vec<u32>,
    pub failed_widths: BTreeMap<u32, String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionSummary {
    pub total: usize,
    pub verified: usize,
    pub extracted: usize,
    pub inferred: usize,
    pub failed: usize,
}

fn width_failure(spec: &TransitionSpec, obs: &TransitionObservation) -> Option<String> {
    if let Some(err) = obs.error.as_deref().filter(|s| !s.is_empty()) {
        return Some(err.to_owned());
    }
    if obs.started_at != spec.from {
        return Some(format!(
            "initial-state-mismatch: expected {}, started at {}",
            spec.from, obs.started_at
        ));
    }
    if !obs.action_executed {
        return Some("action-not-executed".to_owned());
    }
    if obs.landed_at != spec.to {
        return Some(format!(
            "destination-mismatch: expected {}, landed at {}",
            spec.to, obs.landed_at
        ));
    }
    if !obs.destination_ready {
        return Some("destination-not-ready".to_owned());
    }
    if obs.expected_text_matched == Some(false) {
        return Some("destination-text-mismatch".to_owned());
    }
    None
}

/// Judge a candidate transition across widths.
/// An edge is promoted to `EdgeProvenance::Verified` only when at least one width replayed from
/// the initial state, executed the action, landed on the expected destination, passed readiness
/// and text checks, and zero attempted widths failed.
pub fn judge_transition(
    spec: &TransitionSpec,
    observations: &[TransitionObservation],
) -> TransitionVerdict {
    let base_origin = match spec.origin {
        EdgeProvenance::Extracted | EdgeProvenance::Verified => EdgeProvenance::Extracted,
        EdgeProvenance::Inferred => EdgeProvenance::Inferred,
    };

    let mut confirmed = BTreeSet::new();
    let mut failed = BTreeMap::new();

    for obs in observations {
        match width_failure(spec, obs) {
            None => {
                confirmed.insert(obs.width);
            }
            Some(reason) => {
                failed.insert(obs.width, reason);
            }
        }
    }

    let replay_verified = !confirmed.is_empty() && failed.is_empty();
    let provenance = if replay_verified {
        EdgeProvenance::Verified
    } else {
        base_origin
    };

    TransitionVerdict {
        from: spec.from.clone(),
        to: spec.to.clone(),
        action: spec.action.clone(),
        origin: base_origin,
        provenance,
        replay_verified,
        confirmed_widths: confirmed.into_iter().collect(),
        failed_widths: failed,
    }
}

pub fn summarize_transitions(verdicts: &[TransitionVerdict]) -> TransitionSummary {
    let mut verified = 0usize;
    let mut extracted = 0usize;
    let mut inferred = 0usize;
    let mut failed = 0usize;
    for v in verdicts {
        match v.provenance {
            EdgeProvenance::Verified => verified += 1,
            EdgeProvenance::Extracted => extracted += 1,
            EdgeProvenance::Inferred => inferred += 1,
        }
        if !v.failed_widths.is_empty() {
            failed += 1;
        }
    }
    TransitionSummary {
        total: verdicts.len(),
        verified,
        extracted,
        inferred,
        failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spec(origin: EdgeProvenance) -> TransitionSpec {
        TransitionSpec {
            from: "/order".to_owned(),
            to: "/confirm".to_owned(),
            action: TransitionAction {
                kind: "click".to_owned(),
                selector: "[data-testid=cancel-order]".to_owned(),
                label: Some("Cancel order".to_owned()),
            },
            origin,
            expect_selector: Some("[data-testid=confirm-screen]".to_owned()),
            expect_text: Some("Confirm cancellation".to_owned()),
        }
    }

    #[test]
    fn confirms_extracted_edge_when_replayed_from_initial_state() {
        let spec = sample_spec(EdgeProvenance::Extracted);
        let obs = vec![
            TransitionObservation {
                width: 375,
                started_at: "/order".to_owned(),
                landed_at: "/confirm".to_owned(),
                action_executed: true,
                destination_ready: true,
                expected_text_matched: Some(true),
                error: None,
            },
            TransitionObservation {
                width: 1280,
                started_at: "/order".to_owned(),
                landed_at: "/confirm".to_owned(),
                action_executed: true,
                destination_ready: true,
                expected_text_matched: Some(true),
                error: None,
            },
        ];
        let verdict = judge_transition(&spec, &obs);
        assert_eq!(verdict.provenance, EdgeProvenance::Verified);
        assert_eq!(verdict.origin, EdgeProvenance::Extracted);
        assert!(verdict.replay_verified);
        assert_eq!(verdict.confirmed_widths, vec![375, 1280]);
        assert!(verdict.failed_widths.is_empty());
    }

    #[test]
    fn destination_mismatch_keeps_inferred_edge_unverified_with_reason() {
        let spec = sample_spec(EdgeProvenance::Inferred);
        let obs = vec![TransitionObservation {
            width: 375,
            started_at: "/order".to_owned(),
            landed_at: "/order".to_owned(),
            action_executed: true,
            destination_ready: false,
            expected_text_matched: None,
            error: None,
        }];
        let verdict = judge_transition(&spec, &obs);
        assert_eq!(verdict.provenance, EdgeProvenance::Inferred);
        assert!(!verdict.replay_verified);
        assert!(verdict.confirmed_widths.is_empty());
        assert!(
            verdict
                .failed_widths
                .get(&375)
                .is_some_and(|r| r.contains("destination-mismatch"))
        );
    }

    #[test]
    fn never_promotes_without_replay_observations() {
        let spec = sample_spec(EdgeProvenance::Extracted);
        let verdict = judge_transition(&spec, &[]);
        assert_eq!(verdict.provenance, EdgeProvenance::Extracted);
        assert!(!verdict.replay_verified);
    }
}
