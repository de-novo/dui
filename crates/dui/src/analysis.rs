use crate::{
    Reply,
    cache::{CacheLookup, CellCache},
    chromium::Browser,
    failure,
    ports::{CaptureBatchJob, Renderer, TypeOracle},
    typescript::TypeScript,
};
use dui_core::{
    discovery::{FillTermination, assertions, candidates_with_observed, graph, key},
    locale::{flatten_message_keys, is_rtl_locale, missing_translation_keys, summarize_by_locale},
    model::{CacheOutcome, CellStatus},
};
use serde::Deserialize;
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
/// What a cell must show: its route, width, requested state and the manifest's conditions.
fn verification_intent<'a>(
    spec: &'a Value,
    width: u32,
    row: &'a BTreeMap<String, Value>,
    checks: &'a Value,
    analysis: &Value,
    facts: &[Value],
) -> dui_core::verification::Intent<'a> {
    let list = |v: &'a Value| v.as_array().map(Vec::as_slice).unwrap_or_default();
    dui_core::verification::Intent {
        path: spec["path"].as_str().unwrap_or_default(),
        width,
        inputs: row,
        assertions: list(checks),
        required_requests: list(&spec["requiredRequests"]),
        expected_api_errors: list(&spec["expectedApiErrors"]),
        analyzed: analysis["diagnostics"]
            .as_array()
            .is_some_and(Vec::is_empty),
        expects_probe: facts.iter().any(|f| f["kind"] == "branch"),
    }
}

/// Capture phases in `cell.timing`, in the order a capture runs them.
const TIMING_PHASES: [&str; 6] = [
    "navigateMs",
    "settleMs",
    "screenshotMs",
    "snapshotMs",
    "overlaysMs",
    "keyboardMs",
];

/// Run-level timing: wall time, per-phase totals over captured cells, and the slowest cells.
fn timing_summary(cells: &Value, wall: std::time::Duration) -> Value {
    let timed: Vec<&Value> = cells
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["timing"].is_object())
        .collect();
    let total = |key: &str| -> u64 { timed.iter().filter_map(|c| c["timing"][key].as_u64()).sum() };
    let mut totals = serde_json::Map::new();
    for phase in TIMING_PHASES.iter().chain(["totalMs"].iter()) {
        totals.insert((*phase).to_owned(), json!(total(phase)));
    }
    let slowest_phase = TIMING_PHASES
        .iter()
        .max_by_key(|phase| total(phase))
        .map(|phase| phase.trim_end_matches("Ms"));
    let mut slowest: Vec<&Value> = timed.clone();
    slowest.sort_by_key(|c| std::cmp::Reverse(c["timing"]["totalMs"].as_u64().unwrap_or(0)));
    json!({
        "wallMs": wall.as_millis() as u64,
        "capturedCells": timed.len(),
        "totals": totals,
        "slowestPhase": slowest_phase,
        "slowestCells": slowest
            .iter()
            .take(5)
            .map(|c| json!({"path": c["path"], "width": c["width"], "state": c["state"], "totalMs": c["timing"]["totalMs"]}))
            .collect::<Vec<_>>(),
    })
}

/// A capture that opened a compiler-found dialog becomes that screen's overlay node
/// (`<path>#overlay:<slug>`), like a dialog found by clicking, with its source location.
fn present_fact_overlay(
    cell: &mut Value,
    row: &BTreeMap<String, Value>,
    overlay_facts: &BTreeMap<String, &Value>,
) {
    let Some(open) = cell["observation"]["forcedOverlay"]
        .as_object()
        .filter(|o| o["slug"].is_string())
        .cloned()
    else {
        return;
    };
    let Some(parent) = cell["path"].as_str().map(str::to_owned) else {
        return;
    };
    let slug = open["slug"].as_str().unwrap_or("dialog");
    let source: Vec<Value> = row
        .keys()
        .filter_map(|id| overlay_facts.get(id))
        .map(|f| json!({"file": f["file"], "line": f["line"], "element": f["label"]}))
        .collect();
    cell["path"] = json!(format!("{parent}#overlay:{slug}"));
    cell["overlay"] = json!({
        "kind": "dialog",
        "parentPath": parent,
        "slug": slug,
        "title": open["title"],
        "triggerLabel": open["title"],
        "scope": "main",
        "method": "compiler",
        "source": source,
    });
}

pub fn analyze(project: &Path, spec: &Path) -> Reply {
    if let Ok(bytes) = fs::read(spec)
        && let Ok(manifest) = serde_json::from_slice::<Value>(&bytes)
    {
        let surface_id = dui_core::surface::resolve_surface_id(&manifest);
        if surface_id != dui_core::surface::SURFACE_VITE_WEB {
            let registry = crate::surface::SurfaceRegistry::default_registry();
            if let Some(adapter) = registry.find(surface_id) {
                return analyze_with_oracle(adapter.oracle(), project, spec);
            }
        }
    }
    analyze_with_oracle(&TypeScript, project, spec)
}

