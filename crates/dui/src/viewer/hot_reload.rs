//! Source watching and incremental source-model updates for the embedded viewer. This module
//! never writes to the immutable run archive and never starts an app host, browser or emulator.
use crate::tsx_map;
use dui_core::source_model::{Node, SourceModel};
use dui_lang_typescript::{ParseCache, extract_incremental};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const LONG_POLL: Duration = Duration::from_secs(8);

#[derive(Clone)]
pub(crate) struct HotSession {
    inner: Arc<Inner>,
}

struct Inner {
    state: Mutex<HotState>,
    changed: Condvar,
}

struct HotState {
    revision: u64,
    status_revision: u64,
    watch_status: &'static str,
    reason: Option<String>,
    model: SourceModel,
    nodes: Vec<Value>,
    edges: Vec<Value>,
    discovery: Value,
    patch: Option<Value>,
    graph_patch: Option<Value>,
}

fn json_key(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or_default().to_owned()
}

fn keyed(values: &[Value], field: &str) -> BTreeMap<String, Value> {
    values
        .iter()
        .map(|value| (json_key(value, field), value.clone()))
        .collect()
}

fn diff_keyed(old: &[Value], new: &[Value], field: &str) -> Value {
    let old = keyed(old, field);
    let new = keyed(new, field);
    let mut upsert = Vec::new();
    let mut remove = Vec::new();
    for (key, value) in &new {
        if old.get(key) != Some(value) {
            upsert.push(value.clone());
        }
    }
    for key in old.keys() {
        if !new.contains_key(key) {
            remove.push(key.clone());
        }
    }
    json!({"upsert":upsert,"remove":remove})
}

fn edge_key(value: &Value) -> String {
    value["modelEdge"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            format!(
                "{}->{}",
                value["source"].as_str().unwrap_or_default(),
                value["target"].as_str().unwrap_or_default()
            )
        })
}

fn diff_graph(
    old_nodes: &[Value],
    old_edges: &[Value],
    old_discovery: &Value,
    new_nodes: &[Value],
    new_edges: &[Value],
    new_discovery: &Value,
) -> Value {
    let nodes = diff_keyed(old_nodes, new_nodes, "id");
    let old_edges: BTreeMap<String, Value> = old_edges
        .iter()
        .map(|edge| (edge_key(edge), edge.clone()))
        .collect();
    let new_edges: BTreeMap<String, Value> = new_edges
        .iter()
        .map(|edge| (edge_key(edge), edge.clone()))
        .collect();
    let mut upsert = Vec::new();
    let mut remove = Vec::new();
    for (key, value) in &new_edges {
        if old_edges.get(key) != Some(value) {
            let mut value = value.clone();
            value["key"] = json!(key);
            upsert.push(value);
        }
    }
    for key in old_edges.keys() {
        if !new_edges.contains_key(key) {
            remove.push(key.clone());
        }
    }
    let mut patch = json!({"screens":nodes,"links":{"upsert":upsert,"remove":remove}});
    if old_discovery != new_discovery {
        patch["screenDiscovery"] = new_discovery.clone();
    }
    patch
}

fn entity_values<T: serde::Serialize>(items: &[T]) -> Vec<Value> {
    items
        .iter()
        .filter_map(|item| serde_json::to_value(item).ok())
        .collect()
}

fn dependency_graph(model: &SourceModel) -> BTreeMap<String, Vec<String>> {
    model
        .files
        .iter()
        .map(|file| (file.path.clone(), file.imports.clone()))
        .collect()
}

fn reverse_closure(
    old: &BTreeMap<String, Vec<String>>,
    new: &BTreeMap<String, Vec<String>>,
    changed: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut dependents: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for graph in [old, new] {
        for (file, imports) in graph {
            for import in imports {
                dependents
                    .entry(import.clone())
                    .or_default()
                    .insert(file.clone());
            }
        }
    }
    let mut affected = BTreeSet::new();
    let mut pending: Vec<String> = changed.iter().cloned().collect();
    while let Some(file) = pending.pop() {
        if !affected.insert(file.clone()) {
            continue;
        }
        if let Some(next) = dependents.get(&file) {
            pending.extend(next.iter().cloned());
        }
    }
    affected
}

