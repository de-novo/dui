use crate::diff::{RenderNodeSnapshot, RenderRect};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IosSimulatorDevice {
    pub udid: String,
    pub name: String,
    pub state: String,
    pub runtime: String,
    pub is_available: bool,
}

/// Parse JSON output of `xcrun simctl list devices available -j`.
pub fn parse_simctl_devices_json(raw: &str) -> Vec<IosSimulatorDevice> {
    let Ok(parsed) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    let Some(runtimes) = parsed["devices"].as_object() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (runtime, list) in runtimes {
        let Some(entries) = list.as_array() else {
            continue;
        };
        for dev in entries {
            let Some(udid) = dev["udid"].as_str().filter(|s| !s.is_empty()) else {
                continue;
            };
            let name = dev["name"].as_str().unwrap_or("iOS Simulator").to_owned();
            let state = dev["state"].as_str().unwrap_or("Shutdown").to_owned();
            let is_available = dev["isAvailable"].as_bool().unwrap_or(true);
            out.push(IosSimulatorDevice {
                udid: udid.to_owned(),
                name,
                state,
                runtime: runtime.clone(),
                is_available,
            });
        }
    }
    out.sort_by(|a, b| a.udid.cmp(&b.udid));
    out
}

pub fn select_booted_simulator(
    devices: &[IosSimulatorDevice],
    requested: Option<&str>,
) -> Option<IosSimulatorDevice> {
    if let Some(req) = requested {
        return devices
            .iter()
            .find(|d| {
                d.is_available
                    && d.state == "Booted"
                    && (d.udid == req || d.name.eq_ignore_ascii_case(req))
            })
            .cloned();
    }
    devices
        .iter()
        .find(|d| d.is_available && d.state == "Booted")
        .cloned()
}

fn normalize_xcui_type(raw: &str) -> (&'static str, Option<&'static str>) {
    let trimmed = raw.strip_prefix("XCUIElementType").unwrap_or(raw);
    match trimmed {
        "Button" => ("button", Some("button")),
        "StaticText" | "Text" => ("text", Some("text")),
        "TextField" | "SecureTextField" | "SearchField" | "TextView" => ("input", Some("textbox")),
        "Image" => ("img", Some("img")),
        "Switch" | "Toggle" => ("switch", Some("switch")),
        "CheckBox" => ("checkbox", Some("checkbox")),
        "Link" => ("a", Some("link")),
        "NavigationBar" => ("header", Some("banner")),
        "TabBar" => ("nav", Some("navigation")),
        "Tab" => ("tab", Some("tab")),
        "Cell" => ("li", Some("listitem")),
        "Table" | "CollectionView" | "Outline" => ("list", Some("list")),
        "ScrollView" => ("scroll", Some("region")),
        "Alert" | "Sheet" | "Dialog" => ("dialog", Some("dialog")),
        "Heading" => ("h1", Some("heading")),
        _ => ("view", None),
    }
}