pub fn analyze_with_oracle(oracle: &dyn TypeOracle, project: &Path, spec: &Path) -> Reply {
    let mut result = oracle.facts(project, spec)?;
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
    let mut spec: Value =
        serde_json::from_slice(&fs::read(manifest).map_err(|_| failure(2, "missing spec"))?)
            .map_err(|_| failure(2, "invalid spec"))?;
    if matches!(
        dui_core::surface::resolve_surface_id(&spec),
        dui_core::surface::SURFACE_UNIVERSAL_WEB
            | dui_core::surface::SURFACE_ELECTRON_DESKTOP
            | dui_core::surface::SURFACE_NEXTJS_WEB
    ) {
        if spec["path"].as_str().is_none() {
            let default_path = spec["rendererUrl"]
                .as_str()
                .or_else(|| spec["url"].as_str())
                .and_then(|raw| crate::universal_web::parse_loopback_url(raw).ok())
                .map(|(_, path)| path)
                .unwrap_or_else(|| "/".to_owned());
            spec["path"] = json!(default_path);
        }
        if spec["readySelector"].as_str().is_none() {
            spec["readySelector"] = json!("body");
            spec["hasScreenAssertion"] = json!(false);
        }
    }
    if matches!(
        dui_core::surface::resolve_surface_id(&spec),
        dui_core::surface::SURFACE_UNIVERSAL_TUI
            | dui_core::surface::SURFACE_ANDROID_MOBILE
            | dui_core::surface::SURFACE_IOS_MOBILE
    ) && spec["path"].as_str().is_none()
    {
        spec["path"] = json!("/");
    }
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
        if let Some(rel) = spec["storageState"].as_str()
            && let Ok(abs) = base.join(rel).canonicalize()
        {
            spec["storageState"] = json!(abs);
        }
    }
    let redacted_spec = dui_core::auth::redact_manifest_secrets(stored_spec);
    store(&out.join("manifest.json"), &redacted_spec)?;
    let (locales, missing_by_locale) = resolve_locales(&spec, manifest.parent());
    let force = force || spec["force"] == true;
    let dui_home = crate::archive::home_dir().ok();
    let mut cache = crate::cache::open(
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
pub(crate) fn execute_with(
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
    let run_started = std::time::Instant::now();
    // A map without evidence (docs/en/web-viewer.md#shared-screen-maps): cached captures are
    // reused, each screen's first state is loaded once at the widest width to read its links,
    // and every other cell is planned as `not-rendered`.
    let plan_only = spec["capture"] == false;
    let discovery_width = widths.iter().next_back().copied().unwrap_or(0);
    let facts = analysis["facts"]
        .as_array()
        .ok_or_else(|| failure(3, "invalid facts"))?;
    // Dialogs from the compiler's facts are opened by setting their open prop, not by
    // clicking guessed triggers. Runtime trigger probing stays for projects without a compiler.
    let overlay_facts: BTreeMap<String, &Value> = facts
        .iter()
        .filter(|f| f["kind"] == "overlay")
        .map(|f| (key(f), f))
        .collect();
    // Only the Vite host instruments the app's source, so only there can the probe open them.
    let compiler_overlays = analysis["compiler"].as_str().is_some_and(|c| c != "none")
        && dui_core::surface::resolve_surface_id(spec) == dui_core::surface::SURFACE_VITE_WEB;
    // Each dialog in the code is captured once per run, from the first screen that renders it.
    let mut overlays_captured: BTreeSet<String> = BTreeSet::new();
    let mut overlays_found: BTreeSet<String> = BTreeSet::new();
    let mut cells = Vec::new();
    let mut hits = BTreeSet::new();
    let mut verified = 0;
    let mut remaining = 0;
    let mut cache_hits = 0usize;
    let mut cache_misses = 0usize;
    // Cache hits whose stored verdict differs from this build's verifier.
    let mut recomputed_verdicts = 0usize;
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
        if overrides["readySelector"].is_string() && overrides.get("hasScreenAssertion").is_none() {
            case["hasScreenAssertion"] = json!(true);
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
    let mut entry_destination = None;
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
    let selected = crate::recheck::targets(out)?;
    let retry_cases = if selected.is_some() {
        cache = None;
        std::mem::take(&mut scenarios)
    } else {
        Default::default()
    };
    let mut retries = selected.into_iter().flatten().filter_map(|target| {
        let case = crate::recheck::current_spec(spec, &retry_cases, &target)?;
        Some((target.scenario.clone(), case, Some(target)))
    });
    let mut retry_rounds = BTreeMap::new();
    // First-state captures of upcoming scenarios, taken together in parallel browsers and
    // consumed in the usual order: (scenario, path, locale, width) → (image, result).
    type PrefetchKey = (String, String, Option<String>, u32);
    let mut prefetched: BTreeMap<PrefetchKey, (tempfile::NamedTempFile, Result<Value, String>)> =
        BTreeMap::new();
    let mut prefetched_scenarios: BTreeSet<(String, String)> = BTreeSet::new();
    let mut previous_scenario = None;
    let mut baselines = BTreeMap::new();
    let mut proven_visual_slots = BTreeSet::new();
    while let Some((scenario, mut spec, target)) = retries
        .next()
        .or_else(|| scenarios.pop_front().map(|(name, case)| (name, case, None)))
    {
        apply_generated(&mut spec, &analysis["autoMocks"]);
        if compiler_overlays {
            spec["discoverOverlays"] = json!(false);
        }
        let mut scenario_overlays: BTreeSet<String> = BTreeSet::new();
        let scenario_path = spec["path"].as_str().unwrap_or_default().to_owned();
        let concurrency = crate::host::resolve_capture_concurrency(&spec);
        if target.is_none()
            && concurrency > 1
            && !prefetched_scenarios.contains(&(scenario.clone(), scenario_path.clone()))
            && let Some(browser) = renderer.as_deref_mut()
        {
            let wanted = concurrency * 2;
            let mut meta: Vec<(PrefetchKey, Value, tempfile::NamedTempFile)> = Vec::new();
            let upcoming = std::iter::once((scenario.clone(), spec.clone(), true)).chain(
                scenarios
                    .iter()
                    .map(|(name, case)| (name.clone(), case.clone(), false)),
            );
            for (name, mut case, current) in upcoming {
                if meta.len() >= wanted {
                    break;
                }
                if !current {
                    apply_generated(&mut case, &analysis["autoMocks"]);
                    if compiler_overlays {
                        case["discoverOverlays"] = json!(false);
                    }
                }
                let path = case["path"].as_str().unwrap_or_default().to_owned();
                if !prefetched_scenarios.insert((name.clone(), path.clone())) {
                    continue;
                }
                let checks = assertions(facts, &BTreeMap::new(), &case);
                let locale_slots: Vec<Option<&str>> = if locales.is_empty() {
                    vec![None]
                } else {
                    locales.iter().map(|l| Some(l.as_str())).collect()
                };
                for locale in locale_slots {
                    let mut capture_spec = case.clone();
                    if let Some(loc) = locale {
                        capture_spec["locale"] = json!(loc);
                    }
                    capture_spec["assertions"] = checks.clone();
                    for width in widths {
                        if plan_only && *width != discovery_width {
                            continue;
                        }
                        let cached = cache
                            .as_deref()
                            .and_then(|c| {
                                c.build_inputs(
                                    &name,
                                    &capture_spec,
                                    &BTreeMap::new(),
                                    *width,
                                    locale,
                                )
                            })
                            .is_some_and(|inputs| {
                                cache.as_deref_mut().is_some_and(|c| {
                                    matches!(c.lookup(&inputs, out), CacheLookup::Hit { .. })
                                })
                            });
                        if cached {
                            continue;
                        }
                        let staged = tempfile::NamedTempFile::new_in(out)
                            .map_err(|e| failure(3, &e.to_string()))?;
                        let key = (
                            name.clone(),
                            path.clone(),
                            locale.map(str::to_owned),
                            *width,
                        );
                        meta.push((key, capture_spec.clone(), staged));
                    }
                }
            }
            if meta.len() >= 2 {
                let input_val = json!({});
                let jobs: Vec<CaptureBatchJob<'_>> = meta
                    .iter()
                    .map(|((_, _, _, width), cspec, staged)| CaptureBatchJob {
                        spec: cspec,
                        input: &input_val,
                        width: *width,
                        image: staged.path(),
                    })
                    .collect();
                let results = browser.capture_batch(&jobs);
                for ((key, _, staged), res) in meta.into_iter().zip(results) {
                    prefetched.insert(key, (staged, res));
                }
            }
        }
        let max_stagnant = spec["maxNoImprovementRounds"].as_u64().map(|n| n as usize);
        let mut active = BTreeSet::new();
        let mut done = BTreeSet::new();
        let selected_widths = target.as_ref().map(|t| BTreeSet::from([t.width]));
        let widths = selected_widths.as_ref().unwrap_or(widths);
        let row = target
            .as_ref()
            .map(|t| BTreeMap::deserialize(&t.input))
            .transpose()
            .map_err(crate::replay::io)?
            .unwrap_or_default();
        let mut pending = vec![row];
        if previous_scenario.as_ref() != Some(&scenario) {
            baselines.clear();
            proven_visual_slots.clear();
        }
        previous_scenario = Some(scenario.clone());
        let mut observed_slot_values: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        let mut stagnant_rounds = 0usize;
        let mut termination = FillTermination::Exhausted;
        while let Some(row) = pending.first().cloned() {
            let index = target.as_ref().map_or(done.len(), |t| t.plan_index);
            let prev_hits = hits.len();
            let prev_active = active.len();
            done.insert(serde_json::to_string(&row).map_err(|e| failure(3, &e.to_string()))?);
            let checks = assertions(facts, &row, &spec);
            // A state that opens compiler-found dialogs: the capture reports the open dialog.
            let forced_overlay =
                !row.is_empty() && row.keys().all(|k| overlay_facts.contains_key(k));
            let locale_slots: Vec<Option<&str>> = if let Some(t) = &target {
                vec![t.locale.as_deref()]
            } else if locales.is_empty() {
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
                let mut pre_rendered: BTreeMap<
                    u32,
                    (tempfile::NamedTempFile, Result<Value, String>),
                > = BTreeMap::new();
                if index == 0 && target.is_none() {
                    for width in widths {
                        let key = (
                            scenario.clone(),
                            scenario_path.clone(),
                            locale.map(str::to_owned),
                            *width,
                        );
                        if let Some(hit) = prefetched.remove(&key) {
                            pre_rendered.insert(*width, hit);
                        }
                    }
                }
                if widths.len() >= 2
                    && !plan_only
                    && crate::host::resolve_capture_concurrency(&locale_spec) > 1
                    && let Some(browser) = renderer.as_deref_mut()
                {
                    let input_val = json!(row);
                    let mut batch_meta: Vec<(u32, Value, tempfile::NamedTempFile)> = Vec::new();
                    for width in widths {
                        if pre_rendered.contains_key(width) {
                            continue;
                        }
                        let mut capture_spec = locale_spec.clone();
                        capture_spec["assertions"] = checks.clone();
                        if forced_overlay {
                            capture_spec["forcedOverlay"] = json!(true);
                        }
                        if index != 0
                            && let Some(obj) = capture_spec.as_object_mut()
                        {
                            obj.remove("routeSelector");
                        }
                        let is_cached = cache
                            .as_deref()
                            .and_then(|c| {
                                c.build_inputs(&scenario, &capture_spec, &row, *width, *locale)
                            })
                            .is_some_and(|inputs| {
                                cache.as_deref_mut().is_some_and(|c| {
                                    matches!(c.lookup(&inputs, out), CacheLookup::Hit { .. })
                                })
                            });
                        if !is_cached {
                            let staged = tempfile::NamedTempFile::new_in(out)
                                .map_err(|e| failure(3, &e.to_string()))?;
                            batch_meta.push((*width, capture_spec, staged));
                        }
                    }
                    if batch_meta.len() >= 2 {
                        let jobs: Vec<CaptureBatchJob<'_>> = batch_meta
                            .iter()
                            .map(|(w, cspec, staged)| CaptureBatchJob {
                                spec: cspec,
                                input: &input_val,
                                width: *w,
                                image: staged.path(),
                            })
                            .collect();
                        let results = browser.capture_batch(&jobs);
                        for ((w, _, staged), res) in batch_meta.into_iter().zip(results) {
                            pre_rendered.insert(w, (staged, res));
                        }
                    }
                }
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
                    if forced_overlay {
                        capture_spec["forcedOverlay"] = json!(true);
                    }
                    if index != 0 {
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
                            if index == 0 {
                                let base_hash = image_name
                                    .split('-')
                                    .next()
                                    .unwrap_or(&image_name)
                                    .to_owned();
                                let slot_map: BTreeMap<String, Value> =
                                    slots.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                                baselines.insert(
                                    (locale.map(str::to_owned), *width),
                                    (base_hash, slot_map),
                                );
                                collect_select_slot_options(&obs, facts, &mut observed_slot_values);
                            }
                        }
                        if row.len() == 1
                            && let Some(witness_map) = obs["visualWitness"].as_object()
                        {
                            for (k, reason) in witness_map {
                                if reason.as_str() == Some("visible-change") {
                                    proven_visual_slots.insert(k.clone());
                                }
                            }
                        }
                        let observed: Vec<_> = obs["hits"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .collect();
                        hits.extend(observed.iter().map(|s| s.to_string()));
                        // The stored verdict may come from an older verifier; judge the stored
                        // observation with this build's.
                        let corroborated: BTreeSet<String> = obs["visualWitness"]
                            .as_object()
                            .map(|w| w.keys().cloned().collect())
                            .unwrap_or_default();
                        let intent =
                            verification_intent(&spec, *width, &row, &checks, analysis, facts);
                        let failure = intent.failure_with(&obs, &corroborated).or_else(|| {
                            (spec.get("screenDiscovery").is_some()
                                && spec["hasScreenAssertion"] == false)
                                .then_some(dui_core::verification::Failure::MissingScreenAssertion)
                        });
                        let reason = failure
                            .as_ref()
                            .map(|f| intent.describe(f, &obs, spec["readySelector"].as_str()));
                        if reason.is_none() {
                            verified += 1;
                        }
                        if cached_status.reason() != reason.as_deref() {
                            recomputed_verdicts += 1;
                        }
                        let mut cell = json!({
                            "scenario": scenario,
                            "path": spec["path"],
                            "width": width,
                            "input": input,
                            "state": state_id,
                            "planIndex": index,
                            "image": image_name,
                            "status": if reason.is_none() { "verified" } else { "unverified" },
                            "checks": checks,
                            "reason": reason,
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

                    if plan_only && (index != 0 || *width != discovery_width) {
                        let mut cell = json!({"scenario":scenario,"path":spec["path"],"width":width,"input":input,"state":state_id,"planIndex":index,"status":"unverified","reason":"not-rendered"});
                        if let Some(loc) = locale {
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

                    let (staged, reached) =
                        if let Some((pre_staged, pre_res)) = pre_rendered.remove(width) {
                            (pre_staged, pre_res)
                        } else {
                            let res = browser
                                .capture(&capture_spec, &input, *width, staged.path())
                                .map_err(|e| e.to_string());
                            (staged, res)
                        };
                    let mut cell = match reached {
                        Ok(mut obs) => {
                            // Timing describes this run, not the screen: keep it on the cell, out
                            // of the cached observation.
                            let timing = obs.as_object_mut().and_then(|o| o.remove("timing"));
                            let bytes =
                                fs::read(staged.path()).map_err(|e| failure(3, &e.to_string()))?;
                            let png_hash = format!("{:x}", Sha256::digest(&bytes));
                            let name = format!("{png_hash}-{width}.png");
                            if let Some(slots) = obs["slots"].as_object() {
                                active.extend(slots.keys().cloned());
                                if index == 0 {
                                    let slot_map: BTreeMap<String, Value> =
                                        slots.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                                    baselines.insert(
                                        (locale.map(str::to_owned), *width),
                                        (png_hash.clone(), slot_map),
                                    );
                                    collect_select_slot_options(
                                        &obs,
                                        facts,
                                        &mut observed_slot_values,
                                    );
                                }
                            }
                            let observed: Vec<_> = obs["hits"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(Value::as_str)
                                .collect();
                            hits.extend(observed.iter().map(|s| s.to_string()));

                            // Corroborate non-JSX-branch UI states (e.g., CSS theme / i18n dictionary
                            // lookups) when changing the slot produces a verified visual pixel change
                            // relative to the scenario's unmodified baseline capture.
                            let mut corroborated = BTreeSet::new();
                            let mut witness_record: BTreeMap<String, &'static str> =
                                BTreeMap::new();
                            if index > 0
                                && let Some((base_hash, base_slots)) =
                                    baselines.get(&(locale.map(str::to_owned), *width))
                            {
                                let checks_slice =
                                    checks.as_array().map(Vec::as_slice).unwrap_or_default();
                                if row.len() == 1 {
                                    for (k, req_val) in &row {
                                        let has_branch_check =
                                            checks_slice.iter().any(|c| c["slot"] == *k);
                                        if !has_branch_check
                                            && let Some(kind) = dui_core::verification::slot_witness(
                                                req_val,
                                                base_slots.get(k),
                                                &png_hash,
                                                base_hash,
                                            )
                                        {
                                            corroborated.insert(k.clone());
                                            witness_record.insert(k.clone(), kind);
                                            if kind == "visible-change" {
                                                proven_visual_slots.insert(k.clone());
                                            }
                                        }
                                    }
                                } else {
                                    for (k, req_val) in &row {
                                        let has_branch_check =
                                            checks_slice.iter().any(|c| c["slot"] == *k);
                                        if has_branch_check {
                                            continue;
                                        }
                                        if proven_visual_slots.contains(k) {
                                            corroborated.insert(k.clone());
                                            witness_record.insert(k.clone(), "proven-visual-slot");
                                        } else if dui_core::verification::slot_witness(
                                            req_val,
                                            base_slots.get(k),
                                            &png_hash,
                                            base_hash,
                                        ) == Some("matches-unmodified-value")
                                        {
                                            corroborated.insert(k.clone());
                                            witness_record
                                                .insert(k.clone(), "matches-unmodified-value");
                                        }
                                    }
                                }
                            }
                            if !witness_record.is_empty()
                                && let Some(obs_obj) = obs.as_object_mut()
                            {
                                obs_obj.insert("visualWitness".into(), json!(witness_record));
                            }

                            let intent =
                                verification_intent(&spec, *width, &row, &checks, analysis, facts);
                            let reason = intent.failure_with(&obs, &corroborated).or_else(|| {
                                (spec.get("screenDiscovery").is_some()
                                    && spec["hasScreenAssertion"] == false)
                                    .then_some(
                                        dui_core::verification::Failure::MissingScreenAssertion,
                                    )
                            });
                            let cell_status = match &reason {
                                None => {
                                    verified += usize::from(!plan_only);
                                    CellStatus::verified()
                                }
                                Some(f) => {
                                    let r =
                                        intent.describe(f, &obs, spec["readySelector"].as_str());
                                    CellStatus::unverified(r)
                                        .unwrap_or_else(|_| CellStatus::verified())
                                }
                            };
                            let target_png = out.join(&name);
                            if plan_only {
                                // A visit to read links, not evidence: nothing is kept.
                            } else if let Some(c) = cache.as_deref_mut() {
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
                            let mut entry = json!({"scenario":scenario,"path":spec["path"],"width":width,"input":input,"state":state_id,"planIndex":index,"image":name,"status":if reason.is_none(){"verified"}else{"unverified"},"checks":checks,"reason":reason.as_ref().map(|f| intent.describe(f, &obs, spec["readySelector"].as_str())),"observation":obs});
                            if let Some(timing) = timing {
                                entry["timing"] = timing;
                            }
                            if plan_only && let Some(fields) = entry.as_object_mut() {
                                for key in ["image", "checks"] {
                                    fields.remove(key);
                                }
                                fields.insert("status".into(), json!("unverified"));
                                fields.insert("reason".into(), json!("not-rendered"));
                            }
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
            if target.is_some() {
                pending.clear();
                break;
            }
            // This state's cells are the most recent ones.
            let this_state = |c: &Value| c["scenario"] == *scenario && c["planIndex"] == index;
            if index == 0 {
                for cell in cells.iter().rev().take_while(|c| this_state(c)) {
                    for id in cell["observation"]["overlaySlots"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .filter(|id| overlay_facts.contains_key(*id))
                    {
                        scenario_overlays.insert(id.to_owned());
                    }
                }
                overlays_found.extend(scenario_overlays.iter().cloned());
            }
            if forced_overlay {
                overlays_captured.extend(row.keys().cloned());
                for cell in cells.iter_mut().rev().take_while(|c| this_state(c)) {
                    present_fact_overlay(cell, &row, &overlay_facts);
                }
            }
            pending = candidates_with_observed(facts, &active, &observed_slot_values)
                .into_iter()
                .filter(|r| !done.contains(&json!(r).to_string()))
                .collect();
            // One open state per dialog the screen rendered, unless another screen already
            // captured that dialog.
            for id in scenario_overlays.difference(&overlays_captured) {
                let open = BTreeMap::from([(id.clone(), json!(true))]);
                if !done.contains(&json!(open).to_string()) {
                    pending.push(open);
                }
            }
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
            // A direct destination remains independently capturable when a manifest requests it.
            // For link discovery, the entry already covers it if every baseline agrees.
            if scenario == "default" && from_path == "/" {
                let mut baseline = cells
                    .iter()
                    .filter(|cell| cell["scenario"] == scenario && cell["planIndex"] == 0);
                if let Some(first) = baseline.next()
                    && let Some(destination) = first["observation"]["redirectedTo"].as_str()
                    && first["status"] == "verified"
                    && baseline.all(|cell| {
                        cell["status"] == "verified"
                            && cell["observation"]["redirectedTo"] == destination
                    })
                {
                    entry_destination = Some(destination.to_owned());
                }
            }
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
            let max_routes = spec["maxRoutes"].as_u64().map(|n| n as usize);
            for path in paths {
                if entry_destination.as_ref() == Some(&path) {
                    continue;
                }
                if max_routes.is_some_and(|limit| configured_paths.len() >= limit) {
                    break;
                }
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
        if let Some(target) = target {
            let (_, indices) = retry_rounds
                .entry(scenario)
                .or_insert_with(|| (spec["name"].clone(), BTreeSet::new()));
            indices.insert(target.plan_index);
        } else {
            rounds.push(json!({"scenario":scenario,"name":spec["name"],"plannedStates":done.len(),"remainingStates":pending.len(),"termination":termination}));
        }
        remaining += pending.len();
    }
    for (scenario, (name, indices)) in retry_rounds {
        rounds.push(json!({"scenario":scenario,"name":name,"plannedStates":indices.len(),"remainingStates":0,"termination":FillTermination::Exhausted}));
    }
    // Promote runtime-discovered WAI-ARIA/HTML modals, dialogs, and drawers into verified
    // child screen nodes (`<parentPath>#overlay:<slug>`), deduplicating global header overlays
    // once per `(slug, width)` so shared header triggers (e.g. Search) do not spam every route.
    let mut overlay_cells = Vec::new();
    let mut seen_overlay_keys: BTreeSet<(String, u64)> = BTreeSet::new();
    let mut seen_header_overlays: BTreeSet<(String, u64)> = BTreeSet::new();
    for cell in &cells {
        let Some(parent_path) = cell["path"].as_str() else {
            continue;
        };
        let Some(width) = cell["width"].as_u64() else {
            continue;
        };
        let Some(overlays) = cell["observation"]["overlays"].as_array() else {
            continue;
        };
        for ov in overlays {
            let Some(slug) = ov["slug"].as_str() else {
                continue;
            };
            let Some(img) = ov["image"].as_str() else {
                continue;
            };
            let scope = ov["scope"].as_str().unwrap_or("main");
            if scope == "header" {
                if !seen_header_overlays.insert((slug.to_owned(), width)) {
                    continue;
                }
            } else if seen_header_overlays.contains(&(slug.to_owned(), width)) {
                continue;
            }
            let overlay_path = format!("{parent_path}#overlay:{slug}");
            if !seen_overlay_keys.insert((overlay_path.clone(), width)) {
                continue;
            }
            let title = ov["title"].as_str().unwrap_or(slug);
            let trigger_label = ov["triggerLabel"].as_str().unwrap_or(title);
            let mut obs = json!({
                "ready": true,
                "settled": true,
                "width": width,
                "path": overlay_path,
            });
            if let Some(tree) = ov.get("renderTree").filter(|t| t.is_array()) {
                obs["renderTree"] = tree.clone();
            }
            if let Some(dismissal) = ov.get("dismissal").filter(|d| d.is_object()) {
                obs["dismissal"] = dismissal.clone();
            }
            let parent_slug: String = parent_path
                .chars()
                .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
                .collect();
            let state_key = format!("overlay-{parent_slug}-{slug}");
            overlay_cells.push(json!({
                "scenario": state_key,
                "path": overlay_path,
                "width": width,
                "input": {},
                "state": state_key,
                "planIndex": 0,
                "status": "verified",
                "image": img,
                "overlay": {
                    "kind": "dialog",
                    "parentPath": parent_path,
                    "slug": slug,
                    "title": title,
                    "triggerLabel": trigger_label,
                    "scope": scope,
                },
                "observation": obs,
            }));
            verified += 1;
        }
    }
    cells.extend(overlay_cells);
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
    report["timings"] = timing_summary(&report["cells"], run_started.elapsed());
    report["overlayDiscovery"] = json!({
        "method": if compiler_overlays { "compiler" } else { "runtime" },
        "declared": overlay_facts.len(),
        "rendered": overlays_found.len(),
        "captured": report["cells"].as_array().map_or(0, |cells| cells
            .iter()
            .filter(|c| c["overlay"]["method"] == "compiler")
            .count()),
    });
    if !locales.is_empty() {
        report["locales"] = summarize_by_locale(&cells, missing_by_locale);
    }
    if let Some(c) = cache.as_deref() {
        report["cache"] = json!({
            "hits": cache_hits,
            "misses": cache_misses,
            "build": c.build(),
            "recomputedVerdicts": recomputed_verdicts,
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
        // Router routes, with link crawl on top.
        if discovered["mode"] == "router" && spec["routeSelector"].is_string() {
            discovered["mode"] = json!("router+links");
        }
        // The built-in host serves a production build, without the dev server's middleware
        // or proxy. When the app relies on them and screens came out blank, say so.
        if spec["host"] != "url"
            && let Some(dev) = discovered.get("devServer").filter(|d| d.is_object())
            && cells.iter().any(|c| {
                c["observation"]["emptyMount"] == true || c["observation"]["ready"] == false
            })
        {
            let mut needs = Vec::new();
            if let Some(names) = dev["configureServer"].as_array().filter(|n| !n.is_empty()) {
                let names: Vec<_> = names.iter().filter_map(Value::as_str).collect();
                needs.push(format!("configureServer middleware ({})", names.join(", ")));
            }
            if let Some(prefixes) = dev["proxy"].as_array().filter(|p| !p.is_empty()) {
                let prefixes: Vec<_> = prefixes.iter().filter_map(Value::as_str).collect();
                needs.push(format!("server.proxy ({})", prefixes.join(", ")));
            }
            discovered["devServerHint"] = json!(format!(
                "Some screens rendered blank. The app's Vite config uses {}, which only the dev \
                 server runs. Start the dev server and map it with `dui map --url URL`; the \
                 router routes are still read.",
                needs.join(" and ")
            ));
        }
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
                    json!({"kind":"anchor","from":discovered_from.get(path).map_or(entry_path, String::as_str),"selector":selector}),
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
    // Each HTML snapshot is rendered back after capture, in the capture's own browsers: at its
    // own width for its fidelity and, when the manifest asks (#323), at the screen's other
    // captured widths, which renders every pair again (46 s for a 301-page site).
    let reflow = spec["htmlReflow"] == true;
    if let Some(cells) = report["cells"].as_array_mut() {
        let concurrency = crate::host::resolve_capture_concurrency(spec);
        let (fidelity, reflow) =
            crate::chromium::snapshot_render_back(cells, out, reflow, |jobs| match renderer {
                Some(renderer) => renderer.render_back_batch(jobs),
                None => crate::chromium::render_back_in(&mut Vec::new(), None, concurrency, jobs),
            });
        let error = fidelity.get("error").cloned();
        if fidelity["measured"] != 0 || fidelity["reused"] != 0 || error.is_some() {
            report["htmlFidelity"] = fidelity;
        }
        if let Some(mut reflow) = reflow
            && (reflow["pairs"].as_array().is_some_and(|p| !p.is_empty()) || error.is_some())
        {
            if let Some(error) = error {
                reflow["error"] = error;
            }
            report["htmlReflow"] = reflow;
        }
    }
    // Map cards draw a small copy of each capture while they are small on screen (#350). It is
    // derived from the PNG, so cached captures get theirs here too.
    if let Some(cells) = report["cells"].as_array() {
        let images: BTreeSet<&str> = cells.iter().filter_map(|c| c["image"].as_str()).collect();
        if !images.is_empty() {
            report["thumbnails"] = thumbnails(out, &images);
        }
    }
    let graph = crate::graph_view::with_cells(graph, &report)?;
    store(&out.join("report.json"), &report)?;
    store(&out.join("graph.json"), &graph)?;
    store(&out.join("facts.json"), analysis)?;
    // A map without capture claims nothing about cells it did not render: it fails only when a
    // cell it did capture (from the cache) is not verified.
    let complete = if plan_only {
        verified == attempted && unrenderable == 0
    } else {
        verified == renderable && unrenderable == 0
    };
    if complete {
        Ok(report)
    } else {
        Err((5, report))
    }
}

/// Writes the missing thumbnails of `images` in `out`, a few at a time, and says how many it
/// wrote and how long that took.
fn thumbnails(out: &Path, images: &BTreeSet<&str>) -> Value {
    let started = std::time::Instant::now();
    let todo: Vec<&str> = images
        .iter()
        .copied()
        .filter(|image| !out.join(dui_app::thumbnail::name(image)).is_file())
        .collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let made = std::sync::atomic::AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
    std::thread::scope(|scope| {
        for _ in 0..workers.min(todo.len()) {
            let (todo, next, made) = (&todo, &next, &made);
            scope.spawn(move || {
                while let Some(image) =
                    todo.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
                {
                    let Ok(png) = fs::read(out.join(image)) else {
                        continue;
                    };
                    if let Ok(Some(thumb)) = dui_app::thumbnail::make(&png)
                        && fs::write(out.join(dui_app::thumbnail::name(image)), thumb).is_ok()
                    {
                        made.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            });
        }
    });
    json!({
        "made": made.into_inner(),
        "ms": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

/// Match active `string` slots whose baseline value matches a rendered `<select>`'s current
/// value and record real `<option value="...">` values so candidate planning tests real options
/// instead of synthetic `"sample"` placeholders.
fn collect_select_slot_options(
    obs: &Value,
    facts: &[Value],
    out: &mut BTreeMap<String, Vec<Value>>,
) {
    let Some(selects) = obs["selectOptions"].as_array() else {
        return;
    };
    let Some(obs_slots) = obs["slots"].as_object() else {
        return;
    };
    for fact in facts {
        if fact["kind"] != "slot" {
            continue;
        }
        let shape_kind = fact["shape"]["kind"].as_str();
        let is_string_slot = shape_kind == Some("string")
            || (shape_kind == Some("union")
                && fact["shape"]["members"]
                    .as_array()
                    .is_some_and(|m| m.iter().any(|x| x["kind"] == "string")));
        if !is_string_slot {
            continue;
        }
        let sk = key(fact);
        if out.contains_key(&sk) {
            continue;
        }
        let Some(base_str) = obs_slots.get(&sk).and_then(Value::as_str) else {
            continue;
        };
        for sel in selects {
            if sel["current"].as_str() != Some(base_str) {
                continue;
            }
            if let Some(opts) = sel["values"].as_array()
                && let Some(first_other) = opts
                    .iter()
                    .filter_map(Value::as_str)
                    .find(|v| !v.is_empty() && *v != base_str)
            {
                out.insert(sk.clone(), vec![json!(base_str), json!(first_other)]);
                break;
            }
        }
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

    /// Counts the widths it is asked to render.
    struct CountingPage(Vec<u32>);
    impl Renderer for CountingPage {
        fn capture(
            &mut self,
            spec: &Value,
            input: &Value,
            width: u32,
            image: &Path,
        ) -> Result<Value, Box<dyn std::error::Error>> {
            self.0.push(width);
            StaticPage.capture(spec, input, width, image)
        }
    }

    #[test]
    fn a_map_without_capture_plans_cells_and_loads_each_screen_once() {
        let dir = tempfile::tempdir().unwrap();
        let facts = json!({"facts":[{"file":"file","start":1,"end":2,"kind":"slot","owner":"Screen","shape":{"kind":"boolean"},"after":3,"init":[1,2]}],"diagnostics":[]});
        let mut page = CountingPage(Vec::new());
        // Planned cells are not failures: the map succeeds.
        let report = execute_with(
            Some(&mut page),
            &facts,
            &json!({"path": "/", "capture": false, "maxNoImprovementRounds": 1}),
            dir.path(),
            &BTreeSet::from([375, 1280]),
            None,
        )
        .expect("a map without capture succeeds");
        // The first state, once, at the widest width, to read the screen's links.
        assert_eq!(page.0, vec![1280]);
        let cells = report["cells"].as_array().unwrap();
        assert!(cells.len() >= 2, "{cells:?}");
        assert!(
            cells
                .iter()
                .all(|c| c["reason"] == "not-rendered" && c.get("image").is_none())
        );
        assert_eq!(report["coverage"]["attempted"], 0);
        assert_eq!(report["coverage"]["verified"], 0);
        // No evidence is written.
        assert!(
            fs::read_dir(dir.path())
                .unwrap()
                .filter_map(Result::ok)
                .all(|e| !e.file_name().to_string_lossy().ends_with(".png"))
        );
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
        assert_eq!(report["coverage"]["verified"], 4);
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
    fn visual_witness_verifies_non_branch_theme_slot_and_rejects_noop_slot() {
        let theme_slot = json!({
            "file": "rail.tsx",
            "owner": "Rail",
            "start": 1,
            "end": 10,
            "kind": "slot",
            "binding": 1,
            "init": [1, 2],
            "after": 3,
            "shape": {"kind": "union", "members": [{"kind": "literal", "value": "dark"}, {"kind": "literal", "value": "light"}]}
        });
        let noop_slot = json!({
            "file": "rail.tsx",
            "owner": "Rail",
            "start": 20,
            "end": 30,
            "kind": "slot",
            "binding": 2,
            "init": [20, 21],
            "after": 22,
            "shape": {"kind": "boolean"}
        });
        let theme_key = key(&theme_slot);
        let noop_key = key(&noop_slot);

        struct ThemeRenderer {
            theme_key: String,
            noop_key: String,
        }
        impl Renderer for ThemeRenderer {
            fn capture(
                &mut self,
                _spec: &Value,
                inputs: &Value,
                width: u32,
                image: &Path,
            ) -> Result<Value, Box<dyn std::error::Error>> {
                let theme = inputs
                    .get(&self.theme_key)
                    .and_then(Value::as_str)
                    .unwrap_or("dark");
                let noop = inputs
                    .get(&self.noop_key)
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                // Changing `theme` visibly changes the rendered pixels; `noop` does not.
                let pixel_byte = if theme == "light" { 240u8 } else { 20u8 };
                fs::write(image, [pixel_byte, width as u8])?;
                Ok(json!({
                    "ready": true,
                    "settled": true,
                    "width": width,
                    "path": "/",
                    "hits": [],
                    "assertions": [],
                    "responseChecks": {},
                    "responses": {"/": 200},
                    "slots": {
                        &self.theme_key: theme,
                        &self.noop_key: noop,
                    },
                }))
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let mut renderer = ThemeRenderer {
            theme_key: theme_key.clone(),
            noop_key: noop_key.clone(),
        };
        let report = match execute_with(
            Some(&mut renderer),
            &json!({"facts": [theme_slot, noop_slot], "diagnostics": []}),
            &json!({"path": "/"}),
            dir.path(),
            &BTreeSet::from([1280]),
            None,
        ) {
            Ok(body) | Err((_, body)) => body,
        };
        let cells = report["cells"].as_array().unwrap();
        let theme_light = cells
            .iter()
            .find(|c| c["input"] == json!({&theme_key: "light"}))
            .expect("theme=light cell");
        assert_eq!(theme_light["status"], "verified");
        assert_eq!(
            theme_light["observation"]["visualWitness"][&theme_key],
            "visible-change"
        );
        let noop_true = cells
            .iter()
            .find(|c| c["input"] == json!({&noop_key: true}))
            .expect("noop=true cell");
        assert_eq!(noop_true["status"], "unverified");
        assert_eq!(noop_true["reason"], "missing-state-assertion");
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