fn collect_component(model: &SourceModel, start: &str, seen: &mut BTreeSet<String>) {
    if !seen.insert(start.to_owned()) {
        return;
    }
    let Some(component) = model.component(start) else {
        return;
    };
    let mut stack = vec![&component.root];
    while let Some(node) = stack.pop() {
        if let Node::Instance(instance) = node {
            collect_component(model, &instance.component, seen);
        }
        stack.extend(node.children());
    }
}

fn affected_metadata(
    old: &SourceModel,
    new: &SourceModel,
    scan_changed: &[String],
) -> (BTreeSet<String>, Vec<String>, Vec<String>, Vec<String>) {
    let old_graph = dependency_graph(old);
    let new_graph = dependency_graph(new);
    let mut changed: BTreeSet<String> = scan_changed.iter().cloned().collect();
    let old_files: BTreeMap<_, _> = old.files.iter().map(|f| (&f.path, f)).collect();
    let new_files: BTreeMap<_, _> = new.files.iter().map(|f| (&f.path, f)).collect();
    for (path, file) in &new_files {
        if old_files.get(path) != Some(file) {
            changed.insert((*path).clone());
        }
    }
    for path in old_files.keys() {
        if !new_files.contains_key(path) {
            changed.insert((*path).clone());
        }
    }
    let style_global = changed
        .iter()
        .any(|path| path.ends_with(".css") && !path.ends_with(".module.css"));
    let mut affected_files = reverse_closure(&old_graph, &new_graph, &changed);
    if style_global {
        // Theme and ordinary stylesheet cascade is global in schema v1. Module CSS stays scoped
        // to its importers; other CSS invalidates every card rather than hiding a stale style.
        affected_files.extend(new.files.iter().map(|file| file.path.clone()));
    }
    let mut affected_components: BTreeSet<String> = new
        .components
        .iter()
        .filter(|component| affected_files.contains(&component.location.file))
        .map(|component| component.id.clone())
        .collect();
    if style_global {
        affected_components.extend(new.components.iter().map(|component| component.id.clone()));
    }
    let mut affected_screens = BTreeSet::new();
    for screen in &new.screens {
        let mut components = BTreeSet::new();
        collect_component(new, &screen.component, &mut components);
        if components.iter().any(|id| affected_components.contains(id)) {
            affected_screens.insert(screen.id.clone());
        }
    }
    (
        changed,
        affected_files.into_iter().collect(),
        affected_components.into_iter().collect(),
        affected_screens.into_iter().collect(),
    )
}

fn diff_model(
    old: &SourceModel,
    new: &SourceModel,
    base_revision: u64,
    scan_changed: &[String],
    parsed_files: &[String],
) -> (Value, bool) {
    let files = diff_keyed(
        &entity_values(&old.files),
        &entity_values(&new.files),
        "path",
    );
    let screens = diff_keyed(
        &entity_values(&old.screens),
        &entity_values(&new.screens),
        "id",
    );
    let components = diff_keyed(
        &entity_values(&old.components),
        &entity_values(&new.components),
        "id",
    );
    let item_schemas = diff_keyed(
        &entity_values(&old.item_schemas),
        &entity_values(&new.item_schemas),
        "id",
    );
    let tokens = diff_keyed(
        &entity_values(&old.tokens),
        &entity_values(&new.tokens),
        "id",
    );
    let edges = diff_keyed(&entity_values(&old.edges), &entity_values(&new.edges), "id");
    let (changed_files, affected_files, affected_components, affected_screens) =
        affected_metadata(old, new, scan_changed);
    let mut patch = json!({
        "baseRevision":base_revision,
        "files":files,
        "screens":screens,
        "components":components,
        "itemSchemas":item_schemas,
        "tokens":tokens,
        "edges":edges,
        "changedFiles":changed_files,
        "parsedFiles":parsed_files,
        "affectedFiles":affected_files,
        "affectedComponents":affected_components,
        "affectedScreens":affected_screens,
    });
    if old.diagnostics != new.diagnostics {
        patch["diagnostics"] = json!(new.diagnostics);
    }
    let changed = [
        "files",
        "screens",
        "components",
        "itemSchemas",
        "tokens",
        "edges",
    ]
    .iter()
    .any(|name| {
        !patch[name]["upsert"].as_array().is_none_or(Vec::is_empty)
            || !patch[name]["remove"].as_array().is_none_or(Vec::is_empty)
    }) || patch.get("diagnostics").is_some();
    (patch, changed)
}

