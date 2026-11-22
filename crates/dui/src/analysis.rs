use crate::{
    Reply,
    cache::{CacheLookup, CellCache},
    chromium::Browser,
    failure,
    ports::{Renderer, TypeOracle},
    typescript::TypeScript,
};
use dui_core::{
    discovery::{FillTermination, assertions, candidates, graph, key},
    locale::{flatten_message_keys, is_rtl_locale, missing_translation_keys, summarize_by_locale},
    model::{CacheOutcome, CellStatus},
};
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
    let exe = crate::chromium::executable();
    let temp = tempfile::tempdir().map_err(|e| failure(3, &e.to_string()))?;
    let mut browser = if exe.is_file() {
        Some(Browser::local(&exe, temp.path(), port).map_err(|e| failure(3, &e.to_string()))?)
    } else {
        None
    };
    let browser_version = browser.as_mut().and_then(|b| b.version().ok());
    exercise_with_renderer(
        browser.as_mut().map(|b| b as &mut dyn Renderer),
        analysis,
        manifest,
        manifest.parent(),
        out,
        browser_version,
        false,
    )
}
pub(crate) fn exercise_with_renderer(
    renderer: Option<&mut dyn Renderer>,
    analysis: Value,
    manifest: &Path,
    project_dir: Option<&Path>,
    out: &Path,
    browser_version: Option<Value>,
    force: bool,
) -> Reply {
    let spec: Value =
        serde_json::from_slice(&fs::read(manifest).map_err(|_| failure(2, "missing spec"))?)
            .map_err(|_| failure(2, "invalid spec"))?;
    let widths: BTreeSet<u32> =
        serde_json::from_value(spec["widths"].clone()).map_err(|_| failure(2, "invalid widths"))?;
    if widths.is_empty() || widths.contains(&0) {
        return Err(failure(2, "positive widths required"));
    }
    fs::create_dir_all(out).map_err(|e| failure(3, &e.to_string()))?;
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
    let (locales, missing_by_locale) = resolve_locales(&spec, manifest.parent());
    let force = force || spec["force"] == true;
    let dui_home = crate::archive::home_dir().ok();
    let mut cache = CellCache::open(
        dui_home.as_deref(),
        project_dir.or_else(|| manifest.parent()),
        &analysis,
        &spec,
        browser_version.as_ref(),
        force,
    );
    execute_with_locales(
        renderer,
        &analysis,
        &spec,
        out,
        &widths,
        browser_version,
        &locales,
        &missing_by_locale,
        Some(&mut cache),
    )
}

fn resolve_locales(
    spec: &Value,
    base_dir: Option<&Path>,
) -> (Vec<String>, BTreeMap<String, Vec<String>>) {
    let mut configured: Vec<String> = spec["locales"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();

    let mut catalogs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    if let Some(dir) = base_dir {
        let candidate_dirs: Vec<std::path::PathBuf> = if let Some(rel) = spec["localesDir"].as_str()
        {
            vec![dir.join(rel)]
        } else {
            vec![dir.join("locales"), dir.join("messages")]
        };
        for messages_dir in candidate_dirs {
            if !messages_dir.is_dir() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(&messages_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("json") {
                        continue;
                    }
                    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                        continue;
                    };
                    if !configured.is_empty() && !configured.iter().any(|l| l == stem) {
                        continue;
                    }
                    if let Ok(bytes) = fs::read(&path)
                        && let Ok(parsed) = serde_json::from_slice::<Value>(&bytes)
                    {
                        catalogs.insert(stem.to_owned(), flatten_message_keys(&parsed));
                    }
                }
            }
            if !catalogs.is_empty() {
                break;
            }
        }
    }

    if configured.is_empty() {
        configured = catalogs.keys().cloned().collect();
    } else {
        for loc in &configured {
            catalogs.entry(loc.clone()).or_default();
        }
    }

    let missing = if catalogs.len() > 1 {
        missing_translation_keys(&catalogs)
    } else {
        configured.iter().map(|l| (l.clone(), Vec::new())).collect()
    };
    (configured, missing)
}

