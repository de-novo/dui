use gpui_kit::{
    AppContext, Bounds, Context, HeadlessAppContext, Render, Window, WindowBounds, WindowOptions,
    assets::Assets,
    component::{
        ActiveTheme,
        button::{Button, ButtonVariants as _},
        checkbox::Checkbox,
        label::Label,
    },
    div,
    prelude::*,
    px, size,
    test::{TestQueryExt, TestWindowExt},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    env,
    fs::{self, File},
    io::{self, BufRead, BufWriter, Write},
    path::Path,
    sync::Arc,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Density {
    Compact,
    Expanded,
}

impl Density {
    fn as_str(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Expanded => "expanded",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PanelState {
    enabled: bool,
    density: Density,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            enabled: false,
            density: Density::Compact,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
struct SettingsModel {
    #[serde(default)]
    primary: PanelState,
    #[serde(default)]
    secondary: PanelState,
}

struct SettingsScreen {
    model: SettingsModel,
}

impl SettingsScreen {
    fn new(model: SettingsModel) -> Self {
        Self { model }
    }

    fn render_panel(
        &self,
        id_prefix: &'static str,
        title: &'static str,
        panel: PanelState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let density_label = format!("{title} density: {}", panel.density.as_str());
        let status_text = if panel.enabled { "Enabled" } else { "Disabled" };
        let alert_label = format!("{title} alerts");
        let card_bg = if panel.enabled {
            cx.theme().secondary
        } else {
            cx.theme().background
        };

        div()
            .id(format!("{id_prefix}-card"))
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(card_bg)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(Label::new(format!("{title} notifications")))
                    .child(Label::new(status_text)),
            )
            .child(
                Checkbox::new(format!("{id_prefix}-enabled"))
                    .label(alert_label.clone())
                    .accessibility_label(alert_label)
                    .checked(panel.enabled),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new(format!("{id_prefix}-density"))
                            .label(density_label.clone())
                            .when(panel.density == Density::Expanded, |b| b.primary())
                            .when(panel.density == Density::Compact, |b| b.outline()),
                    )
                    .when(panel.density == Density::Expanded, |row| {
                        row.child(Label::new("Instant push + digest"))
                    }),
            )
    }
}

impl Render for SettingsScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-root")
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(Label::new("Notification settings"))
            .child(self.render_panel("primary", "Primary", self.model.primary, cx))
            .child(self.render_panel("secondary", "Secondary", self.model.secondary, cx))
    }
}

fn render_path_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "metal"
    } else if cfg!(target_os = "linux") {
        "wgpu"
    } else {
        "unsupported"
    }
}

fn write_rgba_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    Ok(())
}

fn collect_elements(window: &Window) -> Vec<Value> {
    window
        .elements()
        .into_iter()
        .map(|el| {
            let bounds = el.bounds();
            json!({
                "path": format!("{:?}", el.path()),
                "role": el.role().map(|r| format!("{r:?}")),
                "label": el.label(),
                "checked": el.checked(),
                "visible": el.visible(),
                "bounds": {
                    "x": f32::from(bounds.origin.x),
                    "y": f32::from(bounds.origin.y),
                    "width": f32::from(bounds.size.width),
                    "height": f32::from(bounds.size.height),
                }
            })
        })
        .collect()
}

fn inspect_controls_with_baseline(model: SettingsModel, baseline: SettingsModel) -> Value {
    let entries = [
        ("primary", "Primary", model.primary, baseline.primary),
        (
            "secondary",
            "Secondary",
            model.secondary,
            baseline.secondary,
        ),
    ];
    let mut controls = Vec::new();
    for (inst, title, state, initial) in entries {
        controls.push(json!({
            "instance": inst,
            "owner": "SettingsScreen",
            "slot": "enabled",
            "label": format!("{title} alerts"),
            "type": "bool",
            "value": state.enabled,
            "initialValue": initial.enabled,
            "editable": true,
            "options": [false, true],
        }));
        controls.push(json!({
            "instance": inst,
            "owner": "SettingsScreen",
            "slot": "density",
            "label": format!("{title} density"),
            "type": "Density",
            "value": state.density.as_str(),
            "initialValue": initial.density.as_str(),
            "editable": true,
            "options": ["compact", "expanded"],
        }));
    }
    Value::Array(controls)
}

