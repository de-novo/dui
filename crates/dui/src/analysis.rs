use crate::{
    Reply,
    chromium::Browser,
    failure,
    ports::{Renderer, TypeOracle},
    typescript::TypeScript,
};
use dui_core::discovery::{assertions, candidates, graph, key};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub(crate) fn store(path: &Path, data: &Value) -> Result<(), (u8, Value)> {
    let staged = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(Path::new(".")))
        .map_err(|e| failure(3, &e.to_string()))?;
    serde_json::to_writer_pretty(staged.as_file(), data).map_err(|e| failure(3, &e.to_string()))?;
    staged
        .persist(path)
        .map_err(|e| failure(3, &e.to_string()))?;
    Ok(())
}
pub fn analyze(project: &Path, spec: &Path) -> Reply {
    let mut result = TypeScript.facts(project, spec)?;
    let manifest: Value =
        serde_json::from_slice(&fs::read(spec).map_err(|e| failure(2, &e.to_string()))?)
            .map_err(|e| failure(2, &e.to_string()))?;
    if let Some(tables) = manifest.get("csvMocks") {
        let tables: BTreeMap<String, dui_core::tabular::CsvMock> =
            serde_json::from_value(tables.clone()).map_err(|e| failure(2, &e.to_string()))?;
        let root = project
            .canonicalize()
            .map_err(|e| failure(2, &e.to_string()))?;
        for (path, table) in tables {
            if !path.starts_with('/') || path.starts_with("//") {
                return Err(failure(2, "CSV mock requires a same-origin request path"));
            }
            let sample = root
                .join(&table.sample)
                .canonicalize()
                .map_err(|e| failure(2, &e.to_string()))?;
            if !sample.starts_with(&root) {
                return Err(failure(2, "CSV sample must be inside project"));
            }
            let bytes = fs::read(sample).map_err(|e| failure(2, &e.to_string()))?;
            let body = table
                .generate(&bytes)
                .map_err(|e| failure(2, &e.to_string()))?;
            result["autoMocks"]["csv"][path] = json!(body);
        }
    }
    Ok(result)
}
pub fn exercise(project: &Path, manifest: &Path, out: &Path, port: u16) -> Reply {
    let analysis = analyze(project, manifest)?;
    exercise_snapshot(analysis, manifest, out, port)
}
pub fn exercise_snapshot(analysis: Value, manifest: &Path, out: &Path, port: u16) -> Reply {
    let spec: Value =
        serde_json::from_slice(&fs::read(manifest).map_err(|_| failure(2, "missing spec"))?)
            .map_err(|_| failure(2, "invalid spec"))?;
    let widths: BTreeSet<u32> =
        serde_json::from_value(spec["widths"].clone()).map_err(|_| failure(2, "invalid widths"))?;
    if widths.is_empty() || widths.contains(&0) {
        return Err(failure(2, "positive widths required"));
    }
    let budget = spec["stateBudget"].as_u64().unwrap_or(32).clamp(1, 512) as usize;
    let temp = tempfile::tempdir().map_err(|e| failure(3, &e.to_string()))?;
    fs::create_dir_all(out).map_err(|e| failure(3, &e.to_string()))?;
    let exe = crate::chromium::executable();
    let mut browser = if exe.is_file() {
        Some(Browser::local(&exe, temp.path(), port).map_err(|e| failure(3, &e.to_string()))?)
    } else {
        None
    };
    let browser_version = browser.as_mut().and_then(|b| b.version().ok());
    let mut stored_spec = spec.clone();
    if let Some(base) = manifest.parent() {
        if let Some(packs) = stored_spec["rulePacks"].as_array_mut() {
            for entry in packs.iter_mut() {
                if let Some(rel) = entry.as_str()
                    && let Ok(abs) = base.join(rel).canonicalize()
                {
                    *entry = json!(abs);
                }
            }
        }
        if let Some(rel) = stored_spec["mockPreload"].as_str()
            && let Ok(abs) = base.join(rel).canonicalize()
        {
            stored_spec["mockPreload"] = json!(abs);
        }
    }
    store(&out.join("manifest.json"), &stored_spec)?;
    execute_with(
        browser.as_mut().map(|b| b as &mut dyn Renderer),
        &analysis,
        &spec,
        out,
        &widths,
        budget,
        browser_version,
    )
}

