use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    screen: String,
    widths: BTreeSet<NonZeroU32>,
    states: BTreeMap<String, BTreeMap<String, Value>>,
}

// A structured key also round-trips screen and state names containing separators.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CellId {
    screen: String,
    pub state: String,
    pub width: NonZeroU32,
}

#[derive(Serialize)]
pub struct Job<'a> {
    pub id: CellId,
    pub slots: &'a BTreeMap<String, Value>,
}

pub fn plan(fixture: &Fixture) -> Result<Vec<Job<'_>>, &'static str> {
    if fixture.screen.trim().is_empty() || fixture.widths.is_empty() || fixture.states.is_empty() {
        return Err("screen, widths and states are required");
    }
    let mut jobs = Vec::new();
    for (state, slots) in &fixture.states {
        if state.trim().is_empty() || slots.is_empty() || slots.keys().any(|k| k.trim().is_empty())
        {
            return Err("each state needs a name and named slots");
        }
        for width in &fixture.widths {
            jobs.push(Job {
                id: CellId {
                    screen: fixture.screen.clone(),
                    state: state.clone(),
                    width: *width,
                },
                slots,
            });
        }
    }
    Ok(jobs)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Fixture {
        serde_json::from_value(serde_json::json!({
            "screen":"list","widths":[360,768,1280],
            "states":{"closed":{"open":false},"open":{"open":true}}
        }))
        .unwrap()
    }
    #[test]
    fn planned_jobs_are_unique_and_deterministic() {
        let fixture = fixture();
        let jobs = plan(&fixture).unwrap();
        assert_eq!(jobs.len(), 6);
        assert_eq!(
            jobs.iter()
                .map(|job| &job.id)
                .collect::<BTreeSet<_>>()
                .len(),
            6
        );
        assert_eq!(
            serde_json::to_vec(&jobs).unwrap(),
            serde_json::to_vec(&plan(&fixture).unwrap()).unwrap()
        );
    }

    #[test]
    fn zero_width_is_invalid() {
        assert!(
            serde_json::from_str::<Fixture>(
                r#"{"screen":"s","states":{"a":{"x":false}},"widths":[0]}"#
            )
            .is_err()
        );
    }
}
pub mod discovery;

pub mod mock;
pub mod tabular;
pub mod verification;

pub mod android;
pub mod audit;
pub mod auth;
pub mod compiler_oracle;
pub mod diff;
pub mod edit;
pub mod html_links;
pub mod inspect;
pub mod ios;
pub mod locale;
pub mod model;
pub mod nextjs;
pub mod settle;
pub mod storage;
pub mod surface;
pub mod transition;
pub mod tui;
pub mod version;
