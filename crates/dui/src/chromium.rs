use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Write},
    os::{fd::OwnedFd, unix::net::UnixStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub fn executable() -> PathBuf {
    if let Some(path) = std::env::var_os("DUI_CHROMIUM") {
        return path.into();
    }
    [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/usr/bin/chromium",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium-browser",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_file())
    .unwrap_or_default()
}
pub fn version_tag(executable: &Path) -> Option<Value> {
    if !executable.is_file() {
        return None;
    }
    let output = Command::new(executable).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let product = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!product.is_empty()).then(|| json!({"product": product}))
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ApiMode {
    Mock,
    Hybrid,
    Live,
}

impl ApiMode {
    fn from_spec(spec: &Value) -> Self {
        match spec["apiMode"].as_str().unwrap_or("mock") {
            "live" => Self::Live,
            "hybrid" => Self::Hybrid,
            _ => Self::Mock,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Hybrid => "hybrid",
            Self::Live => "live",
        }
    }
}

fn execute_auth_flow_request(
    origin: &str,
    flow: &Value,
) -> Result<dui_core::auth::AuthSessionBundle> {
    let mut bundle = dui_core::auth::AuthSessionBundle::default();
    let method = flow["method"].as_str().unwrap_or("POST");
    let path = flow["path"].as_str().unwrap_or("/api/login");
    if !path.starts_with('/') || path.starts_with("//") {
        return Err("authFlow.path must be a same-origin path starting with '/'".into());
    }
    let stripped = origin
        .strip_prefix("http://")
        .ok_or("authFlow requires an http:// local origin")?;
    let addr = if stripped.contains(':') {
        stripped.to_owned()
    } else {
        format!("{stripped}:80")
    };
    let mut body_val = flow.get("body").cloned().unwrap_or_else(|| json!({}));
    if let Some(obj) = body_val.as_object_mut() {
        for v in obj.values_mut() {
            if let Some(raw) = v.as_str()
                && let Some(resolved) =
                    dui_core::auth::resolve_env_value(raw, |k| std::env::var(k).ok())
            {
                *v = json!(resolved);
            }
        }
    }
    let body_bytes = serde_json::to_vec(&body_val)?;
    let mut stream = std::net::TcpStream::connect(&addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {stripped}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body_bytes.len()
    );
    stream.write_all(req.as_bytes())?;
    stream.write_all(&body_bytes)?;
    let mut raw_resp = Vec::new();
    std::io::Read::read_to_end(&mut stream, &mut raw_resp)?;
    let split = raw_resp
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("malformed HTTP response from authFlow")?;
    let head = String::from_utf8_lossy(&raw_resp[..split]);
    let resp_body = dui_core::auth::decode_http_body(&head, &raw_resp[split + 4..]);
    let mut set_cookies = Vec::new();
    for line in head.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':')
            && k.trim().eq_ignore_ascii_case("set-cookie")
        {
            set_cookies.push(v.trim().to_owned());
        }
    }
    bundle
        .cookies
        .extend(dui_core::auth::parse_set_cookie_headers(&set_cookies));
    if let (Some(json_path), Some(ls_key)) = (
        flow["tokenJsonPath"].as_str(),
        flow["localStorageKey"].as_str(),
    ) && let Ok(parsed) = serde_json::from_slice::<Value>(&resp_body)
        && let Some(token) = dui_core::auth::extract_json_string_path(&parsed, json_path)
    {
        bundle.local_storage.insert(ls_key.to_owned(), token);
    }
    Ok(bundle)
}

fn collect_auth_session(
    origin: &str,
    spec: &Value,
    flow_cache: &mut Option<dui_core::auth::AuthSessionBundle>,
) -> Result<dui_core::auth::AuthSessionBundle> {
    let mut bundle = dui_core::auth::AuthSessionBundle::default();
    let env_lookup = |k: &str| std::env::var(k).ok();

    if let Some(state_path) = spec["storageState"].as_str().filter(|s| !s.is_empty()) {
        let raw = fs::read_to_string(state_path)?;
        let parsed: Value = serde_json::from_str(&raw)?;
        let from_file = dui_core::auth::parse_storage_state_json(&parsed);
        bundle.cookies.extend(from_file.cookies);
        bundle.local_storage.extend(from_file.local_storage);
        bundle.session_storage.extend(from_file.session_storage);
    }

    if let Some(flow) = spec.get("authFlow").filter(|v| v.is_object())
        && flow["type"].as_str().unwrap_or("request") == "request"
    {
        if flow_cache.is_none() {
            *flow_cache = Some(execute_auth_flow_request(origin, flow)?);
        }
        if let Some(cached) = flow_cache.as_ref() {
            bundle.cookies.extend(cached.cookies.clone());
            bundle.local_storage.extend(cached.local_storage.clone());
            bundle
                .session_storage
                .extend(cached.session_storage.clone());
        }
    }

    if let Some(cookies) = spec["cookies"].as_object() {
        for (name, value) in cookies {
            if let Some(raw) = value.as_str()
                && let Some(resolved) = dui_core::auth::resolve_env_value(raw, env_lookup)
            {
                bundle.cookies.insert(name.clone(), resolved);
            }
        }
    }
    if let Some(ls) = spec["localStorage"].as_object() {
        for (name, value) in ls {
            if let Some(raw) = value.as_str()
                && let Some(resolved) = dui_core::auth::resolve_env_value(raw, env_lookup)
            {
                bundle.local_storage.insert(name.clone(), resolved);
            }
        }
    }
    if let Some(ss) = spec["sessionStorage"].as_object() {
        for (name, value) in ss {
            if let Some(raw) = value.as_str()
                && let Some(resolved) = dui_core::auth::resolve_env_value(raw, env_lookup)
            {
                bundle.session_storage.insert(name.clone(), resolved);
            }
        }
    }
    if let Some(hdrs) = spec["extraHeaders"].as_object() {
        for (name, value) in hdrs {
            if let Some(raw) = value.as_str()
                && let Some(resolved) = dui_core::auth::resolve_env_value(raw, env_lookup)
            {
                bundle.headers.insert(name.clone(), resolved);
            }
        }
    }
    Ok(bundle)
}

fn forward_live_http(
    target_url: &str,
    method: &str,
    extra_headers: &[(String, String)],
) -> Result<(u16, String, Vec<u8>)> {
    let stripped = target_url
        .strip_prefix("http://")
        .ok_or("live API proxy currently requires an http:// loopback or local URL")?;
    let (authority, path) = match stripped.find('/') {
        Some(idx) => (&stripped[..idx], &stripped[idx..]),
        None => (stripped, "/"),
    };
    let addr = if authority.contains(':') {
        authority.to_owned()
    } else {
        format!("{authority}:80")
    };
    let mut stream = std::net::TcpStream::connect(&addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n");
    for (k, v) in extra_headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes())?;
    stream.flush()?;
    let mut raw = Vec::new();
    std::io::Read::read_to_end(&mut stream, &mut raw)?;
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("malformed HTTP response from live API")?;
    let header_text = String::from_utf8_lossy(&raw[..split]);
    let mut body = raw[split + 4..].to_vec();
    let mut lines = header_text.lines();
    let status_line = lines.next().unwrap_or("HTTP/1.1 502 Bad Gateway");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(502);
    let mut content_type = "application/json".to_owned();
    let mut is_chunked = false;
    for line in lines {
        if let Some((name, val)) = line.split_once(':') {
            let lname = name.trim().to_ascii_lowercase();
            let lval = val.trim();
            if lname == "content-type" {
                content_type = lval.to_owned();
            } else if lname == "transfer-encoding" && lval.to_ascii_lowercase().contains("chunked")
            {
                is_chunked = true;
            }
        }
    }
    if is_chunked {
        body = decode_chunked(&body);
    }
    Ok((status, content_type, body))
}