fn inspect_controls(model: SettingsModel) -> Value {
    inspect_controls_with_baseline(model, model)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureRequest {
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default)]
    input: SettingsModel,
    image_path: String,
    #[serde(default)]
    expect: Vec<Value>,
}

fn default_height() -> u32 {
    320
}

fn check_expectations(elements: &[Value], expect: &[Value]) -> Vec<Value> {
    let mut results = Vec::new();
    for rule in expect {
        let label = rule["label"].as_str().unwrap_or("");
        let found = elements
            .iter()
            .find(|el| el["label"].as_str() == Some(label) && el["visible"] == json!(true));
        let passed = match found {
            None => false,
            Some(el) => {
                if let Some(expected_checked) = rule.get("checked").and_then(Value::as_bool) {
                    el["checked"].as_bool() == Some(expected_checked)
                } else {
                    true
                }
            }
        };
        results.push(json!({
            "label": label,
            "checked": rule.get("checked"),
            "passed": passed,
            "observed": found,
        }));
    }
    results
}

fn run_capture(req: CaptureRequest) -> Value {
    let no_render = env::var("DUI_GPUI_NO_RENDER")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if no_render {
        return json!({
            "status": "unrenderable",
            "reason": "no-render-env",
            "os": env::consts::OS,
            "arch": env::consts::ARCH,
            "renderPath": render_path_name(),
            "width": req.width,
            "height": req.height,
            "elementTreeAvailable": false,
            "elements": [],
            "controls": inspect_controls(req.input),
        });
    }

    let renderer_probe = gpui_kit::platform::current_headless_renderer();
    if let Err(err) = renderer_probe {
        return json!({
            "status": "unrenderable",
            "reason": format!("no-render-env: {err}"),
            "os": env::consts::OS,
            "arch": env::consts::ARCH,
            "renderPath": render_path_name(),
            "width": req.width,
            "height": req.height,
            "elementTreeAvailable": false,
            "elements": [],
            "controls": inspect_controls(req.input),
        });
    }

    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let model = req.input;
    let opened = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: size(px(req.width as f32), px(req.height as f32)),
                })),
                focus: false,
                show: false,
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|_| SettingsScreen::new(model)),
        )
    });

    let (handle, _) = match opened {
        Ok(pair) => pair,
        Err(err) => {
            return json!({
                "status": "unrenderable",
                "reason": format!("open-window-failed: {err}"),
                "os": env::consts::OS,
                "arch": env::consts::ARCH,
                "renderPath": render_path_name(),
                "width": req.width,
                "height": req.height,
                "elementTreeAvailable": false,
            });
        }
    };

    let elements = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            collect_elements(window)
        })
        .unwrap_or_default();

    let screenshot = match cx.capture_screenshot(handle) {
        Ok(img) => img,
        Err(err) => {
            return json!({
                "status": "unrenderable",
                "reason": format!("no-render-env: {err}"),
                "os": env::consts::OS,
                "arch": env::consts::ARCH,
                "renderPath": render_path_name(),
                "width": req.width,
                "height": req.height,
                "elementTreeAvailable": !elements.is_empty(),
                "elements": elements,
                "controls": inspect_controls(model),
            });
        }
    };

    let (img_w, img_h) = screenshot.dimensions();
    if let Err(err) = write_rgba_png(
        Path::new(&req.image_path),
        img_w,
        img_h,
        &screenshot.into_raw(),
    ) {
        return json!({
            "status": "unverified",
            "reason": format!("png-write-failed: {err}"),
            "os": env::consts::OS,
            "arch": env::consts::ARCH,
            "renderPath": render_path_name(),
            "width": req.width,
            "height": req.height,
            "elementTreeAvailable": !elements.is_empty(),
            "elements": elements,
            "controls": inspect_controls(model),
        });
    }

    let assertions = check_expectations(&elements, &req.expect);
    let all_passed =
        !assertions.is_empty() && assertions.iter().all(|a| a["passed"] == json!(true));
    let status = if all_passed {
        "verified"
    } else if req.expect.is_empty() {
        "unverified"
    } else {
        "unverified"
    };
    let reason = if all_passed {
        None
    } else if req.expect.is_empty() {
        Some("missing-screen-assertion")
    } else {
        Some("semantic-assertion-failed")
    };

    json!({
        "status": status,
        "reason": reason,
        "os": env::consts::OS,
        "arch": env::consts::ARCH,
        "renderPath": render_path_name(),
        "width": req.width,
        "height": req.height,
        "pixelWidth": img_w,
        "pixelHeight": img_h,
        "elementTreeAvailable": !elements.is_empty(),
        "elements": elements,
        "assertions": assertions,
        "controls": inspect_controls(model),
        "model": model,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionInit {
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default)]
    input: SettingsModel,
}

