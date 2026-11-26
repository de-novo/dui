use crate::{
    Reply, analysis,
    chromium::Browser,
    failure,
    ports::{CaptureBatchJob, Host, Renderer, TypeOracle},
    surface::{NoopOracle, SurfaceAdapter},
};
use dui_core::surface::{
    SURFACE_UNIVERSAL_WEB, SupportTier, SurfaceCapabilities, SurfaceDescriptor, SurfaceKind,
    UnrenderableReason,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_STATIC_CACHE_FILES: usize = 500;

pub(crate) struct StaticServer {
    port: u16,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for StaticServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub(crate) struct ManagedChild {
    child: Child,
    _interrupt: crate::interrupt::Guard,
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub(crate) enum UniversalSession {
    Static(StaticServer),
    Url { port: u16 },
    Command { _child: ManagedChild, port: u16 },
}

impl UniversalSession {
    pub(crate) fn port(&self) -> u16 {
        match self {
            Self::Static(server) => server.port,
            Self::Url { port } => *port,
            Self::Command { port, .. } => *port,
        }
    }
}

pub(crate) fn parse_loopback_url(raw: &str) -> Result<(u16, String), String> {
    let rest = raw
        .strip_prefix("http://")
        .ok_or_else(|| "host \"url\" requires an http:// loopback URL".to_owned())?;
    let (authority, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], rest[idx..].to_owned()),
        None => (rest, "/".to_owned()),
    };
    let (host, port_str) = match authority.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (authority, None),
    };
    if host != "127.0.0.1" && host != "localhost" {
        return Err(format!(
            "host \"url\" only allows loopback origins (127.0.0.1 or localhost), got `{host}`"
        ));
    }
    let port = match port_str {
        Some(p) => p
            .parse::<u16>()
            .map_err(|_| format!("invalid port `{p}` in URL `{raw}`"))?,
        None => 80,
    };
    if port == 0 {
        return Err("port must be greater than 0".to_owned());
    }
    Ok((port, path))
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js" | "mjs" | "cjs") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("txt" | "csv") => "text/plain; charset=utf-8",
        Some("xml") => "application/xml; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(hex_str) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(val) = u8::from_str_radix(hex_str, 16)
        {
            out.push(val);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub(crate) fn resolve_static_file(
    root: &Path,
    raw_target: &str,
    spa_fallback: bool,
) -> Option<PathBuf> {
    let path_only = raw_target.split(['?', '#']).next().unwrap_or("/");
    let decoded = percent_decode(path_only);
    let trimmed = decoded.trim_start_matches('/');
    let rel = Path::new(trimmed);
    for comp in rel.components() {
        if matches!(
            comp,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return None;
        }
    }
    let mut candidates = Vec::new();
    if trimmed.is_empty() {
        candidates.push(root.join("index.html"));
    } else {
        let direct = root.join(rel);
        candidates.push(direct.clone());
        candidates.push(direct.join("index.html"));
        if rel.extension().is_none() {
            candidates.push(root.join(format!("{trimmed}.html")));
        }
    }
    if spa_fallback {
        candidates.push(root.join("index.html"));
    }
    for candidate in candidates {
        if candidate.is_file()
            && let Ok(canonical) = candidate.canonicalize()
            && canonical.starts_with(root)
        {
            return Some(canonical);
        }
    }
    None
}

fn serve_connection(mut stream: TcpStream, root: &Path, spa_fallback: bool) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(clone) => clone,
        Err(_) => return,
    });
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() || request_line.trim().is_empty() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" {
        let _ = stream.write_all(
            b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
        return;
    }
    match resolve_static_file(root, target, spa_fallback) {
        Some(file) => match fs::read(&file) {
            Ok(body) => {
                let ctype = content_type_for(&file);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                if method == "GET" {
                    let _ = stream.write_all(&body);
                }
            }
            Err(_) => {
                let _ = stream.write_all(
                    b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
        },
        None => {
            let body = b"Not Found";
            let header = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            if method == "GET" {
                let _ = stream.write_all(body);
            }
        }
    }
}

fn start_static_server(project: &Path, spec: &Value) -> Result<StaticServer, (u8, Value)> {
    let project_root = project
        .canonicalize()
        .map_err(|e| failure(2, &format!("invalid project directory: {e}")))?;
    let requested = spec["root"].as_str().unwrap_or(".");
    let root = project_root
        .join(requested)
        .canonicalize()
        .map_err(|e| failure(2, &format!("invalid static root `{requested}`: {e}")))?;
    if !root.starts_with(&project_root) || !root.is_dir() {
        return Err(failure(
            2,
            "static root must be a directory inside the project",
        ));
    }
    let spa_fallback = spec["spaFallback"].as_bool().unwrap_or(false);
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| failure(3, &e.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|e| failure(3, &e.to_string()))?
        .port();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        for incoming in listener.incoming() {
            if stop_flag.load(Ordering::SeqCst) {
                break;
            }
            if let Ok(stream) = incoming {
                serve_connection(stream, &root, spa_fallback);
            }
        }
    });
    Ok(StaticServer {
        port,
        stop,
        handle: Some(handle),
    })
}

