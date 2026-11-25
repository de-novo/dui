//! Compile only the entries explicitly declared by the project during mapping.
use super::{Error, failure, io};
use dui_core::model::{Screen, ScreenOrigin, capture::NativeSource};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path},
    process::Command,
};

#[derive(Deserialize)]
struct Entry {
    source: String,
    entry: String,
}

fn install(dir: &Path) -> Result<(), Error> {
    for (name, body) in [
        (
            "scripts/source-preview.mts",
            include_str!("../../../../scripts/source-preview.mts"),
        ),
        (
            "adapters/source-preview/swift/SwiftUI.swift",
            include_str!("../../../../adapters/source-preview/swift/SwiftUI.swift"),
        ),
        (
            "adapters/source-preview/kotlin/Layout.kt",
            include_str!("../../../../adapters/source-preview/kotlin/Layout.kt"),
        ),
        (
            "adapters/source-preview/kotlin/Material.kt",
            include_str!("../../../../adapters/source-preview/kotlin/Material.kt"),
        ),
        (
            "adapters/source-preview/kotlin/Runtime.kt",
            include_str!("../../../../adapters/source-preview/kotlin/Runtime.kt"),
        ),
    ] {
        let dest = dir.join(name);
        fs::create_dir_all(dest.parent().ok_or_else(|| io("missing parent"))?).map_err(io)?;
        fs::write(dest, body).map_err(io)?;
    }
    Ok(())
}
fn export(root: &Path, entry: &Entry, tool: &Path) -> Value {
    let canonical = match root.join(&entry.source).canonicalize() {
        Ok(file) if file.starts_with(root) && file.is_file() => file,
        Ok(_) => {
            return json!({"status":"unsupported","reason":"source-outside-project-or-not-file"});
        }
        Err(e) => return json!({"status":"missing","reason":e.to_string()}),
    };
    let bytes = match fs::read(&canonical) {
        Ok(b) => b,
        Err(e) => return json!({"status":"missing","reason":e.to_string()}),
    };
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let destination = tool.join(format!(
        "{:x}.json",
        Sha256::digest(json!([entry.source, entry.entry]).to_string())
    ));
    let result = Command::new("node")
        .arg(tool.join("scripts/source-preview.mts"))
        .arg(&canonical)
        .arg(&entry.entry)
        .arg(&destination)
        .output();
    let failed =
        |reason: String| json!({"status":"unsupported","reason":reason,"sourceSha256":digest});
    match result {
        Err(error) => failed(error.to_string()),
        Ok(output) if !output.status.success() => failed(
            String::from_utf8_lossy(&output.stderr)
                .trim()
                .chars()
                .take(1600)
                .collect(),
        ),
        Ok(_) => match crate::replay::read(&destination) {
            Ok(scene)
                if scene["entry"] == entry.entry
                    && scene["sourceSha256"] == digest
                    && fs::read(&canonical)
                        .is_ok_and(|b| format!("{:x}", Sha256::digest(b)) == digest) =>
            {
                json!({"status":"ready","scene":scene,"sourceSha256":digest})
            }
            Ok(_) => {
                json!({"status":"stale","reason":"source-changed-during-export","sourceSha256":digest})
            }
            Err(_) => failed("invalid-export".into()),
        },
    }
}

fn project_scope(project: &Path) -> Result<String, Error> {
    let provenance = crate::archive::provenance(project)?;
    Ok(format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&json!([provenance["projectId"], provenance["app"]])).map_err(io)?
        )
    ))
}

pub(super) fn nodes(project: &Path) -> Result<Vec<Value>, Error> {
    let profile = project.join("dui.json");
    if !profile.is_file() {
        return Ok(vec![]);
    }
    let manifest = crate::replay::read(&profile)?;
    let Some(config) = manifest.get("sourcePreviews") else {
        return Ok(vec![]);
    };
    let entries: Vec<Entry> = serde_json::from_value(config.clone())
        .map_err(|e| failure(2, &format!("invalid sourcePreviews: {e}")))?;
    let tool = tempfile::tempdir().map_err(io)?;
    let tool_root = tool.path().canonicalize().map_err(io)?;
    install(&tool_root)?;
    let scope = project_scope(project)?;
    let mut seen = BTreeSet::new();
    let mut nodes = Vec::new();
    for entry in entries {
        let file = Path::new(&entry.source);
        if entry.source.is_empty()
            || entry.entry.trim().is_empty()
            || file
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
            || file.components().collect::<std::path::PathBuf>().to_str() != Some(&entry.source)
        {
            return Err(failure(
                2,
                "expected a project-relative source file and component entry",
            ));
        }
        let declaration = NativeSource {
            name: entry.entry.clone(),
            source: entry.source.clone(),
            line: None,
            project_scope: Some(scope.clone()),
            dynamic: false,
            condition: None,
        };
        let screen = Screen::new(
            format!("{}#{}", entry.source, entry.entry),
            ScreenOrigin::Native {
                declaration: declaration.clone(),
            },
        )
        .map_err(io)?;
        if !seen.insert(screen.id().clone()) {
            return Err(failure(2, "duplicate source preview entry"));
        }
        let mut preview = export(project, &entry, &tool_root);
        preview["screenId"] = json!(screen.id());
        nodes.push(json!({"kind":"screen","id":screen.id(),"label":screen.path(),"nativeSource":declaration,"sourcePreview":preview}));
    }
    Ok(nodes)
}

// A historical archive remains portable. Recheck local files only for the matching project.
pub(crate) fn check_local_sources(graph: &mut Value) {
    let Some(nodes) = graph["nodes"].as_array_mut() else {
        return;
    };
    let Ok(project) = std::env::current_dir().and_then(|p| p.canonicalize()) else {
        return;
    };
    let Ok(scope) = project_scope(&project) else {
        return;
    };
    if !nodes
        .iter()
        .any(|n| n["nativeSource"]["projectScope"] == scope)
    {
        return;
    }
    let manifest = crate::replay::read(&project.join("dui.json")).ok();
    let entries = manifest
        .as_ref()
        .and_then(|m| m["sourcePreviews"].as_array());
    for node in nodes {
        let declaration = &node["nativeSource"];
        if declaration["projectScope"] != scope || node["sourcePreview"]["status"] != "ready" {
            continue;
        }
        let entry = entries.and_then(|entries| {
            entries
                .iter()
                .find(|e| e["source"] == declaration["source"] && e["entry"] == declaration["name"])
        });
        let reason = match entry.and_then(|e| e["source"].as_str()) {
            None => Some(("stale", "entry-no-longer-declared")),
            Some(file) => match project
                .join(file)
                .canonicalize()
                .ok()
                .filter(|p| p.starts_with(&project) && p.is_file())
                .and_then(|p| fs::read(p).ok())
            {
                None => Some(("missing", "source-unavailable")),
                Some(bytes)
                    if node["sourcePreview"]["sourceSha256"]
                        != format!("{:x}", Sha256::digest(&bytes)) =>
                {
                    Some(("stale", "source-digest-changed"))
                }
                Some(_) => None,
            },
        };
        if let Some((status, reason)) = reason {
            node["sourcePreview"] = json!({"status":status,"reason":reason});
        }
    }
}
