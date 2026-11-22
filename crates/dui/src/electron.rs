use crate::{
    Reply, analysis,
    chromium::Browser,
    failure,
    ports::{Renderer, TypeOracle},
    surface::{NoopOracle, SurfaceAdapter},
    universal_web::{UniversalSession, collect_static_source_files, start_universal_session},
};
use dui_core::surface::{
    SURFACE_ELECTRON_DESKTOP, SupportTier, SurfaceCapabilities, SurfaceDescriptor, SurfaceKind,
    UnrenderableReason,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn electron_binary(project: &Path) -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("DUI_ELECTRON") {
        let path = PathBuf::from(explicit);
        return path.is_file().then_some(path);
    }
    let local_bin = project.join("node_modules/.bin/electron");
    if local_bin.is_file() {
        return Some(local_bin);
    }
    Command::new("electron")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .filter(|s| s.success())
        .map(|_| PathBuf::from("electron"))
}

pub(crate) fn build_electron_preload_script(spec: &Value) -> Option<String> {
    let global_name = spec["preloadGlobal"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("electronAPI");
    let mocks = spec.get("ipcMocks").filter(|v| v.is_object())?;
    let global_json = serde_json::to_string(global_name).ok()?;
    let mocks_json = serde_json::to_string(mocks).ok()?;
    Some(format!(
        "(() => {{\n\
         const mocks = {mocks_json};\n\
         const api = {{}};\n\
         for (const [k, v] of Object.entries(mocks)) {{\n\
           api[k] = (..._args) => Promise.resolve(structuredClone(v));\n\
         }}\n\
         if (!globalThis[{global_json}]) {{\n\
           globalThis[{global_json}] = api;\n\
         }}\n\
         globalThis.__duiElectron = {{ runtime: 'electron', preloadGlobal: {global_json} }};\n\
         }})();"
    ))
}

fn renderer_host_spec(spec: &Value) -> Value {
    if let Some(url) = spec["rendererUrl"]
        .as_str()
        .or_else(|| spec["url"].as_str())
    {
        return json!({
            "host": "url",
            "url": url,
        });
    }
    if spec.get("rendererCommand").is_some() || spec.get("command").is_some() {
        let mut mapped = spec.clone();
        mapped["host"] = json!("command");
        if let Some(cmd) = spec.get("rendererCommand") {
            mapped["command"] = cmd.clone();
        }
        return mapped;
    }
    let root = spec["rendererRoot"]
        .as_str()
        .or_else(|| spec["root"].as_str())
        .unwrap_or("out/renderer");
    json!({
        "host": "static",
        "root": root,
        "spaFallback": spec["spaFallback"].as_bool().unwrap_or(true),
    })
}

struct ElectronOracle;

impl TypeOracle for ElectronOracle {
    fn facts(&self, project: &Path, spec: &Path) -> Reply {
        let mut facts = NoopOracle.facts(project, spec)?;
        facts["host"] = json!("electron");
        if let Ok(bytes) = fs::read(spec)
            && let Ok(manifest) = serde_json::from_slice::<Value>(&bytes)
        {
            let root = manifest["rendererRoot"]
                .as_str()
                .or_else(|| manifest["root"].as_str())
                .unwrap_or(".");
            let target = if project.join(root).is_dir() {
                root
            } else {
                "."
            };
            facts["sourceFiles"] = json!(collect_static_source_files(project, target));
        }
        Ok(facts)
    }
}

struct LazyElectronBrowser<'a> {
    project: &'a Path,
    manifest_spec: Value,
    exe: PathBuf,
    profile: Option<tempfile::TempDir>,
    session: Option<UniversalSession>,
    browser: Option<Browser>,
}

impl Renderer for LazyElectronBrowser<'_> {
    fn capture(
        &mut self,
        spec: &Value,
        input: &Value,
        width: u32,
        image: &Path,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        if self.browser.is_none() {
            let host_spec = renderer_host_spec(&self.manifest_spec);
            let session = start_universal_session(self.project, &host_spec)
                .map_err(|(_, v)| v.to_string())?;
            let profile = tempfile::tempdir()?;
            let browser = Browser::local(&self.exe, profile.path(), session.port())?;
            self.session = Some(session);
            self.profile = Some(profile);
            self.browser = Some(browser);
        }
        let mut capture_spec = spec.clone();
        if let Some(script) = build_electron_preload_script(&self.manifest_spec) {
            let combined = match capture_spec["initScript"].as_str() {
                Some(existing) if !existing.is_empty() => format!("{script}\n{existing}"),
                _ => script,
            };
            capture_spec["initScript"] = json!(combined);
        }
        let browser = self.browser.as_mut().ok_or("browser unavailable")?;
        browser.capture(&capture_spec, input, width, image)
    }
}

pub(crate) struct ElectronDesktopSurface;

impl SurfaceAdapter for ElectronDesktopSurface {
    fn descriptor(&self) -> SurfaceDescriptor {
        SurfaceDescriptor {
            id: SURFACE_ELECTRON_DESKTOP.into(),
            kind: SurfaceKind::Desktop,
            tier: SupportTier::Universal,
            host_kinds: vec!["electron".into()],
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
        &ElectronOracle
    }

    fn check_env(&self) -> Result<(), UnrenderableReason> {
        let exe = crate::chromium::executable();
        if exe.is_file() {
            Ok(())
        } else {
            Err(UnrenderableReason::new(
                "no-browser",
                "Chromium/Chrome executable not found for Electron renderer capture",
            ))
        }
    }

    fn run(&self, project: &Path, spec: &Path, out: &Path, force: bool) -> Reply {
        let manifest_spec: Value =
            serde_json::from_slice(&fs::read(spec).map_err(|e| failure(2, &e.to_string()))?)
                .map_err(|e| failure(2, &e.to_string()))?;
        if manifest_spec["mode"].as_str() == Some("binary") && electron_binary(project).is_none() {
            let reason = UnrenderableReason::new(
                "no-electron-binary",
                "Electron binary not found (install electron or set DUI_ELECTRON)",
            );
            let reply = crate::surface::unrenderable_reply(SURFACE_ELECTRON_DESKTOP, &reason);
            let _ = fs::create_dir_all(out);
            let _ = analysis::store(&out.join("report.json"), &reply);
            return Ok(reply);
        }
        let facts = analysis::analyze_with_oracle(self.oracle(), project, spec)?;
        let exe = crate::chromium::executable();
        let browser_version = crate::chromium::version_tag(&exe);
        let mut lazy = LazyElectronBrowser {
            project,
            manifest_spec,
            exe: exe.clone(),
            profile: None,
            session: None,
            browser: None,
        };
        let renderer = exe.is_file().then_some(&mut lazy as &mut dyn Renderer);
        analysis::exercise_with_renderer(
            renderer,
            facts,
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

    #[test]
    fn builds_electron_preload_ipc_mock_script() {
        let spec = json!({
            "host": "electron",
            "preloadGlobal": "desktopBridge",
            "ipcMocks": { "getVersion": "45.0.0" }
        });
        let script = build_electron_preload_script(&spec).expect("preload script");
        assert!(script.contains("\"desktopBridge\""));
        assert!(script.contains("\"getVersion\":\"45.0.0\""));
    }
}
