use crate::{
    Reply,
    analysis::{analyze, state_id, store},
    chromium::{Browser, executable},
    failure,
    host::ViteHost,
    ports::{Host, Renderer},
};
use dui_core::{
    discovery::{assertions, graph, key},
    verification::Intent,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

pub(crate) type Error = (u8, Value);
pub(crate) fn io(e: impl std::fmt::Display) -> Error {
    failure(3, &e.to_string())
}
pub(crate) fn read(path: &Path) -> Reply {
    serde_json::from_slice(&fs::read(path).map_err(io)?).map_err(io)
}
pub(crate) struct Job {
    pub(crate) cell: Value,
    pub(crate) spec: Value,
    pub(crate) inputs: BTreeMap<String, Value>,
    pub(crate) width: u32,
}

impl Job {
    pub(crate) fn scenario(&self) -> &str {
        self.cell["scenario"].as_str().unwrap_or_default()
    }
    pub(crate) fn checks(&self) -> &[Value] {
        self.spec["assertions"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}
fn jobs(
    report: &Value,
    manifest: &Value,
    facts: &[Value],
    auto_mocks: &Value,
) -> Result<Vec<Job>, Error> {
    let slots: BTreeSet<_> = facts
        .iter()
        .filter(|f| f["kind"] == "slot")
        .map(key)
        .collect();
    let mut ids = BTreeSet::new();
    let mut jobs = Vec::new();
    for c in report["cells"]
        .as_array()
        .ok_or_else(|| failure(2, "missing cells"))?
    {
        let scenario = c["scenario"]
            .as_str()
            .ok_or_else(|| failure(2, "missing scenario"))?;
        let mut spec = manifest.clone();
        if let Some(index) = scenario.strip_prefix("link-") {
            let path = index
                .parse::<usize>()
                .ok()
                .and_then(|idx| report["discoveredRoutes"].get(idx))
                .and_then(Value::as_str)
                .filter(|path| {
                    manifest["routeSelector"].is_string()
                        && path.starts_with('/')
                        && !path.starts_with("//")
                })
                .ok_or_else(|| failure(2, "invalid discovered route"))?;
            spec["path"] = json!(path);
        } else if scenario != "default" {
            let i = scenario
                .strip_prefix("case-")
                .and_then(|n| n.parse::<usize>().ok())
                .and_then(|n| n.checked_sub(1))
                .ok_or_else(|| failure(2, "invalid scenario"))?;
            let over = manifest["scenarios"][i]
                .as_object()
                .ok_or_else(|| failure(2, "unknown scenario"))?;
            for (k, v) in over {
                spec[k] = v.clone();
            }
        }
        crate::analysis::apply_generated(&mut spec, auto_mocks);
        let inputs: BTreeMap<String, Value> =
            serde_json::from_value(c["input"].clone()).map_err(io)?;
        let width: u32 = serde_json::from_value(c["width"].clone()).map_err(io)?;
        if c["path"] != spec["path"]
            || c["state"] != state_id(scenario, &spec, &inputs)
            || inputs.keys().any(|k| !slots.contains(k))
            || !manifest["widths"]
                .as_array()
                .is_some_and(|w| w.contains(&json!(width)))
            || !ids.insert((c["state"].clone().to_string(), width))
        {
            return Err(failure(
                2,
                "plan is incompatible with the current manifest or compiler facts",
            ));
        }
        spec["assertions"] = assertions(facts, &inputs, &spec);
        spec["measureSemantics"] = json!(true);
        let cell = json!({"state":c["state"],"scenario":scenario,"path":c["path"],"width":width,"input":inputs,"planIndex":c["planIndex"],"checks":spec["assertions"]});
        jobs.push(Job {
            cell,
            spec,
            inputs,
            width,
        });
    }
    if jobs.is_empty() {
        return Err(failure(2, "empty plan"));
    }
    Ok(jobs)
}

fn capture(browser: &mut Browser, job: &Job, out: &Path) -> Reply {
    let image = tempfile::NamedTempFile::new_in(out).map_err(io)?;

    let observation = Renderer::capture(
        browser,
        &job.spec,
        &json!(job.inputs),
        job.width,
        image.path(),
    )
    .map_err(io)?;
    let data = fs::read(image.path()).map_err(io)?;
    let name = format!("{:x}-{}.png", Sha256::digest(data), job.width);
    image.persist(out.join(&name)).map_err(io)?;
    Ok(json!({"image":name,"observation":observation}))
}

fn collect(jobs: &[Job], out: &Path, port: u16) -> Result<(), Error> {
    let folder = out.join("captures");
    fs::create_dir_all(&folder).map_err(io)?;
    let cursor = AtomicUsize::new(0);
    let size = std::thread::available_parallelism()
        .map_or(2, usize::from)
        .min(6);
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..size {
            handles.push(scope.spawn(|| -> Result<(), Error> {
                let profile = tempfile::tempdir().map_err(io)?;
                let mut browser =
                    Browser::local(&executable(), profile.path(), port).map_err(io)?;
                loop {
                    let index = cursor.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(index) else { break };
                    if index == 0 {
                        let version = browser.version().map_err(io)?;
                        store(
                            &out.join("environment.json"),
                            &json!({"available":true,"browser":version,"os":std::env::consts::OS}),
                        )?;
                    }
                    let p = folder.join(format!("{index}.json"));
                    let mut result = match capture(&mut browser, job, out) {
                        Ok(v) => v,
                        Err((_, first)) => {
                            drop(browser);
                            browser =
                                Browser::local(&executable(), profile.path(), port).map_err(io)?;
                            capture(&mut browser, job, out)
                                .unwrap_or_else(|(_, e)| json!({"error":e,"firstError":first}))
                        }
                    };
                    result["state"] = job.cell["state"].clone();
                    result["width"] = json!(job.width);
                    store(&p, &result)?;
                }
                Ok(())
            }))
        }
        for handle in handles {
            handle
                .join()
                .map_err(|_| failure(3, "capture worker panic"))??;
        }
        Ok(())
    })
}

pub fn run(project: &Path, manifest: &Path, plan: &Path, out: &Path, port: Option<u16>) -> Reply {
    let analysis = analyze(project, manifest)?;
    let facts = analysis["facts"]
        .as_array()
        .ok_or_else(|| failure(3, "missing facts"))?;
    let prior = read(plan)?;
    let manifest_value = read(manifest)?;
    let entries = jobs(&prior, &manifest_value, facts, &analysis["autoMocks"])?;
    fs::create_dir_all(
        out.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
    .map_err(io)?;
    fs::create_dir(out).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            failure(2, "use a new output directory")
        } else {
            io(e)
        }
    })?;
    store(
        &out.join("run.json"),
        &run_binding(&prior, &manifest_value, &analysis),
    )?;
    store(&out.join("facts.json"), &analysis)?;
    if !executable().is_file() {
        store(
            &out.join("environment.json"),
            &json!({"available":false,"os":std::env::consts::OS,"reason":"no-render-env"}),
        )?;
        return unavailable(&entries, &analysis, out);
    }
    let host = if port.is_none() {
        Some(ViteHost.start(project, manifest, &analysis)?)
    } else {
        None
    };
    let port = port
        .or_else(|| host.as_ref().map(|h| h.port))
        .ok_or_else(|| failure(3, "missing host"))?;

    // Reference pages start from the untouched route in separate browser profiles.
    let (captures, controls) = std::thread::scope(|scope| {
        let reference = scope.spawn(|| crate::reference::collect(&entries, out, port));
        let captures =
            collect(&entries, out, port).and_then(|()| crate::retry::run(&entries, out, port));
        let controls = reference
            .join()
            .unwrap_or_else(|_| Err(io("reference collector panic")));
        (captures, controls)
    });
    if captures.is_err() || controls.is_err() {
        // Only a live run can attest that a worker failed. Offline verification
        // still rejects missing or corrupt evidence from a previously saved run.
        if let Err((_, detail)) = &captures {
            let folder = out.join("captures");
            fs::create_dir_all(&folder).map_err(io)?;
            for (index, job) in entries.iter().enumerate() {
                let path = folder.join(format!("{index}.json"));
                if !path.try_exists().map_err(io)? {
                    store(
                        &path,
                        &json!({"state":job.cell["state"],"width":job.width,"error":detail}),
                    )?;
                }
            }
        }
        store(
            &out.join("worker-errors.json"),
            &json!({
                "captures":captures.err().map(|(_, detail)| detail),
                "references":controls.err().map(|(_, detail)| detail)
            }),
        )?;
    }
    finish(&entries, &analysis, out)
}

