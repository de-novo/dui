//! Publish the source model and its declared screen relations without starting a host.
use crate::{Reply, analysis, archive, failure, replay::io};
use dui_core::source_model::EdgeTarget;
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

pub(crate) fn publish(project: &Path, entry: &Path) -> Reply {
    let model = dui_lang_typescript::extract(project, entry).map_err(|e| failure(2, &e))?;
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
    let graph = json!({"nodes":nodes,"edges":edges,"cells":[],"overview":[],"screenDiscovery":discovery,
         "sourceModel":"source-model.json"});
    let report = json!({"screenDiscovery":discovery,"sourceModel":"source-model.json","browser":null,
        "sourceFiles":model.files.iter().map(|f| &f.path).collect::<Vec<_>>(),"cells":[],
        "coverage":{"attempted":0,"renderable":0,"unrenderable":0,"verified":0}});
    archive::publish(archive::provenance(project)?, |out| {
        fs::create_dir_all(out).map_err(io)?;
        fs::write(out.join("source-model.json"), model.to_json().map_err(io)?).map_err(io)?;
        analysis::store(&out.join("graph.json"), &graph)?;
        analysis::store(&out.join("report.json"), &report)?;
        Ok(report)
    })
}
