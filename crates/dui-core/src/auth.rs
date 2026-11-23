//! Pure authentication session helpers (storageState, Set-Cookie parsing,
//! env-var secret indirection, redaction, and browser storage prelude generation).

use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AuthSessionBundle {
    pub cookies: BTreeMap<String, String>,
    pub local_storage: BTreeMap<String, String>,
    pub session_storage: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
}

/// Resolve an `"env:VAR_NAME"` placeholder via a pure lookup callback.
/// Literal strings without `"env:"` are returned as-is.
pub fn resolve_env_value<F>(raw: &str, lookup: F) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(var_name) = raw.strip_prefix("env:") {
        lookup(var_name.trim()).filter(|v| !v.is_empty())
    } else {
        Some(raw.to_owned())
    }
}

/// Parse an industry-standard `storageState.json` object
/// (`{ cookies: [{ name, value }], origins: [{ origin, localStorage: [{ name, value }] }] }`).
pub fn parse_storage_state_json(raw: &Value) -> AuthSessionBundle {
    let mut bundle = AuthSessionBundle::default();
    if let Some(cookies) = raw["cookies"].as_array() {
        for c in cookies {
            if let (Some(name), Some(val)) = (c["name"].as_str(), c["value"].as_str())
                && !name.is_empty()
            {
                bundle.cookies.insert(name.to_owned(), val.to_owned());
            }
        }
    }
    if let Some(origins) = raw["origins"].as_array() {
        for origin in origins {
            if let Some(entries) = origin["localStorage"].as_array() {
                for item in entries {
                    if let (Some(name), Some(val)) = (item["name"].as_str(), item["value"].as_str())
                        && !name.is_empty()
                    {
                        bundle.local_storage.insert(name.to_owned(), val.to_owned());
                    }
                }
            }
            if let Some(entries) = origin["sessionStorage"].as_array() {
                for item in entries {
                    if let (Some(name), Some(val)) = (item["name"].as_str(), item["value"].as_str())
                        && !name.is_empty()
                    {
                        bundle
                            .session_storage
                            .insert(name.to_owned(), val.to_owned());
                    }
                }
            }
        }
    }
    bundle
}

/// Parse one or more HTTP `Set-Cookie` header values (`name=value; Path=/; HttpOnly`)
/// into a deterministic `(name -> value)` cookie map.
pub fn parse_set_cookie_headers(headers: &[String]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for raw in headers {
        let pair = raw.split(';').next().unwrap_or("").trim();
        if let Some((name, val)) = pair.split_once('=') {
            let name = name.trim();
            if !name.is_empty() {
                out.insert(name.to_owned(), val.trim().to_owned());
            }
        }
    }
    out
}

/// Decode an HTTP/1.1 response body, unchunking `Transfer-Encoding: chunked` when present.
pub fn decode_http_body(headers_head: &str, raw_body: &[u8]) -> Vec<u8> {
    let is_chunked = headers_head.lines().any(|line| {
        line.split_once(':').is_some_and(|(k, v)| {
            k.trim().eq_ignore_ascii_case("transfer-encoding")
                && v.to_ascii_lowercase().contains("chunked")
        })
    });
    if !is_chunked {
        return raw_body.to_vec();
    }
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < raw_body.len() {
        let Some(rel_crlf) = raw_body[pos..].windows(2).position(|w| w == b"\r\n") else {
            break;
        };
        let size_line = String::from_utf8_lossy(&raw_body[pos..pos + rel_crlf]);
        let hex_part = size_line.split(';').next().unwrap_or("").trim();
        let Ok(chunk_len) = usize::from_str_radix(hex_part, 16) else {
            break;
        };
        pos += rel_crlf + 2;
        if chunk_len == 0 || pos + chunk_len > raw_body.len() {
            break;
        }
        out.extend_from_slice(&raw_body[pos..pos + chunk_len]);
        pos += chunk_len;
        if raw_body[pos..].starts_with(b"\r\n") {
            pos += 2;
        }
    }
    out
}

/// Extract a nested dot-path string from a JSON response body (e.g., `"data.accessToken"`).
pub fn extract_json_string_path(body: &Value, dot_path: &str) -> Option<String> {
    let mut cur = body;
    for part in dot_path.split('.').filter(|p| !p.is_empty()) {
        cur = cur.get(part)?;
    }
    cur.as_str().map(str::to_owned)
}

