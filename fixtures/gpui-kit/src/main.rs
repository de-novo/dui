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

fn inspect_controls(model: SettingsModel) -> Value {
    let entries = [
        ("primary", "Primary", model.primary),
        ("secondary", "Secondary", model.secondary),
    ];
    let mut controls = Vec::new();
    for (inst, title, state) in entries {
        controls.push(json!({
            "instance": inst,
            "owner": "SettingsScreen",
            "slot": "enabled",
            "label": format!("{title} alerts"),
            "type": "bool",
            "value": state.enabled,
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
            "editable": true,
            "options": ["compact", "expanded"],
        }));
    }
    Value::Array(controls)
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

fn main() {
    let args: Vec<String> = env::args().collect();
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
