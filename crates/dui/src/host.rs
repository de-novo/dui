use crate::{
    Reply, analysis,
    chromium::Browser,
    failure,
    ports::{Host, Renderer},
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct LazyViteBrowser<'a> {
    project: &'a Path,
    spec: &'a Path,
    facts: &'a Value,
    exe: PathBuf,
    profile: Option<tempfile::TempDir>,
    host: Option<RunningHost>,
    browser: Option<Browser>,
}

impl Renderer for LazyViteBrowser<'_> {
    fn capture(
        &mut self,
        spec: &Value,
        input: &Value,
        width: u32,
        image: &Path,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        if self.browser.is_none() {
            let host = ViteHost
                .start(self.project, self.spec, self.facts)
                .map_err(|(_, v)| v.to_string())?;
            let profile = tempfile::tempdir()?;
            let browser = Browser::local(&self.exe, profile.path(), host.port)?;
            self.host = Some(host);
            self.profile = Some(profile);
            self.browser = Some(browser);
        }
        let browser = self.browser.as_mut().ok_or("browser unavailable")?;
        browser.capture(spec, input, width, image)
    }
}

pub fn run_with_options(project: &Path, spec: &Path, out: &Path, force: bool) -> Reply {
    crate::surface::SurfaceRegistry::default_registry().dispatch(project, spec, out, force)
}

pub(crate) fn run_vite_web_with_facts(
    project: &Path,
    spec: &Path,
    facts: Value,
    out: &Path,
    force: bool,
) -> Reply {
    let exe = crate::chromium::executable();
    let browser_version = crate::chromium::version_tag(&exe);
    let mut lazy = LazyViteBrowser {
        project,
        spec,
        facts: &facts,
        exe: exe.clone(),
        profile: None,
        host: None,
        browser: None,
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

pub(crate) struct ViteHost;
pub struct RunningHost {
    _server: Server,
    _work: tempfile::TempDir,
    pub(crate) port: u16,
}
impl Host for ViteHost {
    type Session = RunningHost;
    fn start(
        &self,
        project: &Path,
        spec: &Path,
        facts: &Value,
    ) -> Result<Self::Session, (u8, Value)> {
        let work = tempfile::tempdir().map_err(|e| failure(3, &e.to_string()))?;
        let file = |name: &str| work.path().join(name);
        fs::write(file("facts.json"), facts.to_string())
            .and_then(|_| {
                fs::write(
                    file("host.mjs"),
                    include_str!(concat!(env!("OUT_DIR"), "/vite/host.mjs")),
                )
            })
            .and_then(|_| {
                fs::write(
                    file("instrument.mjs"),
                    include_str!(concat!(env!("OUT_DIR"), "/vite/instrument.mjs")),
                )
            })
            .map_err(|e| failure(3, &e.to_string()))?;
        fs::write(
            file("probe-helpers.ts"),
            include_str!("../../../adapters/vite/probe-helpers.ts"),
        )
        .and_then(|_| {
            fs::write(
                file("probe-helpers.js"),
                include_str!(concat!(env!("OUT_DIR"), "/vite/probe-helpers.js")),
            )
        })
        .map_err(|e| failure(3, &e.to_string()))?;
        fs::write(
            file("nitro.mjs"),
            include_str!(concat!(env!("OUT_DIR"), "/vite/nitro.mjs")),
        )
        .map_err(|e| failure(3, &e.to_string()))?;
        let log = fs::File::create(file("host.log")).map_err(|e| failure(3, &e.to_string()))?;
        let child = Command::new("node")
            .current_dir(work.path())
            .arg(file("host.mjs"))
            .arg(
                project
                    .canonicalize()
                    .map_err(|_| failure(2, "invalid project"))?,
            )
            .arg(
                spec.canonicalize()
                    .map_err(|_| failure(2, "invalid manifest"))?,
            )
            .arg(work.path())
            .stdin(Stdio::piped())
            .stdout(fs::File::create(file("host.stdout")).map_err(|e| failure(3, &e.to_string()))?)
            .stderr(log)
            .spawn()
            .map_err(|e| failure(3, &e.to_string()))?;
        let mut server = Server(child);
        let start = Instant::now();
        let port = loop {
            if let Ok(port) = fs::read_to_string(file("port"))
                && let Ok(port) = port.parse::<u16>()
            {
                break port;
            }
            if let Ok(stdout) = fs::read_to_string(file("host.stdout"))
                && let Some(port) = stdout.lines().find_map(|line| {
                    line.strip_prefix("➜ Listening on: http://127.0.0.1:")
                        .and_then(|p| p.trim_end_matches('/').parse::<u16>().ok())
                })
            {
                break port;
            }
            if server
                .0
                .try_wait()
                .map_err(|e| failure(3, &e.to_string()))?
                .is_some()
            {
                return Err(failure(
                    3,
                    &fs::read_to_string(file("host.log")).unwrap_or_default(),
                ));
            }
            if start.elapsed() > Duration::from_secs(180) {
                return Err(failure(3, "Vite build timed out"));
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        HOST_BUILDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(RunningHost {
            _server: server,
            _work: work,
            port,
        })
    }
}

pub(crate) static HOST_BUILDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
