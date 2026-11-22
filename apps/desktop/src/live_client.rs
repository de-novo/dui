//! Live session client calling the shared `dui live` CLI contract from `apps/desktop`.

use crate::store::decode_png_bytes_to_render_image;
use base64::{Engine, engine::general_purpose::STANDARD};
use gpui_kit::RenderImage;
use serde_json::{Value, json};
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveControl {
    pub instance: String,
    pub owner: String,
    pub slot: String,
    pub label: String,
    pub kind: String,
    pub value: Value,
    pub initial_value: Value,
    pub editable: bool,
    pub options: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct LiveSessionState {
    pub session_id: String,
    pub revision: u64,
    pub state_id: String,
    pub width_px: u32,
    pub controls: Vec<LiveControl>,
    pub frame: Option<Arc<RenderImage>>,
    pub saved_run_id: Option<String>,
    pub error: Option<String>,
}

pub fn resolve_dui_binary() -> PathBuf {
    if let Ok(bin) = env::var("DUI_BIN")
        && !bin.trim().is_empty()
    {
        return PathBuf::from(bin);
    }
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    if let Some(repo_root) = manifest_dir.parent().and_then(Path::parent) {
        for rel in ["target/debug/dui", "target/release/dui"] {
            let candidate = repo_root.join(rel);
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from("dui")
}

fn call_live(payload: &Value) -> Result<Value, String> {
    let bin = resolve_dui_binary();
    let output = Command::new(&bin)
        .args(["live", &payload.to_string()])
        .output()
        .map_err(|e| format!("failed to run {}: {e}", bin.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if stderr.is_empty() {
            "empty response from dui live".into()
        } else {
            stderr
        });
    }
    let parsed: Value =
        serde_json::from_str(&stdout).map_err(|e| format!("invalid dui live JSON: {e}"))?;
    if !output.status.success() || parsed["status"] == "error" {
        let reason = parsed["reason"]
            .as_str()
            .or_else(|| parsed["error"].as_str())
            .unwrap_or("live-command-failed");
        return Err(reason.to_owned());
    }
    Ok(parsed)
}

pub fn decode_controls(raw: &Value) -> Vec<LiveControl> {
    let Some(arr) = raw.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| {
            let slot = item["slot"].as_str()?.to_owned();
            let instance = item["instance"].as_str().unwrap_or("default").to_owned();
            let owner = item["owner"].as_str().unwrap_or("Component").to_owned();
            let label = item["label"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(&slot)
                .to_owned();
            let kind = item["type"].as_str().unwrap_or("unknown").to_owned();
            let editable = item["editable"].as_bool().unwrap_or(false);
            let options = item["options"].as_array().cloned().unwrap_or_default();
            Some(LiveControl {
                instance,
                owner,
                slot,
                label,
                kind,
                value: item["value"].clone(),
                initial_value: item["initialValue"].clone(),
                editable,
                options,
            })
        })
        .collect()
}

pub fn open_session(
    project: &Path,
    run_dir: &Path,
    state_id: &str,
    width_px: u32,
) -> Result<LiveSessionState, String> {
    let opened = call_live(&json!({
        "op": "open",
        "project": project.display().to_string(),
        "run": run_dir.display().to_string(),
        "cell": { "state": state_id, "width": width_px },
    }))?;
    let session_id = opened["session"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "missing session id".to_owned())?
        .to_owned();
    let revision = opened["revision"].as_u64().unwrap_or(0);

    let inspected = call_live(&json!({
        "op": "inspect",
        "session": &session_id,
        "cell": { "state": state_id, "width": width_px },
    }))
    .unwrap_or_else(|_| json!({}));
    let controls = decode_controls(&inspected["controls"]);
    let frame = fetch_frame(&session_id, state_id, width_px).ok();

    Ok(LiveSessionState {
        session_id,
        revision,
        state_id: state_id.to_owned(),
        width_px,
        controls,
        frame,
        saved_run_id: None,
        error: None,
    })
}

pub fn fetch_frame(
    session_id: &str,
    state_id: &str,
    width_px: u32,
) -> Result<Arc<RenderImage>, String> {
    let reply = call_live(&json!({
        "op": "frame",
        "session": session_id,
        "cell": { "state": state_id, "width": width_px },
    }))?;
    let b64 = reply["imageBase64"]
        .as_str()
        .ok_or_else(|| "missing imageBase64".to_owned())?;
    let bytes = STANDARD
        .decode(b64)
        .map_err(|e| format!("invalid base64 frame: {e}"))?;
    decode_png_bytes_to_render_image(&bytes).ok_or_else(|| "failed to decode PNG frame".to_owned())
}

pub fn set_control(
    session: &mut LiveSessionState,
    instance: &str,
    slot: &str,
    value: Value,
) -> Result<(), String> {
    let reply = call_live(&json!({
        "op": "set",
        "session": &session.session_id,
        "cell": { "state": &session.state_id, "width": session.width_px },
        "instance": instance,
        "slot": slot,
        "value": value,
        "expectedRevision": session.revision,
    }))?;
    if let Some(rev) = reply["revision"].as_u64() {
        session.revision = rev;
    }
    if reply["controls"].is_array() {
        session.controls = decode_controls(&reply["controls"]);
    }
    session.frame = fetch_frame(&session.session_id, &session.state_id, session.width_px).ok();
    session.error = None;
    Ok(())
}

pub fn reset_control(
    session: &mut LiveSessionState,
    instance: &str,
    slot: &str,
) -> Result<(), String> {
    let reply = call_live(&json!({
        "op": "reset",
        "session": &session.session_id,
        "cell": { "state": &session.state_id, "width": session.width_px },
        "instance": instance,
        "slot": slot,
        "expectedRevision": session.revision,
    }))?;
    if let Some(rev) = reply["revision"].as_u64() {
        session.revision = rev;
    }
    if reply["controls"].is_array() {
        session.controls = decode_controls(&reply["controls"]);
    }
    session.frame = fetch_frame(&session.session_id, &session.state_id, session.width_px).ok();
    session.error = None;
    Ok(())
}

pub fn save_session(session: &mut LiveSessionState) -> Result<String, String> {
    let reply = call_live(&json!({
        "op": "save",
        "session": &session.session_id,
        "cell": { "state": &session.state_id, "width": session.width_px },
        "expectedRevision": session.revision,
    }))?;
    let saved = reply["snapshot"]["id"]
        .as_str()
        .or_else(|| reply["output"].as_str())
        .unwrap_or("saved")
        .to_owned();
    session.saved_run_id = Some(saved.clone());
    Ok(saved)
}

pub fn close_session(session_id: &str) {
    let _ = call_live(&json!({
        "op": "close",
        "session": session_id,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_boolean_and_enum_live_controls() {
        let raw = json!([
            {
                "instance": "primary",
                "owner": "Settings",
                "slot": "enabled",
                "label": "Enable alerts",
                "type": "boolean",
                "value": false,
                "initialValue": false,
                "editable": true,
                "options": [false, true]
            },
            {
                "instance": "primary",
                "owner": "Settings",
                "slot": "density",
                "type": "\"compact\" | \"expanded\"",
                "value": "compact",
                "initialValue": "compact",
                "editable": true,
                "options": ["compact", "expanded"]
            }
        ]);
        let controls = decode_controls(&raw);
        assert_eq!(controls.len(), 2);
        assert_eq!(controls[0].label, "Enable alerts");
        assert!(controls[0].editable);
        assert_eq!(controls[1].options.len(), 2);
    }
}
