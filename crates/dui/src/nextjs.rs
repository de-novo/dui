use crate::{
    Reply, analysis,
    chromium::Browser,
    failure,
    ports::{Renderer, TypeOracle},
    surface::{NoopOracle, SurfaceAdapter},
    universal_web::{UniversalSession, collect_static_source_files, start_universal_session},
};
use dui_core::{
    nextjs::{NextAppRoute, build_next_route},
    source_model as sm,
    surface::{
        SURFACE_NEXTJS_WEB, SupportTier, SurfaceCapabilities, SurfaceDescriptor, SurfaceKind,
        UnrenderableReason,
    },
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn identifier_at(text: &str, start: usize) -> Option<String> {
    let rest = text.get(start..)?.trim_start();
    let len = rest
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$'))
        .count();
    (len > 0).then(|| rest[..len].to_owned())
}

/// Read the narrow default-export forms used by App Router page and layout modules. Other
/// export shapes stay unsupported rather than being guessed.
fn default_export_name(source: &str) -> Option<String> {
    for marker in ["export default async function", "export default function"] {
        if let Some(at) = source.find(marker) {
            return identifier_at(source, at + marker.len());
        }
    }
    let at = source.find("export default")?;
    identifier_at(source, at + "export default".len()).filter(|name| {
        name.chars().next().is_some_and(char::is_uppercase) && !source[at..].contains("=>")
    })
}

fn layout_chain(project: &Path, page_file: &str) -> Vec<String> {
    let (base, rest) = if let Some(rest) = page_file.strip_prefix("src/app/") {
        ("src/app", rest)
    } else if let Some(rest) = page_file.strip_prefix("app/") {
        ("app", rest)
    } else {
        return Vec::new();
    };
    let mut dirs = vec![base.to_owned()];
    let mut current = base.to_owned();
    let segments: Vec<&str> = rest.split('/').collect();
    for segment in &segments[..segments.len().saturating_sub(1)] {
        current.push('/');
        current.push_str(segment);
        dirs.push(current.clone());
    }
    dirs.into_iter()
        .map(|dir| format!("{dir}/layout.tsx"))
        .filter(|file| project.join(file).is_file())
        .collect()
}

fn route_entries(
    project: &Path,
    routes: &[NextAppRoute],
) -> (Vec<PathBuf>, BTreeMap<String, Vec<String>>) {
    let mut entries = BTreeSet::new();
    let mut chains = BTreeMap::new();
    for route in routes {
        if !route.file.ends_with(".tsx") {
            continue;
        }
        let layouts = layout_chain(project, &route.file);
        for file in layouts.iter().chain(std::iter::once(&route.file)) {
            entries.insert(file.clone());
        }
        chains.insert(route.file.clone(), layouts);
    }
    // Put root layouts first so document style and theme evidence are extracted before page-only modules.
    let mut entries: Vec<String> = entries.into_iter().collect();
    entries.sort_by(|a, b| {
        let depth = |path: &String| path.matches('/').count();
        depth(a).cmp(&depth(b)).then(a.cmp(b))
    });
    (entries.into_iter().map(PathBuf::from).collect(), chains)
}

fn default_component_id(model: &sm::SourceModel, project: &Path, file: &str) -> Option<String> {
    let source = fs::read_to_string(project.join(file)).ok()?;
    if let Some(name) = default_export_name(&source) {
        let id = format!("{file}#{name}");
        if model.component(&id).is_some() {
            return Some(id);
        }
    }
    let components: Vec<_> = model
        .components
        .iter()
        .filter(|component| component.location.file == file)
        .collect();
    if let [component] = components.as_slice() {
        return Some(component.id.clone());
    }
    None
}

fn compose_instance(
    chain: &[String],
    index: usize,
    component_id: &str,
    suffix: String,
    location: &sm::Location,
) -> sm::Node {
    let id = format!("{component_id}@{suffix}");
    let children = if index + 1 < chain.len() {
        vec![compose_instance(
            chain,
            index + 1,
            component_id,
            format!("{suffix}.0"),
            location,
        )]
    } else {
        vec![]
    };
    sm::Node::Instance(sm::Instance {
        id,
        component: chain[index].clone(),
        props: BTreeMap::new(),
        children,
        location: location.clone(),
    })
}

/// Attach Next.js App Router page modules and their nested layout chain as explicit route screens.
/// Unsupported default-export shapes simply produce no composed route rather than invented UI.
pub(crate) fn attach_route_model(
    project: &Path,
    mut model: sm::SourceModel,
    routes: &[NextAppRoute],
) -> Result<sm::SourceModel, String> {
    model.screens.clear();
    let (_, chains) = route_entries(project, routes);
    for route in routes {
        let Some(page) = default_component_id(&model, project, &route.file) else {
            model.diagnostics.push(sm::Diagnostic {
                code: "unsupported-next-page-export".into(),
                message: route.file.clone(),
                location: None,
            });
            continue;
        };
        let mut chain = Vec::new();
        for layout in chains.get(&route.file).into_iter().flatten() {
            if let Some(component) = default_component_id(&model, project, layout) {
                chain.push(component);
            }
        }
        chain.push(page.clone());
        let (page_name, page_location) = model
            .component(&page)
            .map(|component| (component.name.clone(), component.location.clone()))
            .ok_or_else(|| format!("missing extracted page component {page}"))?;
        let component_id = format!("next-app-router:{}#route", route.path);
        let root = compose_instance(&chain, 0, &component_id, "0".into(), &page_location);
        model.components.push(sm::Component {
            id: component_id.clone(),
            name: format!("{} route", route.path),
            location: page_location.clone(),
            slots: vec![],
            root,
        });
        model.screens.push(sm::Screen {
            id: format!("route:{}", route.path),
            name: page_name,
            route: Some(route.path.clone()),
            component: component_id,
            origin: sm::Origin::Syntax,
            location: page_location,
        });
    }
    for edge in &mut model.edges {
        if let sm::EdgeTarget::Route { route } = &edge.to
            && model
                .screens
                .iter()
                .any(|screen| screen.route.as_ref() == Some(route))
        {
            edge.to = sm::EdgeTarget::Screen {
                screen: format!("route:{route}"),
            };
        }
    }
    model.components.sort_by(|a, b| a.id.cmp(&b.id));
    model.screens.sort_by(|a, b| a.id.cmp(&b.id));
    sm::validate(&model).map_err(|errs| format!("invalid Next.js route source model: {errs:?}"))?;
    Ok(model)
}

pub(crate) fn extracted_route_model(
    project: &Path,
) -> Result<Option<(sm::SourceModel, Vec<PathBuf>)>, String> {
    let routes = discover_app_routes(project);
    if routes.is_empty() {
        return Ok(None);
    }
    let (entries, _) = route_entries(project, &routes);
    if entries.is_empty() {
        return Ok(None);
    }
    let extracted = dui_lang_typescript::extract_entries(project, &entries)?;
    attach_route_model(project, extracted, &routes).map(|model| Some((model, entries)))
}

fn merge_model_nodes(graph_nodes: &mut Vec<Value>, projected: Vec<Value>) {
    for projected_node in projected {
        let id = projected_node["id"].as_str().unwrap_or_default().to_owned();
        if let Some(existing) = graph_nodes
            .iter_mut()
            .find(|node| node["id"].as_str() == Some(id.as_str()))
        {
            let discovered = existing.get("discoveredFrom").cloned();
            *existing = projected_node;
            if let Some(discovered) = discovered {
                existing["discoveredFrom"] = discovered;
            }
        } else {
            graph_nodes.push(projected_node);
        }
    }
}

fn merge_model_edges(graph_edges: &mut Vec<Value>, projected: Vec<Value>) {
    graph_edges.retain(|edge| edge.get("modelEdge").is_none());
    for edge in projected {
        let duplicate = graph_edges.iter().any(|existing| {
            existing["source"] == edge["source"]
                && existing["target"] == edge["target"]
                && existing["kind"] == edge["kind"]
                && existing.get("modelEdge").is_some() == edge.get("modelEdge").is_some()
        });
        if !duplicate {
            graph_edges.push(edge);
        }
    }
}

pub(crate) fn merge_projected_graph(
    nodes: &[Value],
    edges: &[Value],
    model: &sm::SourceModel,
) -> (Vec<Value>, Vec<Value>) {
    let projection = crate::tsx_map::project_model(model);
    let mut nodes = nodes.to_vec();
    let mut edges = edges.to_vec();
    merge_model_nodes(&mut nodes, projection.nodes);
    merge_model_edges(&mut edges, projection.edges);
    (nodes, edges)
}

fn integrate_archive_model(
    project: &Path,
    out: &Path,
    model: &sm::SourceModel,
    entries: &[PathBuf],
) -> Result<(), String> {
    fs::write(
        out.join("source-model.json"),
        model.to_json().map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let inputs =
        crate::source_inputs::configured(project, model).map_err(|(_, error)| error.to_string())?;
    if let Some(inputs) = inputs {
        crate::analysis::store(&out.join(crate::source_inputs::ARCHIVE_FILE), &inputs)
            .map_err(|(_, error)| error.to_string())?;
    }
    crate::tsx_map::copy_font_assets(project, out, model)?;
    let mut graph: Value = serde_json::from_slice(
        &fs::read(out.join("graph.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let mut report: Value = serde_json::from_slice(
        &fs::read(out.join("report.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let nodes = graph["nodes"].as_array().cloned().unwrap_or_default();
    let edges = graph["edges"].as_array().cloned().unwrap_or_default();
    let (nodes, edges) = merge_projected_graph(&nodes, &edges, model);
    graph["nodes"] = json!(nodes);
    graph["edges"] = json!(edges);
    let entry_values: Vec<Value> = entries
        .iter()
        .filter_map(|entry| entry.to_str().map(|entry| json!(entry)))
        .collect();
    graph["sourceModel"] = json!("source-model.json");
    graph["sourceEntries"] = json!(entry_values);
    if let Some(first) = entry_values.first() {
        graph["sourceEntry"] = first.clone();
    }
    if out.join(crate::source_inputs::ARCHIVE_FILE).is_file() {
        graph["sourceInputs"] = json!(crate::source_inputs::ARCHIVE_FILE);
    }
    graph["screenDiscovery"]["sourceModel"] = json!({
        "kind":"next-app-router",
        "declared":model.screens.len(),
        "diagnostics":model.diagnostics,
    });
    report["sourceModel"] = graph["sourceModel"].clone();
    report["sourceEntries"] = graph["sourceEntries"].clone();
    if let Some(first) = graph.get("sourceEntry") {
        report["sourceEntry"] = first.clone();
    }
    if graph.get("sourceInputs").is_some() {
        report["sourceInputs"] = json!(crate::source_inputs::ARCHIVE_FILE);
    }
    crate::analysis::store(&out.join("graph.json"), &graph)
        .map_err(|(_, error)| error.to_string())?;
    crate::analysis::store(&out.join("report.json"), &report)
        .map_err(|(_, error)| error.to_string())?;
    Ok(())
}

fn next_binary(project: &Path) -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("DUI_NEXT_BIN") {
        let path = PathBuf::from(explicit);
        return path.is_file().then_some(path);
    }
    let local_bin = project.join("node_modules/.bin/next");
    if local_bin.is_file() {
        return Some(local_bin);
    }
    Command::new("next")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .filter(|s| s.success())
        .map(|_| PathBuf::from("next"))
}

pub(crate) fn discover_app_routes(project: &Path) -> Vec<NextAppRoute> {
    let Ok(project_root) = project.canonicalize() else {
        return Vec::new();
    };
    let mut routes = Vec::new();
    for candidate in ["app", "src/app"] {
        let dir = project_root.join(candidate);
        if !dir.is_dir() {
            continue;
        }
        let mut stack = vec![dir];
        while let Some(cur) = stack.pop() {
            let Ok(entries) = fs::read_dir(&cur) else {
                continue;
            };
            let mut children: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            children.sort();
            for path in children {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') || name == "node_modules" {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                } else if path.is_file()
                    && let Ok(rel) = path.strip_prefix(&project_root)
                {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if let Ok(source) = fs::read_to_string(&path)
                        && let Some(route) = build_next_route(&rel_str, &source)
                    {
                        routes.push(route);
                    }
                }
            }
        }
    }
    routes.sort_by(|a, b| a.path.cmp(&b.path).then(a.file.cmp(&b.file)));
    routes
}

fn next_host_spec(project: &Path, spec: &Value) -> Result<Value, UnrenderableReason> {
    if let Some(url) = spec["url"].as_str() {
        return Ok(json!({ "host": "url", "url": url }));
    }
    if let Some(cmd) = spec.get("command") {
        let mut mapped = spec.clone();
        mapped["host"] = json!("command");
        mapped["command"] = cmd.clone();
        return Ok(mapped);
    }
    if let Some(static_root) = spec["staticRoot"].as_str() {
        return Ok(json!({
            "host": "static",
            "root": static_root,
            "spaFallback": spec["spaFallback"].as_bool().unwrap_or(false),
        }));
    }
    let Some(next_bin) = next_binary(project) else {
        return Err(UnrenderableReason::new(
            "no-next-binary",
            "next executable not found (install Next.js 16 or pass `url`/`command`/`staticRoot`)",
        ));
    };
    let mut mapped = spec.clone();
    mapped["host"] = json!("command");
    mapped["command"] = json!([
        next_bin.to_string_lossy().into_owned(),
        "dev",
        "--hostname",
        "127.0.0.1"
    ]);
    Ok(mapped)
}

struct NextJsOracle;

impl TypeOracle for NextJsOracle {
    fn facts(&self, project: &Path, spec: &Path) -> Reply {
        let mut facts = NoopOracle.facts(project, spec)?;
        let routes = discover_app_routes(project);
        let server_components: Vec<&NextAppRoute> =
            routes.iter().filter(|r| !r.is_client_component).collect();
        facts["host"] = json!("next");
        facts["nextRoutes"] = json!(routes);
        facts["serverComponentBoundaries"] = json!(server_components);
        facts["sourceFiles"] = json!(collect_static_source_files(project, "."));
        Ok(facts)
    }
}

struct LazyNextBrowser<'a> {
    project: &'a Path,
    host_spec: Value,
    exe: PathBuf,
    profile: Option<tempfile::TempDir>,
    session: Option<UniversalSession>,
    browser: Option<Browser>,
}

impl Renderer for LazyNextBrowser<'_> {
    fn capture(
        &mut self,
        spec: &Value,
        input: &Value,
        width: u32,
        image: &Path,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        if self.browser.is_none() {
            let session = start_universal_session(self.project, &self.host_spec)
                .map_err(|(_, v)| v.to_string())?;
            let profile = tempfile::tempdir()?;
            let browser = Browser::local(&self.exe, profile.path(), session.port())?;
            self.session = Some(session);
            self.profile = Some(profile);
            self.browser = Some(browser);
        }
        let browser = self.browser.as_mut().ok_or("browser unavailable")?;
        browser.capture(spec, input, width, image)
    }
}

pub(crate) struct NextJsWebSurface;

impl SurfaceAdapter for NextJsWebSurface {
    fn descriptor(&self) -> SurfaceDescriptor {
        SurfaceDescriptor {
            id: SURFACE_NEXTJS_WEB.into(),
            kind: SurfaceKind::Web,
            tier: SupportTier::Universal,
            host_kinds: vec!["next".into(), "nextjs".into()],
            capabilities: SurfaceCapabilities {
                state_injection: false,
                live_edit: false,
                link_discovery: true,
                render_tree: true,
                box_model: true,
                pixel_capture: true,
            },
        }
    }

    fn oracle(&self) -> &dyn TypeOracle {
        &NextJsOracle
    }

    fn check_env(&self) -> Result<(), UnrenderableReason> {
        let exe = crate::chromium::executable();
        if exe.is_file() {
            Ok(())
        } else {
            Err(UnrenderableReason::new(
                "no-browser",
                crate::chromium::missing_detail(),
            ))
        }
    }

    fn run(&self, project: &Path, spec: &Path, out: &Path, force: bool) -> Reply {
        let manifest_spec: Value =
            serde_json::from_slice(&fs::read(spec).map_err(|e| failure(2, &e.to_string()))?)
                .map_err(|e| failure(2, &e.to_string()))?;
        let plan_only = manifest_spec["capture"] == false;
        let host_spec = if plan_only {
            Value::Null
        } else {
            match next_host_spec(project, &manifest_spec) {
                Ok(h) => h,
                Err(reason) => {
                    let reply = crate::surface::unrenderable_reply(SURFACE_NEXTJS_WEB, &reason);
                    let _ = fs::create_dir_all(out);
                    let _ = analysis::store(&out.join("report.json"), &reply);
                    return Err((5, reply));
                }
            }
        };
        let facts = analysis::analyze_with_oracle(self.oracle(), project, spec)?;

        // Auto-expand discovered static App Router pages into scenarios if none were hardcoded.
        let mut effective_spec = manifest_spec.clone();
        let discovered_routes = discover_app_routes(project);
        let static_routes: Vec<String> = discovered_routes
            .iter()
            .filter(|r| !r.is_dynamic)
            .map(|r| r.path.clone())
            .collect();
        if effective_spec["path"].as_str().is_none() {
            let entry = static_routes
                .first()
                .cloned()
                .unwrap_or_else(|| "/".to_owned());
            effective_spec["path"] = json!(entry);
        }
        if effective_spec.get("scenarios").is_none() && static_routes.len() > 1 {
            let primary = effective_spec["path"].as_str().unwrap_or("/").to_owned();
            let extra: Vec<Value> = static_routes
                .into_iter()
                .filter(|p| p != &primary)
                .map(|p| json!({ "path": p }))
                .collect();
            if !extra.is_empty() {
                effective_spec["scenarios"] = json!(extra);
            }
        }
        let temp_spec_dir = tempfile::tempdir().map_err(|e| failure(3, &e.to_string()))?;
        let effective_manifest = temp_spec_dir.path().join("dui.json");
        analysis::store(&effective_manifest, &effective_spec)?;

        let exe = crate::chromium::executable();
        let browser_version = if plan_only {
            None
        } else {
            crate::chromium::version_tag(&exe)
        };
        let mut lazy = LazyNextBrowser {
            project,
            host_spec,
            exe: exe.clone(),
            profile: None,
            session: None,
            browser: None,
        };
        let renderer = (!plan_only && exe.is_file()).then_some(&mut lazy as &mut dyn Renderer);
        let mut report = analysis::exercise_with_renderer(
            renderer,
            facts,
            &effective_manifest,
            Some(project),
            out,
            browser_version,
            force,
        )?;
        if let Some(obj) = report.as_object_mut() {
            obj.insert("nextAppRoutes".into(), json!(discovered_routes));
            let server_boundaries: Vec<&NextAppRoute> = discovered_routes
                .iter()
                .filter(|r| !r.is_client_component)
                .collect();
            obj.insert("serverComponentBoundaries".into(), json!(server_boundaries));
        }
        match extracted_route_model(project) {
            Ok(Some((model, entries))) => {
                if let Err(error) = integrate_archive_model(project, out, &model, &entries) {
                    report["sourceModelError"] = json!(error);
                    analysis::store(&out.join("report.json"), &report)?;
                }
            }
            Ok(None) => {}
            Err(error) => {
                report["sourceModelError"] = json!(error);
            }
        }
        analysis::store(&out.join("report.json"), &report)?;
        Ok(report)
    }
}
