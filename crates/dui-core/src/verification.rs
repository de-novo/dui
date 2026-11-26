use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub struct Intent<'a> {
    pub path: &'a str,
    pub width: u32,
    pub inputs: &'a BTreeMap<String, Value>,
    pub assertions: &'a [Value],
    pub required_requests: &'a [Value],
    /// Manifest `expectedApiErrors`: API paths (or `prefix*`) whose 401, 403 or 5xx is part of
    /// the screen, so they do not fail it.
    pub expected_api_errors: &'a [Value],
    pub analyzed: bool,
    pub expects_probe: bool,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Failure {
    MissingStateAssertion,
    MissingScreenAssertion,
    HttpError,
    ApiError,
    /// The ready selector matched, but the screen kept changing until the settle budget ran out.
    Unsettled,
    AssertFailed,
}

/// API responses that failed the screen: `observed["apiErrors"]` (path → 401, 403 or 5xx),
/// minus the endpoints the manifest expects to fail. Sorted by path.
pub fn unexpected_api_errors(observed: &Value, expected: &[Value]) -> Vec<(String, u64)> {
    let expected_match = |path: &str| {
        let (base, _) = parse_url_parts(path);
        expected
            .iter()
            .filter_map(Value::as_str)
            .any(|e| match e.strip_suffix('*') {
                Some(prefix) => base.starts_with(prefix),
                None => base == e,
            })
    };
    observed["apiErrors"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(path, status)| Some((path.clone(), status.as_u64()?)))
        .filter(|(path, _)| !expected_match(path))
        .collect()
}

/// Inspect `observed["responses"]` for the primary navigated document status code.
/// Returns `Some(status)` if the navigated document responded with HTTP 4xx/5xx,
/// ignoring ancillary subresources like `/favicon.ico`.
pub fn document_http_error(expected_path: &str, observed: &Value) -> Option<u64> {
    let responses = observed["responses"].as_object()?;
    let obs_path = observed["path"].as_str().unwrap_or(expected_path);
    let (obs_base, _) = parse_url_parts(obs_path);
    let (exp_base, _) = parse_url_parts(expected_path);
    for candidate in [obs_path, expected_path, obs_base, exp_base] {
        if candidate.is_empty() {
            continue;
        }
        if let Some(code) = responses.get(candidate).and_then(Value::as_u64) {
            return (code >= 400).then_some(code);
        }
    }
    None
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlMatch {
    pub matched: bool,
    pub observed_query: BTreeMap<String, String>,
    pub normalized_query: BTreeMap<String, String>,
}

pub fn parse_url_parts(raw: &str) -> (&str, BTreeMap<String, String>) {
    let no_hash = raw.split('#').next().unwrap_or(raw);
    let (pathname, query_str) = match no_hash.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (no_hash, None),
    };
    let mut query = BTreeMap::new();
    if let Some(qs) = query_str {
        for pair in qs.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            query.insert(k.to_owned(), v.to_owned());
        }
    }
    (pathname, query)
}

/// Compare an expected route/query against the observed browser URL.
/// - Pathname must match strictly (redirects to another screen fail).
/// - Every explicitly requested query parameter must match identically.
/// - Extra default query parameters appended by the app on the same pathname are
///   recorded transparently in `normalized_query` without causing a false mismatch.
pub fn analyze_url_match(expected: &str, observed: &str) -> UrlMatch {
    let (exp_path, exp_query) = parse_url_parts(expected);
    let (obs_path, obs_query) = parse_url_parts(observed);
    let pathname_matches = exp_path == obs_path;
    let expected_query_matches = exp_query
        .iter()
        .all(|(k, v)| obs_query.get(k).is_some_and(|ov| ov == v));
    let mut normalized_query = BTreeMap::new();
    if pathname_matches {
        for (k, v) in &obs_query {
            if !exp_query.contains_key(k) {
                normalized_query.insert(k.clone(), v.clone());
            }
        }
    }
    UrlMatch {
        matched: pathname_matches && expected_query_matches,
        observed_query: obs_query,
        normalized_query,
    }
}

