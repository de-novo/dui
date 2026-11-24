use crate::diff::{RenderNodeSnapshot, RenderRect};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidDevice {
    pub serial: String,
    pub state: String,
}

pub fn parse_adb_devices(output: &str) -> Vec<AndroidDevice> {
    let mut devices = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("List of devices") || trimmed.starts_with('*')
        {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(serial) = parts.next() else {
            continue;
        };
        let Some(state) = parts.next() else {
            continue;
        };
        devices.push(AndroidDevice {
            serial: serial.to_owned(),
            state: state.to_owned(),
        });
    }
    devices
}

pub fn select_online_device(
    devices: &[AndroidDevice],
    requested_serial: Option<&str>,
) -> Option<AndroidDevice> {
    if let Some(serial) = requested_serial {
        return devices
            .iter()
            .find(|d| d.serial == serial && d.state == "device")
            .cloned();
    }
    devices.iter().find(|d| d.state == "device").cloned()
}

pub fn parse_android_bounds(raw: &str) -> Option<RenderRect> {
    // Format: "[x1,y1][x2,y2]"
    let rest = raw.trim().strip_prefix('[')?;
    let (first, second) = rest.split_once("][")?;
    let second = second.strip_suffix(']')?;
    let (x1s, y1s) = first.split_once(',')?;
    let (x2s, y2s) = second.split_once(',')?;
    let x1 = x1s.trim().parse::<i32>().ok()?;
    let y1 = y1s.trim().parse::<i32>().ok()?;
    let x2 = x2s.trim().parse::<i32>().ok()?;
    let y2 = y2s.trim().parse::<i32>().ok()?;
    Some(RenderRect {
        x: x1,
        y: y1,
        width: (x2 - x1).max(0),
        height: (y2 - y1).max(0),
    })
}

fn unescape_xml(raw: &str) -> String {
    raw.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn normalize_android_class(class_name: &str) -> (&'static str, Option<&'static str>) {
    let short = class_name.rsplit('.').next().unwrap_or(class_name);
    match short {
        "Button" | "ImageButton" | "MaterialButton" | "AppCompatButton" => {
            ("button", Some("button"))
        }
        "TextView" | "AppCompatTextView" | "MaterialTextView" => ("text", Some("text")),
        "EditText" | "AppCompatEditText" | "TextInputEditText" => ("input", Some("textbox")),
        "ImageView" | "AppCompatImageView" => ("img", Some("img")),
        "CheckBox" | "AppCompatCheckBox" | "MaterialCheckBox" => ("checkbox", Some("checkbox")),
        "Switch" | "SwitchCompat" | "SwitchMaterial" => ("switch", Some("switch")),
        "RadioButton" => ("radio", Some("radio")),
        "RecyclerView" | "ListView" => ("list", Some("list")),
        "ScrollView" | "HorizontalScrollView" | "NestedScrollView" => ("scroll", Some("region")),
        "Toolbar" | "ActionBar" => ("header", Some("banner")),
        "ComposeView" | "AndroidComposeView" => ("compose-view", Some("region")),
        "View" | "ViewGroup" | "FrameLayout" | "LinearLayout" | "ConstraintLayout"
        | "RelativeLayout" | "CardView" => ("view", None),
        _ => ("view", None),
    }
}

fn parse_xml_attributes(tag_body: &str) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();
    let bytes = tag_body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b'/' {
            break;
        }
        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let key = &tag_body[key_start..i];
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'=') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let quote = bytes[i];
        if quote != b'"' && quote != b'\'' {
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            continue;
        }
        i += 1;
        let val_start = i;
        while i < bytes.len() && bytes[i] != quote {
            i += 1;
        }
        let val = &tag_body[val_start..i];
        if i < bytes.len() {
            i += 1;
        }
        if !key.is_empty() {
            attrs.insert(key.to_owned(), unescape_xml(val));
        }
    }
    attrs
}