fn unavailable(jobs: &[Job], analysis: &Value, out: &Path) -> Reply {
    let cells: Vec<_> = jobs
        .iter()
        .map(|job| {
            let mut c = job.cell.clone();
            c["status"] = json!("unrenderable");
            c["reason"] = json!("no-render-env");
            c
        })
        .collect();
    let report = json!({"coverage":{"attempted":0,"renderable":0,"verified":0,"unrenderable":cells.len()},"cells":cells,"compiler":analysis["compiler"]});
    let g = crate::graph_view::with_cells(
        graph(
            analysis["facts"]
                .as_array()
                .ok_or_else(|| io("missing facts"))?,
            &BTreeSet::new(),
        ),
        &report,
    )?;
    store(&out.join("graph.json"), &g)?;
    store(&out.join("report.json"), &report)?;
    Err((5, report))
}

fn intent<'a>(job: &'a Job, analysis: &Value, has_branch: bool) -> Intent<'a> {
    Intent {
        path: job.spec["path"].as_str().unwrap_or(""),
        width: job.width,
        inputs: &job.inputs,
        assertions: job.checks(),
        required_requests: job.spec["requiredRequests"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default(),
        expected_api_errors: job.spec["expectedApiErrors"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default(),
        analyzed: analysis["diagnostics"] == json!([]),
        expects_probe: has_branch,
    }
}
fn run_binding(plan: &Value, manifest: &Value, analysis: &Value) -> Value {
    fn digest(v: &Value) -> String {
        format!("{:x}", Sha256::digest(v.to_string()))
    }
    json!({"formatVersion":1,"planSha256":digest(plan),"manifestSha256":digest(manifest),"factsSha256":digest(analysis)})
}
fn initial(out: &Path, index: usize, job: &Job) -> Reply {
    let raw = read(&out.join("captures").join(format!("{index}.json")))?;
    if raw["state"] != job.cell["state"] || raw["width"] != job.width {
        return Err(failure(3, "capture identity does not match plan"));
    }
    Ok(raw)
}
fn selected(out: &Path, index: usize, job: &Job) -> Reply {
    let mut first = initial(out, index, job)?;
    if !crate::retry::eligible(job, &first) {
        return Ok(first);
    }
    let path = format!("after-mount/{index}.json");
    if !out.join(&path).is_file() {
        return Ok(first);
    }
    let mut attempt = read(&out.join(&path))?;
    if attempt["state"] != job.cell["state"] || attempt["width"] != job.width {
        return Err(io("retry identity does not match plan"));
    }
    let values = &attempt["observation"]["slots"];
    if attempt["restored"] == true
        && attempt["observation"]["ready"] == true
        && attempt["observation"]["width"] == job.width
        && attempt["observation"]["path"] == job.spec["path"]
        && job.inputs.iter().all(|(k, v)| values.get(k) == Some(v))
    {
        attempt["selectedCapture"] = json!(path);
        return Ok(attempt);
    }
    first["rejectedRetry"] = json!(path);
    Ok(first)
}

fn check_image(
    raw: &Value,
    width: u32,
    out: &Path,
    seen: &mut BTreeSet<String>,
) -> Result<(), Error> {
    if raw["error"].is_object() {
        return Ok(());
    }
    let name = raw["image"]
        .as_str()
        .ok_or_else(|| io("missing capture image"))?;
    if seen.contains(name) && name.ends_with(&format!("-{width}.png")) {
        return Ok(());
    }
    let digest = name
        .strip_suffix(&format!("-{width}.png"))
        .filter(|d| d.len() == 64 && d.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| io("invalid capture image name"))?;
    let bytes = fs::read(out.join(name)).map_err(io)?;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.get(16..20) != Some(&width.to_be_bytes())
        || digest != format!("{:x}", Sha256::digest(&bytes))
    {
        return Err(io(
            "capture image content or viewport does not match its evidence",
        ));
    }
    seen.insert(name.into());
    Ok(())
}
fn semantic_hash(o: &Value) -> Option<&str> {
    if o["semantic"]["count"].as_u64()? == 0 {
        return None;
    }
    o["semantic"]["hash"].as_str().filter(|s| s.len() == 64)
}
fn group_key(job: &Job, inputs: &BTreeMap<String, Value>) -> (String, u32, String) {
    (job.scenario().into(), job.width, json!(inputs).to_string())
}
struct ControlObservation {
    hash: String,
    slots: Value,
    index: usize,
}
type Controls = BTreeMap<(String, u32, String), ControlObservation>;

fn compare_views(
    job: &Job,
    obs: &Value,
    reference: &Value,
    controls: &Controls,
) -> Result<(), &'static str> {
    let Some(baseline) = controls.get(&group_key(job, &BTreeMap::new())) else {
        return Err("missing-route-control");
    };
    if reference["state"] != job.cell["state"]
        || reference["widthId"] != job.width
        || reference["restore"] != true
        || reference["applied"] != true
        || reference["ready"] != true
        || reference["width"] != job.width
        || reference["path"] != job.spec["path"]
        || reference["assertions"] != obs["assertions"]
        || !job
            .inputs
            .iter()
            .all(|(k, v)| reference["slots"].get(k) == Some(v))
    {
        return Err("setter-replay-unconfirmed");
    }
    let control = &reference["control"];
    if semantic_hash(control) != Some(&baseline.hash)
        || control["settled"] == false
        || control["ready"] != true
        || control["path"] != job.spec["path"]
        || control["width"] != job.width
        || !control["responseChecks"]
            .as_object()
            .is_some_and(|c| c.values().all(|v| v == true))
        || !job.spec["requiredRequests"]
            .as_array()
            .into_iter()
            .flatten()
            .all(|p| {
                p.as_str()
                    .and_then(|p| control["responses"][p].as_u64())
                    .is_some_and(|s| (200..300).contains(&s))
            })
    {
        return Err("unstable-route-control");
    }
    let Some(actual) = semantic_hash(obs) else {
        return Err("missing-semantic-capture");
    };
    if semantic_hash(reference) != Some(actual) {
        return Err("setter-view-mismatch");
    }
    Ok(())
}
#[derive(Default)]
struct Corroboration {
    supported: BTreeMap<String, Value>,
    missing: BTreeMap<String, &'static str>,
}