fn decode_chunked(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < input.len() {
        let Some(crlf) = input[pos..].windows(2).position(|w| w == b"\r\n") else {
            break;
        };
        let size_str = String::from_utf8_lossy(&input[pos..pos + crlf]);
        let hex_part = size_str.split(';').next().unwrap_or("").trim();
        let Ok(len) = usize::from_str_radix(hex_part, 16) else {
            break;
        };
        pos += crlf + 2;
        if len == 0 || pos + len > input.len() {
            break;
        }
        out.extend_from_slice(&input[pos..pos + len]);
        pos += len;
        if input[pos..].starts_with(b"\r\n") {
            pos += 2;
        }
    }
    out
}

pub struct Browser {
    child: Child,
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    next_id: u64,
    mock_responses: BTreeMap<String, String>,
    generated_get: BTreeMap<String, (&'static str, String)>,
    fulfilled: std::collections::BTreeSet<String>,
    responses: BTreeMap<String, u16>,
    /// Fetch/XHR responses with 401, 403 or 5xx during this visit, by URL.
    api_errors: BTreeMap<String, u16>,
    response_ids: BTreeMap<String, Value>,
    allowed_origin: String,
    api_mode: ApiMode,
    api_base_url: Option<String>,
    api_proxy_prefixes: Vec<String>,
    api_headers: Vec<(String, String)>,
    allow_mutations: bool,
    live_api_requests: std::collections::BTreeSet<String>,
    blocked_mutations: std::collections::BTreeSet<String>,
    api_provenance: BTreeMap<String, &'static str>,
    requests: std::collections::BTreeSet<String>,
    auth_flow_cache: Option<dui_core::auth::AuthSessionBundle>,
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Browser {
    pub fn local(executable: &Path, profile: &Path, port: u16) -> Result<Self> {
        let (stream, chrome) = UnixStream::pair()?;
        stream.set_read_timeout(Some(Duration::from_secs(15)))?;
        stream.set_write_timeout(Some(Duration::from_secs(15)))?;
        let reader = BufReader::new(stream.try_clone()?);
        let child = Command::new("/bin/sh")
            .args([
                "-c",
                r#"exec "$@" 3<&0 4>&1 </dev/null >/dev/null"#,
                "dui-chromium",
            ])
            .arg(executable)
            .args([
                "--remote-debugging-pipe",
                "--headless",
                "--disable-background-networking",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-component-update",
                "--disable-sync",
                "--disable-extensions",
                "--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1",
                "--disable-dev-shm-usage",
            ])
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("about:blank")
            .stdin(Stdio::from(OwnedFd::from(chrome.try_clone()?)))
            .stdout(Stdio::from(OwnedFd::from(chrome)))
            .stderr(Stdio::null())
            .spawn()?;
        BROWSER_LAUNCHES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(Self {
            child,
            stream,
            reader,
            next_id: 0,
            mock_responses: Default::default(),
            generated_get: Default::default(),
            fulfilled: Default::default(),
            responses: Default::default(),
            api_errors: Default::default(),
            response_ids: Default::default(),
            allowed_origin: format!("http://127.0.0.1:{port}"),
            api_mode: ApiMode::Mock,
            api_base_url: None,
            api_proxy_prefixes: Vec::new(),
            api_headers: Vec::new(),
            allow_mutations: false,
            live_api_requests: Default::default(),
            blocked_mutations: Default::default(),
            api_provenance: Default::default(),
            requests: Default::default(),
            auth_flow_cache: None,
        })
    }
    fn send(&mut self, session: Option<&str>, method: &str, params: Value) -> Result<u64> {
        self.next_id += 1;
        let mut msg = json!({"id":self.next_id,"method":method,"params":params});
        if let Some(session) = session {
            msg["sessionId"] = json!(session);
        }
        let mut frame = serde_json::to_vec(&msg)?;
        frame.push(0);
        self.stream.write_all(&frame)?;
        Ok(self.next_id)
    }
    pub(crate) fn call(
        &mut self,
        session: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value> {
        let wanted = self.send(session, method, params)?;
        loop {
            let mut bytes = Vec::new();
            if self.reader.read_until(0, &mut bytes)? == 0 {
                return Err("Chromium disconnected".into());
            }
            bytes.pop();
            let mut msg: Value = serde_json::from_slice(&bytes)?;
            if msg["method"] == "Network.responseReceived" {
                let response = &msg["params"]["response"];
                if let (Some(url), Some(code)) =
                    (response["url"].as_str(), response["status"].as_u64())
                {
                    self.responses.insert(url.to_owned(), code as u16);
                    if matches!(msg["params"]["type"].as_str(), Some("Fetch" | "XHR")) {
                        self.note_api_status(url, code as u16);
                    }
                    self.response_ids
                        .insert(url.to_owned(), msg["params"]["requestId"].clone());
                }
            }
            if msg["method"] == "Fetch.requestPaused" {
                let p = &msg["params"];
                let url = p["request"]["url"].as_str().unwrap_or("");
                let http_method = p["request"]["method"].as_str().unwrap_or("GET");
                let is_api_resource = matches!(p["resourceType"].as_str(), Some("Fetch" | "XHR"));
                let session = msg["sessionId"].as_str();
                let use_mocks = self.api_mode != ApiMode::Live;

                let url_no_query = url.split('?').next().unwrap_or(url);
                if use_mocks
                    && http_method == "GET"
                    && is_api_resource
                    && let Some((mime, body)) = self
                        .generated_get
                        .get(url)
                        .or_else(|| self.generated_get.get(url_no_query))
                {
                    let reply = json!({"requestId":p["requestId"],"responseCode":200,"responseHeaders":[{"name":"Content-Type","value":mime}],"body":body});
                    self.fulfilled.insert(url.into());
                    self.requests.insert(url.into());
                    let label = url
                        .strip_prefix(&self.allowed_origin)
                        .unwrap_or(url)
                        .to_owned();
                    self.api_provenance.insert(label, "mock");
                    self.send(session, "Fetch.fulfillRequest", reply)?;
                    continue;
                }
                if use_mocks
                    && let Some(body) = self
                        .mock_responses
                        .get(url)
                        .or_else(|| self.mock_responses.get(url_no_query))
                {
                    let reply = json!({"requestId":p["requestId"],"responseCode":200,"responseHeaders":[{"name":"Content-Type","value":"application/json"}],"body":body});
                    self.requests.insert(url.to_owned());
                    self.send(session, "Fetch.fulfillRequest", reply)?;
                    self.fulfilled.insert(url.to_owned());
                    let label = url
                        .strip_prefix(&self.allowed_origin)
                        .unwrap_or(url)
                        .to_owned();
                    self.api_provenance.insert(label, "mock");
                    continue;
                }

                let is_same_origin = url
                    .strip_prefix(&self.allowed_origin)
                    .is_some_and(|rest| rest.starts_with('/'));
                let proxied_target = if self.api_mode != ApiMode::Mock
                    && is_same_origin
                    && let Some(base) = &self.api_base_url
                    && let Some(rel) = url.strip_prefix(&self.allowed_origin)
                    && self
                        .api_proxy_prefixes
                        .iter()
                        .any(|prefix| rel.starts_with(prefix.as_str()))
                {
                    Some((format!("{base}{rel}"), rel.to_owned()))
                } else {
                    None
                };
                let is_live_api_origin = self.api_mode != ApiMode::Mock
                    && self.api_base_url.as_deref().is_some_and(|base| {
                        url.strip_prefix(base)
                            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
                    });

                if proxied_target.is_some() || is_live_api_origin {
                    let label = proxied_target
                        .as_ref()
                        .map(|(_, rel)| rel.clone())
                        .or_else(|| {
                            self.api_base_url
                                .as_deref()
                                .and_then(|b| url.strip_prefix(b))
                                .map(str::to_owned)
                        })
                        .unwrap_or_else(|| url.to_owned());
                    let is_safe_method = matches!(http_method, "GET" | "HEAD" | "OPTIONS");
                    if is_api_resource && !self.allow_mutations && !is_safe_method {
                        self.blocked_mutations
                            .insert(format!("{http_method} {label}"));
                        self.api_provenance.insert(label, "blocked-mutation");
                        self.send(
                            session,
                            "Fetch.failRequest",
                            json!({"requestId":p["requestId"],"errorReason":"BlockedByClient"}),
                        )?;
                        continue;
                    }
                    self.requests.insert(url.to_owned());
                    self.live_api_requests.insert(label.clone());
                    self.api_provenance.insert(label, "live-api");
                    if let Some((rewritten, _)) = proxied_target {
                        match forward_live_http(&rewritten, http_method, &self.api_headers) {
                            Ok((status, mime, body)) => {
                                self.responses.insert(url.to_owned(), status);
                                if is_api_resource {
                                    self.note_api_status(url, status);
                                }
                                let reply = json!({
                                    "requestId": p["requestId"],
                                    "responseCode": status,
                                    "responseHeaders": [{"name": "Content-Type", "value": mime}],
                                    "body": STANDARD.encode(body),
                                });
                                self.send(session, "Fetch.fulfillRequest", reply)?;
                            }
                            Err(_) => {
                                self.send(
                                    session,
                                    "Fetch.failRequest",
                                    json!({"requestId":p["requestId"],"errorReason":"ConnectionFailed"}),
                                )?;
                            }
                        }
                    } else {
                        let mut cont = json!({"requestId": p["requestId"]});
                        if !self.api_headers.is_empty() {
                            let mut merged: BTreeMap<String, String> = BTreeMap::new();
                            if let Some(orig) = p["request"]["headers"].as_object() {
                                for (k, v) in orig {
                                    if let Some(vs) = v.as_str() {
                                        merged.insert(k.clone(), vs.to_owned());
                                    }
                                }
                            }
                            for (k, v) in &self.api_headers {
                                merged.insert(k.clone(), v.clone());
                            }
                            cont["headers"] = json!(
                                merged
                                    .into_iter()
                                    .map(|(name, value)| json!({"name": name, "value": value}))
                                    .collect::<Vec<_>>()
                            );
                        }
                        self.send(session, "Fetch.continueRequest", cont)?;
                    }
                } else if is_same_origin {
                    self.requests.insert(url.to_owned());
                    if self.api_mode == ApiMode::Live && is_api_resource {
                        let label = url
                            .strip_prefix(&self.allowed_origin)
                            .unwrap_or(url)
                            .to_owned();
                        self.live_api_requests.insert(label.clone());
                        self.api_provenance.insert(label, "live-api");
                    }
                    self.send(
                        session,
                        "Fetch.continueRequest",
                        json!({"requestId":p["requestId"]}),
                    )?;
                } else {
                    self.send(
                        session,
                        "Fetch.failRequest",
                        json!({"requestId":p["requestId"],"errorReason":"BlockedByClient"}),
                    )?;
                }
            }
            if msg["id"].as_u64() == Some(wanted) {
                if let Some(err) = msg.get("error") {
                    return Err(format!("{method}: {err}").into());
                }
                return Ok(msg["result"].take());
            }
        }
    }
}
impl crate::ports::Renderer for Browser {
    fn capture(&mut self, spec: &Value, input: &Value, width: u32, png: &Path) -> Result<Value> {
        let ctx = self.call(None, "Target.createBrowserContext", json!({}))?["browserContextId"]
            .as_str()
            .ok_or("missing context")?
            .to_owned();
        let result = self.visit_context(&ctx, spec, input, width, png);
        self.call(
            None,
            "Target.disposeBrowserContext",
            json!({"browserContextId":ctx}),
        )?;
        result
    }
}
impl Browser {
    fn note_api_status(&mut self, url: &str, status: u16) {
        if matches!(status, 401 | 403) || status >= 500 {
            self.api_errors.insert(url.to_owned(), status);
        }
    }
    pub(crate) fn open_page(
        &mut self,
        spec: &Value,
        input: &Value,
        width: u32,
        png: &Path,
    ) -> Result<(String, String, Value)> {
        let ctx = self.call(None, "Target.createBrowserContext", json!({}))?["browserContextId"]
            .as_str()
            .ok_or("missing context")?
            .to_owned();
        let (obs, session) = self.visit_with_session(&ctx, spec, input, width, png)?;
        Ok((ctx, session, obs))
    }
    pub(crate) fn close_control(&mut self, ctx: &str) -> Result<()> {
        self.call(
            None,
            "Target.disposeBrowserContext",
            json!({"browserContextId":ctx}),
        )?;
        Ok(())
    }
    pub(crate) fn reference(
        &mut self,
        session: &str,
        input: &Value,
        base: &Value,
        spec: &Value,
    ) -> Result<Value> {
        let baseline = json!({"slots":base["slots"],"semantic":base["semantic"]});
        let expr = format!(
            "globalThis.__duiRuntime.reference({input},{baseline},{},{})",
            spec["readySelector"], spec["assertions"]
        );
        let result = self.call(
            Some(session),
            "Runtime.evaluate",
            json!({"expression":expr,"awaitPromise":true,"returnByValue":true}),
        )?;
        result["result"].get("value").cloned().ok_or_else(|| {
            format!(
                "reference evaluation failed: {}",
                result["exceptionDetails"]["text"]
            )
            .into()
        })
    }
    pub(crate) fn capture_after_mount(
        &mut self,
        session: &str,
        input: &Value,
        base: &Value,
        spec: &Value,
        width: u32,
        image: &Path,
    ) -> Result<(Value, bool)> {
        // This is an injection retry on a separately mounted page. Keep only branch hits from this trial.
        let baseline = json!({"slots":base["slots"],"semantic":base["semantic"]});
        let apply = format!("globalThis.__duiRuntime.apply({input},{baseline})");
        let changed = self.call(
            Some(session),
            "Runtime.evaluate",
            json!({"expression":apply,"awaitPromise":true,"returnByValue":true}),
        )?;
        if changed["result"]["value"] != true {
            return Err("state setter unavailable or initial control changed".into());
        }
        let observed = self.observe(session, spec, width, image)?;
        let restore = format!("globalThis.__duiRuntime.restore({baseline})");
        let reset = self.call(
            Some(session),
            "Runtime.evaluate",
            json!({"expression":restore,"awaitPromise":true,"returnByValue":true}),
        )?;
        Ok((observed, reset["result"]["value"] == true))
    }
    fn visit_context(
        &mut self,
        ctx: &str,
        spec: &Value,
        input: &Value,
        width: u32,
        png: &Path,
    ) -> Result<Value> {
        Ok(self.visit_with_session(ctx, spec, input, width, png)?.0)
    }
    fn visit_with_session(
        &mut self,
        ctx: &str,
        spec: &Value,
        input: &Value,
        width: u32,
        png: &Path,
    ) -> Result<(Value, String)> {
        self.requests.clear();
        self.fulfilled.clear();
        self.responses.clear();
        self.api_errors.clear();
        self.response_ids.clear();
        self.live_api_requests.clear();
        self.blocked_mutations.clear();
        self.api_provenance.clear();
        self.api_mode = ApiMode::from_spec(spec);
        self.allow_mutations = spec["allowMutations"].as_bool().unwrap_or(false);
        self.api_base_url = spec["apiBaseUrl"]
            .as_str()
            .and_then(|raw| {
                if let Some(var_name) = raw.strip_prefix("env:") {
                    std::env::var(var_name).ok()
                } else {
                    Some(raw.to_owned())
                }
            })
            .map(|s| s.trim_end_matches('/').to_owned())
            .filter(|s| !s.is_empty());
        self.api_proxy_prefixes.clear();
        if let Some(prefix) = spec["apiProxyPrefix"]
            .as_str()
            .filter(|s| s.starts_with('/'))
        {
            self.api_proxy_prefixes.push(prefix.to_owned());
        }
        if let Some(prefixes) = spec["apiProxyPrefixes"].as_array() {
            for p in prefixes {
                if let Some(s) = p.as_str().filter(|s| s.starts_with('/')) {
                    self.api_proxy_prefixes.push(s.to_owned());
                }
            }
        }
        self.api_headers.clear();
        if let Some(header_envs) = spec["apiHeaderEnv"].as_object() {
            for (header_name, env_val) in header_envs {
                if let Some(var_name) = env_val.as_str()
                    && let Ok(secret_val) = std::env::var(var_name)
                    && !secret_val.is_empty()
                {
                    self.api_headers.push((header_name.clone(), secret_val));
                }
            }
        }
        let origin = self.allowed_origin.clone();
        let path = spec["path"].as_str().ok_or("missing path")?;
        if !path.starts_with('/') || path.starts_with("//") {
            return Err("route must be a local path".into());
        }
        self.generated_get.clear();
        if let Some(entries) = spec["generatedGetResponses"].as_object() {
            for (path, body) in entries {
                self.generated_get.insert(
                    format!("{origin}{path}"),
                    (
                        "application/json",
                        STANDARD.encode(serde_json::to_vec(body)?),
                    ),
                );
            }
        }
        if let Some(entries) = spec["generatedCsvResponses"].as_object() {
            for (path, body) in entries {
                self.generated_get.insert(
                    format!("{origin}{path}"),
                    (
                        "text/csv",
                        STANDARD.encode(body.as_str().ok_or("invalid CSV mock")?),
                    ),
                );
            }
        }
        self.mock_responses.clear();
        if let Some(mocks) = spec["mockResponses"].as_object() {
            for (path, payload) in mocks {
                self.mock_responses.insert(
                    format!("{origin}{path}"),
                    STANDARD.encode(serde_json::to_vec(payload)?),
                );
            }
        }
        let id = self.call(
            None,
            "Target.createTarget",
            json!({"url":"about:blank","browserContextId":ctx}),
        )?["targetId"]
            .clone();
        let attached = self.call(
            None,
            "Target.attachToTarget",
            json!({"targetId":id,"flatten":true}),
        )?;
        let s = Some(attached["sessionId"].as_str().ok_or("missing session")?);
        self.call(s, "Page.enable", json!({}))?;
        self.call(s, "Profiler.enable", json!({}))?;
        self.call(
            s,
            "Profiler.startPreciseCoverage",
            json!({"callCount":true,"detailed":true}),
        )?;
        self.call(s, "Network.enable", json!({}))?;
        self.call(s, "Network.setBypassServiceWorker", json!({"bypass":true}))?;
        let auth_session = collect_auth_session(&origin, spec, &mut self.auth_flow_cache)?;
        for (name, value) in &auth_session.cookies {
            self.call(
                s,
                "Network.setCookie",
                json!({"url":origin,"name":name,"value":value}),
            )?;
        }
        let mut extra_headers = auth_session.headers.clone();
        if let Some(locale) = spec["locale"].as_str() {
            extra_headers.insert("Accept-Language".into(), locale.to_owned());
        }
        if !extra_headers.is_empty() {
            self.call(
                s,
                "Network.setExtraHTTPHeaders",
                json!({"headers": extra_headers}),
            )?;
        }
        self.call(
            s,
            "Emulation.setDeviceMetricsOverride",
            json!({"width":width,"height":800,"deviceScaleFactor":1,"mobile":false}),
        )?;
        let locale_prelude = if let Some(locale) = spec["locale"].as_str() {
            let _ = self.call(s, "Emulation.setLocaleOverride", json!({"locale": locale}));
            let encoded = serde_json::to_string(locale)?;
            format!(
                "Object.defineProperty(navigator,'language',{{get:()=>({encoded}),configurable:true}});\
                 Object.defineProperty(navigator,'languages',{{get:()=>[{encoded}],configurable:true}});\
                 globalThis.__duiLocale={encoded};\n"
            )
        } else {
            String::new()
        };
        let storage_prelude = dui_core::auth::build_storage_prelude_script(
            &auth_session.local_storage,
            &auth_session.session_storage,
        );
        let init_prelude = spec["initScript"]
            .as_str()
            .map(|s| format!("{s}\n"))
            .unwrap_or_default();
        let script = format!(
            "{storage_prelude}{init_prelude}{locale_prelude}{}\nglobalThis.__duiRuntime.install({input});",
            include_str!(concat!(env!("OUT_DIR"), "/browser/runtime.js"))
        );
        self.call(
            s,
            "Page.addScriptToEvaluateOnNewDocument",
            json!({"source":script}),
        )?;
        self.call(s, "Fetch.enable", json!({"patterns":[{"urlPattern":"*"}]}))?;
        self.call(s, "Page.navigate", json!({"url":format!("{origin}{path}")}))?;
        let session = s.ok_or("missing session")?;
        Ok((self.observe(session, spec, width, png)?, session.to_owned()))
    }
    /// Walks the focus order with trusted Tab presses and records each stop, so audits can check
    /// that every keyboard target is reached and shows a focus indicator.
    fn keyboard_walk(&mut self, s: Option<&str>) -> Option<Value> {
        const MAX_STOPS: usize = 500;
        let eval = |this: &mut Self, expression: &str| {
            this.call(
                s,
                "Runtime.evaluate",
                json!({"expression": expression, "returnByValue": true}),
            )
            .ok()
            .map(|r| r["result"]["value"].clone())
        };
        let targets = eval(self, "globalThis.__duiRuntime?.keyboardTargets()")?;
        let mut stops: Vec<Value> = Vec::new();
        let mut complete = false;
        for _ in 0..MAX_STOPS {
            for kind in ["keyDown", "keyUp"] {
                self.call(
                    s,
                    "Input.dispatchKeyEvent",
                    json!({"type": kind, "key": "Tab", "code": "Tab",
                           "windowsVirtualKeyCode": 9, "nativeVirtualKeyCode": 9}),
                )
                .ok()?;
            }
            let stop = eval(self, "globalThis.__duiRuntime.focusStop()")?;
            if stop.is_null() {
                // Focus left the document: the walk wrapped around.
                if stops.is_empty() {
                    continue;
                }
                complete = true;
                break;
            }
            if stops
                .first()
                .is_some_and(|first| first["tag"] == stop["tag"] && first["rect"] == stop["rect"])
            {
                complete = true;
                break;
            }
            stops.push(stop);
        }
        let _ = eval(self, "globalThis.__duiRuntime.resetFocus()");
        Some(json!({"targets": targets, "stops": stops, "complete": complete}))
    }

    fn observe(&mut self, session: &str, spec: &Value, width: u32, png: &Path) -> Result<Value> {
        let s = Some(session);
        let origin = self.allowed_origin.clone();
        let selector = spec["readySelector"]
            .as_str()
            .ok_or("missing DOM assertion")?;
        let selector = serde_json::to_string(selector)?;
        let expr = format!("globalThis.__duiRuntime?.observe({selector},false)");
        let mut v = Value::Null;
        let mut stable = 0;
        let mut settled = false;
        // Wait between samples in the page so the CDP loop keeps serving intercepted requests.
        let delayed = format!("globalThis.__duiRuntime?.observe({selector},true)");
        let mut empty_since = None;
        for poll in 0..100 {
            let next = self.call(
                s,
                "Runtime.evaluate",
                json!({"expression":if poll == 0 { &expr } else { &delayed },"awaitPromise":true,"returnByValue":true}),
            )?["result"]["value"]
                .clone();
            if next == v && next["ready"] == true && next["width"] == width {
                stable += 1;
            } else {
                stable = 0;
            }
            v = next;
            if v["emptyMount"] == true {
                let start = empty_since.get_or_insert_with(std::time::Instant::now);
                if start.elapsed() >= Duration::from_secs(2) {
                    break;
                }
                continue;
            }
            empty_since = None;
            if stable >= 4
                && spec["requiredRequests"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .all(|r| {
                        r.as_str().is_some_and(|r| {
                            self.responses
                                .get(&format!("{origin}{r}"))
                                .is_some_and(|s| (200..300).contains(s))
                        })
                    })
            {
                settled = true;
                break;
            }
        }
        if let Some(fields) = v.as_object_mut() {
            let document_origin = fields.remove("documentOrigin");
            if spec["path"] != "/"
                || document_origin.as_ref().and_then(Value::as_str) != Some(&origin)
            {
                fields.remove("redirectedTo");
            }
        }
        // Always collect same-origin navigation anchors so the 2D screen map renders
        // connecting edges between captured screens even when BFS crawling is off.
        let link_selector = spec["routeSelector"].as_str().unwrap_or("a[href]");
        let selector_json = serde_json::to_string(link_selector)?;
        if let Ok(links) = self.call(
            s,
            "Runtime.evaluate",
            json!({"expression": format!("globalThis.__duiRuntime.links({selector_json})"), "returnByValue": true}),
        ) && let Some(arr) = links["result"]["value"].as_array()
        {
            v["links"] = arr.clone().into();
        }
        if spec["measureSemantics"] == true {
            let tree = self.call(s, "Runtime.evaluate",json!({"expression":"globalThis.__duiRuntime.semantic()","awaitPromise":true,"returnByValue":true}))?;
            v["semantic"] = tree["result"]["value"].clone();
        }
        if spec["locale"].is_string() {
            let audit = self.call(
                s,
                "Runtime.evaluate",
                json!({"expression":"globalThis.__duiRuntime.auditLocale()","returnByValue":true}),
            )?;
            v["localeAudit"] = audit["result"]["value"].clone();
        }
        const MAX_FULL_PAGE_HEIGHT: u32 = 12_000;
        let reveal_info = self
            .call(
                s,
                "Runtime.evaluate",
                json!({
                    "expression": format!("globalThis.__duiRuntime.revealForFullPageCapture({MAX_FULL_PAGE_HEIGHT})"),
                    "awaitPromise": true,
                    "returnByValue": true
                }),
            )
            .ok()
            .map(|res| res["result"]["value"].clone())
            .unwrap_or_else(|| json!({}));

        let vp_h = reveal_info["vpH"].as_u64().unwrap_or(800).max(1) as u32;
        let target_h = reveal_info["captureH"]
            .as_u64()
            .unwrap_or(vp_h as u64)
            .clamp(vp_h as u64, MAX_FULL_PAGE_HEIGHT as u64) as u32;
        let expanded_shell = reveal_info["expandedShell"].as_bool().unwrap_or(false);
        let infinite_scroll = reveal_info["infiniteScrollDetected"]
            .as_bool()
            .unwrap_or(false);
        let truncated = reveal_info["truncated"].as_bool().unwrap_or(false);
        if infinite_scroll {
            v["infiniteScrollDetected"] = json!(true);
        }
        if truncated {
            v["truncatedHeight"] = json!(true);
        }

        if expanded_shell {
            let _ = self.call(
                s,
                "Emulation.setDeviceMetricsOverride",
                json!({"width": width, "height": target_h, "deviceScaleFactor": 1, "mobile": false}),
            );
            let _ = self.call(
                s,
                "Runtime.evaluate",
                json!({
                    "expression": "new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))",
                    "awaitPromise": true,
                    "returnByValue": true
                }),
            );
        }
        if target_h > vp_h {
            self.call(
                s,
                "Runtime.evaluate",
                json!({"expression": "globalThis.__duiRuntime.setCaptureGutter(true)"}),
            )?;
        }
        if (expanded_shell || target_h > vp_h)
            && let Ok(refreshed) = self.call(
                s,
                "Runtime.evaluate",
                json!({"expression": &expr, "awaitPromise": true, "returnByValue": true}),
            )
            && let Some(tree) = refreshed["result"]["value"]
                .get("renderTree")
                .filter(|t| t.is_array())
        {
            v["renderTree"] = tree.clone();
        }

        let shot_params = if target_h > vp_h {
            json!({
                "format": "png",
                "captureBeyondViewport": true,
                "clip": {
                    "x": 0,
                    "y": 0,
                    "width": width,
                    "height": target_h,
                    "scale": 1
                }
            })
        } else {
            json!({"format": "png"})
        };
        let screenshot = self.call(s, "Page.captureScreenshot", shot_params);
        // Keyboard evidence in the render tree's layout: trusted Tab presses walk the focus order.
        // It costs a CDP round trip per focus stop, so it runs only when an audit asks for it.
        if (spec["keyboardEvidence"] == true
            || std::env::var("DUI_KEYBOARD_EVIDENCE").is_ok_and(|v| v == "1"))
            && let Some(keyboard) = self.keyboard_walk(s)
        {
            v["keyboard"] = keyboard;
        }
        if target_h > vp_h {
            self.call(
                s,
                "Runtime.evaluate",
                json!({"expression": "globalThis.__duiRuntime.setCaptureGutter(false)"}),
            )?;
        }
        if expanded_shell {
            let _ = self.call(
                s,
                "Emulation.setDeviceMetricsOverride",
                json!({"width": width, "height": 800, "deviceScaleFactor": 1, "mobile": false}),
            );
        }
        let data = screenshot?;
        fs::write(
            png,
            STANDARD.decode(data["data"].as_str().ok_or("missing image")?)?,
        )?;
        // After screenshot capture, safely expand collapsed WAI-ARIA nav disclosures to
        // discover hidden submenu links without mutating the captured image.
        if spec["routeSelector"].is_string()
            && let Ok(expanded) = self.call(
                s,
                "Runtime.evaluate",
                json!({
                    "expression": format!("globalThis.__duiRuntime.discoverLinks({selector_json})"),
                    "awaitPromise": true,
                    "returnByValue": true
                }),
            )
            && let Some(arr) = expanded["result"]["value"].as_array()
        {
            v["links"] = arr.clone().into();
        }
        if spec["discoverOverlays"] == true
            && let Some(out_dir) = png.parent()
            && let Ok(listed) = self.call(
                s,
                "Runtime.evaluate",
                json!({
                    "expression": "globalThis.__duiRuntime.listOverlayTriggers()",
                    "returnByValue": true
                }),
            )
            && let Some(triggers) = listed["result"]["value"].as_array()
        {
            let mut overlays = Vec::new();
            let mut seen_slugs = std::collections::BTreeSet::new();
            for trigger in triggers.iter().take(12) {
                if overlays.len() >= 4 {
                    break;
                }
                let Some(idx) = trigger["index"].as_u64() else {
                    continue;
                };
                let Ok(opened) = self.call(
                    s,
                    "Runtime.evaluate",
                    json!({
                        "expression": format!("globalThis.__duiRuntime.openOverlayTrigger({idx})"),
                        "awaitPromise": true,
                        "returnByValue": true
                    }),
                ) else {
                    continue;
                };
                let res = &opened["result"]["value"];
                if res["opened"] != true {
                    continue;
                }
                if res["imagePreview"] != true
                    && let Some(slug) = res["slug"].as_str()
                    && seen_slugs.insert(slug.to_owned())
                    && let Ok(shot) =
                        self.call(s, "Page.captureScreenshot", json!({"format": "png"}))
                    && let Some(b64) = shot["data"].as_str()
                    && let Ok(bytes) = STANDARD.decode(b64)
                {
                    use sha2::{Digest, Sha256};
                    let mut hasher = Sha256::new();
                    hasher.update(&bytes);
                    let hex: String = hasher
                        .finalize()
                        .iter()
                        .take(8)
                        .map(|b| format!("{b:02x}"))
                        .collect();
                    let file_name = format!("{hex}-overlay-{width}-{slug}.png");
                    if fs::write(out_dir.join(&file_name), &bytes).is_ok() {
                        overlays.push(json!({
                            "slug": slug,
                            "title": res["title"],
                            "triggerLabel": res["triggerLabel"],
                            "scope": res["scope"],
                            "image": file_name,
                            "width": width,
                            "renderTree": res["renderTree"],
                        }));
                    }
                }
                let _ = self.call(
                    s,
                    "Input.dispatchKeyEvent",
                    json!({
                        "type": "keyDown",
                        "key": "Escape",
                        "code": "Escape",
                        "windowsVirtualKeyCode": 27,
                        "nativeVirtualKeyCode": 27
                    }),
                );
                let _ = self.call(
                    s,
                    "Input.dispatchKeyEvent",
                    json!({
                        "type": "keyUp",
                        "key": "Escape",
                        "code": "Escape",
                        "windowsVirtualKeyCode": 27,
                        "nativeVirtualKeyCode": 27
                    }),
                );
                // Did the trusted Escape close the overlay and return focus to its trigger?
                let dismissal = self
                    .call(
                        s,
                        "Runtime.evaluate",
                        json!({
                            "expression": "globalThis.__duiRuntime.overlayDismissal()",
                            "awaitPromise": true,
                            "returnByValue": true
                        }),
                    )
                    .ok()
                    .map(|r| r["result"]["value"].clone())
                    .filter(Value::is_object);
                if let (Some(dismissal), Some(last)) = (dismissal, overlays.last_mut())
                    && last["slug"] == res["slug"]
                {
                    last["dismissal"] = dismissal;
                }
                let _ = self.call(
                    s,
                    "Runtime.evaluate",
                    json!({
                        "expression": "globalThis.__duiRuntime.closeOverlay()",
                        "awaitPromise": true,
                        "returnByValue": true
                    }),
                );
            }
            if !overlays.is_empty() {
                v["overlays"] = json!(overlays);
            }
        }
        if !v.is_object() {
            v = json!({"ready":false});
        }
        v["settled"] = json!(settled);
        let assertions = spec.get("assertions").cloned().unwrap_or(json!([]));
        let eval = format!("globalThis.__duiRuntime.assertions({assertions})");
        v["assertions"] = self.call(
            s,
            "Runtime.evaluate",
            json!({"expression":eval,"returnByValue":true}),
        )?["result"]["value"]
            .clone();
        let mut checked = BTreeMap::new();
        if let Some(checks) = spec["responseChecks"].as_object() {
            for (path, expected) in checks {
                let request = self.response_ids.get(&format!("{origin}{path}")).cloned();
                let payload = request
                    .and_then(|id| {
                        self.call(s, "Network.getResponseBody", json!({"requestId":id}))
                            .ok()
                    })
                    .and_then(|r| {
                        r["body"]
                            .as_str()
                            .and_then(|body| serde_json::from_str::<Value>(body).ok())
                    });
                let matches = expected.as_object().is_some_and(|fields| {
                    fields
                        .iter()
                        .all(|(k, value)| payload.as_ref().and_then(|p| p.get(k)) == Some(value))
                });
                checked.insert(path.as_str(), matches);
            }
        }
        for path in spec["requiredRequests"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let url = format!("{origin}{path}");
            if (self.generated_get.contains_key(&url) || self.mock_responses.contains_key(&url))
                && !self.fulfilled.contains(&url)
            {
                checked.insert(path, false);
            }
        }
        v["responseChecks"] = json!(checked);
        let ranges = self.call(s, "Profiler.takePreciseCoverage", json!({}))?;
        let scripts:Vec<_>=ranges["result"].as_array().into_iter().flatten().filter(|script|script["url"].as_str().is_some_and(|u|u.starts_with(&format!("{origin}/")))).map(|script|{
            let ranges:Vec<_>=script["functions"].as_array().into_iter().flatten().flat_map(|f|f["ranges"].as_array().into_iter().flatten()).collect();
            json!({"url":script["url"].as_str().and_then(|u|u.strip_prefix(&origin)),"ranges":ranges.len(),"executed":ranges.iter().filter(|r|r["count"].as_u64().unwrap_or(0)>0).count()})
        }).collect();
        v["v8Scripts"] = json!(scripts);
        v["responses"] = json!(
            self.responses
                .iter()
                .filter_map(|(u, s)| u
                    .strip_prefix(&origin)
                    .filter(|p| !p.starts_with("/assets/"))
                    .map(|p| (p, *s)))
                .collect::<BTreeMap<_, _>>()
        );
        // Failed API calls of this screen: same-origin as a path, the live API origin as a URL.
        v["apiErrors"] = json!(
            self.api_errors
                .iter()
                .filter_map(|(u, s)| {
                    let same_origin = u.strip_prefix(&origin).filter(|p| p.starts_with('/'));
                    let live_api = self.api_base_url.as_deref().is_some_and(|base| {
                        u.strip_prefix(base)
                            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
                    });
                    same_origin
                        .map(str::to_owned)
                        .or_else(|| live_api.then(|| u.clone()))
                        .map(|key| (key, *s))
                })
                .collect::<BTreeMap<_, _>>()
        );
        v["fulfilledMocks"] = json!(
            self.fulfilled
                .iter()
                .filter_map(|url| url.strip_prefix(&origin))
                .collect::<Vec<_>>()
        );
        v["requests"] = json!(
            self.requests
                .iter()
                .filter_map(|url| url.strip_prefix(&origin))
                .filter(|url| !url.starts_with("/assets/"))
                .collect::<Vec<_>>()
        );
        if self.api_mode != ApiMode::Mock {
            v["apiMode"] = json!(self.api_mode.as_str());
            v["liveApiRequests"] = json!(self.live_api_requests.iter().collect::<Vec<_>>());
            v["blockedMutations"] = json!(self.blocked_mutations.iter().collect::<Vec<_>>());
            v["apiProvenance"] = json!(self.api_provenance);
        }
        v["screenEvidence"] = dui_core::verification::screen_evidence(spec, &v);
        Ok(v)
    }
    pub fn version(&mut self) -> Result<Value> {
        self.call(None, "Browser.getVersion", json!({}))
    }

    pub fn replay_transition(
        &mut self,
        base_spec: &Value,
        transition: &dui_core::transition::TransitionSpec,
        width: u32,
        temp_dir: &Path,
    ) -> Result<dui_core::transition::TransitionObservation> {
        use dui_core::transition::TransitionObservation;

        let mut from_spec = base_spec.clone();
        from_spec["path"] = json!(transition.from);
        if let Some(obj) = from_spec.as_object_mut() {
            obj.remove("routeSelector");
            if let Some(scenarios) = base_spec["scenarios"].as_array()
                && let Some(matched) = scenarios
                    .iter()
                    .find(|s| s["path"].as_str() == Some(transition.from.as_str()))
                && let Some(ready) = matched.get("readySelector")
            {
                obj.insert("readySelector".into(), ready.clone());
            }
        }
        let img = temp_dir.join(format!("transition-{width}.png"));
        let (target_id, session_id, baseline) =
            match self.open_page(&from_spec, &json!({}), width, &img) {
                Ok(opened) => opened,
                Err(err) => {
                    return Ok(TransitionObservation {
                        width,
                        started_at: String::new(),
                        landed_at: String::new(),
                        action_executed: false,
                        destination_ready: false,
                        expected_text_matched: None,
                        error: Some(format!("open-initial-failed: {err}")),
                    });
                }
            };
        let s = Some(session_id.as_str());
        let started_at = baseline["path"].as_str().unwrap_or("").to_owned();

        let action_expr = format!(
            r#"(() => {{
                const kind = {kind};
                const selector = {selector};
                if (kind === "navigate") {{
                    window.history.pushState({{}}, "", {to});
                    window.dispatchEvent(new PopStateEvent("popstate"));
                    return {{ executed: true }};
                }}
                const el = document.querySelector(selector);
                if (!el) return {{ executed: false, error: "selector-not-found: " + selector }};
                if (kind === "submit" && typeof el.requestSubmit === "function") {{
                    el.requestSubmit();
                    return {{ executed: true }};
                }}
                if (typeof el.click === "function") {{
                    el.click();
                    return {{ executed: true }};
                }}
                return {{ executed: false, error: "unsupported-action-target" }};
            }})()"#,
            kind = json!(transition.action.kind),
            selector = json!(transition.action.selector),
            to = json!(transition.to),
        );

        let action_res = self
            .call(
                s,
                "Runtime.evaluate",
                json!({"expression": action_expr, "awaitPromise": true, "returnByValue": true}),
            )
            .ok()
            .and_then(|r| r["result"].get("value").cloned())
            .unwrap_or_else(|| json!({"executed": false, "error": "action-eval-failed"}));

        let action_executed = action_res["executed"] == true;
        let action_error = action_res["error"].as_str().map(str::to_owned);

        let mut landed_at = started_at.clone();
        let mut destination_ready = false;
        let mut expected_text_matched: Option<bool> = None;

        if action_executed {
            let check_expr = format!(
                r#"(() => {{
                    const current = location.pathname + location.search + location.hash;
                    const sel = {expect_sel};
                    const expectedText = {expect_text};
                    const el = sel ? document.querySelector(sel) : document.body;
                    const ready = Boolean(el && (sel || (document.body && document.body.innerText.trim().length > 0)));
                    let textMatched = null;
                    if (expectedText !== null) {{
                        const text = el ? (el.innerText || el.textContent || "") : "";
                        textMatched = text.includes(expectedText);
                    }}
                    return {{ path: current, ready, textMatched }};
                }})()"#,
                expect_sel = json!(transition.expect_selector),
                expect_text = json!(transition.expect_text),
            );
            for _ in 0..40 {
                let probe = self
                    .call(
                        s,
                        "Runtime.evaluate",
                        json!({"expression": check_expr, "returnByValue": true}),
                    )
                    .ok()
                    .and_then(|r| r["result"].get("value").cloned());
                if let Some(val) = probe {
                    landed_at = val["path"].as_str().unwrap_or("").to_owned();
                    destination_ready = val["ready"] == true;
                    expected_text_matched = val["textMatched"].as_bool();
                    if landed_at == transition.to
                        && destination_ready
                        && expected_text_matched != Some(false)
                    {
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }

        let _ = self.call(None, "Target.closeTarget", json!({"targetId": target_id}));

        Ok(TransitionObservation {
            width,
            started_at,
            landed_at,
            action_executed,
            destination_ready,
            expected_text_matched,
            error: action_error,
        })
    }
}

pub struct LoginCapture {
    pub url: String,
    pub origin: String,
    pub cookies_count: usize,
    pub local_storage_count: usize,
    pub session_storage_count: usize,
    pub storage_state: Value,
}

pub fn capture_login_session(
    executable: &Path,
    start_url: &str,
    wait_url_substring: Option<&str>,
    timeout: Duration,
    headless: bool,
) -> Result<LoginCapture> {
    if !executable.is_file() {
        return Err("Chromium executable not found; set DUI_CHROMIUM".into());
    }
    let profile = tempfile::tempdir()?;
    let (stream, chrome) = UnixStream::pair()?;
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    stream.set_write_timeout(Some(Duration::from_secs(15)))?;
    let reader = BufReader::new(stream.try_clone()?);
    let mut args: Vec<&str> = vec![
        "--remote-debugging-pipe",
        "--disable-background-networking",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-component-update",
        "--disable-sync",
        "--disable-extensions",
        "--disable-dev-shm-usage",
        "--password-store=basic",
        "--use-mock-keychain",
    ];
    if headless {
        args.push("--headless");
    }
    let child = Command::new("/bin/sh")
        .args([
            "-c",
            r#"exec "$@" 3<&0 4>&1 </dev/null >/dev/null"#,
            "dui-chromium-login",
        ])
        .arg(executable)
        .args(&args)
        .arg(format!("--user-data-dir={}", profile.path().display()))
        .arg("about:blank")
        .stdin(Stdio::from(OwnedFd::from(chrome.try_clone()?)))
        .stdout(Stdio::from(OwnedFd::from(chrome)))
        .stderr(Stdio::null())
        .spawn()?;
    BROWSER_LAUNCHES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut browser = Browser {
        child,
        stream,
        reader,
        next_id: 0,
        mock_responses: Default::default(),
        generated_get: Default::default(),
        fulfilled: Default::default(),
        responses: Default::default(),
        api_errors: Default::default(),
        response_ids: Default::default(),
        allowed_origin: String::new(),
        api_mode: ApiMode::Live,
        api_base_url: None,
        api_proxy_prefixes: Vec::new(),
        api_headers: Vec::new(),
        allow_mutations: true,
        live_api_requests: Default::default(),
        blocked_mutations: Default::default(),
        api_provenance: Default::default(),
        requests: Default::default(),
        auth_flow_cache: None,
    };

    let deadline = std::time::Instant::now() + timeout;
    let target_id =
        browser.call(None, "Target.createTarget", json!({ "url": "about:blank" }))?["targetId"]
            .as_str()
            .ok_or("missing created targetId")?
            .to_owned();

    let session_id = browser.call(
        None,
        "Target.attachToTarget",
        json!({ "targetId": target_id, "flatten": true }),
    )?["sessionId"]
        .as_str()
        .ok_or("missing browser sessionId")?
        .to_owned();
    let s = Some(session_id.as_str());
    let _ = browser.call(s, "Page.enable", json!({}));
    let _ = browser.call(s, "Network.enable", json!({}));
    let _ = browser.call(s, "Runtime.enable", json!({}));
    browser.call(s, "Page.navigate", json!({ "url": start_url }))?;

    if let Some(pattern) = wait_url_substring {
        loop {
            let current_url = browser
                .call(
                    s,
                    "Runtime.evaluate",
                    json!({ "expression": "window.location.href", "returnByValue": true }),
                )
                .ok()
                .and_then(|r| r["result"]["value"].as_str().map(str::to_owned))
                .unwrap_or_default();
            if current_url.contains(pattern) {
                // Allow a brief tick for post-redirect storage writes to settle.
                std::thread::sleep(Duration::from_millis(250));
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err(format!(
                    "timed out waiting for URL containing {pattern:?} (last URL: {current_url:?})"
                )
                .into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    } else {
        eprintln!(
            "Complete login in the Chromium window opened at {start_url}, then press Enter here to save storageState..."
        );
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
    }

    let page_meta = browser.call(
        s,
        "Runtime.evaluate",
        json!({
            "expression": r#"(() => {
                const readStore = (st) => {
                    const out = {};
                    try {
                        for (let i = 0; i < st.length; i++) {
                            const k = st.key(i);
                            if (k !== null) out[k] = st.getItem(k) ?? "";
                        }
                    } catch {}
                    return out;
                };
                return {
                    url: window.location.href,
                    origin: window.location.origin,
                    localStorage: readStore(window.localStorage),
                    sessionStorage: readStore(window.sessionStorage),
                };
            })()"#,
            "returnByValue": true
        }),
    )?;
    let meta_val = &page_meta["result"]["value"];
    let final_url = meta_val["url"].as_str().unwrap_or(start_url).to_owned();
    let origin = meta_val["origin"].as_str().unwrap_or("").to_owned();

    let mut local_storage = BTreeMap::new();
    if let Some(obj) = meta_val["localStorage"].as_object() {
        for (k, v) in obj {
            if let Some(sv) = v.as_str() {
                local_storage.insert(k.clone(), sv.to_owned());
            }
        }
    }
    let mut session_storage = BTreeMap::new();
    if let Some(obj) = meta_val["sessionStorage"].as_object() {
        for (k, v) in obj {
            if let Some(sv) = v.as_str() {
                session_storage.insert(k.clone(), sv.to_owned());
            }
        }
    }

    let cookies_resp = browser
        .call(None, "Storage.getCookies", json!({}))
        .or_else(|_| browser.call(s, "Network.getCookies", json!({})))?;
    let cookies = cookies_resp["cookies"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let storage_state = dui_core::auth::build_storage_state_json(
        &origin,
        &cookies,
        &local_storage,
        &session_storage,
    );
    let cookies_count = storage_state["cookies"].as_array().map_or(0, Vec::len);

    Ok(LoginCapture {
        url: final_url,
        origin,
        cookies_count,
        local_storage_count: local_storage.len(),
        session_storage_count: session_storage.len(),
        storage_state,
    })
}

pub(crate) static BROWSER_LAUNCHES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