#[cfg(test)]
fn execute_with(
    renderer: Option<&mut dyn Renderer>,
    analysis: &Value,
    spec: &Value,
    out: &Path,
    widths: &BTreeSet<u32>,
    browser_version: Option<Value>,
) -> Reply {
    execute_with_locales(
        renderer,
        analysis,
        spec,
        out,
        widths,
        browser_version,
        &[],
        &BTreeMap::new(),
        None,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "keeps capture execution inputs explicit without introducing hidden mutable state"
)]
fn execute_with_locales(
    mut renderer: Option<&mut dyn Renderer>,
    analysis: &Value,
    spec: &Value,
    out: &Path,
    widths: &BTreeSet<u32>,
    browser_version: Option<Value>,
    locales: &[String],
    missing_by_locale: &BTreeMap<String, Vec<String>>,
    mut cache: Option<&mut CellCache>,
) -> Reply {
    let facts = analysis["facts"]
        .as_array()
        .ok_or_else(|| failure(3, "invalid facts"))?;
    let mut cells = Vec::new();
    let mut hits = BTreeSet::new();
    let mut verified = 0;
    let mut remaining = 0;
    let mut cache_hits = 0usize;
    let mut cache_misses = 0usize;
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
    let mut configured_paths: BTreeSet<String> = scenarios
        .iter()
        .filter_map(|(_, s)| s["path"].as_str().map(str::to_owned))
        .collect();
    let mut visited_patterns: BTreeSet<String> = configured_paths
        .iter()
        .map(|p| dui_core::discovery::route_pattern(p))
        .collect();
    let mut discovered_from: BTreeMap<String, String> = BTreeMap::new();
    if spec.get("screenDiscovery").is_none()
        && let Some(base_path) = spec["path"].as_str().filter(|p| !p.is_empty())
    {
        for (qi, q) in dui_core::discovery::query_candidates(facts)
            .into_iter()
            .enumerate()
        {
            let q_path = dui_core::discovery::with_query(base_path, &q);
            if configured_paths.insert(q_path.clone()) {
                let mut case = spec.clone();
                case["path"] = json!(q_path);
                case["queryParams"] = json!(q);
                scenarios.push_back((format!("query-{}", qi + 1), case));
            }
        }
    }
    while let Some((scenario, mut spec)) = scenarios.pop_front() {
        apply_generated(&mut spec, &analysis["autoMocks"]);
        let max_stagnant = spec["maxNoImprovementRounds"].as_u64().map(|n| n as usize);
        let mut active = BTreeSet::new();
        let mut done = BTreeSet::new();
        let mut pending = vec![BTreeMap::new()];
        let mut stagnant_rounds = 0usize;
        let mut termination = FillTermination::Exhausted;
        while let Some(row) = pending.first().cloned() {
            let index = done.len();
            let prev_hits = hits.len();
            let prev_active = active.len();
            done.insert(serde_json::to_string(&row).map_err(|e| failure(3, &e.to_string()))?);
            let checks = assertions(facts, &row, &spec);
            let locale_slots: Vec<Option<&str>> = if locales.is_empty() {
                vec![None]
            } else {
                locales.iter().map(|l| Some(l.as_str())).collect()
            };
            for locale in &locale_slots {
                let mut locale_spec = spec.clone();
                if let Some(loc) = locale {
                    locale_spec["locale"] = json!(loc);
                }
                let state_id = state_id(&scenario, &locale_spec, &row);
                for width in widths {
                    let Some(browser) = renderer.as_deref_mut() else {
                        let mut cell = json!({"scenario":scenario,"path":spec["path"],"width":width,"input":row,"state":state_id,"planIndex":index,"status":"unrenderable","reason":"no-render-env"});
                        if let Some(loc) = locale {
                            cell["locale"] = json!(loc);
                        }
                        cells.push(cell);
                        continue;
                    };
                    let staged = tempfile::NamedTempFile::new_in(out)
                        .map_err(|e| failure(3, &e.to_string()))?;
                    let input = json!(row);

                    let mut capture_spec = locale_spec.clone();
                    capture_spec["assertions"] = checks.clone();
                    if scenario != "default" || index != 0 {
                        capture_spec
                            .as_object_mut()
                            .unwrap()
                            .remove("routeSelector");
                    }
                    let cache_inputs = cache.as_deref().and_then(|c| {
                        c.build_inputs(&scenario, &capture_spec, &row, *width, *locale)
                    });
                    let cached_lookup = match (cache.as_deref_mut(), cache_inputs.as_ref()) {
                        (Some(c), Some(inputs)) => Some(c.lookup(inputs, out)),
                        _ => None,
                    };

                    if let Some(CacheLookup::Hit {
                        key: cache_key,
                        image_name,
                        status: cached_status,
                        observation: obs,
                    }) = cached_lookup
                    {
                        cache_hits += 1;
                        if let Some(slots) = obs["slots"].as_object() {
                            active.extend(slots.keys().cloned());
                        }
                        let observed: Vec<_> = obs["hits"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .collect();
                        hits.extend(observed.iter().map(|s| s.to_string()));
                        if matches!(cached_status, CellStatus::Verified) {
                            verified += 1;
                        }
                        let mut cell = json!({
                            "scenario": scenario,
                            "path": spec["path"],
                            "width": width,
                            "input": input,
                            "state": state_id,
                            "planIndex": index,
                            "image": image_name,
                            "status": cached_status.kind(),
                            "checks": checks,
                            "reason": cached_status.reason(),
                            "observation": obs,
                            "cache": CacheOutcome::Hit { key: cache_key },
                        });
                        if let Some(expected_p) = spec["path"].as_str()
                            && let Some(obs_p) = obs["path"].as_str()
                        {
                            let url_match =
                                dui_core::verification::analyze_url_match(expected_p, obs_p);
                            if !url_match.observed_query.is_empty() {
                                cell["observedQuery"] = json!(url_match.observed_query);
                            }
                            if !url_match.normalized_query.is_empty() {
                                cell["normalizedQuery"] = json!(url_match.normalized_query);
                            }
                        }
                        if let Some(inputs) = cache_inputs.as_ref() {
                            cell["sourceFiles"] =
                                json!(inputs.source_files.keys().cloned().collect::<Vec<_>>());
                        }
                        if let Some(loc) = locale {
                            cell["localeAudit"] =
                                build_locale_audit(loc, &cell["observation"], missing_by_locale);
                            cell["locale"] = json!(loc);
                        }
                        cells.push(cell);
                        continue;
                    }

                    let miss_outcome = match cached_lookup {
                        Some(CacheLookup::Miss { outcome }) => {
                            cache_misses += 1;
                            Some(outcome)
                        }
                        _ => None,
                    };

                    let reached = browser.capture(&capture_spec, &input, *width, staged.path());
                    let mut cell = match reached {
                        Ok(obs) => {
                            let bytes =
                                fs::read(staged.path()).map_err(|e| failure(3, &e.to_string()))?;
                            let name = format!("{:x}-{width}.png", Sha256::digest(&bytes));
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
                                assertions: checks
                                    .as_array()
                                    .map(Vec::as_slice)
                                    .unwrap_or_default(),
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
                                    .then_some(
                                        dui_core::verification::Failure::MissingScreenAssertion,
                                    )
                            });
                            let cell_status = match &reason {
                                None => {
                                    verified += 1;
                                    CellStatus::verified()
                                }
                                Some(f) => {
                                    let r = serde_json::to_value(f)
                                        .ok()
                                        .and_then(|v| v.as_str().map(str::to_owned))
                                        .unwrap_or_else(|| "assert-failed".to_owned());
                                    CellStatus::unverified(r)
                                        .unwrap_or_else(|_| CellStatus::verified())
                                }
                            };
                            let target_png = out.join(&name);
                            if let Some(c) = cache.as_deref_mut() {
                                c.store_rendered(
                                    cache_inputs.as_ref(),
                                    &bytes,
                                    &target_png,
                                    &cell_status,
                                    &obs,
                                )
                                .map_err(|e| failure(3, &e))?;
                            } else {
                                staged
                                    .persist(&target_png)
                                    .map_err(|e| failure(3, &e.to_string()))?;
                            }
                            let mut entry = json!({"scenario":scenario,"path":spec["path"],"width":width,"input":input,"state":state_id,"planIndex":index,"image":name,"status":if reason.is_none(){"verified"}else{"unverified"},"checks":checks,"reason":reason,"observation":obs});
                            if let Some(expected_p) = spec["path"].as_str()
                                && let Some(obs_p) = entry["observation"]["path"].as_str()
                            {
                                let url_match =
                                    dui_core::verification::analyze_url_match(expected_p, obs_p);
                                if !url_match.observed_query.is_empty() {
                                    entry["observedQuery"] = json!(url_match.observed_query);
                                }
                                if !url_match.normalized_query.is_empty() {
                                    entry["normalizedQuery"] = json!(url_match.normalized_query);
                                }
                            }
                            if let Some(loc) = locale {
                                entry["localeAudit"] = build_locale_audit(
                                    loc,
                                    &entry["observation"],
                                    missing_by_locale,
                                );
                            }
                            if let Some(outcome) = miss_outcome {
                                entry["cache"] = json!(outcome);
                            }
                            entry
                        }
                        Err(e) => {
                            let mut entry = json!({"scenario":scenario,"path":spec["path"],"width":width,"input":input,"state":state_id,"planIndex":index,"status":"unverified","reason":e.to_string()});
                            if let Some(outcome) = miss_outcome {
                                entry["cache"] = json!(outcome);
                            }
                            entry
                        }
                    };
                    if let Some(inputs) = cache_inputs.as_ref() {
                        cell["sourceFiles"] =
                            json!(inputs.source_files.keys().cloned().collect::<Vec<_>>());
                    }
                    if let Some(loc) = locale {
                        cell["locale"] = json!(loc);
                    }
                    cells.push(cell);
                }
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
                (
                    !new_target,
                    !bound,
                    candidate.len(),
                    json!(candidate).to_string(),
                )
            });
            if index > 0 {
                if hits.len() > prev_hits || active.len() > prev_active {
                    stagnant_rounds = 0;
                } else {
                    stagnant_rounds += 1;
                    if let Some(limit) = max_stagnant
                        && stagnant_rounds >= limit
                        && !pending.is_empty()
                    {
                        termination = FillTermination::NoImprovement;
                        break;
                    }
                }
            }
        }
        if spec["routeSelector"].is_string() {
            let from_path = spec["path"].as_str().unwrap_or("/").to_owned();
            let paths: BTreeSet<String> = cells
                .iter()
                .filter(|cell| {
                    cell["scenario"] == scenario
                        && cell["planIndex"] == 0
                        && cell["observation"]["ready"] == true
                })
                .flat_map(|cell| {
                    cell["observation"]["links"]
                        .as_array()
                        .into_iter()
                        .flatten()
                })
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            for path in paths {
                let pattern = dui_core::discovery::route_pattern(&path);
                if configured_paths.insert(path.clone()) && visited_patterns.insert(pattern) {
                    let next = linked_spec(&spec, &path);
                    let index = discovered_routes.len();
                    discovered_from.insert(path.clone(), from_path.clone());
                    discovered_routes.push(path);
                    scenarios.push_back((format!("link-{index}"), next));
                }
            }
        }
        for (offset, row) in pending.iter().enumerate() {
            let locale_slots: Vec<Option<&str>> = if locales.is_empty() {
                vec![None]
            } else {
                locales.iter().map(|l| Some(l.as_str())).collect()
            };
            for locale in &locale_slots {
                let mut locale_spec = spec.clone();
                if let Some(loc) = locale {
                    locale_spec["locale"] = json!(loc);
                }
                let state = state_id(&scenario, &locale_spec, row);
                for width in widths {
                    let mut cell = json!({"scenario":scenario,"path":spec["path"],"width":width,"input":row,"state":state,"planIndex":done.len()+offset,"status":"unverified","reason":"not-rendered"});
                    if let Some(loc) = locale {
                        cell["locale"] = json!(loc);
                    }
                    cells.push(cell);
                }
            }
        }
        seen_slots.extend(active);
        rounds.push(json!({"scenario":scenario,"name":spec["name"],"plannedStates":done.len(),"remainingStates":pending.len(),"termination":termination}));
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
        .collect::<BTreeSet<_>>()
        .into_iter()
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
    let mut report = json!({"browser":browser_version,"os":std::env::consts::OS,"scenarios":rounds,"discoveredRoutes":discovered_routes,"unobservedSlots":unseen,"diagnostics":analysis["diagnostics"],"sourceFiles":analysis["sourceFiles"],"versions":analysis["versions"],"compiler":analysis["compiler"],"path":spec["path"],"remainingPlans":remaining,"coverage":{"verified":verified,"renderable":renderable,"attempted":attempted,"unrenderable":unrenderable,"branchOutcomes":expected.len(),"observedBranchOutcomes":expected.intersection(&hits).count()},"unobservedBranches":expected.difference(&hits).collect::<Vec<_>>(),"runtimeOnlyInstrumentedBranches":hits.difference(&expected).collect::<Vec<_>>(),"cells":cells});
    if !locales.is_empty() {
        report["locales"] = summarize_by_locale(&cells, missing_by_locale);
    }
    if let Some(c) = cache.as_deref() {
        report["cache"] = json!({
            "hits": cache_hits,
            "misses": cache_misses,
            "rendered": cache_misses,
            "forced": c.is_forced(),
        });
        let hashes = c.source_file_hashes();
        if !hashes.is_empty() {
            report["sourceFileHashes"] = json!(hashes);
        }
    }
    if let Some(discovery) = spec.get("screenDiscovery") {
        let mut discovered = discovery.clone();
        if let Some(paths) = report["discoveredRoutes"].as_array() {
            for path in paths.iter().filter_map(Value::as_str) {
                let origin_from = discovered_from
                    .get(path)
                    .map(|p| json!(p))
                    .unwrap_or_else(|| spec["path"].clone());
                let mut source = json!({"kind":"anchor","from":origin_from});
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
    if let Some(locale) = spec.get("locale").filter(|v| v.is_string()) {
        parts.as_array_mut().unwrap().push(locale.clone());
    }
    format!("{:x}", Sha256::digest(parts.to_string()))
}

fn build_locale_audit(
    locale: &str,
    obs: &Value,
    missing_by_locale: &BTreeMap<String, Vec<String>>,
) -> Value {
    let raw = &obs["localeAudit"];
    let mut missing: BTreeSet<String> = missing_by_locale
        .get(locale)
        .into_iter()
        .flatten()
        .cloned()
        .collect();
    for key in raw["runtimeMissingKeys"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        missing.insert(key.to_owned());
    }
    let overflows = raw["overflows"].as_array().cloned().unwrap_or_default();
    let rtl_expected = is_rtl_locale(locale);
    let document_dir = raw["documentDir"].as_str().unwrap_or("ltr");
    let rtl_mismatch = if rtl_expected && document_dir != "rtl" {
        json!({"expected": "rtl", "observed": document_dir})
    } else {
        Value::Null
    };
    json!({
        "locale": locale,
        "missingKeys": missing.into_iter().collect::<Vec<_>>(),
        "overflows": overflows,
        "rtlExpected": rtl_expected,
        "documentDir": document_dir,
        "rtlMismatch": rtl_mismatch,
    })
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
            None,
        )
        .unwrap_err();
        assert_eq!(report["screenDiscovery"]["capturedCells"], 1);
        assert_eq!(report["coverage"]["verified"], 0);
        assert_eq!(report["cells"][0]["reason"], "missing-screen-assertion");
    }

    #[test]
    fn stagnation_preserves_unattempted_cells_in_the_graph_and_denominator() {
        let dir = tempfile::tempdir().unwrap();
        let facts = json!({"facts":[{"file":"file","start":1,"end":2,"kind":"slot","owner":"Screen","shape":{"kind":"boolean"},"after":3,"init":[1,2]}],"diagnostics":[]});
        let widths = BTreeSet::from([375, 768]);
        let (code, report) = execute_with(
            Some(&mut StaticPage),
            &facts,
            &json!({"path":"/","maxNoImprovementRounds":1}),
            dir.path(),
            &widths,
            None,
        )
        .unwrap_err();
        assert_eq!(code, 5);
        assert_eq!(report["scenarios"][0]["termination"], "no-improvement");
        assert_eq!(report["coverage"]["attempted"], 4);
        assert_eq!(report["coverage"]["verified"], 2);
        assert_eq!(report["coverage"]["renderable"], 6);
        assert_eq!(
            report["cells"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["reason"] == "not-rendered")
                .count(),
            2
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
    fn default_run_exhausts_every_planned_state() {
        let dir = tempfile::tempdir().unwrap();
        let facts = json!({"facts":[{"file":"file","start":1,"end":2,"kind":"slot","owner":"Screen","shape":{"kind":"boolean"},"after":3,"init":[1,2]}],"diagnostics":[]});
        let widths = BTreeSet::from([375, 768]);
        let report = match execute_with(
            Some(&mut StaticPage),
            &facts,
            &json!({"path":"/"}),
            dir.path(),
            &widths,
            None,
        ) {
            Ok(body) | Err((_, body)) => body,
        };
        assert_eq!(report["remainingPlans"], 0);
        assert_eq!(report["scenarios"][0]["termination"], "exhausted");
        assert_eq!(
            report["coverage"]["attempted"],
            report["coverage"]["renderable"]
        );
    }

    #[test]
    fn bfs_discovers_multi_hop_routes_without_cycles_or_dynamic_id_explosion() {
        struct MultiHopSite;
        impl Renderer for MultiHopSite {
            fn capture(
                &mut self,
                spec: &Value,
                _inputs: &Value,
                width: u32,
                image: &Path,
            ) -> Result<Value, Box<dyn std::error::Error>> {
                fs::write(image, [width as u8])?;
                let path = spec["path"].as_str().unwrap_or("/");
                let links = match path {
                    "/" => json!(["/settings", "/orders/1", "/orders/2", "/orders/3"]),
                    "/settings" => json!(["/", "/settings/keys"]),
                    "/settings/keys" => json!(["/", "/settings", "/orders/4"]),
                    _ => json!(["/"]),
                };
                Ok(json!({
                    "ready": true,
                    "settled": true,
                    "width": width,
                    "path": path,
                    "hits": [],
                    "assertions": [],
                    "slots": {},
                    "instances": {},
                    "links": links,
                }))
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let report = match execute_with(
            Some(&mut MultiHopSite),
            &json!({"facts": [], "diagnostics": []}),
            &json!({"path": "/", "routeSelector": "a[href]", "hasScreenAssertion": true}),
            dir.path(),
            &BTreeSet::from([1280]),
            None,
        ) {
            Ok(body) | Err((_, body)) => body,
        };
        let discovered: Vec<&str> = report["discoveredRoutes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        // Discovers depth-1 (`/settings`, `/orders/1`) AND depth-2 (`/settings/keys`),
        // ignores cyclic `/` & `/settings`, and collapses `/orders/2`, `/orders/3`, `/orders/4`.
        assert_eq!(discovered, vec!["/orders/1", "/settings", "/settings/keys"]);
        assert_eq!(report["cells"].as_array().unwrap().len(), 4);
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
            execute_with(Some(renderer), &facts, &spec, dir.path(), &widths, None)
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
        let graph: Value =
            serde_json::from_slice(&fs::read(dir.path().join("graph.json")).unwrap()).unwrap();
        let snap = json!({"id":"run-partial","projectRoot":"/tmp/demo","projectName":"demo","timestampMs":1});
        let imported = crate::archive::import_run(&snap, &saved, &graph).unwrap();
        let summary = imported.aggregate.coverage();
        assert_eq!(summary.verified, 10);
        assert_eq!(summary.unverified, 5);
        assert_eq!(summary.renderable, 15);
        assert_eq!(summary.unrenderable, 0);
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
            None,
        )
        .unwrap_err();
        assert_eq!(report["coverage"]["renderable"], 0);
        assert_eq!(report["coverage"]["unrenderable"], 1);
        assert_eq!(report["cells"][0]["reason"], "no-render-env");
        let graph: Value =
            serde_json::from_slice(&fs::read(dir.path().join("graph.json")).unwrap()).unwrap();
        let snap = json!({"id":"run-unrenderable","projectRoot":"/tmp/demo","projectName":"demo","timestampMs":1});
        let imported = crate::archive::import_run(&snap, &report, &graph).unwrap();
        let summary = imported.aggregate.coverage();
        assert_eq!(summary.renderable, 0);
        assert_eq!(summary.unrenderable, 1);
        assert_eq!(
            imported.aggregate.cells()[0].status(),
            &dui_core::model::CellStatus::unrenderable("no-render-env").unwrap()
        );
    }

    #[test]
    fn runtime_only_branches_and_stagnation_termination_are_reported_separately() {
        let slot_a = json!({"file":"screen.tsx","owner":"Panel","start":1,"end":2,"kind":"slot","binding":1,"init":[1,2],"after":3,"shape":{"kind":"boolean"}});
        let slot_b = json!({"file":"screen.tsx","owner":"Panel","start":4,"end":5,"kind":"slot","binding":2,"init":[4,5],"after":6,"shape":{"kind":"boolean"}});
        let branch_a = json!({"file":"screen.tsx","owner":"Panel","start":10,"end":20,"kind":"branch","predicate":{"binding":1,"operator":"truthy"},"textWhenTrue":"On","textWhenFalse":"Off"});
        let key_a = key(&slot_a);
        let key_b = key(&slot_b);
        let branch_key = key(&branch_a);

        struct RuntimeBranchRenderer {
            key_a: String,
            key_b: String,
            branch_key: String,
        }
        impl Renderer for RuntimeBranchRenderer {
            fn capture(
                &mut self,
                spec: &Value,
                inputs: &Value,
                width: u32,
                image: &Path,
            ) -> Result<Value, Box<dyn std::error::Error>> {
                fs::write(image, format!("{inputs}-{width}"))?;
                let a = inputs
                    .get(&self.key_a)
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let b = inputs
                    .get(&self.key_b)
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let assertions: Vec<bool> = spec["assertions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|_| true)
                    .collect();
                Ok(json!({
                    "ready": true,
                    "settled": true,
                    "width": width,
                    "path": spec["path"],
                    "assertions": assertions,
                    "slots": { &self.key_a: a, &self.key_b: b },
                    "responses": {},
                    "responseChecks": {},
                    "hits": [
                        format!("{}:{a}", self.branch_key),
                        "runtime-only.tsx#1:2:branch:true"
                    ]
                }))
            }
        }

        let dir_fwd = tempfile::tempdir().unwrap();
        let dir_rev = tempfile::tempdir().unwrap();
        let fwd_facts =
            json!({"facts":[slot_a.clone(), slot_b.clone(), branch_a.clone()],"diagnostics":[]});
        let rev_facts = json!({"facts":[branch_a, slot_b, slot_a],"diagnostics":[]});
        let spec = json!({"path":"/","maxNoImprovementRounds":1});
        let widths = BTreeSet::from([375]);

        let mut r1 = RuntimeBranchRenderer {
            key_a: key_a.clone(),
            key_b: key_b.clone(),
            branch_key: branch_key.clone(),
        };
        let (code_fwd, report_fwd) = execute_with(
            Some(&mut r1),
            &fwd_facts,
            &spec,
            dir_fwd.path(),
            &widths,
            None,
        )
        .unwrap_err();
        assert_eq!(code_fwd, 5, "unexpected error: {report_fwd}");

        let mut r2 = RuntimeBranchRenderer {
            key_a,
            key_b,
            branch_key,
        };
        let (_, report_rev) = execute_with(
            Some(&mut r2),
            &rev_facts,
            &spec,
            dir_rev.path(),
            &widths,
            None,
        )
        .unwrap_err();

        // Identical plans, cell IDs, verdicts, and coverage regardless of fact ordering.
        assert_eq!(report_fwd["cells"], report_rev["cells"]);
        assert_eq!(report_fwd["coverage"], report_rev["coverage"]);
        assert_eq!(
            report_fwd["runtimeOnlyInstrumentedBranches"],
            json!(["runtime-only.tsx#1:2:branch:true"])
        );
        assert_eq!(report_fwd["coverage"]["branchOutcomes"], 2);
        assert_eq!(report_fwd["coverage"]["observedBranchOutcomes"], 2);
        assert_eq!(report_fwd["scenarios"][0]["termination"], "no-improvement");
        assert!(report_fwd["remainingPlans"].as_u64().unwrap() > 0);
        // Every remaining planned state is preserved as an explicit unverified cell.
        assert!(
            report_fwd["cells"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["status"] == "unverified" && c["reason"] == "not-rendered")
        );
    }
}
