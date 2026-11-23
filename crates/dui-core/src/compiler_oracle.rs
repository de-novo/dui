//! Pure compiler-deep state extraction domain model and toolchain version policy
//! for TUI (Ratatui, Ink, Bubble Tea) and Mobile (React Native/Expo, SwiftUI,
//! Jetpack Compose, Flutter) frameworks.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolchainKind {
    TypeScript,
    Rustc,
    Go,
    Swiftc,
    Kotlinc,
    Dart,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrameworkPackKind {
    InkTui,
    RatatuiTui,
    BubbleTeaTui,
    ReactNative,
    SwiftUi,
    JetpackCompose,
    Flutter,
}

impl FrameworkPackKind {
    pub const fn toolchain(self) -> ToolchainKind {
        match self {
            Self::InkTui | Self::ReactNative => ToolchainKind::TypeScript,
            Self::RatatuiTui => ToolchainKind::Rustc,
            Self::BubbleTeaTui => ToolchainKind::Go,
            Self::SwiftUi => ToolchainKind::Swiftc,
            Self::JetpackCompose => ToolchainKind::Kotlinc,
            Self::Flutter => ToolchainKind::Dart,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedStateSlot {
    pub name: String,
    pub owner: String,
    pub file: String,
    pub type_name: String,
    pub domain: Vec<String>,
    pub framework: FrameworkPackKind,
    pub toolchain: ToolchainKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolchainSupport {
    Supported,
    Unsupported(String),
}

fn parse_major_minor(raw: &str) -> Option<(u32, u32)> {
    for token in raw.split(|c: char| !c.is_ascii_digit() && c != '.') {
        let mut parts = token.split('.');
        if let (Some(maj_s), Some(min_s)) = (parts.next(), parts.next())
            && let (Ok(major), Ok(minor)) = (maj_s.parse::<u32>(), min_s.parse::<u32>())
        {
            return Some((major, minor));
        }
    }
    None
}

/// Enforce official latest-stable-major toolchain policy across all supported compiler backends.
pub fn check_toolchain_version(kind: ToolchainKind, version_output: &str) -> ToolchainSupport {
    let Some((major, minor)) = parse_major_minor(version_output) else {
        return ToolchainSupport::Unsupported(format!(
            "unable to parse official toolchain version from `{}`",
            version_output.trim()
        ));
    };
    match kind {
        ToolchainKind::TypeScript => {
            if major == 6 || major == 7 {
                ToolchainSupport::Supported
            } else {
                ToolchainSupport::Unsupported(format!(
                    "TypeScript requires stable 6.x or 7.x; found {major}.{minor}"
                ))
            }
        }
        ToolchainKind::Rustc => {
            if major >= 1 && minor >= 85 {
                ToolchainSupport::Supported
            } else {
                ToolchainSupport::Unsupported(format!(
                    "Rust edition 2024 requires rustc >= 1.85; found {major}.{minor}"
                ))
            }
        }
        ToolchainKind::Go => {
            if major > 1 || (major == 1 && minor >= 24) {
                ToolchainSupport::Supported
            } else {
                ToolchainSupport::Unsupported(format!(
                    "Go requires stable 1.24+; found {major}.{minor}"
                ))
            }
        }
        ToolchainKind::Swiftc => {
            if major >= 6 {
                ToolchainSupport::Supported
            } else {
                ToolchainSupport::Unsupported(format!(
                    "Swift requires stable 6.x+; found {major}.{minor}"
                ))
            }
        }
        ToolchainKind::Kotlinc => {
            if major >= 2 {
                ToolchainSupport::Supported
            } else {
                ToolchainSupport::Unsupported(format!(
                    "Kotlin requires stable 2.x+; found {major}.{minor}"
                ))
            }
        }
        ToolchainKind::Dart => {
            if major >= 3 {
                ToolchainSupport::Supported
            } else {
                ToolchainSupport::Unsupported(format!(
                    "Dart requires stable 3.x+; found {major}.{minor}"
                ))
            }
        }
    }
}

/// Normalize structured compiler-emitted JSON slots (`{ name, owner, file, type, domain }`) into
/// deterministic, deduplicated `ExtractedStateSlot`s.
pub fn parse_compiler_slots(raw: &Value, framework: FrameworkPackKind) -> Vec<ExtractedStateSlot> {
    let toolchain = framework.toolchain();
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let Some(arr) = raw.as_array() else {
        return out;
    };
    for item in arr {
        let Some(name) = item["name"].as_str().filter(|s| !s.trim().is_empty()) else {
            continue;
        };
        let owner = item["owner"].as_str().unwrap_or("Screen").to_owned();
        let file = item["file"].as_str().unwrap_or("").to_owned();
        let type_name = item["type"].as_str().unwrap_or("unknown").to_owned();
        let domain: Vec<String> = item["domain"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Bool(b) => Some(b.to_string()),
                Value::Number(n) => Some(n.to_string()),
                Value::Null | Value::Array(_) | Value::Object(_) => None,
            })
            .collect();
        if domain.is_empty() {
            continue;
        }
        let dedup_key = format!("{file}::{owner}::{name}");
        if !seen.insert(dedup_key) {
            continue;
        }
        out.push(ExtractedStateSlot {
            name: name.to_owned(),
            owner,
            file,
            type_name,
            domain,
            framework,
            toolchain,
        });
    }
    out.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.owner.cmp(&b.owner))
            .then(a.name.cmp(&b.name))
    });
    out
}

