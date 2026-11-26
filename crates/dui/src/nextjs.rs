use crate::{
    Reply, analysis,
    chromium::Browser,
    failure,
    ports::{Renderer, TypeOracle},
    surface::{NoopOracle, SurfaceAdapter},
    universal_web::{UniversalSession, collect_static_source_files, start_universal_session},
};
use dui_core::{
    nextjs::{NextAppRoute, build_next_route},
    surface::{
        SURFACE_NEXTJS_WEB, SupportTier, SurfaceCapabilities, SurfaceDescriptor, SurfaceKind,
        UnrenderableReason,
    },
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn next_binary(project: &Path) -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("DUI_NEXT_BIN") {
        let path = PathBuf::from(explicit);
        return path.is_file().then_some(path);
    }
    let local_bin = project.join("node_modules/.bin/next");
    if local_bin.is_file() {
        return Some(local_bin);
    }
    Command::new("next")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .filter(|s| s.success())
        .map(|_| PathBuf::from("next"))
}

pub(crate) fn discover_app_routes(project: &Path) -> Vec<NextAppRoute> {
    let Ok(project_root) = project.canonicalize() else {
        return Vec::new();
    };
    let mut routes = Vec::new();
    for candidate in ["app", "src/app"] {
        let dir = project_root.join(candidate);
        if !dir.is_dir() {
            continue;
        }
        let mut stack = vec![dir];
        while let Some(cur) = stack.pop() {
            let Ok(entries) = fs::read_dir(&cur) else {
                continue;
            };
            let mut children: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            children.sort();
            for path in children {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') || name == "node_modules" {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                } else if path.is_file()
                    && let Ok(rel) = path.strip_prefix(&project_root)
                {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if let Ok(source) = fs::read_to_string(&path)
                        && let Some(route) = build_next_route(&rel_str, &source)
                    {
                        routes.push(route);
                    }
                }
            }
        }
    }
    routes.sort_by(|a, b| a.path.cmp(&b.path).then(a.file.cmp(&b.file)));
    routes
}

fn next_host_spec(project: &Path, spec: &Value) -> Result<Value, UnrenderableReason> {
    if let Some(url) = spec["url"].as_str() {
        return Ok(json!({ "host": "url", "url": url }));
    }
    if let Some(cmd) = spec.get("command") {
        let mut mapped = spec.clone();
        mapped["host"] = json!("command");
        mapped["command"] = cmd.clone();
        return Ok(mapped);
    }
    if let Some(static_root) = spec["staticRoot"].as_str() {
        return Ok(json!({
            "host": "static",
            "root": static_root,
            "spaFallback": spec["spaFallback"].as_bool().unwrap_or(false),
        }));
    }
    let Some(next_bin) = next_binary(project) else {
        return Err(UnrenderableReason::new(
            "no-next-binary",
            "next executable not found (install Next.js 16 or pass `url`/`command`/`staticRoot`)",
        ));
    };
    let mut mapped = spec.clone();
    mapped["host"] = json!("command");
    mapped["command"] = json!([
        next_bin.to_string_lossy().into_owned(),
        "dev",
        "--hostname",
        "127.0.0.1"
    ]);
    Ok(mapped)
}

struct NextJsOracle;

impl TypeOracle for NextJsOracle {
    fn facts(&self, project: &Path, spec: &Path) -> Reply {
        let mut facts = NoopOracle.facts(project, spec)?;
        let routes = discover_app_routes(project);
        let server_components: Vec<&NextAppRoute> =
            routes.iter().filter(|r| !r.is_client_component).collect();
        facts["host"] = json!("next");
        facts["nextRoutes"] = json!(routes);
        facts["serverComponentBoundaries"] = json!(server_components);
        facts["sourceFiles"] = json!(collect_static_source_files(project, "."));
        Ok(facts)
    }
}

struct LazyNextBrowser<'a> {
    project: &'a Path,
    host_spec: Value,
    exe: PathBuf,
    profile: Option<tempfile::TempDir>,
    session: Option<UniversalSession>,
    browser: Option<Browser>,
}

impl Renderer for LazyNextBrowser<'_> {
    fn capture(
        &mut self,
        spec: &Value,
        input: &Value,
        width: u32,
        image: &Path,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        if self.browser.is_none() {
            let session = start_universal_session(self.project, &self.host_spec)
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
}

pub(crate) struct NextJsWebSurface;

impl SurfaceAdapter for NextJsWebSurface {
    fn descriptor(&self) -> SurfaceDescriptor {
        SurfaceDescriptor {
            id: SURFACE_NEXTJS_WEB.into(),
            kind: SurfaceKind::Web,
            tier: SupportTier::Universal,
            host_kinds: vec!["next".into(), "nextjs".into()],
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
        &NextJsOracle
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
        let manifest_spec: Value =
            serde_json::from_slice(&fs::read(spec).map_err(|e| failure(2, &e.to_string()))?)
                .map_err(|e| failure(2, &e.to_string()))?;
        let host_spec = match next_host_spec(project, &manifest_spec) {
            Ok(h) => h,
            Err(reason) => {
                let reply = crate::surface::unrenderable_reply(SURFACE_NEXTJS_WEB, &reason);
                let _ = fs::create_dir_all(out);
                let _ = analysis::store(&out.join("report.json"), &reply);
                return Err((5, reply));
            }
        };
        let facts = analysis::analyze_with_oracle(self.oracle(), project, spec)?;

        // Auto-expand discovered static App Router pages into scenarios if none were hardcoded.
        let mut effective_spec = manifest_spec.clone();
        let discovered_routes = discover_app_routes(project);
        let static_routes: Vec<String> = discovered_routes
            .iter()
            .filter(|r| !r.is_dynamic)
            .map(|r| r.path.clone())
            .collect();
        if effective_spec["path"].as_str().is_none() {
            let entry = static_routes
                .first()
                .cloned()
                .unwrap_or_else(|| "/".to_owned());
            effective_spec["path"] = json!(entry);
        }
        if effective_spec.get("scenarios").is_none() && static_routes.len() > 1 {
            let primary = effective_spec["path"].as_str().unwrap_or("/").to_owned();
            let extra: Vec<Value> = static_routes
                .into_iter()
                .filter(|p| p != &primary)
                .map(|p| json!({ "path": p }))
                .collect();
            if !extra.is_empty() {
                effective_spec["scenarios"] = json!(extra);
            }
        }
        let temp_spec_dir = tempfile::tempdir().map_err(|e| failure(3, &e.to_string()))?;
        let effective_manifest = temp_spec_dir.path().join("dui.json");
        analysis::store(&effective_manifest, &effective_spec)?;

        let exe = crate::chromium::executable();
        let browser_version = crate::chromium::version_tag(&exe);
        let mut lazy = LazyNextBrowser {
            project,
            host_spec,
            exe: exe.clone(),
            profile: None,
            session: None,
            browser: None,
        };
        let renderer = exe.is_file().then_some(&mut lazy as &mut dyn Renderer);
        let mut report = analysis::exercise_with_renderer(
            renderer,
            facts,
            &effective_manifest,
            Some(project),
            out,
            browser_version,
            force,
        )?;
        if let Some(obj) = report.as_object_mut() {
            obj.insert("nextAppRoutes".into(), json!(discovered_routes));
            let server_boundaries: Vec<&NextAppRoute> = discovered_routes
                .iter()
                .filter(|r| !r.is_client_component)
                .collect();
            obj.insert("serverComponentBoundaries".into(), json!(server_boundaries));
        }
        analysis::store(&out.join("report.json"), &report)?;
        Ok(report)
    }
}