fn patch_has_changes(patch: &Value) -> bool {
    patch["screens"]["upsert"]
        .as_array()
        .is_some_and(|v| !v.is_empty())
        || patch["screens"]["remove"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
        || patch["links"]["upsert"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
        || patch["links"]["remove"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
        || patch.get("screenDiscovery").is_some()
}

fn scan_sources(root: &Path) -> BTreeMap<String, (u64, u128)> {
    let mut out = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(
                name.as_ref(),
                ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".dui"
            ) {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                pending.push(path);
                continue;
            }
            if !kind.is_file()
                || !matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("ts" | "tsx" | "css")
                )
            {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let modified = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |time| time.as_nanos());
            if let Ok(relative) = path.strip_prefix(root) {
                if let Some(path) = relative.to_str() {
                    out.insert(
                        path.replace(std::path::MAIN_SEPARATOR, "/"),
                        (meta.len(), modified),
                    );
                }
            }
        }
    }
    out
}

fn scan_changed(
    before: &BTreeMap<String, (u64, u128)>,
    after: &BTreeMap<String, (u64, u128)>,
) -> Vec<String> {
    before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect()
}

impl HotSession {
    pub(crate) fn start(
        project: PathBuf,
        entry: PathBuf,
        archive_model: SourceModel,
        archive_nodes: Vec<Value>,
        archive_edges: Vec<Value>,
        archive_discovery: Value,
    ) -> Self {
        let inner = Arc::new(Inner {
            state: Mutex::new(HotState {
                revision: 0,
                status_revision: 0,
                watch_status: "starting",
                reason: None,
                model: archive_model.clone(),
                nodes: archive_nodes,
                edges: archive_edges,
                discovery: archive_discovery,
                patch: None,
                graph_patch: None,
            }),
            changed: Condvar::new(),
        });
        let thread_inner = Arc::clone(&inner);
        thread::spawn(move || {
            watch_loop(thread_inner, project, entry, archive_model);
        });
        Self { inner }
    }

    pub(crate) fn response(
        &self,
        since: Option<u64>,
        status_since: Option<u64>,
        wait: bool,
    ) -> Value {
        let Ok(mut guard) = self.inner.state.lock() else {
            return json!({"status":"error","reason":"watch-state-poisoned"});
        };
        if wait {
            let deadline = Instant::now() + LONG_POLL;
            loop {
                let unchanged_revision = since.is_some_and(|revision| guard.revision == revision);
                let unchanged_status =
                    status_since.is_none_or(|revision| guard.status_revision == revision);
                if !unchanged_revision || !unchanged_status {
                    break;
                }
                let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                    break;
                };
                let Ok((next, timeout)) = self.inner.changed.wait_timeout(guard, remaining) else {
                    return json!({"status":"error","reason":"watch-state-poisoned"});
                };
                guard = next;
                if timeout.timed_out() {
                    break;
                }
            }
        }
        let revision = guard.revision;
        let status_revision = guard.status_revision;
        let mut body = json!({
            "status":"ok",
            "revision":revision,
            "statusRevision":status_revision,
            "watch":{"status":guard.watch_status,"reason":guard.reason},
        });
        let stale_client = since.is_some_and(|since| {
            revision != since
                && guard
                    .patch
                    .as_ref()
                    .and_then(|patch| patch["baseRevision"].as_u64())
                    != Some(since)
        });
        if since.is_none() || stale_client {
            body["kind"] = json!("snapshot");
            if let Ok(model) = serde_json::to_value(&guard.model) {
                body["model"] = model;
            }
            body["graph"] = json!({
                "nodes":guard.nodes,
                "edges":guard.edges,
                "screenDiscovery":guard.discovery,
            });
        } else if revision != since.unwrap_or(revision) {
            body["kind"] = json!("patch");
        } else {
            body["kind"] = json!("unchanged");
        }
        if let Some(patch) = &guard.patch {
            body["patch"] = patch.clone();
        }
        if let Some(patch) = &guard.graph_patch {
            body["graphPatch"] = patch.clone();
        }
        body
    }
}