fn corroborate(
    job: &Job,
    obs: &Value,
    reference: &Value,
    controls: &Controls,
) -> Result<Corroboration, &'static str> {
    compare_views(job, obs, reference, controls)?;
    let actual = semantic_hash(obs).ok_or("missing-semantic-capture")?;
    let mut result = Corroboration::default();
    for (id, value) in &job.inputs {
        if job.checks().iter().any(|c| c["slot"] == *id) {
            continue;
        }
        let mut other = job.inputs.clone();
        other.remove(id);
        let Some(control) = controls.get(&group_key(job, &other)) else {
            result
                .missing
                .insert(id.clone(), "comparison-control-unavailable");
            continue;
        };
        let Some(prior) = control.slots.get(id) else {
            result
                .missing
                .insert(id.clone(), "slot-not-observed-in-control");
            continue;
        };
        if let Some(kind) =
            dui_core::verification::slot_witness(value, Some(prior), actual, &control.hash)
        {
            result.supported.insert(
                id.clone(),
                json!({"kind":kind,"controlIndex":control.index,"controlHash":control.hash}),
            );
        } else {
            result.missing.insert(
                id.clone(),
                if value == prior {
                    "same-input-view-mismatch"
                } else {
                    "no-visible-witness"
                },
            );
        }
    }
    Ok(result)
}