pub fn parse_uiautomator_xml(xml: &str) -> Vec<RenderNodeSnapshot> {
    let mut nodes = Vec::new();
    let mut id_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut parent_stack: Vec<Option<String>> = Vec::new();

    let mut pos = 0;
    while let Some(rel_lt) = xml[pos..].find('<') {
        let lt = pos + rel_lt;
        let Some(rel_gt) = xml[lt..].find('>') else {
            break;
        };
        let gt = lt + rel_gt;
        let token = xml[lt + 1..gt].trim();
        pos = gt + 1;

        if token.starts_with("?xml") || token.starts_with('!') || token.starts_with("hierarchy") {
            continue;
        }
        if token.starts_with("/node") {
            parent_stack.pop();
            continue;
        }
        let Some(rest) = token.strip_prefix("node") else {
            continue;
        };
        if !rest.is_empty() && !rest.starts_with(|c: char| c.is_ascii_whitespace() || c == '/') {
            continue;
        }
        let self_closing = rest.trim_end().ends_with('/');
        let attr_str = rest.trim_end().trim_end_matches('/');
        let raw_attrs = parse_xml_attributes(attr_str);

        let rect = raw_attrs
            .get("bounds")
            .and_then(|b| parse_android_bounds(b))
            .unwrap_or(RenderRect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            });
        let class_name = raw_attrs
            .get("class")
            .map(String::as_str)
            .unwrap_or("android.view.View");
        let (tag, base_role) = normalize_android_class(class_name);
        let text = raw_attrs
            .get("text")
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                raw_attrs
                    .get("content-desc")
                    .filter(|s| !s.trim().is_empty())
            })
            .cloned()
            .unwrap_or_default();
        let resource_id = raw_attrs
            .get("resource-id")
            .filter(|s| !s.is_empty())
            .cloned();
        let clickable = raw_attrs.get("clickable").map(String::as_str) == Some("true");
        let checkable = raw_attrs.get("checkable").map(String::as_str) == Some("true");
        let meaningful =
            !text.is_empty() || resource_id.is_some() || clickable || checkable || tag != "view";

        let emitted_id = if meaningful && rect.width > 0 && rect.height > 0 {
            let key_base = resource_id
                .as_deref()
                .and_then(|r| r.rsplit('/').next())
                .filter(|s| !s.is_empty())
                .unwrap_or(tag)
                .to_owned();
            let count = id_counts.entry(key_base.clone()).or_insert(0);
            let id = format!("{key_base}#{count}");
            *count += 1;

            let parent_id = parent_stack.iter().rev().find_map(|entry| entry.clone());
            let role = base_role
                .map(str::to_owned)
                .or_else(|| clickable.then(|| "button".to_owned()));

            let mut attributes = BTreeMap::new();
            attributes.insert("class".into(), class_name.to_owned());
            if let Some(rid) = resource_id {
                attributes.insert("resourceId".into(), rid);
            }
            for (xml_key, attr_key) in [
                ("package", "package"),
                ("content-desc", "contentDesc"),
                ("clickable", "clickable"),
                ("enabled", "enabled"),
                ("checkable", "checkable"),
                ("checked", "checked"),
                ("focused", "focused"),
                ("selected", "selected"),
            ] {
                if let Some(val) = raw_attrs.get(xml_key).filter(|v| !v.is_empty()) {
                    attributes.insert(attr_key.into(), val.clone());
                }
            }

            nodes.push(RenderNodeSnapshot {
                id: id.clone(),
                tag: tag.into(),
                role,
                text,
                rect,
                style: BTreeMap::new(),
                parent_id,
                box_model: None,
                attributes,
            });
            Some(id)
        } else {
            None
        };

        if !self_closing {
            parent_stack.push(emitted_id);
        }
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::diff_render_trees;

    #[test]
    fn keeps_checked_state_for_an_unnamed_checkable_view() {
        let xml = r#"<hierarchy>
          <node class="android.view.View" checkable="true" checked="false" bounds="[8,16][48,56]" />
          <node class="android.view.View" checkable="false" checked="false" bounds="[8,64][48,104]" />
        </hierarchy>"#;
        let nodes = parse_uiautomator_xml(xml);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].attributes["checkable"], "true");
        assert_eq!(nodes[0].attributes["checked"], "false");
    }

    #[test]
    fn parses_adb_devices_and_selects_online_device() {
        let output = "\
List of devices attached
emulator-5554\tdevice
emulator-5556\toffline
R5CR123456\tunauthorized
";
        let devices = parse_adb_devices(output);
        assert_eq!(devices.len(), 3);
        assert_eq!(
            select_online_device(&devices, None).map(|d| d.serial),
            Some("emulator-5554".into())
        );
        assert_eq!(select_online_device(&devices, Some("emulator-5556")), None);
        assert_eq!(select_online_device(&[], None), None);
    }

    #[test]
    fn parses_uiautomator_xml_and_diffs_through_shared_diff_render_trees() {
        let xml_before = r#"<?xml version='1.0' encoding='UTF-8' standalone='yes' ?>
<hierarchy rotation="0">
  <node index="0" text="" resource-id="com.example.shop:id/root" class="android.widget.FrameLayout" package="com.example.shop" clickable="false" enabled="true" bounds="[0,0][1080,2400]">
    <node index="0" text="Checkout &amp; Pay" resource-id="com.example.shop:id/title" class="android.widget.TextView" package="com.example.shop" clickable="false" enabled="true" bounds="[48,120][640,200]" />
    <node index="1" text="Pay $29" resource-id="com.example.shop:id/pay_btn" class="android.widget.Button" package="com.example.shop" clickable="true" enabled="true" bounds="[48,2100][1032,2244]" />
  </node>
</hierarchy>"#;

        let xml_after = r#"<?xml version='1.0' encoding='UTF-8' standalone='yes' ?>
<hierarchy rotation="0">
  <node index="0" text="" resource-id="com.example.shop:id/root" class="android.widget.FrameLayout" package="com.example.shop" clickable="false" enabled="true" bounds="[0,0][1080,2400]">
    <node index="0" text="Checkout &amp; Pay" resource-id="com.example.shop:id/title" class="android.widget.TextView" package="com.example.shop" clickable="false" enabled="true" bounds="[48,120][640,200]" />
    <node index="1" text="Pay $39" resource-id="com.example.shop:id/pay_btn" class="android.widget.Button" package="com.example.shop" clickable="true" enabled="true" bounds="[48,2100][1032,2244]" />
  </node>
</hierarchy>"#;

        let before = parse_uiautomator_xml(xml_before);
        let after = parse_uiautomator_xml(xml_after);
        assert_eq!(before.len(), 3);
        assert_eq!(before[1].id, "title#0");
        assert_eq!(before[1].text, "Checkout & Pay");
        assert_eq!(before[1].parent_id.as_deref(), Some("root#0"));
        assert_eq!(before[2].id, "pay_btn#0");
        assert_eq!(before[2].tag, "button");
        assert_eq!(before[2].rect.height, 144);
        assert_eq!(
            before[2].attributes.get("clickable").map(String::as_str),
            Some("true")
        );

        let diff = diff_render_trees(&before, &after, true);
        assert_eq!(diff.text_changed, 1);
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(diff.changes[0].id, "pay_btn#0");
        assert_eq!(diff.changes[0].before_text.as_deref(), Some("Pay $29"));
        assert_eq!(diff.changes[0].after_text.as_deref(), Some("Pay $39"));
    }
}
