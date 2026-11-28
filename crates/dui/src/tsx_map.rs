//! Publish the source model and its declared screen relations without starting a host.
use crate::{Reply, analysis, archive, failure, replay::io};
use dui_core::source_model::{EdgeTarget, SourceModel};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// Select an unambiguous conventional component, or the only top-level TSX module.
pub(crate) fn entry(project: &Path) -> Result<Option<PathBuf>, (u8, Value)> {
    let mut candidates = Vec::new();
    for directory in [project.join("src"), project.to_owned()] {
        if !directory.is_dir() {
            continue;
        }
        for file in fs::read_dir(directory).map_err(io)? {
            let path = file.map_err(io)?.path();
            if path.extension().is_some_and(|ext| ext == "tsx") && path.is_file() {
                candidates.push(path);
            }
        }
    }
    let app: Vec<_> = candidates
        .iter()
        .filter(|p| p.file_name().is_some_and(|name| name == "App.tsx"))
        .collect();
    match (app.as_slice(), candidates.as_slice()) {
        ([path], _) => Ok(Some((*path).clone())),
        (_, [path]) => Ok(Some(path.clone())),
        (_, []) => Ok(None),
        _ => Err(failure(
            2,
            "multiple TSX entry candidates; select one with --entry",
        )),
    }
}

pub(crate) fn project_relative_entry(project: &Path, entry: &Path) -> Option<String> {
    let root = project.canonicalize().ok()?;
    let full = if entry.is_absolute() {
        entry.to_owned()
    } else {
        root.join(entry)
    };
    let full = full.canonicalize().ok()?;
    full.strip_prefix(root)
        .ok()?
        .to_str()
        .map(|path| path.replace(std::path::MAIN_SEPARATOR, "/"))
}

pub(crate) struct SourceProjection {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
    pub discovery: Value,
}

/// Project a source model into the viewer's graph nodes, links and discovery metadata.
pub(crate) fn project_model(model: &SourceModel) -> SourceProjection {
    let nodes: Vec<_> = model
        .screens
        .iter()
        .map(|s| {
            json!({
                "id":s.id, "kind":"screen", "label":s.route.as_ref().unwrap_or(&s.name), "name":s.name,
                "component":s.component, "location":s.location
            })
        })
        .collect();
    let by_id: BTreeMap<_, _> = model.edges.iter().map(|e| (&e.id, e)).collect();
    let mut edges = Vec::new();
    let mut unresolved = Vec::new();
    for relation in model.screen_edges() {
        let edge = by_id[&relation.edge];
        match &edge.to {
            EdgeTarget::Screen { screen } => edges.push(json!({
                "source":relation.from, "target":screen, "kind":"linksTo", "modelEdge":edge.id,
                "navigationKind":edge.kind, "origin":edge.origin, "location":edge.location
            })),
            EdgeTarget::Route { .. } | EdgeTarget::Opaque { .. } => unresolved.push(relation),
        }
    }
    let mut limitations = vec![
        "Only navigation declared in supported TSX syntax is mapped. Runtime-only navigation is unknown.",
        "React Router object routes support statically resolvable paths, nested children, element components and literal lazy imports; wildcard routes are not named screens.",
        "Imperative navigation is mapped only when a useNavigate call is reachable from an onClick handler. Forwarded callbacks, effects and runtime-only navigation are not resolved.",
        "Nested routes that resolve to the same path use the deepest leaf screen; parent layout composition is not represented.",
    ];
    if nodes.is_empty() {
        limitations.push(
            "No supported route declaration or unambiguous root component was found at the entry.",
        );
    }
    let discovery = json!({"kind":"tsx-source","declared":nodes.len(),"resolvedRelations":edges.len(),
        "unresolvedRelations":unresolved,"limitations":limitations,"diagnostics":model.diagnostics});
    SourceProjection {
        nodes,
        edges,
        discovery,
    }
}

fn font_magic_matches(extension: &str, bytes: &[u8]) -> bool {
    match extension {
        "woff2" => bytes.starts_with(b"wOF2"),
        "woff" => bytes.starts_with(b"wOFF"),
        "ttf" => bytes.starts_with(&[0, 1, 0, 0]) || bytes.starts_with(b"true"),
        "otf" => bytes.starts_with(b"OTTO"),
        _ => false,
    }
}

/// Copy only the bounded local font assets named by the model. The archive write re-checks
/// path, type, byte length and digest instead of trusting the model JSON in memory.
fn copy_font_assets(project: &Path, out: &Path, model: &SourceModel) -> Result<(), String> {
    use dui_core::source_model::FontSource;
    use sha2::{Digest, Sha256};
    let project = project.canonicalize().map_err(|e| e.to_string())?;
    let mut total = 0u64;
    let mut copied = BTreeMap::new();
    for face in model
        .document_style
        .as_ref()
        .map(|document| document.font_faces.as_slice())
        .unwrap_or_default()
    {
        for source in &face.sources {
            let FontSource::Asset {
                path,
                asset,
                sha256,
                bytes,
                ..
            } = source
            else {
                continue;
            };
            if copied.insert(asset.clone(), ()).is_some() {
                continue;
            }
            if path.is_empty()
                || path.starts_with('/')
                || path.contains('\\')
                || path
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
                || *bytes == 0
                || *bytes > 4 * 1024 * 1024
            {
                return Err(format!("unsafe font asset path in source model: {path}"));
            }
            let full = project
                .join(path)
                .canonicalize()
                .map_err(|e| format!("{path}: {e}"))?;
            if !full.starts_with(&project) || !full.is_file() {
                return Err(format!("font asset escapes the project: {path}"));
            }
            let data = fs::read(&full).map_err(|e| format!("{path}: {e}"))?;
            let extension = path
                .rsplit_once('.')
                .map(|(_, ext)| ext)
                .unwrap_or_default();
            if data.len() as u64 != *bytes
                || format!("{:x}", Sha256::digest(&data)) != *sha256
                || format!("asset-{sha256}.{extension}") != *asset
                || !font_magic_matches(extension, &data)
            {
                return Err(format!(
                    "font asset does not match its source model metadata: {path}"
                ));
            }
            total += bytes;
            if total > 12 * 1024 * 1024 {
                return Err("font assets exceed the bounded archive size".into());
            }
            fs::write(out.join(asset), &data).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub(crate) fn publish(project: &Path, entry: &Path) -> Reply {
    let model = dui_lang_typescript::extract(project, entry).map_err(|e| failure(2, &e))?;
    let projection = project_model(&model);
    let source_entry = project_relative_entry(project, entry);
    let graph = json!({"nodes":projection.nodes,"edges":projection.edges,"cells":[],"overview":[],"screenDiscovery":projection.discovery,
         "sourceModel":"source-model.json","sourceEntry":source_entry});
    let report = json!({"screenDiscovery":projection.discovery,"sourceModel":"source-model.json","sourceEntry":source_entry,"browser":null,
        "sourceFiles":model.files.iter().map(|f| &f.path).collect::<Vec<_>>(),"cells":[],
        "coverage":crate::coverage::base(&[])});
    archive::publish(archive::provenance(project)?, |out| {
        fs::create_dir_all(out).map_err(io)?;
        fs::write(out.join("source-model.json"), model.to_json().map_err(io)?).map_err(io)?;
        copy_font_assets(project, out, &model).map_err(|e| failure(2, &e))?;
        analysis::store(&out.join("graph.json"), &graph)?;
        analysis::store(&out.join("report.json"), &report)?;
        Ok(report)
    })
}
