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

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Failure {
    MissingStateAssertion,
    MissingScreenAssertion,
    HttpError,
    ApiError,
    /// The page ended on another route than the one requested.
    Redirected,
    /// The ready selector never matched (possibly while the page kept changing).
    NotReady,
    /// The ready selector matched, but the screen kept changing until the settle budget ran out.
    Unsettled,
    WidthMismatch,
    /// A required request or a response check did not succeed.
    RequestFailed,
    /// The compiler reported diagnostics, so the state plan cannot be trusted.
    NotAnalyzed,
    /// A requested state was not observed: a slot value, a state assertion or the branch probe.
    AssertFailed,
}

/// `NODE CHANGE (+N more)` from `observation.unsettled`, or `None` without that evidence.
fn unsettled_detail(observed: &Value) -> Option<String> {
    let all = observed["unsettled"].as_array()?;
    let first = all.first()?;
    let what = match (first["node"].as_str(), first["field"].as_str()) {
        (Some(node), _) => format!("{node} {}", first["change"].as_str().unwrap_or("changed")),
        (None, Some(field)) => format!("{field} changed"),
        (None, None) => "changed".to_owned(),
    };
    Some(match all.len() {
        1 => what,
        n => format!("{what} (+{} more)", n - 1),
    })
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
        if !route_reached(self.path, observed) {
            return Some(Failure::Redirected);
        }
        if observed["ready"] != true {
            return Some(Failure::NotReady);
        }
        if observed["settled"] == false {
            return Some(Failure::Unsettled);
        }
        if observed["width"] != self.width {
            return Some(Failure::WidthMismatch);
        }
        if self.failed_request(observed).is_some() {
            return Some(Failure::RequestFailed);
        }
        if !self.analyzed {
            return Some(Failure::NotAnalyzed);
        }
        self.unobserved_state(observed)
            .map(|_| Failure::AssertFailed)
    }

    /// The first required request or response check that did not succeed, described.
    fn failed_request(&self, observed: &Value) -> Option<String> {
        if let Some((path, _)) = observed["responseChecks"]
            .as_object()
            .and_then(|checks| checks.iter().find(|(_, ok)| **ok != true))
        {
            return Some(format!("{path} response check"));
        }
        if observed["responseChecks"].as_object().is_none() {
            return Some("no response checks recorded".to_owned());
        }
        self.required_requests.iter().find_map(|r| {
            let path = r.as_str()?;
            match observed["responses"][path].as_u64() {
                Some(status) if (200..300).contains(&status) => None,
                Some(status) => Some(format!("{path} {status}")),
                None => Some(format!("{path} no response")),
            }
        })
    }

    /// The first requested state that was not observed, described.
    fn unobserved_state(&self, observed: &Value) -> Option<String> {
        let values = observed["slots"].as_object();
        if let Some((k, v)) = self
            .inputs
            .iter()
            .find(|(k, v)| values.and_then(|slots| slots.get(*k)) != Some(*v))
        {
            let seen = values
                .and_then(|slots| slots.get(k))
                .unwrap_or(&Value::Null);
            return Some(format!("slot {k} is {seen} (expected {v})"));
        }
        match observed["assertions"].as_array() {
            Some(results) if results.len() != self.assertions.len() => {
                return Some(format!(
                    "{} of {} state assertions evaluated",
                    results.len(),
                    self.assertions.len()
                ));
            }
            Some(results) => {
                if let Some(i) = results.iter().position(|v| v != true) {
                    return Some(format!("state assertion {} failed", i + 1));
                }
            }
            None => return Some("no state assertions recorded".to_owned()),
        }
        let probe_ran = observed["hits"]
            .as_array()
            .is_some_and(|hits| hits.iter().any(Value::is_string));
        (self.expects_probe && !probe_ran).then(|| "branch probe did not run".to_owned())
    }

    /// The wire reason of an unverified cell: the failure's name and what failed, for example
    /// `redirected: /a → /b`, `not-ready: main h1 + unsettled: li#3 added` or
    /// `api-error: 401 /api/me`.
    pub fn describe(&self, failure: &Failure, observed: &Value, selector: Option<&str>) -> String {
        let name = serde_json::to_value(failure)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        let detail = match failure {
            Failure::ApiError => {
                let errors = unexpected_api_errors(observed, self.expected_api_errors);
                errors.first().map(|(path, status)| match errors.len() {
                    1 => format!("{status} {path}"),
                    n => format!("{status} {path} (+{} more)", n - 1),
                })
            }
            Failure::Redirected => Some(format!(
                "{} → {}",
                self.path,
                observed["path"].as_str().unwrap_or("(no route)")
            )),
            Failure::NotReady => {
                let wanted = selector.unwrap_or("ready selector").to_owned();
                Some(match unsettled_detail(observed) {
                    Some(still) if observed["settled"] == false => {
                        format!("{wanted} + unsettled: {still}")
                    }
                    _ => wanted,
                })
            }
            Failure::Unsettled => unsettled_detail(observed),
            Failure::WidthMismatch => Some(format!(
                "observed {} (expected {})",
                observed["width"], self.width
            )),
            Failure::RequestFailed => self.failed_request(observed),
            Failure::NotAnalyzed => Some("compiler diagnostics".to_owned()),
            Failure::AssertFailed => self.unobserved_state(observed),
            Failure::MissingStateAssertion
            | Failure::MissingScreenAssertion
            | Failure::HttpError => None,
        };
        match detail {
            Some(detail) => format!("{name}: {detail}"),
            None => name,
        }
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
        for (key, value, failure, reason) in [
            (
                "path",
                json!("/grid?item=B"),
                Failure::Redirected,
                "redirected: /grid?item=A → /grid?item=B",
            ),
            (
                "hits",
                json!([]),
                Failure::AssertFailed,
                "assert-failed: branch probe did not run",
            ),
            ("settled", json!(false), Failure::Unsettled, "unsettled"),
            (
                "slots",
                json!({}),
                Failure::AssertFailed,
                "assert-failed: slot slot is null (expected false)",
            ),
            (
                "assertions",
                json!([]),
                Failure::AssertFailed,
                "assert-failed: 0 of 1 state assertions evaluated",
            ),
            (
                "responses",
                json!({"/queue":404}),
                Failure::RequestFailed,
                "request-failed: /queue 404",
            ),
            (
                "responseChecks",
                json!({"/queue":false}),
                Failure::RequestFailed,
                "request-failed: /queue response check",
            ),
            ("ready", json!(false), Failure::NotReady, "not-ready: main"),
            (
                "width",
                json!(375),
                Failure::WidthMismatch,
                "width-mismatch: observed 375 (expected 768)",
            ),
        ] {
            let mut broken = baseline.clone();
            broken[key] = value;
            let got = intent.failure(&broken);
            assert_eq!(got, Some(failure), "{key}");
            assert_eq!(
                intent.describe(&failure, &broken, Some("main")),
                reason,
                "{key}"
            );
        }
        let mut stuck = baseline.clone();
        stuck["ready"] = json!(false);
        stuck["settled"] = json!(false);
        stuck["unsettled"] = json!([{"node":"li#3","change":"added"},{"field":"text"}]);
        assert_eq!(intent.failure(&stuck), Some(Failure::NotReady));
        assert_eq!(
            intent.describe(&Failure::NotReady, &stuck, Some("main")),
            "not-ready: main + unsettled: li#3 added (+1 more)"
        );
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
