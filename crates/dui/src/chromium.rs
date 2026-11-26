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
    candidates()
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_default()
}

/// Every location `executable()` checks, in order. `DUI_CHROMIUM` replaces the search.
pub fn candidates() -> Vec<PathBuf> {
    if let Some(path) = std::env::var_os("DUI_CHROMIUM") {
        return vec![path.into()];
    }
    let mut paths = installed_paths();
    if let Some(cache) = playwright_cache() {
        paths.extend(playwright_candidates(&cache));
    }
    paths
}

fn installed_paths() -> Vec<PathBuf> {
    [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
        "/usr/bin/chromium",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium-browser",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

/// The `no-browser` detail: what was checked and how to override it.
pub fn missing_detail() -> String {
    if let Some(path) = std::env::var_os("DUI_CHROMIUM") {
        return format!(
            "Chromium/Chrome executable not found: DUI_CHROMIUM={} is not a file",
            PathBuf::from(path).display()
        );
    }
    let mut checked: Vec<String> = installed_paths()
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    if let Some(cache) = playwright_cache() {
        checked.push(format!("{}/chromium-*", cache.display()));
        checked.push(format!("{}/chromium_headless_shell-*", cache.display()));
    }
    format!(
        "Chromium/Chrome executable not found (set DUI_CHROMIUM to override). Checked: {}",
        checked.join(", ")
    )
}

/// Playwright's browser cache: `PLAYWRIGHT_BROWSERS_PATH`, else the platform default.
fn playwright_cache() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PLAYWRIGHT_BROWSERS_PATH")
        && !path.is_empty()
        && path != "0"
    {
        return Some(path.into());
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    if cfg!(target_os = "macos") {
        Some(home.join("Library/Caches/ms-playwright"))
    } else {
        Some(
            std::env::var_os("XDG_CACHE_HOME")
                .map_or_else(|| home.join(".cache"), PathBuf::from)
                .join("ms-playwright"),
        )
    }
}

/// Browsers in a Playwright cache: the newest `chromium-*` revision first, then the newest
/// `chromium_headless_shell-*`. Each revision lists the layouts Playwright has shipped.
fn playwright_candidates(cache: &Path) -> Vec<PathBuf> {
    const CHROMIUM: &[&str] = &[
        "chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
        "chrome-mac/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
        "chrome-mac/Chromium.app/Contents/MacOS/Chromium",
        "chrome-linux64/chrome",
        "chrome-linux/chrome",
    ];
    const HEADLESS_SHELL: &[&str] = &[
        "chrome-headless-shell-mac-arm64/chrome-headless-shell",
        "chrome-headless-shell-mac-x64/chrome-headless-shell",
        "chrome-headless-shell-linux64/chrome-headless-shell",
        "chrome-mac/headless_shell",
        "chrome-linux/headless_shell",
    ];
    let revisions = |prefix: &str| {
        let mut found: Vec<(u64, PathBuf)> = fs::read_dir(cache)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name();
                let revision = name.to_str()?.strip_prefix(prefix)?.parse().ok()?;
                Some((revision, entry.path()))
            })
            .collect();
        found.sort_by(|a, b| b.0.cmp(&a.0));
        found
    };
    let mut paths = Vec::new();
    for (prefix, layouts) in [
        ("chromium-", CHROMIUM),
        ("chromium_headless_shell-", HEADLESS_SHELL),
    ] {
        for (_, dir) in revisions(prefix) {
            paths.extend(layouts.iter().map(|layout| dir.join(layout)));
        }
    }
    paths
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

/// How long one Chrome DevTools command may wait for its answer.
const CDP_TIMEOUT: Duration = Duration::from_secs(15);

/// How long the live API proxy waits for an upstream response.
const LIVE_PROXY_TIMEOUT: Duration = Duration::from_secs(10);

/// Request headers the proxy never forwards: hop-by-hop headers, the original Host, and
/// Accept-Encoding (the proxy returns bodies as received, so it asks for them uncompressed).
fn forwardable_request_header(name: &str) -> bool {
    !matches!(
        name.to_ascii_lowercase().as_str(),
        "host"
            | "connection"
            | "keep-alive"
            | "content-length"
            | "transfer-encoding"
            | "accept-encoding"
            | "proxy-connection"
            | "upgrade"
    )
}

/// Status, headers and body of a live API response.
type LiveResponse = (u16, Vec<(String, String)>, Vec<u8>);

/// Send one request to the live API and return its status, headers and body. Runs on a
/// worker thread, so a slow endpoint never blocks the capture loop.
fn forward_live_http(
    target_url: &str,
    method: &str,
    headers: &[(String, String)],
    body: Option<&[u8]>,
) -> std::result::Result<LiveResponse, String> {
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
    let timed_out = |e: &std::io::Error| {
        if matches!(
            e.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ) {
            format!(
                "timed out after {} s waiting for {method} {target_url}",
                LIVE_PROXY_TIMEOUT.as_secs()
            )
        } else {
            format!("{method} {target_url}: {e}")
        }
    };
    let mut stream = std::net::TcpStream::connect(&addr).map_err(|e| timed_out(&e))?;
    stream
        .set_read_timeout(Some(LIVE_PROXY_TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(LIVE_PROXY_TIMEOUT)))
        .map_err(|e| timed_out(&e))?;
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n");
    for (k, v) in headers {
        if forwardable_request_header(k) {
            req.push_str(&format!("{k}: {v}\r\n"));
        }
    }
    if let Some(body) = body {
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    req.push_str("\r\n");
    let mut raw = Vec::new();
    stream
        .write_all(req.as_bytes())
        .and_then(|()| stream.write_all(body.unwrap_or_default()))
        .and_then(|()| stream.flush())
        .and_then(|()| std::io::Read::read_to_end(&mut stream, &mut raw).map(|_| ()))
        .map_err(|e| timed_out(&e))?;
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| format!("{method} {target_url}: malformed HTTP response"))?;
    let header_text = String::from_utf8_lossy(&raw[..split]);
    let mut body = raw[split + 4..].to_vec();
    let mut lines = header_text.lines();
    let status_line = lines.next().unwrap_or("HTTP/1.1 502 Bad Gateway");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(502);
    let mut response_headers = Vec::new();
    let mut is_chunked = false;
    for line in lines {
        if let Some((name, val)) = line.split_once(':') {
            let (name, val) = (name.trim(), val.trim());
            match name.to_ascii_lowercase().as_str() {
                "transfer-encoding" => is_chunked |= val.to_ascii_lowercase().contains("chunked"),
                "content-length" | "connection" | "keep-alive" => {}
                _ => response_headers.push((name.to_owned(), val.to_owned())),
            }
        }
    }
    if is_chunked {
        body = decode_chunked(&body);
    }
    Ok((status, response_headers, body))
}

/// CDP commands are written from the capture loop and from live proxy workers.
struct CdpWriter {
    stream: std::sync::Mutex<UnixStream>,
    next_id: std::sync::atomic::AtomicU64,
}

impl CdpWriter {
    fn send(&self, session: Option<&str>, method: &str, params: Value) -> Result<u64> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        let mut msg = json!({"id":id,"method":method,"params":params});
        if let Some(session) = session {
            msg["sessionId"] = json!(session);
        }
        let mut frame = serde_json::to_vec(&msg)?;
        frame.push(0);
        self.stream
            .lock()
            .map_err(|_| "CDP writer poisoned")?
            .write_all(&frame)?;
        Ok(id)
    }
}

