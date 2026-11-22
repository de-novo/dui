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
        if let Some(cookies) = spec["cookies"].as_object() {
            for (name, value) in cookies {
                self.call(
                    s,
                    "Network.setCookie",
                    json!({"url":origin,"name":name,"value":value}),
                )?;
            }
        }
        self.call(
            s,
            "Emulation.setDeviceMetricsOverride",
            json!({"width":width,"height":800,"deviceScaleFactor":1,"mobile":false}),
        )?;
        let locale_prelude = if let Some(locale) = spec["locale"].as_str() {
            let _ = self.call(s, "Emulation.setLocaleOverride", json!({"locale": locale}));
            self.call(
                s,
                "Network.setExtraHTTPHeaders",
                json!({"headers": {"Accept-Language": locale}}),
            )?;
            let encoded = serde_json::to_string(locale)?;
            format!(
                "Object.defineProperty(navigator,'language',{{get:()=>({encoded}),configurable:true}});\
                 Object.defineProperty(navigator,'languages',{{get:()=>[{encoded}],configurable:true}});\
                 globalThis.__duiLocale={encoded};\n"
            )
        } else {
            String::new()
        };
        let init_prelude = spec["initScript"]
            .as_str()
            .map(|s| format!("{s}\n"))
            .unwrap_or_default();
        let script = format!(
            "{init_prelude}{locale_prelude}{}\nglobalThis.__duiRuntime.install({input});",
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
        let data = self.call(s, "Page.captureScreenshot", json!({"format":"png"}))?;
        fs::write(
            png,
            STANDARD.decode(data["data"].as_str().ok_or("missing image")?)?,
        )?;
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

pub(crate) static BROWSER_LAUNCHES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
