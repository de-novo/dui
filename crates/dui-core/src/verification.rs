use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub struct Intent<'a> {
    pub path: &'a str,
    pub width: u32,
    pub inputs: &'a BTreeMap<String, Value>,
    pub assertions: &'a [Value],
    pub required_requests: &'a [Value],
    pub analyzed: bool,
    pub expects_probe: bool,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Failure {
    MissingStateAssertion,
    AssertFailed,
}

impl Intent<'_> {
    pub fn failure(&self, observed: &Value) -> Option<Failure> {
        self.failure_with(observed, &std::collections::BTreeSet::new())
    }

    pub fn failure_with(
        &self,
        observed: &Value,
        corroborated: &std::collections::BTreeSet<String>,
    ) -> Option<Failure> {
        let bound = self.inputs.keys().all(|k| {
            corroborated.contains(k)
                || self
                    .assertions
                    .iter()
                    .any(|assertion| assertion["slot"] == *k)
        });
        if !bound {
            return Some(Failure::MissingStateAssertion);
        }
        self.observation_failure(observed)
    }

    pub fn observation_failure(&self, observed: &Value) -> Option<Failure> {
        let values = observed["slots"].as_object();
        let values_match = self
            .inputs
            .iter()
            .all(|(k, v)| values.and_then(|slots| slots.get(k)) == Some(v));
        let targets_match = observed["assertions"].as_array().is_some_and(|results| {
            results.len() == self.assertions.len() && results.iter().all(|v| v == true)
        });
        let responses_match = observed["responseChecks"]
            .as_object()
            .is_some_and(|checks| checks.values().all(|v| v == true))
            && self.required_requests.iter().all(|r| {
                r.as_str()
                    .and_then(|path| observed["responses"][path].as_u64())
                    .is_some_and(|s| (200..300).contains(&s))
            });
        let probe_ran = observed["hits"]
            .as_array()
            .is_some_and(|hits| hits.iter().any(Value::is_string));
        let matches = self.analyzed
            && values_match
            && targets_match
            && responses_match
            && observed["settled"] != false
            && observed["ready"] == true
            && observed["width"] == self.width
            && observed["path"] == self.path
            && (!self.expects_probe || probe_ran);
        (!matches).then_some(Failure::AssertFailed)
    }
}

/// Compare two captures that differ in just this slot, after an independent setter
/// replay has already confirmed the complete target view.
pub fn slot_witness(
    requested: &Value,
    control_value: Option<&Value>,
    target_hash: &str,
    control_hash: &str,
) -> Option<&'static str> {
    let control_value = control_value?;
    if target_hash.is_empty() || control_hash.is_empty() {
        return None;
    }
    match (requested == control_value, target_hash == control_hash) {
        (true, true) => Some("matches-unmodified-value"),
        (false, false) => Some("visible-change"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_matching_image_location_cannot_replace_runtime_evidence() {
        let values = BTreeMap::from([("slot".into(), json!(false))]);
        let assertions = [json!({"slot":"slot"})];
        let requests = [json!("/queue")];
        let intent = Intent {
            path: "/grid?item=A",
            width: 768,
            inputs: &values,
            assertions: &assertions,
            required_requests: &requests,
            analyzed: true,
            expects_probe: true,
        };
        let baseline = json!({"ready":true,"width":768,"path":"/grid?item=A","assertions":[true],"slots":{"slot":false},"responses":{"/queue":200},"responseChecks":{"/queue":true},"hits":["branch:false"]});
        assert_eq!(intent.failure(&baseline), None);
        for (key, value) in [
            ("path", json!("/grid?item=B")),
            ("hits", json!([])),
            ("settled", json!(false)),
            ("slots", json!({})),
            ("assertions", json!([])),
            ("responses", json!({"/queue":404})),
            ("responseChecks", json!({"/queue":false})),
        ] {
            let mut broken = baseline.clone();
            broken[key] = value;
            assert_eq!(intent.failure(&broken), Some(Failure::AssertFailed));
        }
    }

    #[test]
    fn controls_must_distinguish_changed_values_or_reproduce_a_default() {
        let on = json!(true);
        let off = json!(false);
        assert_eq!(
            slot_witness(&on, Some(&off), "visible", "hidden"),
            Some("visible-change")
        );
        assert_eq!(
            slot_witness(&on, Some(&off), "unchanged", "unchanged"),
            None
        );
        assert_eq!(
            slot_witness(&off, Some(&off), "hidden", "hidden"),
            Some("matches-unmodified-value")
        );
        assert_eq!(slot_witness(&off, Some(&off), "unexpected", "hidden"), None);
        assert_eq!(slot_witness(&on, None, "visible", "hidden"), None);
    }
}