/// Build a deterministic covering state plan (baseline + single-slot flips) from compiler-extracted slots.
pub fn plan_states_from_slots(slots: &[ExtractedStateSlot]) -> Vec<BTreeMap<String, String>> {
    if slots.is_empty() {
        return vec![BTreeMap::new()];
    }
    let mut baseline = BTreeMap::new();
    for slot in slots {
        if let Some(first) = slot.domain.first() {
            baseline.insert(format!("{}.{}", slot.owner, slot.name), first.clone());
        }
    }
    let mut rows = vec![baseline.clone()];
    for slot in slots {
        let key = format!("{}.{}", slot.owner, slot.name);
        for val in slot.domain.iter().skip(1) {
            let mut variant = baseline.clone();
            variant.insert(key.clone(), val.clone());
            rows.push(variant);
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn enforces_latest_stable_major_policy_across_all_official_toolchains() {
        assert_eq!(
            check_toolchain_version(ToolchainKind::TypeScript, "Version 7.0.2"),
            ToolchainSupport::Supported
        );
        assert_eq!(
            check_toolchain_version(ToolchainKind::Rustc, "rustc 1.100.0 (0ef1bc386)"),
            ToolchainSupport::Supported
        );
        assert_eq!(
            check_toolchain_version(ToolchainKind::Go, "go version go1.25.0 darwin/arm64"),
            ToolchainSupport::Supported
        );
        assert_eq!(
            check_toolchain_version(ToolchainKind::Swiftc, "Apple Swift version 6.1.0"),
            ToolchainSupport::Supported
        );
        assert_eq!(
            check_toolchain_version(ToolchainKind::Kotlinc, "kotlinc-jvm 2.1.20"),
            ToolchainSupport::Supported
        );
        assert_eq!(
            check_toolchain_version(ToolchainKind::Dart, "Dart SDK version: 3.7.2"),
            ToolchainSupport::Supported
        );

        assert!(matches!(
            check_toolchain_version(ToolchainKind::Swiftc, "Apple Swift version 5.9.2"),
            ToolchainSupport::Unsupported(_)
        ));
        assert!(matches!(
            check_toolchain_version(ToolchainKind::Kotlinc, "kotlinc-jvm 1.9.24"),
            ToolchainSupport::Unsupported(_)
        ));
        assert!(matches!(
            check_toolchain_version(ToolchainKind::Dart, "Dart SDK version: 2.19.6"),
            ToolchainSupport::Unsupported(_)
        ));
    }

    #[test]
    fn parses_compiler_slots_and_plans_deterministic_states_for_tui_and_mobile() {
        let raw = json!([
            {
                "name": "tab",
                "owner": "AppModel",
                "file": "src/app.rs",
                "type": "ActiveTab",
                "domain": ["Dashboard", "Processes", "Logs"]
            },
            {
                "name": "showHelp",
                "owner": "AppModel",
                "file": "src/app.rs",
                "type": "bool",
                "domain": [false, true]
            }
        ]);
        let slots = parse_compiler_slots(&raw, FrameworkPackKind::RatatuiTui);
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].toolchain, ToolchainKind::Rustc);

        let plan = plan_states_from_slots(&slots);
        // Baseline + 1 flip for showHelp + 2 flips for tab = 4 deterministic rows
        assert_eq!(plan.len(), 4);
        assert_eq!(
            plan[0].get("AppModel.tab").map(String::as_str),
            Some("Dashboard")
        );
        assert_eq!(
            plan[1].get("AppModel.showHelp").map(String::as_str),
            Some("true")
        );
    }
}