fn panel_mut<'a>(
    model: &'a mut SettingsModel,
    instance: &str,
) -> Result<&'a mut PanelState, &'static str> {
    match instance {
        "primary" => Ok(&mut model.primary),
        "secondary" => Ok(&mut model.secondary),
        _ => Err("unknown-instance"),
    }
}

fn panel_ref(model: &SettingsModel, instance: &str) -> Result<PanelState, &'static str> {
    match instance {
        "primary" => Ok(model.primary),
        "secondary" => Ok(model.secondary),
        _ => Err("unknown-instance"),
    }
}

fn read_slot(panel: PanelState, slot: &str) -> Result<Value, &'static str> {
    match slot {
        "enabled" => Ok(json!(panel.enabled)),
        "density" => Ok(json!(panel.density.as_str())),
        _ => Err("unknown-slot"),
    }
}

fn apply_slot(panel: &mut PanelState, slot: &str, value: &Value) -> Result<(), &'static str> {
    match slot {
        "enabled" => {
            let b = value.as_bool().ok_or("invalid-value")?;
            panel.enabled = b;
            Ok(())
        }
        "density" => {
            let d = match value.as_str() {
                Some("compact") => Density::Compact,
                Some("expanded") => Density::Expanded,
                _ => return Err("invalid-value"),
            };
            panel.density = d;
            Ok(())
        }
        _ => Err("unknown-slot"),
    }
}

fn evaluate_semantic_expect(
    elements: &[Value],
    expect: Option<&Value>,
) -> (bool, Option<&'static str>, Value) {
    let Some(rule) = expect.filter(|v| v.is_object()) else {
        return (
            false,
            Some("missing-semantic-evidence"),
            json!({
                "elementTreeAvailable": !elements.is_empty(),
                "elementsCount": elements.len(),
                "expect": null,
                "matched": false,
                "observed": null,
            }),
        );
    };
    let expected_label = rule
        .get("label")
        .and_then(Value::as_str)
        .or_else(|| rule.get("text").and_then(Value::as_str))
        .unwrap_or("");
    let expected_checked = rule.get("checked").and_then(Value::as_bool);
    let found = elements
        .iter()
        .find(|el| el["label"].as_str() == Some(expected_label) && el["visible"] == json!(true));
    let matched = match found {
        None => false,
        Some(el) => match expected_checked {
            Some(b) => el["checked"].as_bool() == Some(b),
            None => true,
        },
    };
    let reason = if matched {
        None
    } else {
        Some("semantic-mismatch")
    };
    (
        matched,
        reason,
        json!({
            "elementTreeAvailable": !elements.is_empty(),
            "elementsCount": elements.len(),
            "expect": rule,
            "matched": matched,
            "observed": found,
        }),
    )
}

