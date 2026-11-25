//! Only the Vite project in which the local viewer was started may be mounted.
use super::{ViewerState, list_archives, read_optional_json};
use crate::{Reply, failure, live};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub(super) fn available(state: &ViewerState, project: Option<&Path>, root: &Path) -> bool {
    let Some(project) = project.and_then(|p| p.canonicalize().ok()) else {
        return false;
    };
    if Some(&project) != state.live_project.as_ref() {
        return false;
    }
    let manifest = read_optional_json(&root.join("manifest.json")).unwrap_or_default();
    let app = project.join(manifest["appRoot"].as_str().unwrap_or("."));
    matches!(manifest["host"].as_str(), None | Some("vite"))
        && app.join("node_modules/vite/package.json").is_file()
}

pub(super) fn command(state: &Arc<Mutex<ViewerState>>, body: &Value) -> Reply {
    let lock = || state.lock().map_err(|_| failure(3, "viewer-unavailable"));
    let op = body["op"].as_str().unwrap_or_default();
    if op == "open" {
        let (project, root) = {
            let guard = lock()?;
            let archive = list_archives(&guard)
                .into_iter()
                .find(|a| a.run["key"] == body["run"] && a.run["responsiveAvailable"] == true)
                .ok_or_else(|| failure(2, "project-unavailable"))?;
            (
                guard
                    .live_project
                    .clone()
                    .ok_or_else(|| failure(2, "project-unavailable"))?,
                archive.root,
            )
        };
        let mut request = json!({"op":"open","project":project,"run":root,"cell":body["cell"],"webLayoutOnly":true,"requestId":body["requestId"]});
        // Map archives used route discovery. Explicit runs use the trusted local manifest.
        let manifest = project.join("dui.json");
        let archived = read_optional_json(&root.join("manifest.json")).unwrap_or_default();
        if manifest.is_file() && archived.get("screenDiscovery").is_none() {
            request["manifest"] = json!(manifest);
        }
        let opened = live::command(&request)?;
        if let Some(id) = opened["session"].as_str() {
            lock()?.responsive_sessions.insert(id.into());
        }
        return Ok(opened);
    }
    let id = body["session"]
        .as_str()
        .ok_or_else(|| failure(2, "missing-session"))?;
    if !lock()?.responsive_sessions.contains(id)
        || !matches!(
            op,
            "layout" | "resize" | "sample" | "fill" | "frame" | "close"
        )
    {
        return Err(failure(2, "session-unavailable"));
    }
    let result = live::command(body);
    if op == "close" {
        lock()?.responsive_sessions.remove(id);
    }
    result
}