fn finish(jobs: &[Job], analysis: &Value, out: &Path) -> Reply {
    let facts = analysis["facts"]
        .as_array()
        .ok_or_else(|| io("missing facts"))?;
    let has_branch = facts.iter().any(|f| f["kind"] == "branch");
    let mut controls = Controls::new();
    // Single-slot plans and the unmodified screen are controls for pairwise plans.
    for slots in [0, 1] {
        for (i, job) in jobs
            .iter()
            .enumerate()
            .filter(|(_, job)| job.inputs.len() == slots)
        {
            let raw = selected(out, i, job)?;
            let obs = &raw["observation"];
            let expected = intent(job, analysis, has_branch);
            let Some(hash) = semantic_hash(obs) else {
                continue;
            };
            if expected.observation_failure(obs).is_some() {
                continue;
            }
            if expected.failure(obs).is_some() {
                let reference = crate::reference::load(&out.join(format!("reference/{i}.json")))?;
                if reference.is_none_or(|r| compare_views(job, obs, &r, &controls).is_err()) {
                    continue;
                }
            }
            controls.insert(
                group_key(job, &job.inputs),
                ControlObservation {
                    hash: hash.into(),
                    slots: obs["slots"].clone(),
                    index: i,
                },
            );
        }
    }
    let mut cells = Vec::new();
    let mut hits = BTreeSet::new();
    let mut verified = 0;
    let mut reasons = BTreeMap::<String, usize>::new();
    let mut checked_images = BTreeSet::new();
    for (i, job) in jobs.iter().enumerate() {
        let capture_file = format!("captures/{i}.json");
        let got = selected(out, i, job)?;
        check_image(&got, job.width, out, &mut checked_images)?;
        let obs = &got["observation"];
        let mut cell = job.cell.clone();
        let expected = intent(job, analysis, has_branch);
        let mut evidence = BTreeMap::new();
        let mut method = "declared-assertions";
        let mut reason = Value::Null;
        if got["error"].is_object() {
            reason = json!("render-failed")
        } else if let Some(f) = expected.observation_failure(obs) {
            reason = json!(f)
        } else if expected.failure(obs).is_some() {
            let path = format!("reference/{i}.json");
            let reference = crate::reference::load(&out.join(&path))?;
            if let Some(r) = reference {
                match corroborate(job, obs, &r, &controls) {
                    Ok(result) => {
                        cell["referenceViewMatched"] = json!(true);
                        evidence = result.supported;
                        let backed = evidence.keys().cloned().collect();
                        if expected.failure_with(obs, &backed).is_none() {
                            method = "independent-setter-replay";
                        } else {
                            let first = result
                                .missing
                                .values()
                                .copied()
                                .min()
                                .unwrap_or("missing-state-assertion");
                            reason = json!(first);
                            cell["witnessFailures"] = json!(result.missing);
                        }
                    }
                    Err(why) => {
                        reason = json!(why);
                        cell["referenceViewMatched"] = json!(false);
                    }
                }
                cell["reference"] = json!(path);
            } else {
                reason = json!("missing-state-assertion")
            }
        }
        if reason.is_null() {
            verified += 1;
            cell["verificationMethod"] = json!(method)
        } else {
            cell["unconfirmedSlots"] = json!(
                job.inputs
                    .keys()
                    .filter(|id| !job
                        .checks()
                        .iter()
                        .any(|c| c["slot"].as_str() == Some(id.as_str()))
                        && !evidence.contains_key(*id))
                    .collect::<Vec<_>>()
            );
            *reasons
                .entry(reason.as_str().unwrap_or("unknown").into())
                .or_default() += 1;
        }
        cell["status"] = json!(if reason.is_null() {
            "verified"
        } else {
            "unverified"
        });
        cell["reason"] = reason;
        if got["rejectedRetry"].is_string() {
            cell["rejectedRetry"] = got["rejectedRetry"].clone();
        }
        cell["witnesses"] = json!(evidence);
        if got["selectedCapture"].is_string() {
            cell["injectionPhase"] = json!("after-mount");
            cell["initialCapture"] = json!(capture_file);
            cell["capture"] = got["selectedCapture"].clone();
        } else {
            cell["capture"] = json!(capture_file);
            cell["injectionPhase"] = json!("initial");
        }
        cell["image"] = got["image"].clone();
        cell["observation"] = json!({"hits":obs["hits"],"semantic":obs["semantic"]});
        hits.extend(
            obs["hits"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned),
        );
        cells.push(cell);
    }
    let expected: BTreeSet<_> = facts
        .iter()
        .filter(|f| f["kind"] == "branch")
        .flat_map(|f| [format!("{}:true", key(f)), format!("{}:false", key(f))])
        .collect();
    let summary = json!({
        "declared":cells.iter().filter(|c|c["verificationMethod"]=="declared-assertions").count(),
        "independentReplay":cells.iter().filter(|c|c["verificationMethod"]=="independent-setter-replay").count(),
        "referenceViewMatches":cells.iter().filter(|c|c["referenceViewMatched"]==true).count(),
        "postMountAttempts":cells.iter().filter(|c|c["injectionPhase"]=="after-mount" || c.get("rejectedRetry").is_some()).count(),
        "verifiedAfterMount":cells.iter().filter(|c|c["injectionPhase"]=="after-mount" && c["status"]=="verified").count(),
    });
    let mut report = json!({"evidenceSummary":summary,"versions":analysis["versions"],"sourceFiles":analysis["sourceFiles"],"os":std::env::consts::OS,"unverifiedReasons":reasons,"coverage":{"attempted":jobs.len(),"renderable":jobs.len(),"verified":verified,"unrenderable":0,"branchOutcomes":expected.len(),"observedBranchOutcomes":expected.intersection(&hits).count()},"remainingPlans":0,"unobservedBranches":expected.difference(&hits).collect::<Vec<_>>(),"cells":cells,"compiler":analysis["compiler"],"diagnostics":analysis["diagnostics"]});
    if let Some(environment) = crate::reference::load(&out.join("environment.json"))? {
        report["environment"] = environment;
    }
    if let Some(errors) = crate::reference::load(&out.join("worker-errors.json"))? {
        report["workerErrors"] = errors;
    }
    let g = crate::graph_view::with_cells(graph(facts, &hits), &report)?;
    if let Some(cells) = report["cells"].as_array_mut() {
        for cell in cells {
            cell["semantic"] = cell["observation"]["semantic"].clone();
            cell.as_object_mut().unwrap().remove("observation");
        }
    }
    store(&out.join("graph.json"), &g)?;
    store(&out.join("report.json"), &report)?;
    if verified == jobs.len() {
        Ok(report)
    } else {
        Err((5, report))
    }
}