/// A finished live proxy request: the visit it belongs to, its page URL, and its status or
/// failure.
struct ProxyDone {
    visit: u64,
    url: String,
    outcome: std::result::Result<u16, String>,
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
    /// The Chrome process and its profile, killed and removed if dui is interrupted.
    _interrupt: [crate::interrupt::Guard; 2],
    cdp: std::sync::Arc<CdpWriter>,
    reader: BufReader<UnixStream>,
    visit: u64,
    proxy_done: (
        std::sync::mpsc::Sender<ProxyDone>,
        std::sync::mpsc::Receiver<ProxyDone>,
    ),
    /// Live proxy requests of this visit still waiting for the API, and the ones that failed.
    proxy_pending: std::collections::BTreeSet<String>,
    /// Requests each tab has sent and not finished, by CDP session and request ID, with when
    /// they started. A screen is not settled while its tab waits for a recent one.
    in_flight: BTreeMap<(String, String), std::time::Instant>,
    proxy_failures: BTreeMap<String, String>,
    mock_responses: BTreeMap<String, String>,
    generated_get: BTreeMap<String, (&'static str, String)>,
    fulfilled: std::collections::BTreeSet<String>,
    responses: BTreeMap<String, u16>,
    /// The tab that renders HTML snapshots back for their fidelity, reused across captures.
    render_back_session: Option<String>,
    /// Shared header triggers already probed in this run, by width, label and rectangle:
    /// the overlay they opened, or `None` when they opened nothing worth capturing.
    overlay_probes: BTreeMap<String, Option<Value>>,
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
        stream.set_read_timeout(Some(CDP_TIMEOUT))?;
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
        let interrupt = [
            crate::interrupt::kill_on_interrupt(child.id()),
            crate::interrupt::remove_on_interrupt(profile),
        ];
        Ok(Self {
            child,
            _interrupt: interrupt,
            cdp: std::sync::Arc::new(CdpWriter {
                stream: std::sync::Mutex::new(stream),
                next_id: Default::default(),
            }),
            reader,
            visit: 0,
            proxy_done: std::sync::mpsc::channel(),
            proxy_pending: Default::default(),
            in_flight: Default::default(),
            proxy_failures: Default::default(),
            mock_responses: Default::default(),
            generated_get: Default::default(),
            fulfilled: Default::default(),
            responses: Default::default(),
            overlay_probes: Default::default(),
            render_back_session: None,
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
        self.cdp.send(session, method, params)
    }
    /// Record live proxy requests that finished since the last check.
    fn drain_proxy(&mut self) {
        while let Ok(done) = self.proxy_done.1.try_recv() {
            if done.visit != self.visit {
                continue;
            }
            self.proxy_pending.remove(&done.url);
            match done.outcome {
                Ok(status) => {
                    self.responses.insert(done.url.clone(), status);
                    self.note_api_status(&done.url, status);
                }
                Err(reason) => {
                    self.proxy_failures.insert(done.url, reason);
                }
            }
        }
    }
    pub(crate) fn call(
        &mut self,
        session: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value> {
        let wanted = self.send(session, method, params)?;
        loop {
            self.drain_proxy();
            let mut bytes = Vec::new();
            let read = self.reader.read_until(0, &mut bytes).map_err(|e| {
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) {
                    format!(
                        "Chromium did not answer {method} within {} s",
                        CDP_TIMEOUT.as_secs()
                    )
                } else {
                    format!("{method}: {e}")
                }
            })?;
            if read == 0 {
                return Err(format!("Chromium disconnected during {method}").into());
            }
            bytes.pop();
            let mut msg: Value = serde_json::from_slice(&bytes)?;
            match (
                msg["method"].as_str(),
                msg["sessionId"].as_str(),
                msg["params"]["requestId"].as_str(),
            ) {
                (Some("Network.requestWillBeSent"), Some(tab), Some(id)) => {
                    // Long-lived connections (polling, streams) never finish; forget old ones.
                    self.in_flight
                        .retain(|_, started| started.elapsed() < Duration::from_secs(60));
                    self.in_flight
                        .insert((tab.to_owned(), id.to_owned()), std::time::Instant::now());
                }
                (
                    Some("Network.loadingFinished" | "Network.loadingFailed"),
                    Some(tab),
                    Some(id),
                ) => {
                    self.in_flight.remove(&(tab.to_owned(), id.to_owned()));
                }
                _ => {}
            }
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
                        // The page's own headers (including Authorization) and body go
                        // upstream, with configured `apiHeaders` on top. A worker thread
                        // answers the paused request, so the capture loop keeps running and
                        // several proxied requests can be in flight.
                        let mut headers: Vec<(String, String)> = p["request"]["headers"]
                            .as_object()
                            .into_iter()
                            .flatten()
                            .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                            .filter(|(k, _)| {
                                !self
                                    .api_headers
                                    .iter()
                                    .any(|(name, _)| name.eq_ignore_ascii_case(k))
                            })
                            .collect();
                        headers.extend(self.api_headers.iter().cloned());
                        let body: Option<Vec<u8>> = p["request"]["postDataEntries"]
                            .as_array()
                            .map(|entries| {
                                entries
                                    .iter()
                                    .filter_map(|e| e["bytes"].as_str())
                                    .filter_map(|b| STANDARD.decode(b).ok())
                                    .flatten()
                                    .collect()
                            })
                            .or_else(|| {
                                p["request"]["postData"]
                                    .as_str()
                                    .map(|d| d.as_bytes().to_vec())
                            });
                        let (cdp, done, visit) =
                            (self.cdp.clone(), self.proxy_done.0.clone(), self.visit);
                        let (page_url, method, request_id) = (
                            url.to_owned(),
                            http_method.to_owned(),
                            p["requestId"].clone(),
                        );
                        let session = session.map(str::to_owned);
                        self.proxy_pending.insert(page_url.clone());
                        std::thread::spawn(move || {
                            let outcome =
                                forward_live_http(&rewritten, &method, &headers, body.as_deref());
                            let reply = match &outcome {
                                Ok((status, response_headers, body)) => (
                                    "Fetch.fulfillRequest",
                                    json!({
                                        "requestId": request_id,
                                        "responseCode": status,
                                        "responseHeaders": response_headers
                                            .iter()
                                            .map(|(name, value)| json!({"name": name, "value": value}))
                                            .collect::<Vec<_>>(),
                                        "body": STANDARD.encode(body),
                                    }),
                                ),
                                Err(reason) => (
                                    "Fetch.failRequest",
                                    json!({
                                        "requestId": request_id,
                                        "errorReason": if reason.starts_with("timed out") { "TimedOut" } else { "ConnectionFailed" },
                                    }),
                                ),
                            };
                            let _ = cdp.send(session.as_deref(), reply.0, reply.1);
                            let _ = done.send(ProxyDone {
                                visit,
                                url: page_url,
                                outcome: outcome.map(|(status, _, _)| status),
                            });
                        });
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
        let visit_started = std::time::Instant::now();
        self.requests.clear();
        self.fulfilled.clear();
        self.responses.clear();
        self.api_errors.clear();
        self.visit += 1;
        self.proxy_pending.clear();
        self.proxy_failures.clear();
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
        let navigated = visit_started.elapsed();
        let mut v = self.observe(session, spec, width, png)?;
        v["timing"]["navigateMs"] = json!(navigated.as_millis() as u64);
        v["timing"]["totalMs"] = json!(visit_started.elapsed().as_millis() as u64);
        Ok((v, session.to_owned()))
    }
    /// Save the page as a script-free HTML snapshot next to the PNG. Its fidelity is measured
    /// after capture, with the other snapshots of the run (`snapshot_render_back`). Failures are
    /// recorded, never fatal: the PNG is the evidence.
    fn html_snapshot(&mut self, s: Option<&str>, width: u32, out_dir: &Path) -> Value {
        let taken = self
            .call(
                s,
                "Runtime.evaluate",
                json!({
                    "expression": format!("globalThis.__duiRuntime.htmlSnapshot({width})"),
                    "awaitPromise": true,
                    "returnByValue": true
                }),
            )
            .map(|r| r["result"]["value"].clone());
        let snapshot = match taken {
            Ok(value) if value["html"].is_string() => value,
            Ok(value) => {
                return json!({"error": value["error"].as_str().unwrap_or("not serializable")});
            }
            Err(e) => return json!({"error": e.to_string()}),
        };
        let inline = snapshot["html"].as_str().unwrap_or_default();
        use sha2::{Digest, Sha256};
        let (html, assets) = match crate::snapshot_assets::externalize(inline, out_dir) {
            Ok(split) => split,
            Err(e) => return json!({"error": e.to_string()}),
        };
        let file = format!("{:x}-{width}.html", Sha256::digest(html.as_bytes()));
        if let Err(e) = fs::write(out_dir.join(&file), &html) {
            return json!({"error": e.to_string()});
        }
        let mut record = json!({
            "file": file,
            "bytes": html.len(),
            "omitted": snapshot["omitted"],
        });
        if !assets.is_empty() {
            record["assets"] = json!(assets);
        }
        record
    }

    /// Open `path` in this browser's render-back tab at `width` × the PNG's height and return the share of pixels that
    /// differ from `png`.
    pub(crate) fn render_back(&mut self, path: &Path, width: u32, png: &[u8]) -> Result<f64> {
        let height = u32::from_be_bytes(png.get(20..24).ok_or("short PNG")?.try_into()?);
        let session = match self.render_back_session.clone() {
            Some(session) => session,
            None => {
                let target = self.call(
                    None,
                    "Target.createTarget",
                    json!({"url": "about:blank"}),
                )?["targetId"]
                    .as_str()
                    .ok_or("missing target")?
                    .to_owned();
                let session = self.call(
                    None,
                    "Target.attachToTarget",
                    json!({"targetId": target, "flatten": true}),
                )?["sessionId"]
                    .as_str()
                    .ok_or("missing session")?
                    .to_owned();
                self.call(Some(&session), "Page.enable", json!({}))?;
                self.render_back_session = Some(session.clone());
                session
            }
        };
        let result = (|| -> Result<f64> {
            let s = Some(session.as_str());
            // A hidden tab neither paints nor runs animation frames.
            self.call(s, "Page.bringToFront", json!({}))?;
            self.call(
                s,
                "Emulation.setDeviceMetricsOverride",
                json!({"width": width, "height": height, "deviceScaleFactor": 1, "mobile": false}),
            )?;
            let url = format!("file://{}", path.display());
            self.call(s, "Page.navigate", json!({"url": url}))?;
            // The tab is reused: wait until it shows this snapshot, fully loaded.
            let loaded = format!(
                "document.URL === {} && document.readyState === 'complete'",
                serde_json::to_string(&url)?
            );
            let started = std::time::Instant::now();
            while self
                .call(
                    s,
                    "Runtime.evaluate",
                    json!({"expression": loaded, "returnByValue": true}),
                )
                .ok()
                .and_then(|r| r["result"]["value"].as_bool())
                != Some(true)
            {
                if started.elapsed() > Duration::from_secs(10) {
                    return Err("the snapshot did not load within 10 s".into());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            self.call(
                s,
                "Runtime.evaluate",
                json!({
                    "expression": "document.fonts.ready.then(() => Promise.all([...document.images].map(i => i.complete ? 0 : new Promise(r => { i.onload = i.onerror = r; })))).then(() => new Promise(r => setTimeout(r, 30)))",
                    "awaitPromise": true,
                }),
            )?;
            let shot = self.call(
                s,
                "Page.captureScreenshot",
                json!({
                    "format": "png",
                    "captureBeyondViewport": true,
                    "clip": {"x": 0, "y": 0, "width": width, "height": height, "scale": 1}
                }),
            )?;
            let bytes = STANDARD.decode(shot["data"].as_str().ok_or("missing image")?)?;
            Ok(dui_app::diff::png_mismatch(png, &bytes, 24)?)
        })();
        if result.is_err() {
            // A broken tab is replaced on the next capture.
            self.render_back_session = None;
        }
        result
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
        let observe_started = std::time::Instant::now();
        let ms = |d: Duration| d.as_millis() as u64;
        let s = Some(session);
        let origin = self.allowed_origin.clone();
        let selector = spec["readySelector"]
            .as_str()
            .ok_or("missing DOM assertion")?;
        let selector = serde_json::to_string(selector)?;
        let expr = format!("globalThis.__duiRuntime?.observe({selector},false)");
        let mut v;
        let mut settled = false;
        let mut settle = dui_core::settle::Settle::default();
        // Wait between samples in the page so the CDP loop keeps serving intercepted requests.
        let delayed = format!("globalThis.__duiRuntime?.observe({selector},true)");
        let mut empty_since = None;
        let budget = Duration::from_millis(
            spec["settleTimeoutMs"]
                .as_u64()
                .unwrap_or(dui_core::settle::DEFAULT_TIMEOUT_MS),
        );
        let started = std::time::Instant::now();
        // The first sample waits for the loaded page (at most 3 s, never past the budget), so a
        // page that is finished by then settles after three identical samples.
        let first_wait = budget.min(Duration::from_secs(3)).as_millis();
        self.call(
            s,
            "Runtime.evaluate",
            json!({"expression":format!("globalThis.__duiRuntime?.whenLoaded({first_wait})"),"awaitPromise":true,"returnByValue":true}),
        )?;
        let mut polls = 0u32;
        loop {
            let next = self.call(
                s,
                "Runtime.evaluate",
                json!({"expression":if polls == 0 { &expr } else { &delayed },"awaitPromise":true,"returnByValue":true}),
            )?["result"]["value"]
                .clone();
            polls += 1;
            let stable = settle.sample(&next, width);
            v = next;
            if v["emptyMount"] == true {
                let start = empty_since.get_or_insert_with(std::time::Instant::now);
                if start.elapsed() >= Duration::from_secs(2) {
                    break;
                }
                continue;
            }
            empty_since = None;
            // A request the tab started recently can still change the screen; one open for
            // more than two seconds is a background connection.
            // Proxied live API requests are in it too, as the page's own requests.
            let waiting = v["pendingTimers"].as_u64().is_some_and(|n| n > 0)
                || self.in_flight.iter().any(|((tab, _), started)| {
                    tab == session && started.elapsed() < Duration::from_secs(2)
                });
            if stable
                && !waiting
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
            if started.elapsed() >= budget {
                break;
            }
        }
        if let Some(fields) = v.as_object_mut() {
            fields.remove("volatile");
        }
        let settle_ms = ms(observe_started.elapsed());
        let shot_started = std::time::Instant::now();
        let mut keyboard_ms = 0;
        // A state that set a compiler-found dialog open: record the dialog it shows.
        if spec["forcedOverlay"] == true
            && let Ok(open) = self.call(
                s,
                "Runtime.evaluate",
                json!({"expression": "globalThis.__duiRuntime.openDialog()", "returnByValue": true}),
            )
            && v.is_object()
        {
            v["forcedOverlay"] = open["result"]["value"].clone();
        }
        let ignored = settle.ignored();
        if !ignored.is_empty() {
            v["ignoredChanges"] = json!(ignored);
        }
        if !settled && v.is_object() {
            v["unsettled"] = json!(settle.unsettled(10));
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
        // A blinking caret would make identical screens differ; hidden before geometry is
        // measured so the render tree and the image see the same document.
        let caret =
            |on: bool| json!({"expression": format!("globalThis.__duiRuntime.hideCaret({on})")});
        let _ = self.call(s, "Runtime.evaluate", caret(true));
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
        // The same moment as the PNG, as HTML (docs/en/web-viewer.md#html-cards): taken before
        // the scrollbar gutter, caret and viewport set for the screenshot are restored, so the
        // snapshot lays out exactly as the PNG did.
        let snapshot_started = std::time::Instant::now();
        if spec["htmlSnapshots"] != false
            && let Some(out_dir) = png.parent()
            && screenshot.is_ok()
        {
            v["htmlSnapshot"] = self.html_snapshot(s, width, out_dir);
        }
        let snapshot_ms = ms(snapshot_started.elapsed());
        let _ = self.call(s, "Runtime.evaluate", caret(false));
        // Keyboard evidence in the render tree's layout: trusted Tab presses walk the focus order.
        // It costs a CDP round trip per focus stop, so it runs only when an audit asks for it.
        let keyboard_started = std::time::Instant::now();
        if (spec["keyboardEvidence"] == true
            || std::env::var("DUI_KEYBOARD_EVIDENCE").is_ok_and(|v| v == "1"))
            && let Some(keyboard) = self.keyboard_walk(s)
        {
            v["keyboard"] = keyboard;
            keyboard_ms = ms(keyboard_started.elapsed());
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
        let png_bytes = STANDARD.decode(data["data"].as_str().ok_or("missing image")?)?;
        fs::write(png, &png_bytes)?;
        let screenshot_ms = ms(shot_started.elapsed())
            .saturating_sub(keyboard_ms)
            .saturating_sub(snapshot_ms);
        let mut overlays_ms = 0;
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
        let overlays_started = std::time::Instant::now();
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
                // A header trigger (same label and rectangle) opens the same overlay on every
                // screen, so it is probed once per width and later screens reuse the result.
                let probe_key = (trigger["scope"] == "header")
                    .then(|| format!("{width}|{}|{}", trigger["label"], trigger["rect"]));
                if let Some(cached) = probe_key.as_ref().and_then(|k| self.overlay_probes.get(k)) {
                    if let Some(overlay) = cached
                        && overlay["slug"]
                            .as_str()
                            .is_some_and(|slug| seen_slugs.insert(slug.to_owned()))
                    {
                        let mut overlay = overlay.clone();
                        overlay["probe"] = json!("reused");
                        overlays.push(overlay);
                    }
                    continue;
                }
                let captured_before = overlays.len();
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
                    if let Some(key) = probe_key {
                        self.overlay_probes.insert(key, None);
                    }
                    continue;
                }
                if res["imagePreview"] != true
                    && let Some(slug) = res["slug"].as_str()
                    && seen_slugs.insert(slug.to_owned())
                    && self.call(s, "Runtime.evaluate", caret(true)).is_ok()
                    && let Ok(shot) =
                        self.call(s, "Page.captureScreenshot", json!({"format": "png"}))
                    && self.call(s, "Runtime.evaluate", caret(false)).is_ok()
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
                // Remember a captured overlay or an image preview. A repeated slug on this
                // screen says nothing about the trigger, so it is probed again next time.
                if let Some(key) = probe_key {
                    if overlays.len() > captured_before {
                        self.overlay_probes.insert(key, overlays.last().cloned());
                    } else if res["imagePreview"] == true {
                        self.overlay_probes.insert(key, None);
                    }
                }
            }
            if !overlays.is_empty() {
                v["overlays"] = json!(overlays);
            }
            overlays_ms = ms(overlays_started.elapsed());
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
        // Live proxy requests still waiting for the API when the screen was captured, and the
        // ones that failed (timeout or connection error), by page path.
        self.drain_proxy();
        let page_path = |u: &String| u.strip_prefix(&origin).unwrap_or(u).to_owned();
        if !self.proxy_pending.is_empty() {
            v["apiPending"] = json!(self.proxy_pending.iter().map(page_path).collect::<Vec<_>>());
        }
        if !self.proxy_failures.is_empty() {
            v["apiFailures"] = json!(
                self.proxy_failures
                    .iter()
                    .map(|(u, reason)| (page_path(u), reason.clone()))
                    .collect::<BTreeMap<_, _>>()
            );
        }
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
        // Where this capture spent its time; `navigateMs` and `totalMs` are added by the visit.
        v["timing"] = json!({
            "settleMs": settle_ms,
            "settlePolls": polls,
            "screenshotMs": screenshot_ms,
            "overlaysMs": overlays_ms,
            "keyboardMs": keyboard_ms,
            "snapshotMs": snapshot_ms,
        });
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

/// Renders HTML snapshots back in `browsers` and compares them with PNGs, `jobs` shared out to
/// one thread per browser. With no browser at hand, up to `concurrency` are launched into `pool`.
pub(crate) fn render_back_in(
    pool: &mut crate::host::BrowserPool,
    single: Option<&mut Browser>,
    concurrency: usize,
    jobs: &[dui_ports::RenderBackJob<'_>],
) -> Vec<std::result::Result<f64, String>> {
    if jobs.is_empty() {
        return Vec::new();
    }
    if pool.is_empty() && single.is_none() {
        while pool.len() < concurrency.clamp(1, 4).min(jobs.len()) {
            let launched = tempfile::tempdir()
                .map_err(|e| e.to_string())
                .and_then(|profile| {
                    Browser::local(&executable(), profile.path(), 0)
                        .map(|b| (profile, b))
                        .map_err(|e| e.to_string())
                });
            match launched {
                Ok(entry) => pool.push(entry),
                Err(e) if pool.is_empty() => return jobs.iter().map(|_| Err(e.clone())).collect(),
                Err(_) => break,
            }
        }
    }
    let mut browsers: Vec<&mut Browser> = pool.iter_mut().map(|(_, b)| b).collect();
    browsers.extend(single);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: std::sync::Mutex<Vec<Option<std::result::Result<f64, String>>>> =
        std::sync::Mutex::new(jobs.iter().map(|_| None).collect());
    std::thread::scope(|scope| {
        for browser in browsers.into_iter().take(jobs.len()) {
            let (next, results) = (&next, &results);
            scope.spawn(move || {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(job) = jobs.get(i) else {
                        break;
                    };
                    let measured = fs::read(job.png)
                        .map_err(|e| e.to_string())
                        .and_then(|png| {
                            browser
                                .render_back(job.html, job.width, &png)
                                .map_err(|e| e.to_string())
                        });
                    if let Ok(mut slots) = results.lock() {
                        slots[i] = Some(measured);
                    }
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.unwrap_or_else(|| Err("render-back worker exited without result".into())))
        .collect()
}

/// Renders this run's HTML snapshots back after capture (docs/en/web-viewer.md#html-cards).
/// Each snapshot is rendered at its own width and the share of pixels that differ from its PNG
/// becomes its `fidelity`. With `reflow` (docs/en/web-viewer.md#width-preview), each snapshot is
/// also rendered at every other captured width of the same screen, state, scenario and locale and
/// compared with that width's PNG: agreement means the screen adapts with CSS alone, while layout
/// chosen by scripts shows up as a mismatch. `measure` renders the pairs not measured before.
/// Returns the fidelity summary and, with `reflow`, the reflow pairs.
pub(crate) fn snapshot_render_back(
    cells: &mut [Value],
    out: &Path,
    reflow: bool,
    measure: impl FnOnce(&[dui_ports::RenderBackJob<'_>]) -> Vec<std::result::Result<f64, String>>,
) -> (Value, Option<Value>) {
    let started = std::time::Instant::now();
    // Snapshots whose files are in this run: (cell, identity, width, snapshot, PNG).
    let mut snapshots = Vec::new();
    for (i, cell) in cells.iter().enumerate() {
        let (Some(file), Some(image), Some(width)) = (
            cell["observation"]["htmlSnapshot"]["file"].as_str(),
            cell["image"].as_str(),
            cell["width"].as_u64().and_then(|w| u32::try_from(w).ok()),
        ) else {
            continue;
        };
        if !out.join(file).is_file() || !out.join(image).is_file() {
            continue;
        }
        let identity = json!([
            cell["path"],
            cell["state"],
            cell["scenario"],
            cell["locale"]
        ]);
        snapshots.push((i, identity, width, file.to_owned(), image.to_owned()));
    }
    // One job per snapshot at its own width, then one per ordered pair of a screen's widths:
    // (cell for fidelity, identity, from, snapshot, to, PNG).
    type Job = (Option<usize>, Value, u32, String, u32, String);
    let mut jobs: Vec<Job> = snapshots
        .iter()
        .filter(|(i, ..)| cells[*i]["observation"]["htmlSnapshot"]["fidelity"].is_null())
        .map(|(i, identity, width, file, image)| {
            (
                Some(*i),
                identity.clone(),
                *width,
                file.clone(),
                *width,
                image.clone(),
            )
        })
        .collect();
    if reflow {
        let mut groups: BTreeMap<String, Vec<(u32, String, String)>> = BTreeMap::new();
        for (_, identity, width, file, image) in &snapshots {
            groups.entry(identity.to_string()).or_default().push((
                *width,
                file.clone(),
                image.clone(),
            ));
        }
        for (key, mut widths) in groups {
            widths.sort();
            let identity: Value = serde_json::from_str(&key).unwrap_or(Value::Null);
            for (from, file, _) in &widths {
                for (to, _, image) in &widths {
                    if from != to {
                        jobs.push((
                            None,
                            identity.clone(),
                            *from,
                            file.clone(),
                            *to,
                            image.clone(),
                        ));
                    }
                }
            }
        }
    }
    // Snapshot and PNG names are hashes of their content, so a pair measured before with the same
    // dui and Chromium build has the same result: it is read back instead of rendered again.
    let exe = executable();
    let build = json!([
        crate::update::VERSION,
        exe.to_string_lossy(),
        fs::metadata(&exe)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs()),
    ]);
    let cache = crate::archive::home_dir()
        .ok()
        .map(|home| home.join("reflow"));
    let cache_file = |(_, _, _, file, to, image): &Job| {
        use sha2::{Digest, Sha256};
        let key = json!([build, file, to, image]).to_string();
        cache
            .as_ref()
            .map(|dir| dir.join(format!("{:x}.json", Sha256::digest(key.as_bytes()))))
    };
    let mut results: Vec<Option<std::result::Result<f64, String>>> = jobs
        .iter()
        .map(|job| {
            cache_file(job)
                .and_then(|path| fs::read(path).ok())
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .and_then(|v| v["mismatch"].as_f64())
                .map(Ok)
        })
        .collect();
    let todo: Vec<usize> = (0..jobs.len()).filter(|i| results[*i].is_none()).collect();
    let paths: Vec<(std::path::PathBuf, std::path::PathBuf)> = todo
        .iter()
        .map(|&i| (out.join(&jobs[i].3), out.join(&jobs[i].5)))
        .collect();
    let batch: Vec<dui_ports::RenderBackJob<'_>> = todo
        .iter()
        .zip(&paths)
        .map(|(&i, (html, png))| dui_ports::RenderBackJob {
            html,
            width: jobs[i].4,
            png,
        })
        .collect();
    let measured = if batch.is_empty() {
        Vec::new()
    } else {
        measure(&batch)
    };
    let mut first_error = None;
    for (&i, outcome) in todo.iter().zip(measured) {
        match &outcome {
            Ok(mismatch) => {
                if let Some(path) = cache_file(&jobs[i])
                    && let Some(dir) = path.parent()
                    && fs::create_dir_all(dir).is_ok()
                {
                    let _ = fs::write(path, json!({"mismatch": mismatch}).to_string());
                }
            }
            Err(e) => {
                first_error.get_or_insert_with(|| e.clone());
            }
        }
        results[i] = Some(outcome);
    }
    let count = |fidelity: bool, set: &[usize]| {
        set.iter()
            .filter(|&&i| jobs[i].0.is_some() == fidelity)
            .count()
    };
    let reused: Vec<usize> = (0..jobs.len()).filter(|i| !todo.contains(i)).collect();
    let mut pairs = Vec::new();
    for (job, result) in jobs.iter().zip(results) {
        let (cell, identity, from, _, to, _) = job;
        let Some(result) = result else {
            continue;
        };
        if let Some(i) = cell {
            let snapshot = &mut cells[*i]["observation"]["htmlSnapshot"];
            match result {
                Ok(m) => snapshot["fidelity"] = json!((m * 1e4).round() / 1e4),
                Err(e) => snapshot["fidelityError"] = json!(e),
            }
            continue;
        }
        let mut pair = json!({"path": identity[0], "from": from, "to": to});
        for (i, name) in [(1, "state"), (2, "scenario"), (3, "locale")] {
            if !identity[i].is_null() {
                pair[name] = identity[i].clone();
            }
        }
        match result {
            Ok(m) => pair["mismatch"] = json!((m * 1e4).round() / 1e4),
            Err(e) => pair["error"] = json!(e),
        }
        pairs.push(pair);
    }
    let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let mut fidelity = json!({
        "measured": count(true, &todo),
        "reused": count(true, &reused),
        "ms": ms,
    });
    if let Some(e) = &first_error {
        fidelity["error"] = json!(e);
    }
    let reflow = reflow.then(|| {
        json!({
            "pairs": pairs,
            "measured": count(false, &todo),
            "reused": count(false, &reused),
            "ms": ms,
        })
    });
    (fidelity, reflow)
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
    stream.set_read_timeout(Some(CDP_TIMEOUT))?;
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
    let interrupt = [
        crate::interrupt::kill_on_interrupt(child.id()),
        crate::interrupt::remove_on_interrupt(profile.path()),
    ];
    let mut browser = Browser {
        child,
        _interrupt: interrupt,
        cdp: std::sync::Arc::new(CdpWriter {
            stream: std::sync::Mutex::new(stream),
            next_id: Default::default(),
        }),
        reader,
        visit: 0,
        proxy_done: std::sync::mpsc::channel(),
        proxy_pending: Default::default(),
        in_flight: Default::default(),
        proxy_failures: Default::default(),
        mock_responses: Default::default(),
        generated_get: Default::default(),
        fulfilled: Default::default(),
        responses: Default::default(),
        overlay_probes: Default::default(),
        render_back_session: None,
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

#[cfg(test)]
mod tests {
    use super::playwright_candidates;

    #[test]
    fn playwright_cache_prefers_the_newest_full_chromium_over_headless_shells() {
        let cache = tempfile::tempdir().expect("cache");
        for dir in [
            "chromium-1208",
            "chromium-1248",
            "chromium_headless_shell-1300",
            "chromium-tip",
            "firefox-1509",
        ] {
            std::fs::create_dir(cache.path().join(dir)).expect("revision");
        }
        let revisions: Vec<String> = playwright_candidates(cache.path())
            .iter()
            .filter_map(|p| p.strip_prefix(cache.path()).ok()?.iter().next())
            .map(|d| d.to_string_lossy().into_owned())
            .collect();
        let mut order = revisions.clone();
        order.dedup();
        assert_eq!(
            order,
            [
                "chromium-1248",
                "chromium-1208",
                "chromium_headless_shell-1300"
            ]
        );
        let first = &playwright_candidates(cache.path())[0];
        assert!(first.ends_with(
            "chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"
        ));
    }
}