/// Generate a safe prelude script that seeds `localStorage` and `sessionStorage`
/// before page scripts execute.
pub fn build_storage_prelude_script(
    local_storage: &BTreeMap<String, String>,
    session_storage: &BTreeMap<String, String>,
) -> String {
    if local_storage.is_empty() && session_storage.is_empty() {
        return String::new();
    }
    let mut out = String::from("(() => { try {\n");
    for (k, v) in local_storage {
        if let (Ok(kj), Ok(vj)) = (serde_json::to_string(k), serde_json::to_string(v)) {
            out.push_str(&format!("  window.localStorage.setItem({kj}, {vj});\n"));
        }
    }
    for (k, v) in session_storage {
        if let (Ok(kj), Ok(vj)) = (serde_json::to_string(k), serde_json::to_string(v)) {
            out.push_str(&format!("  window.sessionStorage.setItem({kj}, {vj});\n"));
        }
    }
    out.push_str("} catch {} })();\n");
    out
}

/// Validate that a login start URL uses http:// or https:// and includes a host.
pub fn validate_login_url(raw: &str) -> Result<String, &'static str> {
    let trimmed = raw.trim();
    let rest = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))
        .ok_or("login URL must start with http:// or https://")?;
    let host = rest.split('/').next().unwrap_or("").trim();
    if host.is_empty() || host.starts_with(':') {
        return Err("login URL is missing a host");
    }
    Ok(trimmed.to_owned())
}

/// Build a Playwright-compatible `storageState.json` payload from captured browser
/// cookies and origin storage maps.
pub fn build_storage_state_json(
    origin: &str,
    cookies: &[Value],
    local_storage: &BTreeMap<String, String>,
    session_storage: &BTreeMap<String, String>,
) -> Value {
    let mut normalized_cookies: Vec<Value> = cookies
        .iter()
        .filter_map(|c| {
            let name = c["name"].as_str().filter(|s| !s.is_empty())?;
            let value = c["value"].as_str().unwrap_or("");
            let domain = c["domain"].as_str().unwrap_or("");
            let path = c["path"].as_str().unwrap_or("/");
            Some(json!({
                "name": name,
                "value": value,
                "domain": domain,
                "path": path,
                "httpOnly": c["httpOnly"].as_bool().unwrap_or(false),
                "secure": c["secure"].as_bool().unwrap_or(false),
                "sameSite": c["sameSite"].as_str().unwrap_or("Lax"),
            }))
        })
        .collect();
    normalized_cookies.sort_by(|a, b| {
        let ka = (
            a["domain"].as_str().unwrap_or(""),
            a["path"].as_str().unwrap_or(""),
            a["name"].as_str().unwrap_or(""),
        );
        let kb = (
            b["domain"].as_str().unwrap_or(""),
            b["path"].as_str().unwrap_or(""),
            b["name"].as_str().unwrap_or(""),
        );
        ka.cmp(&kb)
    });

    let ls_entries: Vec<Value> = local_storage
        .iter()
        .map(|(k, v)| json!({ "name": k, "value": v }))
        .collect();
    let ss_entries: Vec<Value> = session_storage
        .iter()
        .map(|(k, v)| json!({ "name": k, "value": v }))
        .collect();

    let origins = if origin.is_empty() || (ls_entries.is_empty() && ss_entries.is_empty()) {
        Vec::new()
    } else {
        vec![json!({
            "origin": origin,
            "localStorage": ls_entries,
            "sessionStorage": ss_entries,
        })]
    };

    json!({
        "cookies": normalized_cookies,
        "origins": origins,
    })
}

