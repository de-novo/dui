use crate::{
    Reply, analysis, failure,
    ports::{Renderer, TypeOracle},
    surface::{NoopOracle, SurfaceAdapter},
};
use dui_core::{
    android::{parse_adb_devices, parse_uiautomator_xml, select_online_device},
    surface::{
        SURFACE_ANDROID_MOBILE, SupportTier, SurfaceCapabilities, SurfaceDescriptor, SurfaceKind,
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

const MAX_ANDROID_CACHE_FILES: usize = 500;

fn adb_executable() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("DUI_ADB") {
        let path = PathBuf::from(explicit);
        return path.is_file().then_some(path);
    }
    for env_key in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(sdk) = std::env::var_os(env_key) {
            let candidate = PathBuf::from(sdk).join("platform-tools").join("adb");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let mac_sdk = PathBuf::from(&home).join("Library/Android/sdk/platform-tools/adb");
        if mac_sdk.is_file() {
            return Some(mac_sdk);
        }
        let linux_sdk = PathBuf::from(home).join("Android/Sdk/platform-tools/adb");
        if linux_sdk.is_file() {
            return Some(linux_sdk);
        }
    }
    Command::new("adb")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .filter(|s| s.success())
        .map(|_| PathBuf::from("adb"))
}

fn find_online_android_device(
    adb: &Path,
    requested_serial: Option<&str>,
) -> Result<String, UnrenderableReason> {
    let output = Command::new(adb)
        .arg("devices")
        .output()
        .map_err(|e| UnrenderableReason::new("no-android-emulator", format!("adb failed: {e}")))?;
    if !output.status.success() {
        return Err(UnrenderableReason::new(
            "no-android-emulator",
            format!(
                "adb devices failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let devices = parse_adb_devices(&stdout);
    select_online_device(&devices, requested_serial)
        .map(|d| d.serial)
        .ok_or_else(|| {
            UnrenderableReason::new(
                "no-android-emulator",
                "no online Android emulator or adb device detected (start an AVD or connect via adb)",
            )
        })
}

fn collect_android_source_files(project: &Path) -> Vec<String> {
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
                || matches!(name, "node_modules" | "target" | "build" | ".gradle")
            {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file()
                && let Ok(rel) = path.strip_prefix(&project_root)
            {
                files.push(rel.to_string_lossy().replace('\\', "/"));
                if files.len() >= MAX_ANDROID_CACHE_FILES {
                    files.sort();
                    return files;
                }
            }
        }
    }
    files.sort();
    files
}

struct AndroidOracle;

impl TypeOracle for AndroidOracle {
    fn facts(&self, project: &Path, spec: &Path) -> Reply {
        let mut facts = NoopOracle.facts(project, spec)?;
        facts["host"] = json!("android");
        facts["sourceFiles"] = json!(collect_android_source_files(project));
        Ok(facts)
    }
}

struct AdbRenderer {
    adb: PathBuf,
    serial: String,
    resized: bool,
}

impl Drop for AdbRenderer {
    fn drop(&mut self) {
        if self.resized {
            let _ = Command::new(&self.adb)
                .args(["-s", &self.serial, "shell", "wm", "size", "reset"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

impl AdbRenderer {
    fn run_adb(&self, args: &[&str]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let output = Command::new(&self.adb)
            .arg("-s")
            .arg(&self.serial)
            .args(args)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("adb {:?} failed: {}", args, stderr.trim()).into());
        }
        Ok(output.stdout)
    }

    fn dump_hierarchy_xml(&self) -> Result<String, Box<dyn std::error::Error>> {
        if let Ok(raw) = self.run_adb(&["exec-out", "uiautomator", "dump", "/dev/tty"]) {
            let text = String::from_utf8_lossy(&raw).into_owned();
            if text.contains("<hierarchy") {
                return Ok(text);
            }
        }
        let remote = "/sdcard/dui-window-dump.xml";
        let _ = self.run_adb(&["shell", "uiautomator", "dump", remote])?;
        let raw = self.run_adb(&["exec-out", "cat", remote])?;
        let _ = self.run_adb(&["shell", "rm", "-f", remote]);
        Ok(String::from_utf8_lossy(&raw).into_owned())
    }
}

impl Renderer for AdbRenderer {
    fn capture(
        &mut self,
        spec: &Value,
        _input: &Value,
        width: u32,
        image: &Path,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        if spec["resizeDevice"] == true {
            let height = spec["height"]
                .as_u64()
                .filter(|h| *h > 0)
                .unwrap_or_else(|| u64::from(width) * 2) as u32;
            let size_arg = format!("{width}x{height}");
            self.run_adb(&["shell", "wm", "size", &size_arg])?;
            self.resized = true;
        }

        if let Some(deep_link) = spec["deepLink"].as_str().filter(|s| !s.is_empty()) {
            let mut args = vec![
                "shell",
                "am",
                "start",
                "-W",
                "-a",
                "android.intent.action.VIEW",
                "-d",
                deep_link,
            ];
            if let Some(pkg) = spec["package"].as_str().filter(|s| !s.is_empty()) {
                args.push(pkg);
            }
            self.run_adb(&args)?;
        } else if let Some(activity) = spec["activity"].as_str().filter(|s| !s.is_empty()) {
            self.run_adb(&["shell", "am", "start", "-W", "-n", activity])?;
        }

        if let Some(events) = spec["keyEvents"].as_array() {
            for ev in events.iter().filter_map(Value::as_str) {
                self.run_adb(&["shell", "input", "keyevent", ev])?;
            }
        }

        let settle_ms = spec["settleMs"].as_u64().unwrap_or(200);
        if settle_ms > 0 {
            thread::sleep(Duration::from_millis(settle_ms));
        }

        let png_bytes = self.run_adb(&["exec-out", "screencap", "-p"])?;
        if png_bytes.len() < 8 || &png_bytes[..8] != b"\x89PNG\r\n\x1a\n" {
            return Err("adb screencap did not return valid PNG data".into());
        }
        fs::write(image, &png_bytes)?;

        let xml = self.dump_hierarchy_xml().unwrap_or_default();
        let render_tree = parse_uiautomator_xml(&xml);
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
            "android": {
                "serial": self.serial,
                "package": spec["package"],
                "activity": spec["activity"],
                "deepLink": spec["deepLink"]
            }
        }))
    }
}

pub(crate) struct AndroidSurface;

impl SurfaceAdapter for AndroidSurface {
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
                box_model: false,
                pixel_capture: true,
            },
        }
    }

    fn oracle(&self) -> &dyn TypeOracle {
        &AndroidOracle
    }

    fn check_env(&self) -> Result<(), UnrenderableReason> {
        let Some(adb) = adb_executable() else {
            return Err(UnrenderableReason::new(
                "no-android-emulator",
                "adb executable not found (install Android SDK platform-tools or set DUI_ADB)",
            ));
        };
        find_online_android_device(&adb, None).map(|_| ())
    }

    fn run(&self, project: &Path, spec: &Path, out: &Path, force: bool) -> Reply {
        let Some(adb) = adb_executable() else {
            return Err(failure(3, "adb executable not found"));
        };
        let manifest: Value =
            serde_json::from_slice(&fs::read(spec).map_err(|e| failure(2, &e.to_string()))?)
                .map_err(|e| failure(2, &e.to_string()))?;
        let requested_serial = manifest["serial"].as_str();
        let serial = match find_online_android_device(&adb, requested_serial) {
            Ok(s) => s,
            Err(reason) => {
                let reply = crate::surface::unrenderable_reply(SURFACE_ANDROID_MOBILE, &reason);
                let _ = fs::create_dir_all(out);
                let _ = analysis::store(&out.join("report.json"), &reply);
                return Ok(reply);
            }
        };
        let facts = analysis::analyze_with_oracle(self.oracle(), project, spec)?;
        let mut renderer = AdbRenderer {
            adb,
            serial: serial.clone(),
            resized: false,
        };
        let runtime_tag = Some(json!({"product": "android-adb", "serial": serial}));
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
