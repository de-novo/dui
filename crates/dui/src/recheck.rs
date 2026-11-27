//! Fresh, selective capture of unsuccessful cells from an immutable run.
use crate::{
    Reply, analysis, archive, failure, host,
    replay::{Error, io, read},
    screen_map,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    path::Path,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Target {
    pub scenario: String,
    pub path: String,
    pub state: String,
    pub width: u32,
    pub locale: Option<String>,
    pub input: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    link_origin: Option<Origin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    case_name: Option<String>,
    #[serde(default)]
    pub plan_index: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Origin {
    scenario: String,
    path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

// Link captures inherit the settings of the scenario that discovered the link.
// Keep its identity across partial runs, where the parent is usually absent.
fn link_origins<'a>(cells: &'a [Value], names: &BTreeMap<&str, &str>) -> BTreeMap<&'a str, Origin> {
    let mut origins = BTreeMap::new();
    for cell in cells
        .iter()
        .filter(|c| c["planIndex"] == 0 && c["observation"]["ready"] == true)
    {
        let (Some(path), Some(scenario), Some(links)) = (
            cell["path"].as_str(),
            cell["scenario"].as_str(),
            cell["observation"]["links"].as_array(),
        ) else {
            continue;
        };
        let origin = if scenario.starts_with("link-") {
            origins.get(path).cloned()
        } else {
            Some(Origin {
                path: path.into(),
                scenario: scenario.into(),
                name: names.get(scenario).map(|n| (*n).to_owned()),
            })
        };
        if let Some(origin) = origin {
            for link in links.iter().filter_map(Value::as_str) {
                origins.entry(link).or_insert_with(|| origin.clone());
            }
        }
    }
    origins
}

impl Target {
    fn key(&self) -> (String, String, u32, Option<String>, String) {
        (
            self.scenario.clone(),
            self.path.clone(),
            self.width,
            self.locale.clone(),
            self.input.to_string(),
        )
    }
}

const PLAN_FILE: &str = "recheck-plan.json";

pub fn targets(output: &Path) -> Result<Option<Vec<Target>>, Error> {
    let path = output.join(PLAN_FILE);
    if !path.try_exists().map_err(io)? {
        return Ok(None);
    }
    serde_json::from_value(read(&path)?).map(Some).map_err(io)
}

type Scenarios = VecDeque<(String, Value)>;

pub fn current_spec(spec: &Value, planned: &Scenarios, target: &Target) -> Option<Value> {
    let current_case = |scenario: &str, path: &str, case_name: Option<&str>| {
        let mut candidates = planned.iter().filter(|(_, case)| {
            case["path"] == path && case_name.is_none_or(|name| case["name"] == name)
        });
        if let Some((_, case)) = candidates.clone().find(|(n, _)| n == scenario) {
            return Some(case);
        }
        // A named case or a discovered route can move to a different index.
        let (_, only) = candidates.next()?;
        ((case_name.is_some() || spec.get("screenDiscovery").is_some())
            && candidates.next().is_none())
        .then_some(only)
    };
    let mut current = if let Some(case) =
        current_case(&target.scenario, &target.path, target.case_name.as_deref())
    {
        case.clone()
    } else if target
        .scenario
        .strip_prefix("link-")
        .and_then(|n| n.parse::<usize>().ok())
        .is_some()
        && target.path.starts_with('/')
        && !target.path.starts_with("//")
        && !target.path.contains('\\')
        && !target.path.chars().any(char::is_control)
    {
        let parent = match &target.link_origin {
            Some(origin) => current_case(&origin.scenario, &origin.path, origin.name.as_deref())?,
            None => spec,
        };
        analysis::linked_spec(parent, &target.path)
    } else {
        return None;
    };
    current.as_object_mut()?.remove("routeSelector");
    current["discoverOverlays"] = json!(false);
    Some(current)
}

fn screen_status(cell: &Value) -> &Value {
    &cell["observation"]["screenEvidence"]["screenStatus"]
}

fn unsuccessful(cell: &Value) -> bool {
    cell["status"] != "verified"
        || screen_status(cell) == "unverified"
        || (cell["recheckScreenRequired"] == true && screen_status(cell) != "verified")
}

pub fn run(run_dir: &Path, project: &Path, manifest: Option<&Path>) -> Reply {
    let source = run_dir.canonicalize().map_err(io)?;
    let original = read(&source.join("report.json"))?;
    let cells = original["cells"]
        .as_array()
        .ok_or_else(|| failure(2, "source report has no capture cells"))?;
    let names = original["scenarios"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| Some((s["scenario"].as_str()?, s["name"].as_str()?)))
        .filter(|(id, _)| id.starts_with("case-"))
        .collect::<BTreeMap<_, _>>();
    let origins = link_origins(cells, &names);
    let mut selected = Vec::new();
    for cell in cells.iter().filter(|c| unsuccessful(c)) {
        let mut target = Target::deserialize(cell)
            .map_err(|e| failure(2, &format!("invalid source cell: {e}")))?;
        if target.width == 0 {
            return Err(failure(2, "invalid source cell width"));
        }
        if target.case_name.is_none() {
            target.case_name = names.get(target.scenario.as_str()).map(|n| (*n).to_owned());
        }
        if target.scenario.starts_with("link-") && target.link_origin.is_none() {
            target.link_origin = origins.get(target.path.as_str()).cloned();
        }
        selected.push(target);
    }
    let summary = json!({"source":source,"selected":selected.len(),"skipped":cells.len()-selected.len(),
        "resolved":0,"remaining":selected.len(),"results":[]});
    if selected.is_empty() {
        return Ok(json!({"recheck":summary}));
    }

    let project = project.canonicalize().map_err(io)?;
    let metadata = archive::provenance(&project)?;
    let snapshot = source.join("snapshot.json");
    if snapshot.exists() {
        let prior = read(&snapshot)?;
        if prior["projectId"] != metadata["projectId"] || prior["app"] != metadata["app"] {
            return Err(failure(2, "source run belongs to a different project"));
        }
    }
    let current = manifest
        .map(Path::to_owned)
        .unwrap_or_else(|| project.join("dui.json"));
    let temp = tempfile::tempdir().map_err(io)?;
    let mapped = read(&source.join("manifest.json"))
        .ok()
        .is_some_and(|m| m.get("screenDiscovery").is_some());
    let effective =
        if manifest.is_some() || (!mapped && crate::manifest_has_explicit_target(&current)) {
            current
        } else {
            let discovered = temp.path().join("map.json");
            screen_map::discover(&project, &discovered)?;
            discovered
        };
    archive::publish(metadata, |out| {
        std::fs::create_dir_all(out).map_err(io)?;
        analysis::store(&out.join(PLAN_FILE), &json!(selected))?;
        let mut report = match host::run_with_options(&project, &effective, out, true) {
            Ok(value) | Err((5, value)) => value,
            Err(error) => return Err(error),
        };
        std::fs::remove_file(out.join(PLAN_FILE)).map_err(io)?;
        summarize(out, &mut report, summary, &selected, cells)?;
        analysis::store(&out.join("report.json"), &report)?;
        if report["recheck"]["remaining"] == 0 {
            Ok(report)
        } else {
            Err((5, report))
        }
    })
}

fn summarize(
    out: &Path,
    report: &mut Value,
    mut summary: Value,
    selected: &[Target],
    cells: &[Value],
) -> Result<(), Error> {
    let surface_reason = report["reason"]
        .as_str()
        .unwrap_or("target-not-produced")
        .to_owned();
    let mut captured = BTreeMap::new();
    if let Some(cells) = report["cells"].as_array_mut() {
        for cell in cells.drain(..) {
            let key = Target::deserialize(&cell).map_err(io)?.key();
            if captured.insert(key, cell).is_some() {
                return Err(failure(3, "selective capture produced duplicate cells"));
            }
        }
    }
    let mut fresh = Vec::with_capacity(selected.len());
    let mut results = Vec::new();
    let mut missing = false;
    let mut resolved = 0;
    for (target, prior) in selected
        .iter()
        .zip(cells.iter().filter(|c| unsuccessful(c)))
    {
        let mut current = captured.remove(&target.key()).unwrap_or_else(|| {
            missing = true;
            let mut cell = json!(target);
            cell["status"] = json!(if report["status"] == "unrenderable" {
                "unrenderable"
            } else {
                "unverified"
            });
            cell["reason"] = json!(surface_reason);
            cell
        });
        if let Some(name) = &target.case_name {
            current["caseName"] = json!(name);
        }
        if let Some(origin) = &target.link_origin {
            current["linkOrigin"] = json!(origin);
        }
        if (!screen_status(prior).is_null() || prior["recheckScreenRequired"] == true)
            && screen_status(&current).is_null()
        {
            current["recheckScreenRequired"] = json!(true);
        }
        let passed = !unsuccessful(&current);
        resolved += usize::from(passed);
        results.push(json!({"previousState":target.state, "state":current["state"], "scenario":target.scenario,
                "path":target.path,"width":target.width,"locale":target.locale,"resolved":passed,
                "previousStatus":prior["status"],"status":current["status"],"previousReason":prior["reason"],
                "reason":current["reason"],"previousScreenStatus":screen_status(prior),"screenStatus":screen_status(&current)}));
        fresh.push(current);
    }
    if !captured.is_empty() {
        return Err(failure(3, "selective capture produced unexpected cells"));
    }
    summary["resolved"] = json!(resolved);
    summary["remaining"] = json!(selected.len() - resolved);
    summary["results"] = json!(results);
    report["recheck"] = summary;
    report["cells"] = Value::Array(fresh);
    if missing {
        include_missing_in_graph(out, report)?;
    }
    Ok(())
}

fn include_missing_in_graph(out: &Path, report: &mut Value) -> Result<(), Error> {
    let fresh = report["cells"]
        .as_array()
        .ok_or_else(|| failure(3, "expected capture cells"))?;
    let facts = read(&out.join("facts.json")).ok();
    let facts = facts
        .as_ref()
        .and_then(|f| f["facts"].as_array())
        .map(Vec::as_slice)
        .unwrap_or_default();
    let hits = fresh
        .iter()
        .flat_map(|c| c["observation"]["hits"].as_array())
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let mut coverage = crate::coverage::base(fresh);
    if !report["coverage"].is_null() {
        for key in ["branchOutcomes", "observedBranchOutcomes"] {
            if let Some(value) = report["coverage"].get(key).cloned() {
                coverage[key] = value;
            }
        }
    }
    report["coverage"] = coverage;
    let graph = crate::graph_view::with_cells(dui_core::discovery::graph(facts, &hits), report)?;
    analysis::store(&out.join("graph.json"), &graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Renderer;
    use std::collections::BTreeSet;

    #[test]
    fn recheck_uses_archived_inputs_and_current_scenario_without_expanding_the_plan() {
        struct Page {
            calls: usize,
        }
        impl Renderer for Page {
            fn capture(
                &mut self,
                spec: &Value,
                input: &Value,
                width: u32,
                image: &Path,
            ) -> Result<Value, Box<dyn std::error::Error>> {
                self.calls += 1;
                assert_eq!(spec["path"], "/settings");
                assert_eq!(spec["locale"], "ko");
                assert_eq!(spec["readySelector"], "[data-current]");
                assert_eq!(input, &json!({"toggle":true}));
                assert_eq!(width, 412);
                std::fs::write(image, b"fresh")?;
                Ok(
                    json!({"ready":true,"settled":true,"path":"/settings","width":width,"hits":[],
                    "slots":input,"assertions":[],"responseChecks":{}}),
                )
            }
        }
        let out = tempfile::tempdir().expect("output");
        let original = json!({"path":"/settings","state":"previous-id","scenario":"case-1",
            "input":{"toggle":true},"width":412,"locale":"ko","planIndex":3});
        analysis::store(&out.path().join(PLAN_FILE), &json!([original])).expect("write plan");
        let current =
            json!({"path":"/","scenarios":[{"path":"/settings","readySelector":"[data-current]"}]});
        let mut page = Page { calls: 0 };
        let (code, report) = analysis::execute_with(
            Some(&mut page),
            &json!({"facts":[],"diagnostics":[]}),
            &current,
            out.path(),
            &BTreeSet::from([375, 1280]),
            None,
        )
        .expect_err("no current state assertion");
        assert_eq!(page.calls, 1);
        assert_eq!(code, 5);
        let cells = report["cells"].as_array().expect("cells");
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0]["planIndex"], 3);
        assert_eq!(cells[0]["status"], "unverified");
        assert_eq!(cells[0]["reason"], "missing-state-assertion");
    }

    #[test]
    fn recheck_follows_a_named_case_when_same_path_scenarios_change_order() {
        let spec = json!({"path":"/"});
        let planned = VecDeque::from([
            (
                "case-1".into(),
                json!({"path":"/cart","name":"empty","readySelector":"#empty"}),
            ),
            (
                "case-2".into(),
                json!({"path":"/cart","name":"full","readySelector":"#full"}),
            ),
        ]);
        let target =
            Target::deserialize(&json!({"scenario":"case-1","caseName":"full","state":"old",
            "path":"/cart","width":375,"input":{}}))
            .expect("target");
        assert_eq!(
            current_spec(&spec, &planned, &target).expect("case")["readySelector"],
            "#full"
        );
    }

    #[test]
    fn recheck_preserves_screen_requirement_when_a_renderer_returns_no_screen_evidence() {
        let out = tempfile::tempdir().expect("output");
        let prior = json!({"scenario":"default","path":"/","state":"old","input":{},"width":375,
            "status":"verified","observation":{"screenEvidence":{"screenStatus":"unverified"}}});
        let fresh = json!({"scenario":"default","path":"/","state":"new","input":{},"width":375,"status":"verified"});
        let mut report = json!({"cells":[fresh]});
        let targets = [Target::deserialize(&prior).expect("target")];
        summarize(out.path(), &mut report, json!({}), &targets, &[prior]).expect("summary");
        assert_eq!(report["recheck"]["remaining"], 1);
        assert!(unsuccessful(&report["cells"][0]));
        assert!(!unsuccessful(&json!({"status":"verified"})));
    }
}