fn run_session() {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut out = io::stdout().lock();

    let Some(Ok(init_line)) = lines.next() else {
        let _ = writeln!(
            out,
            "{}",
            json!({"ok": false, "error": "missing-session-init"})
        );
        let _ = out.flush();
        std::process::exit(2);
    };
    let init: SessionInit = match serde_json::from_str(&init_line) {
        Ok(i) => i,
        Err(err) => {
            let _ = writeln!(
                out,
                "{}",
                json!({"ok": false, "error": format!("invalid-session-init: {err}")})
            );
            let _ = out.flush();
            std::process::exit(2);
        }
    };

    let no_render = env::var("DUI_GPUI_NO_RENDER")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if no_render {
        let _ = writeln!(out, "{}", json!({"ok": false, "error": "no-render-env"}));
        let _ = out.flush();
        std::process::exit(1);
    }
    if let Err(err) = gpui_kit::platform::current_headless_renderer() {
        let _ = writeln!(
            out,
            "{}",
            json!({"ok": false, "error": format!("no-render-env: {err}")})
        );
        let _ = out.flush();
        std::process::exit(1);
    }

    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let baseline = init.input;
    let opened = cx.update(|cx| {
        let mut screen_entity = None;
        let window_res = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: size(px(init.width as f32), px(init.height as f32)),
                })),
                focus: false,
                show: false,
                ..Default::default()
            },
            cx,
            |_, cx| {
                let entity = cx.new(|_| SettingsScreen::new(baseline));
                screen_entity = Some(entity.clone());
                entity
            },
        );
        window_res.map(|(handle, _)| (handle, screen_entity))
    });

    let (handle, Some(entity)) = (match opened {
        Ok(pair) => pair,
        Err(err) => {
            let _ = writeln!(
                out,
                "{}",
                json!({"ok": false, "error": format!("open-window-failed: {err}")})
            );
            let _ = out.flush();
            std::process::exit(1);
        }
    }) else {
        let _ = writeln!(
            out,
            "{}",
            json!({"ok": false, "error": "missing-screen-entity"})
        );
        let _ = out.flush();
        std::process::exit(1);
    };

    let initial_elements = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            collect_elements(window)
        })
        .unwrap_or_default();

    let _ = writeln!(
        out,
        "{}",
        json!({
            "ok": true,
            "os": env::consts::OS,
            "arch": env::consts::ARCH,
            "renderPath": render_path_name(),
            "width": init.width,
            "height": init.height,
            "elementTreeAvailable": !initial_elements.is_empty(),
            "controls": inspect_controls_with_baseline(baseline, baseline),
            "elements": initial_elements,
            "model": baseline,
        })
    );
    let _ = out.flush();

    for line in lines {
        let Ok(line) = line else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(err) => {
                let _ = writeln!(
                    out,
                    "{}",
                    json!({"ok": false, "error": format!("invalid-json: {err}")})
                );
                let _ = out.flush();
                continue;
            }
        };
        let op = req["op"].as_str().unwrap_or("");
        let reply = match op {
            "close" => {
                let _ = writeln!(out, "{}", json!({"ok": true, "closed": true}));
                let _ = out.flush();
                break;
            }
            "tick" => json!({"ok": true}),
            "inspect" => {
                let model = cx.read_entity(&entity, |view, _| view.model);
                let elements = cx
                    .update_window(handle, |_, window, cx| {
                        window.render_frame(cx);
                        collect_elements(window)
                    })
                    .unwrap_or_default();
                json!({
                    "ok": true,
                    "controls": inspect_controls_with_baseline(model, baseline),
                    "elements": elements,
                    "model": model,
                })
            }
            "set" | "reset" => {
                let instance = req["instance"].as_str().unwrap_or("");
                let slot = req["slot"].as_str().unwrap_or("");
                let target_value = if op == "reset" {
                    panel_ref(&baseline, instance).and_then(|p| read_slot(p, slot))
                } else {
                    req.get("value").cloned().ok_or("missing-value")
                };
                match target_value {
                    Err(err) => json!({"ok": false, "error": err}),
                    Ok(expected_value) => {
                        let apply_res = cx.update_entity(&entity, |view, cx| {
                            let panel = panel_mut(&mut view.model, instance)?;
                            apply_slot(panel, slot, &expected_value)?;
                            cx.notify();
                            Ok::<(), &'static str>(())
                        });
                        match apply_res {
                            Err(err) => json!({"ok": false, "error": err}),
                            Ok(()) => {
                                let elements = cx
                                    .update_window(handle, |_, window, cx| {
                                        window.render_frame(cx);
                                        collect_elements(window)
                                    })
                                    .unwrap_or_default();
                                let model = cx.read_entity(&entity, |view, _| view.model);
                                let observed_value = panel_ref(&model, instance)
                                    .and_then(|p| read_slot(p, slot))
                                    .unwrap_or(Value::Null);
                                let value_matches = observed_value == expected_value;
                                let (sem_ok, sem_reason, evidence) =
                                    evaluate_semantic_expect(&elements, req.get("expect"));
                                let (status, reason) = if !value_matches {
                                    ("unverified", Some("value-mismatch"))
                                } else if !sem_ok {
                                    ("unverified", sem_reason)
                                } else {
                                    ("verified", None)
                                };
                                json!({
                                    "ok": true,
                                    "expectedValue": expected_value,
                                    "observedValue": observed_value,
                                    "status": status,
                                    "reason": reason,
                                    "evidence": evidence,
                                    "controls": inspect_controls_with_baseline(model, baseline),
                                    "model": model,
                                })
                            }
                        }
                    }
                }
            }
            "frame" => {
                let image_path = req["imagePath"].as_str().unwrap_or("");
                if image_path.is_empty() {
                    json!({"ok": false, "error": "missing-image-path"})
                } else {
                    let elements = cx
                        .update_window(handle, |_, window, cx| {
                            window.render_frame(cx);
                            collect_elements(window)
                        })
                        .unwrap_or_default();
                    let model = cx.read_entity(&entity, |view, _| view.model);
                    match cx.capture_screenshot(handle) {
                        Ok(img) => {
                            let (w, h) = img.dimensions();
                            match write_rgba_png(Path::new(image_path), w, h, &img.into_raw()) {
                                Ok(()) => json!({
                                    "ok": true,
                                    "pixelWidth": w,
                                    "pixelHeight": h,
                                    "controls": inspect_controls_with_baseline(model, baseline),
                                    "elements": elements,
                                    "model": model,
                                }),
                                Err(err) => json!({"ok": false, "error": err}),
                            }
                        }
                        Err(err) => json!({"ok": false, "error": err.to_string()}),
                    }
                }
            }
            _ => json!({"ok": false, "error": "unknown-operation"}),
        };
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) == Some("--session") {
        run_session();
        return;
    }
    if args.get(1).map(String::as_str) == Some("--capture") {
        let mut line = String::new();
        if io::stdin().lock().read_line(&mut line).is_err() || line.trim().is_empty() {
            eprintln!("expected CaptureRequest JSON on stdin");
            std::process::exit(2);
        }
        let req: CaptureRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(err) => {
                eprintln!("invalid CaptureRequest JSON: {err}");
                std::process::exit(2);
            }
        };
        let reply = run_capture(req);
        let mut out = io::stdout().lock();
        let _ = writeln!(out, "{reply}");
        return;
    }

    eprintln!("usage: dui-gpui-kit-fixture --capture < request.json");
    std::process::exit(2);
}