fn wait_for_port(
    port: u16,
    timeout: Duration,
    mut child: Option<&mut Child>,
) -> Result<(), String> {
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(proc_child) = child.as_deref_mut()
            && let Ok(Some(status)) = proc_child.try_wait()
        {
            return Err(format!("host command exited early with status {status}"));
        }
        if TcpStream::connect_timeout(&addr, Duration::from_millis(120)).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(60));
    }
    Err(format!(
        "timed out waiting for local server on 127.0.0.1:{port}"
    ))
}

fn pick_free_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    drop(listener);
    Ok(port)
}

fn start_command_server(project: &Path, spec: &Value) -> Result<UniversalSession, (u8, Value)> {
    let port = if let Some(explicit) = spec["port"].as_u64() {
        u16::try_from(explicit)
            .ok()
            .filter(|p| *p > 0)
            .ok_or_else(|| failure(2, "invalid port in command host manifest"))?
    } else if let Some(raw_url) = spec["url"].as_str() {
        parse_loopback_url(raw_url)
            .map_err(|msg| failure(2, &msg))?
            .0
    } else {
        pick_free_port().map_err(|msg| failure(3, &msg))?
    };

    let mut cmd = match spec.get("command") {
        Some(Value::Array(items)) if !items.is_empty() => {
            let parts: Vec<&str> = items
                .iter()
                .map(|v| {
                    v.as_str()
                        .ok_or_else(|| failure(2, "command array entries must be strings"))
                })
                .collect::<Result<_, _>>()?;
            let mut c = Command::new(parts[0]);
            c.args(&parts[1..]);
            c
        }
        Some(Value::String(script)) if !script.trim().is_empty() => {
            let mut c = Command::new("/bin/sh");
            c.args(["-c", script]);
            c
        }
        _ => {
            return Err(failure(
                2,
                "host \"command\" requires a non-empty `command` string or string array",
            ));
        }
    };

    cmd.current_dir(project)
        .env("PORT", port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(envs) = spec["env"].as_object() {
        for (k, v) in envs {
            if let Some(val) = v.as_str() {
                cmd.env(k, val);
            }
        }
    }
    let mut child = cmd.spawn().map_err(|e| failure(3, &e.to_string()))?;
    let timeout_ms = spec["readyTimeoutMs"].as_u64().unwrap_or(30_000);
    if let Err(msg) = wait_for_port(port, Duration::from_millis(timeout_ms), Some(&mut child)) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(failure(3, &msg));
    }
    Ok(UniversalSession::Command {
        _child: {
            let guard = crate::interrupt::kill_on_interrupt(child.id());
            ManagedChild {
                child,
                _interrupt: guard,
            }
        },
        port,
    })
}

pub(crate) fn start_universal_session(
    project: &Path,
    spec: &Value,
) -> Result<UniversalSession, (u8, Value)> {
    match spec["host"].as_str() {
        Some("static") => Ok(UniversalSession::Static(start_static_server(
            project, spec,
        )?)),
        Some("url") => {
            let raw = spec["url"]
                .as_str()
                .ok_or_else(|| failure(2, "host \"url\" requires a `url` field"))?;
            let (port, _) = parse_loopback_url(raw).map_err(|msg| failure(2, &msg))?;
            wait_for_port(port, Duration::from_secs(2), None).map_err(|msg| failure(3, &msg))?;
            Ok(UniversalSession::Url { port })
        }
        Some("command") => start_command_server(project, spec),
        other => Err(failure(
            2,
            &format!("unsupported universal web host `{}`", other.unwrap_or("")),
        )),
    }
}

struct UniversalWebHost;

impl Host for UniversalWebHost {
    type Session = UniversalSession;