fn publish_state(inner: &Inner, update: impl FnOnce(&mut HotState)) {
    if let Ok(mut guard) = inner.state.lock() {
        update(&mut guard);
        inner.changed.notify_all();
    }
}

fn watch_loop(inner: Arc<Inner>, project: PathBuf, entry: PathBuf, archive_model: SourceModel) {
    let mut cache = ParseCache::default();
    let mut scan = scan_sources(&project);
    let mut previous = archive_model;
    loop {
        match extract_incremental(&project, &entry, &mut cache) {
            Ok((model, stats)) => {
                let changed_scan = Vec::new();
                if model != previous {
                    let (patch, model_changed) =
                        diff_model(&previous, &model, 0, &changed_scan, &stats.parsed);
                    let projection = tsx_map::project_model(&model);
                    let (graph_patch, nodes, edges, discovery) = {
                        let current = tsx_map::project_model(&previous);
                        (
                            diff_graph(
                                &current.nodes,
                                &current.edges,
                                &current.discovery,
                                &projection.nodes,
                                &projection.edges,
                                &projection.discovery,
                            ),
                            projection.nodes,
                            projection.edges,
                            projection.discovery,
                        )
                    };
                    publish_state(&inner, |state| {
                        state.revision += 1;
                        state.status_revision += 1;
                        state.watch_status = "ready";
                        state.reason = None;
                        state.model = model.clone();
                        state.nodes = nodes;
                        state.edges = edges;
                        state.discovery = discovery;
                        if model_changed {
                            let mut patch = patch;
                            patch["baseRevision"] = json!(state.revision - 1);
                            state.patch = Some(patch);
                        }
                        if patch_has_changes(&graph_patch) {
                            state.graph_patch = Some(graph_patch);
                        } else {
                            state.graph_patch = None;
                        }
                    });
                    previous = model;
                } else {
                    publish_state(&inner, |state| {
                        state.status_revision += 1;
                        state.watch_status = "ready";
                        state.reason = None;
                        // The archive may predate the watcher. The first extraction still publishes
                        // the current source as a patch relative to that immutable baseline.
                        if state.revision == 0 && state.model != previous {
                            state.model = previous.clone();
                        }
                    });
                }
            }
            Err(reason) => publish_state(&inner, |state| {
                state.status_revision += 1;
                state.watch_status = "stale";
                state.reason = Some(reason);
            }),
        }
        loop {
            thread::sleep(POLL_INTERVAL);
            let next = scan_sources(&project);
            if next == scan {
                continue;
            }
            let changed = scan_changed(&scan, &next);
            scan = next;
            match extract_incremental(&project, &entry, &mut cache) {
                Ok((model, stats)) => {
                    if model == previous {
                        continue;
                    }
                    let current_projection = tsx_map::project_model(&previous);
                    let projection = tsx_map::project_model(&model);
                    let graph_patch = diff_graph(
                        &current_projection.nodes,
                        &current_projection.edges,
                        &current_projection.discovery,
                        &projection.nodes,
                        &projection.edges,
                        &projection.discovery,
                    );
                    let (patch, model_changed) =
                        diff_model(&previous, &model, 0, &changed, &stats.parsed);
                    publish_state(&inner, |state| {
                        state.revision += 1;
                        state.status_revision += 1;
                        state.watch_status = "ready";
                        state.reason = None;
                        state.model = model.clone();
                        state.nodes = projection.nodes;
                        state.edges = projection.edges;
                        state.discovery = projection.discovery;
                        if model_changed {
                            let mut patch = patch;
                            patch["baseRevision"] = json!(state.revision - 1);
                            state.patch = Some(patch);
                        }
                        state.graph_patch = patch_has_changes(&graph_patch).then_some(graph_patch);
                    });
                    previous = model;
                }
                Err(reason) => publish_state(&inner, |state| {
                    state.status_revision += 1;
                    state.watch_status = "stale";
                    state.reason = Some(reason);
                }),
            }
        }
    }
}