fn route_reached(requested: &str, observed: &Value) -> bool {
    observed["path"].as_str().is_some_and(|obs_path| {
        analyze_url_match(requested, obs_path).matched
            || (requested == "/"
                && observed["redirectedTo"].as_str() == Some(obs_path)
                && obs_path.starts_with('/')
                && !obs_path.starts_with("//")
                && observed["responses"]["/"] == 200
                && observed["renderTree"]
                    .as_array()
                    .is_some_and(|tree| !tree.is_empty()))
    })
}

/// A captured document can be ready even when no project condition identifies the screen.
pub fn screen_evidence(spec: &Value, observed: &Value) -> Value {
    let requested = spec["path"].as_str().unwrap_or("/");
    let configured = spec["hasScreenAssertion"] != false && spec["readySelector"].is_string();
    let heading = observed["renderTree"].as_array().and_then(|tree| {
        tree.iter().find_map(|node| {
            (node["tag"] == "h1" || node["role"] == "heading")
                .then(|| node["text"].as_str())
                .flatten()
                .filter(|text| !text.is_empty())
        })
    });
    let route_matches = observed["path"].as_str().map(|actual| {
        analyze_url_match(requested, actual).matched
            && requested.split_once('#').is_none_or(|(_, fragment)| {
                actual
                    .split_once('#')
                    .is_some_and(|(_, got)| got == fragment)
            })
    });
    let verified = configured
        && observed["selectorVisible"] == true
        && observed["ready"] == true
        && observed["settled"] == true
        && observed["documentStatus"]
            .as_u64()
            .is_some_and(|s| (200..300).contains(&s))
        && (route_matches == Some(true)
            || (requested == "/" && route_reached(requested, observed)));
    serde_json::json!({
        "canonicalPath": observed["canonicalPath"],
        "canonicalMatches": observed["canonicalPath"].as_str().zip(observed["path"].as_str())
            .map(|(declared, actual)| declared == actual),
        "screenStatus": if verified { "verified" } else { "unverified" },
        "requestedPath": requested, "observedPath": observed["path"],
        "routeMatches": route_matches,
        "documentStatus": observed["documentStatus"], "heading": heading, "settled": observed["settled"],
        "selector": spec["readySelector"], "selectorMatched": observed["ready"], "selectorVisible": observed["selectorVisible"],
        "selectorSource": if configured { "project" } else { "default" },
    })
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
        if document_http_error(self.path, observed).is_some() {
            return Some(Failure::HttpError);
        }
        if !unexpected_api_errors(observed, self.expected_api_errors).is_empty() {
            return Some(Failure::ApiError);
        }
        if observed["settled"] == false && observed["ready"] == true {
            return Some(Failure::Unsettled);
        }
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
            && route_reached(self.path, observed)
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
    fn screen_verification_requires_independent_observations() {
        let spec = json!({"path": "/record", "readySelector": "[data-ready]"});
        let observed = json!({"path": "/record", "documentStatus": 200, "ready": true,
            "selectorVisible": true, "settled": true, "renderTree":[{"tag":"h1","text":"404"}]});
        assert_eq!(
            screen_evidence(&spec, &observed)["screenStatus"],
            "verified"
        );
        for (key, value) in [
            ("path", json!("/elsewhere")),
            ("documentStatus", json!(404)),
            ("documentStatus", Value::Null),
            ("ready", json!(false)),
            ("selectorVisible", json!(false)),
            ("settled", json!(false)),
        ] {
            let mut changed = observed.clone();
            changed[key] = value;
            assert_eq!(
                screen_evidence(&spec, &changed)["screenStatus"],
                "unverified"
            );
        }
        let mut fallback = spec;
        fallback["hasScreenAssertion"] = json!(false);
        assert_eq!(
            screen_evidence(&fallback, &observed)["screenStatus"],
            "unverified"
        );
    }

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
            expected_api_errors: &[],
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
            let expected = if key == "settled" {
                Failure::Unsettled
            } else {
                Failure::AssertFailed
            };
            assert_eq!(intent.failure(&broken), Some(expected));
        }
    }

    #[test]
    fn a_failed_api_call_fails_the_screen_unless_the_manifest_expects_it() {
        let values = BTreeMap::new();
        let expected = [json!("/api/session"), json!("/api/admin/*")];
        let intent = Intent {
            path: "/account",
            width: 375,
            inputs: &values,
            assertions: &[],
            required_requests: &[],
            expected_api_errors: &expected,
            analyzed: true,
            expects_probe: false,
        };
        let clean = json!({"ready":true,"width":375,"path":"/account","assertions":[],"slots":{},"responseChecks":{},"apiErrors":{}});
        assert_eq!(intent.failure(&clean), None);
        let mut allowed = clean.clone();
        allowed["apiErrors"] = json!({"/api/session?x=1":401,"/api/admin/users":403});
        assert_eq!(intent.failure(&allowed), None);
        let mut failed = allowed.clone();
        failed["apiErrors"]["/api/me"] = json!(401);
        failed["apiErrors"]["/api/orders"] = json!(503);
        assert_eq!(intent.failure(&failed), Some(Failure::ApiError));
        assert_eq!(
            unexpected_api_errors(&failed, &expected),
            [("/api/me".to_owned(), 401), ("/api/orders".to_owned(), 503)]
        );
    }

    #[test]
    fn entry_redirect_requires_document_and_render_evidence() {
        let intent = Intent {
            path: "/",
            width: 375,
            inputs: &BTreeMap::new(),
            assertions: &[],
            required_requests: &[],
            expected_api_errors: &[],
            analyzed: true,
            expects_probe: false,
        };
        let sample = json!({"ready": true, "width": 375, "path": "/dashboard", "redirectedTo": "/dashboard",
            "responses": {"/": 200}, "renderTree": [{"tag":"main"}], "responseChecks": {}, "assertions": []});
        assert_eq!(intent.failure(&sample), None);
        for (key, value) in [
            ("redirectedTo", Value::Null),
            ("renderTree", json!([])),
            ("responses", json!({})),
        ] {
            let mut broken = sample.clone();
            broken[key] = value;
            assert!(intent.failure(&broken).is_some());
        }
        assert!(
            Intent {
                path: "/admin",
                ..intent
            }
            .failure(&sample)
            .is_some()
        );
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

    #[test]
    fn normalizes_extra_default_query_params_while_rejecting_mismatched_routes_or_expected_queries()
    {
        let norm = analyze_url_match(
            "/o/local/b/local-team/generation",
            "/o/local/b/local-team/generation?project=prj_local-team",
        );
        assert!(norm.matched);
        assert_eq!(
            norm.normalized_query.get("project").map(String::as_str),
            Some("prj_local-team")
        );

        let wrong_query = analyze_url_match("/grid?item=A", "/grid?item=B");
        assert!(!wrong_query.matched);

        let redirect = analyze_url_match("/admin", "/login?next=%2Fadmin");
        assert!(!redirect.matched);
    }

    #[test]
    fn rejects_404_and_500_document_responses_while_ignoring_favicon_404() {
        let intent = Intent {
            path: "/missing",
            width: 1280,
            inputs: &BTreeMap::new(),
            assertions: &[],
            required_requests: &[],
            expected_api_errors: &[],
            analyzed: true,
            expects_probe: false,
        };
        let page_404 = serde_json::json!({
            "ready": true,
            "settled": true,
            "path": "/missing",
            "width": 1280,
            "slots": {},
            "assertions": [],
            "responseChecks": {},
            "responses": {"/missing": 404}
        });
        assert_eq!(intent.failure(&page_404), Some(Failure::HttpError));

        let ok_intent = Intent {
            path: "/home",
            ..intent
        };
        let page_200_with_missing_favicon = serde_json::json!({
            "ready": true,
            "settled": true,
            "path": "/home",
            "width": 1280,
            "slots": {},
            "assertions": [],
            "responseChecks": {},
            "responses": {"/home": 200, "/favicon.ico": 404}
        });
        assert_eq!(ok_intent.failure(&page_200_with_missing_favicon), None);

        let page_500 = serde_json::json!({
            "ready": true,
            "settled": true,
            "path": "/home",
            "width": 1280,
            "slots": {},
            "assertions": [],
            "responseChecks": {},
            "responses": {"/home": 500}
        });
        assert_eq!(ok_intent.failure(&page_500), Some(Failure::HttpError));
    }
}