fn walk_xcui_node(
    val: &Value,
    parent_id: Option<&str>,
    id_counts: &mut BTreeMap<String, usize>,
    out: &mut Vec<RenderNodeSnapshot>,
) {
    if out.len() >= 250 {
        return;
    }
    if let Some(arr) = val.as_array() {
        for item in arr {
            walk_xcui_node(item, parent_id, id_counts, out);
        }
        return;
    }
    let Some(obj) = val.as_object() else {
        return;
    };
    let raw_type = obj
        .get("type")
        .or_else(|| obj.get("role"))
        .and_then(Value::as_str)
        .unwrap_or("Other");
    let (tag, base_role) = normalize_xcui_type(raw_type);

    let frame = obj.get("frame").and_then(Value::as_object);
    let rect = frame
        .map(|f| RenderRect {
            x: f.get("x").and_then(Value::as_f64).unwrap_or(0.0).round() as i32,
            y: f.get("y").and_then(Value::as_f64).unwrap_or(0.0).round() as i32,
            width: f
                .get("width")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
                .round() as i32,
            height: f
                .get("height")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
                .round() as i32,
        })
        .unwrap_or(RenderRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        });

    let text = obj
        .get("AXLabel")
        .or_else(|| obj.get("label"))
        .or_else(|| obj.get("AXTitle"))
        .or_else(|| obj.get("title"))
        .or_else(|| obj.get("AXValue"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_owned();

    let identifier = obj
        .get("AXUniqueId")
        .or_else(|| obj.get("identifier"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);

    let meaningful = !text.is_empty() || identifier.is_some() || tag != "view";
    let next_parent = if meaningful && rect.width > 0 && rect.height > 0 {
        let key_base = identifier.clone().unwrap_or_else(|| tag.to_owned());
        let count = id_counts.entry(key_base.clone()).or_insert(0);
        let id = format!("{key_base}#{count}");
        *count += 1;

        let mut attributes = BTreeMap::new();
        attributes.insert("xcuiType".into(), raw_type.to_owned());
        if let Some(ident) = identifier {
            attributes.insert("identifier".into(), ident);
        }
        if let Some(enabled) = obj.get("enabled").and_then(Value::as_bool) {
            attributes.insert("enabled".into(), enabled.to_string());
        }
        if let Some(selected) = obj.get("selected").and_then(Value::as_bool) {
            attributes.insert("selected".into(), selected.to_string());
        }
        if let Some(val_str) = obj
            .get("AXValue")
            .or_else(|| obj.get("value"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            attributes.insert("value".into(), val_str.to_owned());
        }

        out.push(RenderNodeSnapshot {
            id: id.clone(),
            tag: tag.into(),
            role: base_role.map(str::to_owned),
            text,
            rect,
            style: BTreeMap::new(),
            parent_id: parent_id.map(str::to_owned),
            box_model: None,
            attributes,
        });
        Some(id)
    } else {
        parent_id.map(str::to_owned)
    };

    if let Some(children) = obj.get("children") {
        walk_xcui_node(children, next_parent.as_deref(), id_counts, out);
    }
}

/// Normalize XCUI / `idb ui describe-all` accessibility JSON into unified `RenderNodeSnapshot`s.
pub fn parse_xcui_accessibility_tree(raw: &Value) -> Vec<RenderNodeSnapshot> {
    let mut nodes = Vec::new();
    let mut id_counts = BTreeMap::new();
    walk_xcui_node(raw, None, &mut id_counts, &mut nodes);
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::diff_render_trees;
    use serde_json::json;

    #[test]
    fn parses_simctl_devices_json_and_picks_booted_simulator() {
        let sample = r#"{
          "devices": {
            "com.apple.CoreSimulator.SimRuntime.iOS-18-4": [
              {"udid":"AAA-111","name":"iPhone 16","state":"Shutdown","isAvailable":true},
              {"udid":"BBB-222","name":"iPhone 16 Pro","state":"Booted","isAvailable":true}
            ]
          }
        }"#;
        let devices = parse_simctl_devices_json(sample);
        assert_eq!(devices.len(), 2);
        assert_eq!(
            select_booted_simulator(&devices, None).map(|d| d.udid),
            Some("BBB-222".into())
        );
        assert_eq!(
            select_booted_simulator(&devices, Some("iPhone 16 Pro")).map(|d| d.udid),
            Some("BBB-222".into())
        );
        assert_eq!(select_booted_simulator(&devices, Some("AAA-111")), None);
    }

    #[test]
    fn parses_xcui_accessibility_tree_and_diffs_through_shared_diff_render_trees() {
        let before_tree = json!({
            "type": "Application",
            "frame": {"x": 0, "y": 0, "width": 393, "height": 852},
            "children": [
                {
                    "type": "XCUIElementTypeNavigationBar",
                    "AXUniqueId": "nav_bar",
                    "frame": {"x": 0, "y": 54, "width": 393, "height": 44},
                    "children": [
                        {
                            "type": "XCUIElementTypeStaticText",
                            "AXUniqueId": "title",
                            "AXLabel": "Checkout",
                            "frame": {"x": 140, "y": 64, "width": 110, "height": 24}
                        }
                    ]
                },
                {
                    "type": "XCUIElementTypeButton",
                    "AXUniqueId": "pay_btn",
                    "AXLabel": "Pay $29",
                    "enabled": true,
                    "frame": {"x": 16, "y": 760, "width": 361, "height": 50}
                }
            ]
        });
        let after_tree = json!({
            "type": "Application",
            "frame": {"x": 0, "y": 0, "width": 393, "height": 852},
            "children": [
                {
                    "type": "XCUIElementTypeNavigationBar",
                    "AXUniqueId": "nav_bar",
                    "frame": {"x": 0, "y": 54, "width": 393, "height": 44},
                    "children": [
                        {
                            "type": "XCUIElementTypeStaticText",
                            "AXUniqueId": "title",
                            "AXLabel": "Checkout",
                            "frame": {"x": 140, "y": 64, "width": 110, "height": 24}
                        }
                    ]
                },
                {
                    "type": "XCUIElementTypeButton",
                    "AXUniqueId": "pay_btn",
                    "AXLabel": "Pay $39",
                    "enabled": true,
                    "frame": {"x": 16, "y": 760, "width": 361, "height": 50}
                }
            ]
        });

        let before = parse_xcui_accessibility_tree(&before_tree);
        let after = parse_xcui_accessibility_tree(&after_tree);
        assert_eq!(before.len(), 3);
        assert_eq!(before[1].parent_id.as_deref(), Some("nav_bar#0"));
        assert_eq!(before[2].id, "pay_btn#0");
        assert_eq!(before[2].tag, "button");

        let diff = diff_render_trees(&before, &after, true);
        assert_eq!(diff.text_changed, 1);
        assert_eq!(diff.changes[0].id, "pay_btn#0");
        assert_eq!(diff.changes[0].before_text.as_deref(), Some("Pay $29"));
        assert_eq!(diff.changes[0].after_text.as_deref(), Some("Pay $39"));
    }
}
