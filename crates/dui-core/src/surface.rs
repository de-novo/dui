use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SurfaceKind {
    Web,
    Desktop,
    Mobile,
    Tui,
}

impl fmt::Display for SurfaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Web => f.write_str("web"),
            Self::Desktop => f.write_str("desktop"),
            Self::Mobile => f.write_str("mobile"),
            Self::Tui => f.write_str("tui"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SupportTier {
    Universal,
    CompilerDeep,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceCapabilities {
    pub state_injection: bool,
    pub live_edit: bool,
    pub link_discovery: bool,
    pub render_tree: bool,
    pub box_model: bool,
    pub pixel_capture: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceDescriptor {
    pub id: String,
    pub kind: SurfaceKind,
    pub tier: SupportTier,
    pub host_kinds: Vec<String>,
    pub capabilities: SurfaceCapabilities,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnrenderableReason {
    pub code: String,
    pub detail: String,
}

impl UnrenderableReason {
    pub fn new(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            detail: detail.into(),
        }
    }
}

pub const SURFACE_VITE_WEB: &str = "vite-web";
pub const SURFACE_UNIVERSAL_WEB: &str = "universal-web";
pub const SURFACE_NEXTJS_WEB: &str = "nextjs-web";
pub const SURFACE_GPUI_DESKTOP: &str = "gpui-desktop";
pub const SURFACE_ELECTRON_DESKTOP: &str = "electron-desktop";
pub const SURFACE_UNIVERSAL_TUI: &str = "universal-tui";
pub const SURFACE_ANDROID_MOBILE: &str = "android-mobile";
pub const SURFACE_IOS_MOBILE: &str = "ios-mobile";

pub fn resolve_surface_id(manifest: &Value) -> &'static str {
    if let Some(surface) = manifest.get("surface").and_then(Value::as_str) {
        match surface {
            "gpui" | "gpui-desktop" => return SURFACE_GPUI_DESKTOP,
            "electron" | "electron-desktop" => return SURFACE_ELECTRON_DESKTOP,
            "tui" | "universal-tui" => return SURFACE_UNIVERSAL_TUI,
            "android" | "android-mobile" => return SURFACE_ANDROID_MOBILE,
            "ios" | "ios-mobile" => return SURFACE_IOS_MOBILE,
            "universal-web" => return SURFACE_UNIVERSAL_WEB,
            "next" | "nextjs" | "nextjs-web" => return SURFACE_NEXTJS_WEB,
            "vite" | "vite-web" => return SURFACE_VITE_WEB,
            _ => {}
        }
    }
    if manifest.get("gpui").is_some()
        || matches!(
            manifest.get("runtime").and_then(Value::as_str),
            Some("gpui" | "gpui-kit")
        )
    {
        return SURFACE_GPUI_DESKTOP;
    }
    if let Some(host) = manifest.get("host").and_then(Value::as_str) {
        match host {
            "static" | "url" | "command" => return SURFACE_UNIVERSAL_WEB,
            "next" | "nextjs" => return SURFACE_NEXTJS_WEB,
            "pty" => return SURFACE_UNIVERSAL_TUI,
            "android" => return SURFACE_ANDROID_MOBILE,
            "ios" => return SURFACE_IOS_MOBILE,
            "electron" => return SURFACE_ELECTRON_DESKTOP,
            _ => {}
        }
    }
    SURFACE_VITE_WEB
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_manifests_deterministically_across_all_four_surfaces() {
        assert_eq!(
            resolve_surface_id(&json!({"widths": [375]})),
            SURFACE_VITE_WEB
        );
        assert_eq!(
            resolve_surface_id(&json!({"host": "static", "widths": [375]})),
            SURFACE_UNIVERSAL_WEB
        );
        assert_eq!(
            resolve_surface_id(&json!({"host": "url", "url": "http://127.0.0.1:3000"})),
            SURFACE_UNIVERSAL_WEB
        );
        assert_eq!(
            resolve_surface_id(&json!({"host": "command", "command": "npm run dev"})),
            SURFACE_UNIVERSAL_WEB
        );
        assert_eq!(
            resolve_surface_id(&json!({"gpui": {"fixture": "card"}})),
            SURFACE_GPUI_DESKTOP
        );
        assert_eq!(
            resolve_surface_id(&json!({"surface": "gpui"})),
            SURFACE_GPUI_DESKTOP
        );
        assert_eq!(
            resolve_surface_id(&json!({"runtime": "gpui-kit"})),
            SURFACE_GPUI_DESKTOP
        );
        assert_eq!(
            resolve_surface_id(&json!({"runtime": "gpui"})),
            SURFACE_GPUI_DESKTOP
        );
        assert_eq!(
            resolve_surface_id(&json!({"host": "electron"})),
            SURFACE_ELECTRON_DESKTOP
        );
        assert_eq!(
            resolve_surface_id(&json!({"host": "pty"})),
            SURFACE_UNIVERSAL_TUI
        );
        assert_eq!(
            resolve_surface_id(&json!({"surface": "tui"})),
            SURFACE_UNIVERSAL_TUI
        );
        assert_eq!(
            resolve_surface_id(&json!({"host": "android"})),
            SURFACE_ANDROID_MOBILE
        );
        assert_eq!(
            resolve_surface_id(&json!({"surface": "ios"})),
            SURFACE_IOS_MOBILE
        );
    }

    #[test]
    fn surface_kind_display_matches_kebab_case_serialization() {
        for kind in [
            SurfaceKind::Web,
            SurfaceKind::Desktop,
            SurfaceKind::Mobile,
            SurfaceKind::Tui,
        ] {
            let serialized = serde_json::to_string(&kind).expect("serialize surface kind");
            assert_eq!(serialized, format!("\"{kind}\""));
        }
    }
}
