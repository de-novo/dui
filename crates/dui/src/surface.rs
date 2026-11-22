use crate::{Reply, analysis, failure, ports::TypeOracle, typescript::TypeScript};
use dui_core::surface::{
    SURFACE_GPUI_DESKTOP, SURFACE_VITE_WEB, SupportTier, SurfaceCapabilities, SurfaceDescriptor,
    SurfaceKind, UnrenderableReason, resolve_surface_id,
};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub struct NoopOracle;

impl TypeOracle for NoopOracle {
    fn facts(&self, _project: &Path, spec: &Path) -> Reply {
        let bytes = fs::read(spec).map_err(|e| failure(2, &e.to_string()))?;
        let manifest: Value =
            serde_json::from_slice(&bytes).map_err(|e| failure(2, &e.to_string()))?;
        let path = manifest.get("path").and_then(Value::as_str).unwrap_or("/");
        Ok(json!({
            "status": "ok",
            "tier": "universal",
            "compiler": "none",
            "path": path,
            "diagnostics": [],
            "facts": [],
            "autoMocks": {},
            "files": []
        }))
    }
}

pub trait SurfaceAdapter {
    fn descriptor(&self) -> SurfaceDescriptor;
    fn oracle(&self) -> &dyn TypeOracle {
        &NoopOracle
    }
    fn check_env(&self) -> Result<(), UnrenderableReason> {
        Ok(())
    }
    fn run(&self, project: &Path, spec: &Path, out: &Path, force: bool) -> Reply;
}

pub fn unrenderable_reply(surface_id: &str, reason: &UnrenderableReason) -> Value {
    json!({
        "status": "unrenderable",
        "surface": surface_id,
        "reason": reason.code,
        "detail": reason.detail,
        "verified": 0,
        "attempted": 0
    })
}

struct ViteWebSurface;

impl SurfaceAdapter for ViteWebSurface {
    fn descriptor(&self) -> SurfaceDescriptor {
        SurfaceDescriptor {
            id: SURFACE_VITE_WEB.into(),
            kind: SurfaceKind::Web,
            tier: SupportTier::CompilerDeep,
            host_kinds: vec!["vite".into(), "nitro".into()],
            capabilities: SurfaceCapabilities {
                state_injection: true,
                live_edit: true,
                link_discovery: true,
                render_tree: true,
                box_model: false,
                pixel_capture: true,
            },
        }
    }

    fn oracle(&self) -> &dyn TypeOracle {
        &TypeScript
    }

    fn run(&self, project: &Path, spec: &Path, out: &Path, force: bool) -> Reply {
        let facts = analysis::analyze_with_oracle(self.oracle(), project, spec)?;
        crate::host::run_vite_web_with_facts(project, spec, facts, out, force)
    }
}

struct GpuiDesktopSurface;

impl SurfaceAdapter for GpuiDesktopSurface {
    fn descriptor(&self) -> SurfaceDescriptor {
        SurfaceDescriptor {
            id: SURFACE_GPUI_DESKTOP.into(),
            kind: SurfaceKind::Desktop,
            tier: SupportTier::CompilerDeep,
            host_kinds: vec!["gpui".into()],
            capabilities: SurfaceCapabilities {
                state_injection: true,
                live_edit: true,
                link_discovery: false,
                render_tree: true,
                box_model: false,
                pixel_capture: true,
            },
        }
    }

    fn run(&self, project: &Path, spec: &Path, out: &Path, _force: bool) -> Reply {
        crate::gpui::run(project, spec, out)
    }
}

pub struct SurfaceRegistry {
    adapters: Vec<Box<dyn SurfaceAdapter>>,
}

