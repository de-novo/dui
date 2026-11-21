//! Action-driven transition replay and edge provenance verification (`v0.0.4`).

use crate::{
    Reply, analysis,
    chromium::{self, Browser},
    failure,
    host::ViteHost,
    ports::Host,
};
use dui_core::transition::{
    EdgeProvenance, TransitionAction, TransitionSpec, TransitionVerdict, judge_transition,
    summarize_transitions,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

fn parse_provenance(raw: Option<&str>) -> EdgeProvenance {
    match raw {
        Some("inferred") => EdgeProvenance::Inferred,
        Some("verified") => EdgeProvenance::Verified,
        _ => EdgeProvenance::Extracted,
    }
}

fn collect_transition_specs(spec: &Value, run_dir: Option<&Path>) -> Vec<TransitionSpec> {
    let mut specs = Vec::new();
    let mut seen: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut seen_pairs: BTreeSet<(String, String)> = BTreeSet::new();

    if let Some(items) = spec["transitions"].as_array() {
        for item in items {
            let Some(from) = item["from"].as_str() else {
                continue;
            };
            let Some(to) = item["to"].as_str() else {
                continue;
            };
            let selector = item["action"]["selector"]
                .as_str()
                .or_else(|| item["selector"].as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("a[href=\"{to}\"]"));
            let kind = item["action"]["kind"]
                .as_str()
                .unwrap_or("click")
                .to_owned();
            let label = item["action"]["label"]
                .as_str()
                .or_else(|| item["label"].as_str())
                .map(str::to_owned);
            let origin =
                parse_provenance(item["origin"].as_str().or_else(|| item["source"].as_str()));
            let expect_selector = item["expectSelector"]
                .as_str()
                .or_else(|| item["expect"]["selector"].as_str())
                .map(str::to_owned);
            let expect_text = item["expectText"]
                .as_str()
                .or_else(|| item["expect"]["text"].as_str())
                .map(str::to_owned);

            let key = (from.to_owned(), to.to_owned(), selector.clone());
            if seen.insert(key) {
                seen_pairs.insert((from.to_owned(), to.to_owned()));
                specs.push(TransitionSpec {
                    from: from.to_owned(),
                    to: to.to_owned(),
                    action: TransitionAction {
                        kind,
                        selector,
                        label,
                    },
                    origin,
                    expect_selector,
                    expect_text,
                });
            }
        }
    }

    if let Some(dir) = run_dir
        && let Ok(bytes) = fs::read(dir.join("graph.json"))
        && let Ok(graph) = serde_json::from_slice::<Value>(&bytes)
        && let Some(edges) = graph["edges"].as_array()
    {
        for edge in edges {
            if edge["kind"].as_str() != Some("linksTo") {
                continue;
            }
            let Some(from) = edge["source"]
                .as_str()
                .and_then(|s| s.strip_prefix("route:"))
            else {
                continue;
            };
            let Some(to) = edge["target"]
                .as_str()
                .and_then(|s| s.strip_prefix("route:"))
            else {
                continue;
            };
            if seen_pairs.contains(&(from.to_owned(), to.to_owned())) {
                continue;
            }
            let selector = edge["action"]["selector"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("a[href=\"{to}\"]"));
            let kind = edge["action"]["kind"]
                .as_str()
                .unwrap_or("click")
                .to_owned();
            let label = edge["action"]["label"].as_str().map(str::to_owned);
            let origin = parse_provenance(edge["origin"].as_str());
            let key = (from.to_owned(), to.to_owned(), selector.clone());
            if seen.insert(key) {
                seen_pairs.insert((from.to_owned(), to.to_owned()));
                specs.push(TransitionSpec {
                    from: from.to_owned(),
                    to: to.to_owned(),
                    action: TransitionAction {
                        kind,
                        selector,
                        label,
                    },
                    origin,
                    expect_selector: None,
                    expect_text: None,
                });
            }
        }
    }

    specs
}

fn update_run_graph(run_dir: &Path, verdicts: &[TransitionVerdict]) -> Result<(), (u8, Value)> {
    let graph_path = run_dir.join("graph.json");
    if !graph_path.is_file() {
        return Ok(());
    }
    let bytes = fs::read(&graph_path).map_err(|e| failure(3, &e.to_string()))?;
    let mut graph: Value =
        serde_json::from_slice(&bytes).map_err(|e| failure(3, &e.to_string()))?;

    let mut by_pair: BTreeMap<(String, String), &TransitionVerdict> = BTreeMap::new();
    for v in verdicts {
        by_pair.insert((format!("route:{}", v.from), format!("route:{}", v.to)), v);
    }

    let mut updated_pairs: BTreeSet<(String, String)> = BTreeSet::new();
    if let Some(edges) = graph["edges"].as_array_mut() {
        for edge in edges.iter_mut() {
            if edge["kind"].as_str() != Some("linksTo") {
                continue;
            }
            let (Some(src), Some(dst)) = (edge["source"].as_str(), edge["target"].as_str()) else {
                continue;
            };
            let key = (src.to_owned(), dst.to_owned());
            if let Some(v) = by_pair.get(&key) {
                edge["provenance"] = json!(v.provenance.as_str());
                edge["origin"] = json!(v.origin.as_str());
                edge["replayVerified"] = json!(v.replay_verified);
                edge["action"] = json!(v.action);
                edge["confirmedWidths"] = json!(v.confirmed_widths);
                if !v.failed_widths.is_empty() {
                    edge["failedWidths"] = json!(v.failed_widths);
                }
                updated_pairs.insert(key);
            }
        }
        for (key, v) in &by_pair {
            if updated_pairs.contains(key) {
                continue;
            }
            let mut edge = json!({
                "source": key.0,
                "target": key.1,
                "kind": "linksTo",
                "origin": v.origin.as_str(),
                "provenance": v.provenance.as_str(),
                "replayVerified": v.replay_verified,
                "action": v.action,
                "selector": v.action.selector,
                "confirmedWidths": v.confirmed_widths,
                "observedWidths": v.confirmed_widths,
            });
            if !v.failed_widths.is_empty() {
                edge["failedWidths"] = json!(v.failed_widths);
            }
            edges.push(edge);
        }
    }

    analysis::store(&graph_path, &graph)?;
    analysis::store(
        &run_dir.join("transitions.json"),
        &json!({
            "summary": summarize_transitions(verdicts),
            "transitions": verdicts,
        }),
    )?;
    Ok(())
}

pub fn replay(project: &Path, manifest: &Path, run_dir: Option<&Path>) -> Reply {
    let mut spec: Value =
        serde_json::from_slice(&fs::read(manifest).map_err(|_| failure(2, "missing spec"))?)
            .map_err(|_| failure(2, "invalid spec"))?;
    let widths: Vec<u32> = spec["widths"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_u64().and_then(|n| u32::try_from(n).ok()))
        .filter(|&w| w > 0)
        .collect();
    if widths.is_empty() {
        return Err(failure(2, "positive widths required"));
    }

    let specs = collect_transition_specs(&spec, run_dir);
    let facts = analysis::analyze(project, manifest)?;
    analysis::apply_generated(&mut spec, &facts["autoMocks"]);

    let host = ViteHost.start(project, manifest, &facts)?;
    let exe = chromium::executable();
    if !exe.is_file() {
        return Err(failure(3, "chromium executable unavailable"));
    }
    let temp = tempfile::tempdir().map_err(|e| failure(3, &e.to_string()))?;
    let mut browser =
        Browser::local(&exe, temp.path(), host.port).map_err(|e| failure(3, &e.to_string()))?;

    let mut verdicts = Vec::with_capacity(specs.len());
    for transition in &specs {
        let mut observations = Vec::with_capacity(widths.len());
        for &width in &widths {
            let obs = browser
                .replay_transition(&spec, transition, width, temp.path())
                .map_err(|e| failure(3, &e.to_string()))?;
            observations.push(obs);
        }
        verdicts.push(judge_transition(transition, &observations));
    }

    if let Some(dir) = run_dir {
        update_run_graph(dir, &verdicts)?;
    }

    let summary = summarize_transitions(&verdicts);
    let mut body = json!({
        "summary": summary,
        "transitions": verdicts,
    });
    if let Some(dir) = run_dir {
        body["runDir"] = json!(dir.display().to_string());
    }
    Ok(body)
}