    fn start(
        &self,
        project: &Path,
        manifest: &Path,
        _facts: &Value,
    ) -> Result<Self::Session, (u8, Value)> {
        let spec: Value =
            serde_json::from_slice(&fs::read(manifest).map_err(|e| failure(2, &e.to_string()))?)
                .map_err(|e| failure(2, &e.to_string()))?;
        start_universal_session(project, &spec)
    }
}

pub(crate) fn collect_static_source_files(project: &Path, root_rel: &str) -> Vec<String> {
    let Ok(project_root) = project.canonicalize() else {
        return Vec::new();
    };
    let Ok(start) = project_root.join(root_rel).canonicalize() else {
        return Vec::new();
    };
    if !start.starts_with(&project_root) || !start.is_dir() {
        return Vec::new();
    }
    let mut files = Vec::new();
    let mut stack = vec![start];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        children.sort();
        for path in children {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if path.is_symlink()
                || name.starts_with('.')
                || matches!(name, "node_modules" | "target" | "dist")
            {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file()
                && let Ok(rel) = path.strip_prefix(&project_root)
            {
                files.push(rel.to_string_lossy().replace('\\', "/"));
                if files.len() >= MAX_STATIC_CACHE_FILES {
                    files.sort();
                    return files;
                }
            }
        }
    }
    files.sort();
    files
}

struct UniversalWebOracle;

impl TypeOracle for UniversalWebOracle {
    fn facts(&self, project: &Path, spec: &Path) -> Reply {
        let mut facts = NoopOracle.facts(project, spec)?;
        if let Ok(bytes) = fs::read(spec)
            && let Ok(manifest) = serde_json::from_slice::<Value>(&bytes)
        {
            let host = manifest["host"].as_str().unwrap_or("static");
            facts["host"] = json!(host);
            if host == "static" {
                let root_rel = manifest["root"].as_str().unwrap_or(".");
                facts["sourceFiles"] = json!(collect_static_source_files(project, root_rel));
            }
        }
        Ok(facts)
    }
}

struct LazyUniversalBrowser<'a> {
    project: &'a Path,
    spec: &'a Path,
    facts: &'a Value,
    exe: PathBuf,
    profile: Option<tempfile::TempDir>,
    session: Option<UniversalSession>,
    browser: Option<Browser>,
    pool: crate::host::BrowserPool,
}