/// Redact secret-bearing auth fields before persisting a manifest to disk or SQLite.
pub fn redact_manifest_secrets(mut spec: Value) -> Value {
    let Some(obj) = spec.as_object_mut() else {
        return spec;
    };
    for key in ["cookies", "localStorage", "sessionStorage", "extraHeaders"] {
        if let Some(map) = obj.get_mut(key).and_then(Value::as_object_mut) {
            for val in map.values_mut() {
                *val = json!("<redacted>");
            }
        }
    }
    if let Some(flow) = obj.get_mut("authFlow").and_then(Value::as_object_mut)
        && let Some(body) = flow.get_mut("body").and_then(Value::as_object_mut)
    {
        for val in body.values_mut() {
            *val = json!("<redacted>");
        }
    }
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_storage_state_and_set_cookie_headers() {
        let state = json!({
            "cookies": [
                {"name": "session_id", "value": "abc123", "domain": "127.0.0.1"}
            ],
            "origins": [{
                "origin": "http://127.0.0.1:3000",
                "localStorage": [
                    {"name": "jwtToken", "value": "tok_xyz"}
                ],
                "sessionStorage": [
                    {"name": "workspace", "value": "team-1"}
                ]
            }]
        });
        let bundle = parse_storage_state_json(&state);
        assert_eq!(
            bundle.cookies.get("session_id").map(String::as_str),
            Some("abc123")
        );
        assert_eq!(
            bundle.local_storage.get("jwtToken").map(String::as_str),
            Some("tok_xyz")
        );
        assert_eq!(
            bundle.session_storage.get("workspace").map(String::as_str),
            Some("team-1")
        );

        let parsed = parse_set_cookie_headers(&[
            "sid=hello123; Path=/; HttpOnly; SameSite=Lax".into(),
            "csrf=xyz; Path=/".into(),
        ]);
        assert_eq!(parsed.get("sid").map(String::as_str), Some("hello123"));
        assert_eq!(parsed.get("csrf").map(String::as_str), Some("xyz"));
    }

    #[test]
    fn resolves_env_indirection_and_redacts_manifest_secrets() {
        let resolved = resolve_env_value("env:MY_TOKEN", |k| {
            (k == "MY_TOKEN").then(|| "secret-value".into())
        });
        assert_eq!(resolved.as_deref(), Some("secret-value"));
        assert_eq!(
            resolve_env_value("plain", |_| None).as_deref(),
            Some("plain")
        );

        let spec = json!({
            "path": "/dashboard",
            "cookies": {"sid": "raw-secret"},
            "localStorage": {"jwt": "jwt-secret"},
            "authFlow": {"type": "request", "path": "/api/login", "body": {"password": "pw"}}
        });
        let redacted = redact_manifest_secrets(spec);
        let serialized = redacted.to_string();
        assert!(!serialized.contains("raw-secret"));
        assert!(!serialized.contains("jwt-secret"));
        assert!(!serialized.contains("\"pw\""));
        assert_eq!(redacted["cookies"]["sid"], "<redacted>");
    }

    #[test]
    fn builds_storage_prelude_and_extracts_nested_json_token() {
        let script = build_storage_prelude_script(
            &BTreeMap::from([("jwt".into(), "tok".into())]),
            &BTreeMap::from([("tab".into(), "1".into())]),
        );
        assert!(script.contains("window.localStorage.setItem(\"jwt\", \"tok\")"));
        assert!(script.contains("window.sessionStorage.setItem(\"tab\", \"1\")"));

        let body = json!({"data": {"accessToken": "tok-999"}});
        assert_eq!(
            extract_json_string_path(&body, "data.accessToken").as_deref(),
            Some("tok-999")
        );
    }

    #[test]
    fn validates_login_urls_and_round_trips_storage_state_json() {
        assert_eq!(
            validate_login_url("http://127.0.0.1:5173/login").as_deref(),
            Ok("http://127.0.0.1:5173/login")
        );
        assert_eq!(
            validate_login_url("https://app.example.com/auth").as_deref(),
            Ok("https://app.example.com/auth")
        );
        assert!(validate_login_url("javascript:alert(1)").is_err());
        assert!(validate_login_url("http:///no-host").is_err());

        let state = build_storage_state_json(
            "http://127.0.0.1:5173",
            &[
                json!({"name": "sid", "value": "cookie-val", "domain": "127.0.0.1", "path": "/", "httpOnly": true}),
            ],
            &BTreeMap::from([("jwt".into(), "tok-123".into())]),
            &BTreeMap::from([("role".into(), "admin".into())]),
        );
        let parsed = parse_storage_state_json(&state);
        assert_eq!(
            parsed.cookies.get("sid").map(String::as_str),
            Some("cookie-val")
        );
        assert_eq!(
            parsed.local_storage.get("jwt").map(String::as_str),
            Some("tok-123")
        );
        assert_eq!(
            parsed.session_storage.get("role").map(String::as_str),
            Some("admin")
        );
    }
}
