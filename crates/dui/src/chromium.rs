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
                let session = msg["sessionId"].as_str();
                if p["request"]["method"] == "GET"
                    && matches!(p["resourceType"].as_str(), Some("Fetch" | "XHR"))
                    && let Some((mime, body)) = self.generated_get.get(url)
                {
                    let reply = json!({"requestId":p["requestId"],"responseCode":200,"responseHeaders":[{"name":"Content-Type","value":mime}],"body":body});
                    self.fulfilled.insert(url.into());
                    self.requests.insert(url.into());
                    self.send(session, "Fetch.fulfillRequest", reply)?;
                    continue;
                }
                if let Some(body) = self.mock_responses.get(url) {
                    let reply = json!({"requestId":p["requestId"],"responseCode":200,"responseHeaders":[{"name":"Content-Type","value":"application/json"}],"body":body});
                    self.requests.insert(url.to_owned());
                    self.send(session, "Fetch.fulfillRequest", reply)?;
                    self.fulfilled.insert(url.to_owned());
                    continue;
                }

                if url
                    .strip_prefix(&self.allowed_origin)
                    .is_some_and(|rest| rest.starts_with('/'))
                {
                    self.requests.insert(url.to_owned());
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
        let script = format!(
            "{locale_prelude}{}\nglobalThis.__duiRuntime.install({input});",
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
        // Route discovery may stop at the entry page. Map evidence comes from every screen.
        let link_selector = spec["routeSelector"]
            .as_str()
            .or_else(|| spec.get("screenDiscovery").map(|_| "a[href]"));
        if let Some(selector) = link_selector {
            let selector = serde_json::to_string(selector)?;
            let links = self.call(s, "Runtime.evaluate", json!({"expression": format!("globalThis.__duiRuntime.links({selector})"), "returnByValue": true}))?;
            v["links"] = links["result"]["value"]
                .as_array()
                .ok_or("could not read navigation links")?
                .clone()
                .into();
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
        Ok(v)
    }
    pub fn version(&mut self) -> Result<Value> {
        self.call(None, "Browser.getVersion", json!({}))
    }
}

pub(crate) static BROWSER_LAUNCHES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