pub fn summarize(manifest: &Path, plan: &Path, out: &Path) -> Reply {
    let analysis = read(&out.join("facts.json"))?;
    let plan = read(plan)?;
    let spec = read(manifest)?;
    if read(&out.join("run.json"))? != run_binding(&plan, &spec, &analysis) {
        return Err(failure(
            2,
            "plan, manifest or facts differ from the recorded run",
        ));
    }
    let entries = jobs(
        &plan,
        &spec,
        analysis["facts"]
            .as_array()
            .ok_or_else(|| io("missing facts"))?,
        &analysis["autoMocks"],
    )?;
    if crate::reference::load(&out.join("environment.json"))?
        .is_some_and(|e| e["available"] == false)
    {
        return unavailable(&entries, &analysis, out);
    }
    finish(&entries, &analysis, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_identity_includes_the_scenario_inputs_and_width() {
        let spec = json!({"path":"/page","widths":[768]});
        let state = state_id("default", &spec, &BTreeMap::new());
        let cell =
            json!({"scenario":"default","state":state,"path":"/page","width":768,"input":{}});
        assert!(jobs(&json!({"cells":[cell.clone()]}), &spec, &[], &Value::Null).is_ok());
        assert!(
            jobs(
                &json!({"cells":[cell.clone(),cell.clone()]}),
                &spec,
                &[],
                &Value::Null
            )
            .is_err()
        );
        let mut wrong = cell;
        wrong["width"] = json!(375);
        assert!(jobs(&json!({"cells":[wrong]}), &spec, &[], &Value::Null).is_err());
    }

    #[test]
    fn replays_discovered_hash_routes_with_the_recorded_csv() {
        let spec = json!({"path":"/","widths":[768],"routeSelector":"nav a"});
        let mocks = json!({"csv":{"/data":"one,2"}});
        let mut selected = spec.clone();
        selected["path"] = json!("/#/list");
        crate::analysis::apply_generated(&mut selected, &mocks);
        let state = state_id("link-0", &selected, &BTreeMap::new());
        let report = json!({"discoveredRoutes":["/#/list"],"cells":[{"scenario":"link-0","state":state,"path":"/#/list","width":768,"input":{}}]});
        assert!(jobs(&report, &spec, &[], &mocks).is_ok());
        assert!(jobs(&report, &spec, &[], &json!({"csv":{"/data":"one,3"}})).is_err());
    }

    #[test]
    fn mismatched_expected_screen_cannot_supply_a_witness() {
        let spec = json!({"path":"/page","assertions":[]});
        let job = Job {
            cell: json!({"state":"a","scenario":"default"}),
            spec,
            inputs: BTreeMap::from([("toggle".into(), json!(true))]),
            width: 375,
        };
        let digest = "a".repeat(64);
        let other = "b".repeat(64);
        let controls = BTreeMap::from([(
            group_key(&job, &BTreeMap::new()),
            ControlObservation {
                hash: other.clone(),
                slots: json!({"toggle":false}),
                index: 0,
            },
        )]);
        let target = json!({"assertions":[],"semantic":{"count":1,"hash":digest}});
        let mut reference = json!({"state":"a","widthId":375,"width":375,"path":"/page","ready":true,"restore":true,"applied":true,"assertions":[],"slots":{"toggle":true},"semantic":target["semantic"],"control":{"ready":true,"path":"/page","width":375,"responseChecks":{},"semantic":{"count":1,"hash":other}}});
        assert!(
            corroborate(&job, &target, &reference, &controls)
                .unwrap()
                .supported
                .contains_key("toggle")
        );
        reference["semantic"]["hash"] = json!(other);
        assert!(corroborate(&job, &target, &reference, &controls).is_err());
        reference["semantic"] = target["semantic"].clone();
        reference["restore"] = json!(false);
        assert!(corroborate(&job, &target, &reference, &controls).is_err());
    }
}