fn execute_with(
    mut renderer: Option<&mut dyn Renderer>,
    analysis: &Value,
    spec: &Value,
    out: &Path,
    widths: &BTreeSet<u32>,
    budget: usize,
    browser_version: Option<Value>,
) -> Reply {
    let facts = analysis["facts"]
        .as_array()
        .ok_or_else(|| failure(3, "invalid facts"))?;
    let mut cells = Vec::new();
    let mut hits = BTreeSet::new();
    let mut verified = 0;
    let mut remaining = 0;
    let mut seen_slots = BTreeSet::new();
    let mut rounds = Vec::new();
    let mut discovered_routes = Vec::new();
    let mut scenarios = std::collections::VecDeque::from([("default".to_owned(), spec.clone())]);
    for (i, overrides) in spec["scenarios"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let mut case = spec.clone();
        for (k, v) in overrides
            .as_object()
            .ok_or_else(|| failure(2, "invalid scenario"))?
        {
            case[k] = v.clone();
        }
        scenarios.push_back((format!("case-{}", i + 1), case));
    }
    let configured_paths: BTreeSet<String> = scenarios
        .iter()
        .filter_map(|(_, s)| s["path"].as_str().map(str::to_owned))
        .collect();
    while let Some((scenario, mut spec)) = scenarios.pop_front() {
        apply_generated(&mut spec, &analysis["autoMocks"]);
        let budget = spec["stateBudget"]
            .as_u64()
            .unwrap_or(budget as u64)
            .clamp(1, 512) as usize;
        let mut active = BTreeSet::new();
        let mut done = BTreeSet::new();
        let mut pending = vec![BTreeMap::new()];
        while let Some(row) = pending.first().cloned() {
            let index = done.len();
            if index >= budget {
                break;
            }
            done.insert(serde_json::to_string(&row).map_err(|e| failure(3, &e.to_string()))?);
            let checks = assertions(facts, &row, &spec);
            let state_id = state_id(&scenario, &spec, &row);
            for width in widths {
                let Some(browser) = renderer.as_deref_mut() else {
                    cells.push(json!({"scenario":scenario,"path":spec["path"],"width":width,"input":row,"state":state_id,"planIndex":index,"status":"unrenderable","reason":"no-render-env"}));
                    continue;
                };
                let staged =
                    tempfile::NamedTempFile::new_in(out).map_err(|e| failure(3, &e.to_string()))?;
                let input = json!(row);

                let mut capture_spec = spec.clone();
                capture_spec["assertions"] = checks.clone();
                if scenario != "default" || index != 0 {
                    capture_spec
                        .as_object_mut()
                        .unwrap()
                        .remove("routeSelector");
                }
                let reached = browser.capture(&capture_spec, &input, *width, staged.path());
                let cell = match reached {
                    Ok(obs) => {
                        let bytes =
                            fs::read(staged.path()).map_err(|e| failure(3, &e.to_string()))?;
                        let name = format!("{:x}-{width}.png", Sha256::digest(bytes));
                        staged
                            .persist(out.join(&name))
                            .map_err(|e| failure(3, &e.to_string()))?;
                        let slot_values = obs["slots"].as_object();
                        if let Some(slots) = slot_values {
                            active.extend(slots.keys().cloned());
                        }
                        let observed: Vec<_> = obs["hits"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .collect();
                        hits.extend(observed.iter().map(|s| s.to_string()));
                        let intent = dui_core::verification::Intent {
                            path: spec["path"].as_str().unwrap_or_default(),
                            width: *width,
                            inputs: &row,
                            assertions: checks.as_array().map(Vec::as_slice).unwrap_or_default(),
                            required_requests: spec["requiredRequests"]
                                .as_array()
                                .map(Vec::as_slice)
                                .unwrap_or_default(),
                            analyzed: analysis["diagnostics"]
                                .as_array()
                                .is_some_and(Vec::is_empty),
                            expects_probe: facts.iter().any(|f| f["kind"] == "branch"),
                        };
                        let reason = intent.failure(&obs).or_else(|| {
                            (spec.get("screenDiscovery").is_some()
                                && spec["hasScreenAssertion"] == false)
                                .then_some(dui_core::verification::Failure::MissingScreenAssertion)
                        });
                        if reason.is_none() {
                            verified += 1;
                        }
                        json!({"scenario":scenario,"path":spec["path"],"width":width,"input":input,"state":state_id,"planIndex":index,"image":name,"status":if reason.is_none(){"verified"}else{"unverified"},"checks":checks,"reason":reason,"observation":obs})
                    }
                    Err(e) => {
                        json!({"scenario":scenario,"path":spec["path"],"width":width,"input":input,"state":state_id,"planIndex":index,"status":"unverified","reason":e.to_string()})
                    }
                };
                cells.push(cell);
            }
            pending = candidates(facts, &active)
                .into_iter()
                .filter(|r| !done.contains(&json!(r).to_string()))
                .collect();
            pending.sort_by_cached_key(|candidate| {
                let checks = assertions(facts, candidate, &spec);
                let list = checks.as_array().map(Vec::as_slice).unwrap_or_default();
                let bound = candidate
                    .keys()
                    .all(|key| list.iter().any(|check| check["slot"] == *key));
                let new_target = list
                    .iter()
                    .any(|check| check["hit"].as_str().is_some_and(|hit| !hits.contains(hit)));
                (!new_target, !bound, candidate.len())
            });
        }
        if scenario == "default" && spec["routeSelector"].is_string() {
            let paths: BTreeSet<&str> = cells
                .iter()
                .filter(|cell| cell["planIndex"] == 0 && cell["observation"]["ready"] == true)
                .flat_map(|cell| {
                    cell["observation"]["links"]
                        .as_array()
                        .into_iter()
                        .flatten()
                })
                .filter_map(Value::as_str)
                .filter(|path| !configured_paths.contains(*path))
                .collect();
            for path in paths {
                let next = linked_spec(&spec, path);
                let index = discovered_routes.len();
                discovered_routes.push(path.to_owned());
                scenarios.push_back((format!("link-{index}"), next));
            }
        }
        for (offset, row) in pending.iter().enumerate() {
            let state = state_id(&scenario, &spec, row);
            for width in widths {
                cells.push(json!({"scenario":scenario,"path":spec["path"],"width":width,"input":row,"state":state,"planIndex":done.len()+offset,"status":"unverified","reason":"not-rendered"}));
            }
        }
        seen_slots.extend(active);
        rounds.push(json!({"scenario":scenario,"name":spec["name"],"stateBudget":budget,"plannedStates":done.len(),"remainingStates":pending.len()}));
        remaining += pending.len();
    }
    let graph = graph(facts, &hits);
    let expected: BTreeSet<_> = facts
        .iter()
        .filter(|f| f["kind"] == "branch")
        .flat_map(|f| [format!("{}:true", key(f)), format!("{}:false", key(f))])
        .collect();
    let unseen: Vec<_> = facts
        .iter()
        .filter(|f| f["kind"] == "slot" && !seen_slots.contains(&key(f)))
        .map(key)
        .collect();
    let unrenderable = cells
        .iter()
        .filter(|c| c["status"] == "unrenderable")
        .count();
    let attempted = cells
        .iter()
        .filter(|c| c["reason"] != "not-rendered" && c["status"] != "unrenderable")
        .count();
    let renderable = cells.len() - unrenderable;
    let mut report = json!({"browser":browser_version,"os":std::env::consts::OS,"scenarios":rounds,"discoveredRoutes":discovered_routes,"unobservedSlots":unseen,"diagnostics":analysis["diagnostics"],"sourceFiles":analysis["sourceFiles"],"versions":analysis["versions"],"compiler":analysis["compiler"],"path":spec["path"],"budget":budget,"remainingPlans":remaining,"coverage":{"verified":verified,"renderable":renderable,"attempted":attempted,"unrenderable":unrenderable,"branchOutcomes":expected.len(),"observedBranchOutcomes":expected.intersection(&hits).count()},"unobservedBranches":expected.difference(&hits).collect::<Vec<_>>(),"runtimeOnlyInstrumentedBranches":hits.difference(&expected).collect::<Vec<_>>(),"cells":cells});
    if let Some(discovery) = spec.get("screenDiscovery") {
        let mut discovered = discovery.clone();
        if let Some(paths) = report["discoveredRoutes"].as_array() {
            for path in paths.iter().filter_map(Value::as_str) {
                let mut source = json!({"kind":"anchor","from":spec["path"]});
                if let Some(selector) = spec["routeSelector"].as_str() {
                    source["selector"] = json!(selector);
                }
                discovered["sources"][path] = source;
            }
        }
        if let Some(cells) = report["cells"].as_array() {
            discovered["capturedCells"] =
                json!(cells.iter().filter(|c| c["image"].is_string()).count());
        }
        report["screenDiscovery"] = discovered;
    } else if let Some(selector) = spec["routeSelector"].as_str() {
        let entry_path = spec["path"].as_str().unwrap_or("/");
        let mut sources = serde_json::Map::new();
        sources.insert(entry_path.to_owned(), json!({"kind":"entry"}));
        if let Some(paths) = report["discoveredRoutes"].as_array() {
            for path in paths.iter().filter_map(Value::as_str) {
                sources.insert(
                    path.to_owned(),
                    json!({"kind":"anchor","from":entry_path,"selector":selector}),
                );
            }
        }
        let captured = report["cells"].as_array().map_or(0, |c| {
            c.iter().filter(|cell| cell["image"].is_string()).count()
        });
        report["screenDiscovery"] = json!({
            "mode": "links",
            "sources": sources,
            "capturedCells": captured,
        });
    }
    let graph = crate::graph_view::with_cells(graph, &report)?;
    crate::graph_view::write(out, &graph)?;
    store(&out.join("report.json"), &report)?;
    store(&out.join("graph.json"), &graph)?;
    store(&out.join("facts.json"), analysis)?;
    if verified == renderable && unrenderable == 0 {
        Ok(report)
    } else {
        Err((5, report))
    }
}

pub(crate) fn linked_spec(spec: &Value, path: &str) -> Value {
    let mut next = spec.clone();
    next["path"] = json!(path);
    if let Some(defaults) = spec.get("linkDefaults") {
        let options = spec["routeOptions"].get(path);
        for key in ["cookies", "readySelector"] {
            next[key] = options
                .and_then(|o| o.get(key))
                .unwrap_or(&defaults[key])
                .clone();
        }
        next["hasScreenAssertion"] = json!(
            defaults["hasScreenAssertion"] == true
                || options
                    .and_then(|o| o["readySelector"].as_str())
                    .is_some_and(|s| !s.is_empty())
        );
    }
    next
}
pub(crate) fn apply_generated(spec: &mut Value, mocks: &Value) {
    spec.as_object_mut()
        .map(|o| o.remove("generatedGetResponses"));
    spec.as_object_mut()
        .map(|o| o.remove("generatedCsvResponses"));
    if let Some(entries) = mocks["csv"].as_object() {
        for (path, body) in entries {
            if spec["mockResponses"].get(path).is_none() {
                spec["generatedCsvResponses"][path] = body.clone();
            }
        }
    }
    if let Some(entries) = mocks["generated"].as_object() {
        for (path, payload) in entries {
            if spec["mockResponses"].get(path).is_none() {
                spec["generatedGetResponses"][path] = payload.clone();
            }
        }
    }
}
pub(crate) fn state_id(scenario: &str, spec: &Value, row: &BTreeMap<String, Value>) -> String {
    let mut parts = json!([
        scenario,
        spec["path"],
        spec["cookies"],
        spec["mockResponses"],
        row
    ]);
    if let Some(generated) = spec.get("generatedGetResponses") {
        parts.as_array_mut().unwrap().push(generated.clone());
    }
    if let Some(csv) = spec.get("generatedCsvResponses") {
        parts.as_array_mut().unwrap().push(json!({"csv":csv}));
    }
    format!("{:x}", Sha256::digest(parts.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StaticPage;
    impl Renderer for StaticPage {
        fn capture(
            &mut self,
            _: &Value,
            _: &Value,
            width: u32,
            image: &Path,
        ) -> Result<Value, Box<dyn std::error::Error>> {
            fs::write(image, b"test renderer artifact")?;
            Ok(
                json!({"ready":true,"path":"/","width":width,"assertions":[],"responseChecks":{},"slots":{"file#1:2:slot":false},"hits":[]}),
            )
        }
    }

    #[test]
    fn captured_map_without_content_assertion_remains_unverified() {
        let dir = tempfile::tempdir().unwrap();
        let (_, report) = execute_with(
            Some(&mut StaticPage),
            &json!({"facts": [], "diagnostics": []}),
            &json!({"path": "/", "screenDiscovery": {"sources": {}}, "hasScreenAssertion": false}),
            dir.path(),
            &BTreeSet::from([375]),
            1,
            None,
        )
        .unwrap_err();
        assert_eq!(report["screenDiscovery"]["capturedCells"], 1);
        assert_eq!(report["coverage"]["verified"], 0);
        assert_eq!(report["cells"][0]["reason"], "missing-screen-assertion");
    }

    #[test]
    fn budget_preserves_unattempted_cells_in_the_graph_and_denominator() {
        let dir = tempfile::tempdir().unwrap();
        let facts = json!({"facts":[{"file":"file","start":1,"end":2,"kind":"slot","owner":"Screen","shape":{"kind":"boolean"},"after":3,"init":[1,2]}],"diagnostics":[]});
        let widths = BTreeSet::from([375, 768]);
        let (code, report) = execute_with(
            Some(&mut StaticPage),
            &facts,
            &json!({"path":"/"}),
            dir.path(),
            &widths,
            1,
            None,
        )
        .unwrap_err();
        assert_eq!(code, 5);
        assert_eq!(report["coverage"]["verified"], 2);
        assert_eq!(report["coverage"]["renderable"], 6);
        assert_eq!(
            report["cells"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["reason"] == "not-rendered")
                .count(),
            4
        );
        let graph: Value =
            serde_json::from_slice(&fs::read(dir.path().join("graph.json")).unwrap()).unwrap();
        assert_eq!(
            graph["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|n| n["kind"] == "cell")
                .count(),
            6
        );
    }

    #[test]
    fn capture_error_preserves_every_planned_cell_in_report_and_graph() {
        struct FailingWidth;
        impl Renderer for FailingWidth {
            fn capture(
                &mut self,
                spec: &Value,
                inputs: &Value,
                width: u32,
                image: &Path,
            ) -> Result<Value, Box<dyn std::error::Error>> {
                if width == 768 {
                    return Err("capture interrupted".into());
                }
                StaticPage.capture(spec, inputs, width, image)
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let facts = json!({"facts":[],"diagnostics":[]});
        let spec = json!({"path":"/","scenarios":[{"name":"two"},{"name":"three"},{"name":"four"},{"name":"five"}]});
        let widths = BTreeSet::from([375, 768, 1280]);
        let run = |renderer: &mut dyn Renderer| {
            execute_with(Some(renderer), &facts, &spec, dir.path(), &widths, 5, None)
        };
        let ids = |report: &Value| -> BTreeSet<(String, u32)> {
            report["cells"]
                .as_array()
                .unwrap()
                .iter()
                .map(|cell| {
                    (
                        cell["state"].as_str().unwrap().to_owned(),
                        cell["width"].as_u64().unwrap() as u32,
                    )
                })
                .collect()
        };
        let graph_ids = || -> BTreeSet<String> {
            let graph: Value =
                serde_json::from_reader(fs::File::open(dir.path().join("graph.json")).unwrap())
                    .unwrap();
            graph["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|node| node["kind"] == "cell")
                .map(|node| node["id"].as_str().unwrap().to_owned())
                .collect()
        };
        let expected = ids(&run(&mut StaticPage).unwrap());
        let expected_graph = graph_ids();
        assert_eq!(expected.len(), 15);
        assert_eq!(expected_graph.len(), 15);

        let (code, report) = run(&mut FailingWidth).unwrap_err();
        assert_eq!(code, 5);
        let saved: Value =
            serde_json::from_reader(fs::File::open(dir.path().join("report.json")).unwrap())
                .unwrap();
        assert_eq!(saved, report);
        assert_eq!(ids(&saved), expected);
        assert_eq!(graph_ids(), expected_graph);
        assert_eq!(saved["coverage"]["verified"], 10);
        assert_eq!(saved["coverage"]["renderable"], 15);
        for cell in saved["cells"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["width"] == 768)
        {
            assert_eq!(cell["status"], "unverified");
            assert_eq!(cell["reason"], "capture interrupted");
            assert!(cell.get("image").is_none());
        }
    }

    #[test]
    fn missing_environment_is_counted_separately() {
        let dir = tempfile::tempdir().unwrap();
        let (_, report) = execute_with(
            None,
            &json!({"facts":[],"diagnostics":[]}),
            &json!({"path":"/"}),
            dir.path(),
            &BTreeSet::from([375]),
            1,
            None,
        )
        .unwrap_err();
        assert_eq!(report["coverage"]["renderable"], 0);
        assert_eq!(report["coverage"]["unrenderable"], 1);
        assert_eq!(report["cells"][0]["reason"], "no-render-env");
    }
}