impl SurfaceRegistry {
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    pub fn default_registry() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(ViteWebSurface));
        registry.register(Box::new(crate::universal_web::UniversalWebSurface));
        registry.register(Box::new(GpuiDesktopSurface));
        registry.register(Box::new(crate::electron::ElectronDesktopSurface));
        registry.register(Box::new(crate::tui::UniversalTuiSurface));
        registry.register(Box::new(crate::android::AndroidSurface));
        registry
    }

    pub fn register(&mut self, adapter: Box<dyn SurfaceAdapter>) {
        self.adapters.push(adapter);
    }

    #[cfg(test)]
    pub fn descriptors(&self) -> Vec<SurfaceDescriptor> {
        self.adapters.iter().map(|a| a.descriptor()).collect()
    }

    pub fn find(&self, surface_id: &str) -> Option<&dyn SurfaceAdapter> {
        self.adapters
            .iter()
            .find(|a| a.descriptor().id == surface_id)
            .map(Box::as_ref)
    }

    pub fn dispatch(&self, project: &Path, spec: &Path, out: &Path, force: bool) -> Reply {
        let manifest: Value = match fs::read(spec)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        {
            Some(value) => value,
            None => Value::Null,
        };
        let surface_id = resolve_surface_id(&manifest);
        let Some(adapter) = self.find(surface_id) else {
            let reply = unrenderable_reply(
                surface_id,
                &UnrenderableReason::new(
                    "surface-not-registered",
                    format!("surface adapter `{surface_id}` is not registered in this build"),
                ),
            );
            let _ = fs::create_dir_all(out);
            let _ = crate::analysis::store(&out.join("report.json"), &reply);
            return Ok(reply);
        };
        if let Err(reason) = adapter.check_env() {
            let reply = unrenderable_reply(surface_id, &reason);
            let _ = fs::create_dir_all(out);
            let _ = crate::analysis::store(&out.join("report.json"), &reply);
            return Ok(reply);
        }
        adapter.run(project, spec, out, force)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dui_core::surface::SURFACE_ANDROID_MOBILE;

    struct MockAndroidSurface {
        has_adb: bool,
    }

    impl SurfaceAdapter for MockAndroidSurface {
        fn descriptor(&self) -> SurfaceDescriptor {
            SurfaceDescriptor {
                id: SURFACE_ANDROID_MOBILE.into(),
                kind: SurfaceKind::Mobile,
                tier: SupportTier::Universal,
                host_kinds: vec!["android".into()],
                capabilities: SurfaceCapabilities {
                    state_injection: false,
                    live_edit: false,
                    link_discovery: false,
                    render_tree: true,
                    box_model: true,
                    pixel_capture: true,
                },
            }
        }

        fn check_env(&self) -> Result<(), UnrenderableReason> {
            if self.has_adb {
                Ok(())
            } else {
                Err(UnrenderableReason::new(
                    "no-android-emulator",
                    "adb executable or Android emulator not available",
                ))
            }
        }

        fn run(&self, _project: &Path, _spec: &Path, _out: &Path, _force: bool) -> Reply {
            Ok(json!({"status": "ok", "surface": SURFACE_ANDROID_MOBILE, "verified": 1}))
        }
    }

    #[test]
    fn default_registry_contains_web_desktop_tui_and_android_surfaces() {
        let registry = SurfaceRegistry::default_registry();
        let ids: Vec<String> = registry.descriptors().into_iter().map(|d| d.id).collect();
        assert_eq!(
            ids,
            vec![
                SURFACE_VITE_WEB,
                dui_core::surface::SURFACE_UNIVERSAL_WEB,
                SURFACE_GPUI_DESKTOP,
                dui_core::surface::SURFACE_ELECTRON_DESKTOP,
                dui_core::surface::SURFACE_UNIVERSAL_TUI,
                SURFACE_ANDROID_MOBILE,
            ]
        );
        assert!(registry.find(SURFACE_VITE_WEB).is_some());
        assert!(
            registry
                .find(dui_core::surface::SURFACE_UNIVERSAL_WEB)
                .is_some()
        );
        assert!(registry.find(SURFACE_GPUI_DESKTOP).is_some());
        assert!(
            registry
                .find(dui_core::surface::SURFACE_ELECTRON_DESKTOP)
                .is_some()
        );
        assert!(
            registry
                .find(dui_core::surface::SURFACE_UNIVERSAL_TUI)
                .is_some()
        );
        assert!(registry.find(SURFACE_ANDROID_MOBILE).is_some());
    }

    #[test]
    fn custom_surface_adapter_reports_unrenderable_when_env_missing_and_runs_when_ready() {
        let dir = tempfile::tempdir().expect("tempdir");
        let spec = dir.path().join("dui.json");
        fs::write(&spec, r#"{"host":"android","widths":[390]}"#).expect("write manifest");

        let mut missing_env = SurfaceRegistry::new();
        missing_env.register(Box::new(MockAndroidSurface { has_adb: false }));
        let reply = missing_env
            .dispatch(dir.path(), &spec, &dir.path().join("out"), false)
            .expect("unrenderable reply is Ok");
        assert_eq!(reply["status"], "unrenderable");
        assert_eq!(reply["surface"], SURFACE_ANDROID_MOBILE);
        assert_eq!(reply["reason"], "no-android-emulator");

        let mut ready = SurfaceRegistry::new();
        ready.register(Box::new(MockAndroidSurface { has_adb: true }));
        let ok = ready
            .dispatch(dir.path(), &spec, &dir.path().join("out"), false)
            .expect("ready reply");
        assert_eq!(ok["status"], "ok");
        assert_eq!(ok["surface"], SURFACE_ANDROID_MOBILE);
    }

    #[test]
    fn unregistered_surface_returns_honest_unrenderable_reply() {
        let dir = tempfile::tempdir().expect("tempdir");
        let spec = dir.path().join("dui.json");
        fs::write(&spec, r#"{"host":"ios","widths":[390]}"#).expect("write manifest");

        let registry = SurfaceRegistry::default_registry();
        let reply = registry
            .dispatch(dir.path(), &spec, &dir.path().join("out"), false)
            .expect("unrenderable reply");
        assert_eq!(reply["status"], "unrenderable");
        assert_eq!(reply["surface"], "ios-mobile");
        assert_eq!(reply["reason"], "surface-not-registered");
    }

    #[test]
    fn noop_oracle_returns_deterministic_universal_facts_without_compiler() {
        let dir = tempfile::tempdir().expect("tempdir");
        let spec = dir.path().join("dui.json");
        fs::write(
            &spec,
            r#"{"host":"static","path":"/home","widths":[375,1280]}"#,
        )
        .expect("write manifest");

        let first = NoopOracle.facts(dir.path(), &spec).expect("first facts");
        let second = NoopOracle.facts(dir.path(), &spec).expect("second facts");
        assert_eq!(first, second);
        assert_eq!(first["tier"], "universal");
        assert_eq!(first["compiler"], "none");
        assert_eq!(first["path"], "/home");
        assert_eq!(first["facts"], json!([]));
    }
}
