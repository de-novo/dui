use crate::{
    Reply, analysis, failure,
    ports::{Renderer, TypeOracle},
    surface::{NoopOracle, SurfaceAdapter},
};
use dui_core::{
    ios::{parse_simctl_devices_json, parse_xcui_accessibility_tree, select_booted_simulator},
    surface::{
        SURFACE_IOS_MOBILE, SupportTier, SurfaceCapabilities, SurfaceDescriptor, SurfaceKind,
        UnrenderableReason,
    },
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

const MAX_IOS_CACHE_FILES: usize = 500;

struct SimctlCmd {
    program: PathBuf,
    prefix: Vec<&'static str>,
}

impl SimctlCmd {
    fn resolve() -> Result<Self, UnrenderableReason> {
        if let Some(explicit) = std::env::var_os("DUI_SIMCTL") {
            let path = PathBuf::from(explicit);
            if path.is_file() {
                return Ok(Self {
                    program: path,
                    prefix: Vec::new(),
                });
            }
            return Err(UnrenderableReason::new(
                "no-ios-simulator",
                "configured DUI_SIMCTL executable not found",
            ));
        }
        if !cfg!(target_os = "macos") {
            return Err(UnrenderableReason::new(
                "macos-required",
                "iOS Simulator requires macOS with Xcode simctl (set DUI_SIMCTL to override)",
            ));
        }
        let xcrun = PathBuf::from("/usr/bin/xcrun");
        if xcrun.is_file() {
            return Ok(Self {
                program: xcrun,
                prefix: vec!["simctl"],
            });
        }
        Err(UnrenderableReason::new(
            "no-ios-simulator",
            "xcrun simctl not found (install Xcode Command Line Tools)",
        ))
    }

    fn output(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        Command::new(&self.program)
            .args(&self.prefix)
            .args(args)
            .output()
    }
}

fn find_booted_ios_simulator(
    cmd: &SimctlCmd,
    requested: Option<&str>,
) -> Result<(String, String), UnrenderableReason> {
    let output = cmd
        .output(&["list", "devices", "available", "-j"])
        .map_err(|e| {
            UnrenderableReason::new("no-ios-simulator", format!("simctl list failed: {e}"))
        })?;
    if !output.status.success() {
        return Err(UnrenderableReason::new(
            "no-ios-simulator",
            format!(
                "simctl list devices failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let devices = parse_simctl_devices_json(&stdout);
    select_booted_simulator(&devices, requested)
        .map(|d| (d.udid, d.name))
        .ok_or_else(|| {
            UnrenderableReason::new(
                "no-ios-simulator",
                "no booted iOS Simulator found (boot one in Xcode or via `xcrun simctl boot`)",
            )
        })
}

fn collect_ios_source_files(project: &Path) -> Vec<String> {
    let Ok(project_root) = project.canonicalize() else {
        return Vec::new();
    };
    let mut files = Vec::new();
    let mut stack = vec![project_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        children.sort();
        for path in children {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.')
                || matches!(
                    name,
                    "node_modules" | "target" | "DerivedData" | "Pods" | "build"
                )
            {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file()
                && let Ok(rel) = path.strip_prefix(&project_root)
            {
                files.push(rel.to_string_lossy().replace('\\', "/"));
                if files.len() >= MAX_IOS_CACHE_FILES {
                    files.sort();
                    return files;
                }
            }
        }
    }
    files.sort();
    files
}

struct IosOracle;

impl TypeOracle for IosOracle {
    fn facts(&self, project: &Path, spec: &Path) -> Reply {
        let mut facts = NoopOracle.facts(project, spec)?;
        facts["host"] = json!("ios");
        facts["sourceFiles"] = json!(collect_ios_source_files(project));
        Ok(facts)
    }
}

struct SimctlRenderer {
    cmd: SimctlCmd,
    udid: String,
    device_name: String,
    status_bar_overridden: bool,
}

impl Drop for SimctlRenderer {
    fn drop(&mut self) {
        if self.status_bar_overridden {
            let _ = self.cmd.output(&["status_bar", &self.udid, "clear"]);
        }
    }
}

impl SimctlRenderer {
    fn run_simctl(&self, args: &[&str]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let output = self.cmd.output(args)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("simctl {:?} failed: {}", args, stderr.trim()).into());
        }
        Ok(output.stdout)
    }

    fn dump_accessibility_tree(&self) -> Option<Value> {
        if let Some(tree_cmd) = std::env::var_os("DUI_IOS_TREE_CMD") {
            let path = PathBuf::from(tree_cmd);
            if path.is_file()
                && let Ok(out) = Command::new(path).arg(&self.udid).output()
                && out.status.success()
                && let Ok(parsed) = serde_json::from_slice::<Value>(&out.stdout)
            {
                return Some(parsed);
            }
        }
        if let Ok(out) = Command::new("idb")
            .args(["ui", "describe-all", "--udid", &self.udid, "--json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            && out.status.success()
            && let Ok(parsed) = serde_json::from_slice::<Value>(&out.stdout)
        {
            return Some(parsed);
        }
        None
    }
}

impl Renderer for SimctlRenderer {
    fn capture(
        &mut self,
        spec: &Value,
        _input: &Value,
        width: u32,
        image: &Path,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        if spec["deterministicStatusBar"].as_bool().unwrap_or(false) && !self.status_bar_overridden
        {
            let _ = self.run_simctl(&[
                "status_bar",
                &self.udid,
                "override",
                "--time",
                "9:41",
                "--batteryState",
                "charged",
                "--batteryLevel",
                "100",
            ]);
            self.status_bar_overridden = true;
        }

        if let Some(bundle_id) = spec["bundleId"].as_str().filter(|s| !s.is_empty()) {
            self.run_simctl(&["launch", &self.udid, bundle_id])?;
        }
        if let Some(deep_link) = spec["deepLink"].as_str().filter(|s| !s.is_empty()) {
            self.run_simctl(&["openurl", &self.udid, deep_link])?;
        }

        let settle_ms = spec["settleMs"].as_u64().unwrap_or(200);
        if settle_ms > 0 {
            thread::sleep(Duration::from_millis(settle_ms));
        }

        let img_str = image.to_str().ok_or("non-UTF-8 screenshot output path")?;
        self.run_simctl(&["io", &self.udid, "screenshot", img_str])?;
        let png_bytes = fs::read(image)?;
        if png_bytes.len() < 8 || &png_bytes[..8] != b"\x89PNG\r\n\x1a\n" {
            return Err("simctl io screenshot did not write valid PNG data".into());
        }

        let render_tree = self
            .dump_accessibility_tree()
            .map(|raw| parse_xcui_accessibility_tree(&raw))
            .unwrap_or_default();
        let text: String = render_tree
            .iter()
            .filter(|n| !n.text.is_empty())
            .map(|n| n.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let path = spec["path"].as_str().unwrap_or("/");
        Ok(json!({
            "ready": true,
            "settled": true,
            "path": path,
            "width": width,
            "slots": {},
            "hits": [],
            "links": [],
            "responses": {},
            "responseChecks": {},
            "assertions": [],
            "text": text,
            "renderTree": render_tree,
            "ios": {
                "udid": self.udid,
                "deviceName": self.device_name,
                "bundleId": spec["bundleId"],
                "deepLink": spec["deepLink"]
            }
        }))
    }
}

pub(crate) struct IosMobileSurface;

impl SurfaceAdapter for IosMobileSurface {
    fn descriptor(&self) -> SurfaceDescriptor {
        SurfaceDescriptor {
            id: SURFACE_IOS_MOBILE.into(),
            kind: SurfaceKind::Mobile,
            tier: SupportTier::Universal,
            host_kinds: vec!["ios".into()],
            capabilities: SurfaceCapabilities {
                state_injection: false,
                live_edit: false,
                link_discovery: false,
                render_tree: true,
                box_model: false,
                pixel_capture: true,
            },
        }
    }

    fn oracle(&self) -> &dyn TypeOracle {
        &IosOracle
    }

    fn check_env(&self) -> Result<(), UnrenderableReason> {
        let cmd = SimctlCmd::resolve()?;
        find_booted_ios_simulator(&cmd, None).map(|_| ())
    }

    fn run(&self, project: &Path, spec: &Path, out: &Path, force: bool) -> Reply {
        let manifest: Value =
            serde_json::from_slice(&fs::read(spec).map_err(|e| failure(2, &e.to_string()))?)
                .map_err(|e| failure(2, &e.to_string()))?;
        let plan_only = manifest["capture"] == false;
        if plan_only {
            let facts = analysis::analyze_with_oracle(self.oracle(), project, spec)?;
            return analysis::exercise_with_renderer(
                None,
                facts,
                spec,
                Some(project),
                out,
                None,
                force,
            );
        }
        let cmd = match SimctlCmd::resolve() {
            Ok(c) => c,
            Err(reason) => {
                let reply = crate::surface::unrenderable_reply(SURFACE_IOS_MOBILE, &reason);
                let _ = fs::create_dir_all(out);
                let _ = analysis::store(&out.join("report.json"), &reply);
                return Err((5, reply));
            }
        };
        let requested = manifest["udid"]
            .as_str()
            .or_else(|| manifest["device"].as_str());
        let (udid, device_name) = match find_booted_ios_simulator(&cmd, requested) {
            Ok(pair) => pair,
            Err(reason) => {
                let reply = crate::surface::unrenderable_reply(SURFACE_IOS_MOBILE, &reason);
                let _ = fs::create_dir_all(out);
                let _ = analysis::store(&out.join("report.json"), &reply);
                return Err((5, reply));
            }
        };
        let facts = analysis::analyze_with_oracle(self.oracle(), project, spec)?;
        let mut renderer = SimctlRenderer {
            cmd,
            udid: udid.clone(),
            device_name: device_name.clone(),
            status_bar_overridden: false,
        };
        let runtime_tag = Some(json!({
            "product": "ios-simctl",
            "udid": udid,
            "deviceName": device_name
        }));
        analysis::exercise_with_renderer(
            Some(&mut renderer),
            facts,
            spec,
            Some(project),
            out,
            runtime_tag,
            force,
        )
    }
}