impl Renderer for LazyUniversalBrowser<'_> {
    fn capture(
        &mut self,
        spec: &Value,
        input: &Value,
        width: u32,
        image: &Path,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        if self.browser.is_none() {
            let session = UniversalWebHost
                .start(self.project, self.spec, self.facts)
                .map_err(|(_, v)| v.to_string())?;
            let profile = tempfile::tempdir()?;
            let browser = Browser::local(&self.exe, profile.path(), session.port())?;
            self.session = Some(session);
            self.profile = Some(profile);
            self.browser = Some(browser);
        }
        let browser = self.browser.as_mut().ok_or("browser unavailable")?;
        browser.capture(spec, input, width, image)
    }

    fn render_back_batch(
        &mut self,
        jobs: &[dui_ports::RenderBackJob<'_>],
    ) -> Vec<Result<f64, String>> {
        let concurrency = crate::host::resolve_capture_concurrency(&serde_json::Value::Null);
        crate::chromium::render_back_in(&mut self.pool, self.browser.as_mut(), concurrency, jobs)
    }

    fn capture_batch(&mut self, jobs: &[CaptureBatchJob<'_>]) -> Vec<Result<Value, String>> {
        let concurrency = jobs
            .first()
            .map(|j| crate::host::resolve_capture_concurrency(j.spec))
            .unwrap_or(1);
        if jobs.len() < 2 || concurrency <= 1 {
            return jobs
                .iter()
                .map(|job| {
                    self.capture(job.spec, job.input, job.width, job.image)
                        .map_err(|e| e.to_string())
                })
                .collect();
        }
        if self.session.is_none() {
            match UniversalWebHost.start(self.project, self.spec, self.facts) {
                Ok(session) => self.session = Some(session),
                Err((_, v)) => {
                    let msg = v.to_string();
                    return (0..jobs.len()).map(|_| Err(msg.clone())).collect();
                }
            }
        }
        let Some(session) = self.session.as_ref() else {
            return (0..jobs.len())
                .map(|_| Err("host unavailable".into()))
                .collect();
        };
        crate::host::parallel_chromium_capture(
            &self.exe,
            session.port(),
            concurrency,
            &mut self.pool,
            jobs,
        )
    }
}

pub(crate) struct UniversalWebSurface;

impl SurfaceAdapter for UniversalWebSurface {
    fn descriptor(&self) -> SurfaceDescriptor {
        SurfaceDescriptor {
            id: SURFACE_UNIVERSAL_WEB.into(),
            kind: SurfaceKind::Web,
            tier: SupportTier::Universal,
            host_kinds: vec!["static".into(), "url".into(), "command".into()],
            capabilities: SurfaceCapabilities {
                state_injection: false,
                live_edit: false,
                link_discovery: true,
                render_tree: true,
                box_model: true,
                pixel_capture: true,
            },
        }
    }

    fn oracle(&self) -> &dyn TypeOracle {
        &UniversalWebOracle
    }

    fn check_env(&self) -> Result<(), UnrenderableReason> {
        let exe = crate::chromium::executable();
        if exe.is_file() {
            Ok(())
        } else {
            Err(UnrenderableReason::new(
                "no-browser",
                crate::chromium::missing_detail(),
            ))
        }
    }

    fn run(&self, project: &Path, spec: &Path, out: &Path, force: bool) -> Reply {
        let facts = analysis::analyze_with_oracle(self.oracle(), project, spec)?;
        let exe = crate::chromium::executable();
        let browser_version = crate::chromium::version_tag(&exe);
        let mut lazy = LazyUniversalBrowser {
            project,
            spec,
            facts: &facts,
            exe: exe.clone(),
            profile: None,
            session: None,
            browser: None,
            pool: Vec::new(),
        };
        let renderer = exe.is_file().then_some(&mut lazy as &mut dyn Renderer);
        analysis::exercise_with_renderer(
            renderer,
            facts.clone(),
            spec,
            Some(project),
            out,
            browser_version,
            force,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn parses_loopback_urls_and_rejects_external_origins() {
        assert_eq!(
            parse_loopback_url("http://127.0.0.1:4173/app").expect("127.0.0.1 ok"),
            (4173, "/app".to_owned())
        );
        assert_eq!(
            parse_loopback_url("http://localhost:3000").expect("localhost ok"),
            (3000, "/".to_owned())
        );
        assert!(parse_loopback_url("https://example.com").is_err());
        assert!(parse_loopback_url("http://10.0.0.1:8080").is_err());
    }

    #[test]
    fn static_server_serves_html_blocks_traversal_and_supports_spa_fallback() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("index.html"), "<h1>Home</h1>").expect("write index");
        fs::write(dir.path().join("pricing.html"), "<h1>Pricing</h1>").expect("write pricing");

        let server = start_static_server(dir.path(), &json!({"host":"static","spaFallback":true}))
            .expect("start server");

        let fetch = |path: &str| -> String {
            let mut stream = TcpStream::connect(("127.0.0.1", server.port)).expect("connect");
            let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
            stream.write_all(req.as_bytes()).expect("write");
            let mut buf = String::new();
            stream.read_to_string(&mut buf).expect("read");
            buf
        };

        let home = fetch("/");
        assert!(home.starts_with("HTTP/1.1 200 OK"));
        assert!(home.contains("text/html"));
        assert!(home.contains("<h1>Home</h1>"));

        let clean = fetch("/pricing");
        assert!(clean.starts_with("HTTP/1.1 200 OK"));
        assert!(clean.contains("<h1>Pricing</h1>"));

        let spa = fetch("/unknown/route");
        assert!(spa.starts_with("HTTP/1.1 200 OK"));
        assert!(spa.contains("<h1>Home</h1>"));

        let traversal = fetch("/../secret.txt");
        assert!(traversal.starts_with("HTTP/1.1 404 Not Found"));
    }

    #[test]
    fn universal_oracle_collects_static_source_files_for_cache_invalidation() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("index.html"), "<h1>Hi</h1>").expect("write html");
        fs::write(dir.path().join("style.css"), "body{}").expect("write css");
        let spec = dir.path().join("dui.json");
        fs::write(&spec, r#"{"host":"static","widths":[375]}"#).expect("write spec");

        let facts = UniversalWebOracle
            .facts(dir.path(), &spec)
            .expect("universal facts");
        assert_eq!(facts["tier"], "universal");
        assert_eq!(facts["host"], "static");
        let files: Vec<&str> = facts["sourceFiles"]
            .as_array()
            .expect("sourceFiles array")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(files.contains(&"index.html"));
        assert!(files.contains(&"style.css"));
    }
}
