use crate::{Reply, failure};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn with_cells(mut graph: Value, report: &Value) -> Reply {
    let cells = report["cells"]
        .as_array()
        .ok_or_else(|| failure(3, "missing cells"))?;
    let mut extra = Vec::new();
    let mut links = Vec::new();
    let mut known: BTreeSet<String> = graph["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| n["id"].as_str().map(str::to_owned))
        .collect();
    let screen_paths: BTreeSet<_> = cells.iter().filter_map(|c| c["path"].as_str()).collect();
    let mut navigation = BTreeMap::<(&str, &str), BTreeSet<u64>>::new();
    let mut overlay_edges = BTreeMap::<(&str, &str), (BTreeSet<u64>, Option<String>)>::new();
    // Large sweeps share many identical branch traces. Keep the full membership once.
    let mut traces = std::collections::BTreeMap::<Vec<&str>, String>::new();
    for cell in cells {
        let state = format!(
            "state/{}/{}",
            cell["scenario"].as_str().unwrap_or(""),
            cell["state"].as_str().unwrap_or("")
        );
        let path = cell["path"].as_str().unwrap_or("/");
        let route = format!("route:{path}");
        if known.insert(route.clone()) {
            let mut screen = json!({"id":route,"kind":"screen","label":path});
            if let Some(source) = report["screenDiscovery"]["sources"].get(path) {
                screen["discoveredFrom"] = source.clone();
            }
            extra.push(screen);
        }
        if let Some(parent_path) = cell["overlay"]["parentPath"].as_str()
            && screen_paths.contains(parent_path)
            && parent_path != path
            && let Some(width) = cell["width"].as_u64()
        {
            let trigger_label = cell["overlay"]["triggerLabel"].as_str().map(str::to_owned);
            let entry = overlay_edges
                .entry((parent_path, path))
                .or_insert_with(|| (BTreeSet::new(), trigger_label.clone()));
            entry.0.insert(width);
            if entry.1.is_none() {
                entry.1 = trigger_label;
            }
        }
        if let Some(obs_path) = cell["observation"]["path"].as_str()
            && let Some(width) = cell["width"].as_u64()
            && let Some(anchors) = cell["observation"]["links"].as_array()
        {
            let from = if screen_paths.contains(obs_path) {
                Some(obs_path)
            } else {
                obs_path
                    .split('?')
                    .next()
                    .filter(|base| screen_paths.contains(*base))
            };
            if let Some(from) = from {
                for raw_target in anchors.iter().filter_map(Value::as_str) {
                    let target = if screen_paths.contains(raw_target) {
                        Some(raw_target)
                    } else {
                        raw_target
                            .split('?')
                            .next()
                            .filter(|base| screen_paths.contains(*base))
                    };
                    if let Some(target) = target
                        && from != target
                    {
                        navigation.entry((from, target)).or_default().insert(width);
                    }
                }
            }
        }
        if known.insert(state.clone()) {
            extra.push(json!({"id":state,"kind":"state","label":format!("{} · {}",cell["scenario"].as_str().unwrap_or(""),cell["planIndex"]),"input":cell["input"]}));
            links.push(json!({"source":route,"target":state,"kind":"plans"}));
        }
        let id = format!("{state}/{}", cell["width"]);
        extra.push(json!({"id":id,"kind":"cell","label":format!("{} px · {}",cell["width"],cell["status"].as_str().unwrap_or("")),"status":cell["status"],"reason":cell["reason"],"image":cell["image"]}));
        links.push(json!({"source":state,"target":id,"kind":"rendersAt"}));
        let mut hits: Vec<_> = cell["observation"]["hits"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        for hit in &hits {
            if known.insert((*hit).to_owned()) {
                extra.push(json!({
                    "id": hit,
                    "kind": "runtimeOutcome",
                    "label": hit,
                    "observed": true,
                    "runtimeOnly": true
                }));
            }
        }
        if !hits.is_empty() && cells.len() > 256 {
            hits.sort_unstable();
            hits.dedup();
            let idx = traces.len();
            let trace = traces.entry(hits.clone()).or_insert_with(|| {
                let trace = format!("trace/{idx}");
                extra.push(
                    json!({"id":trace,"kind":"trace","label":format!("실행 분기 {}개",hits.len())}),
                );
                links.extend(
                    hits.iter()
                        .map(|hit| json!({"source":trace,"target":hit,"kind":"executed"})),
                );
                trace
            });
            links.push(json!({"source":id,"target":trace,"kind":"observed"}));
        } else {
            links.extend(
                hits.iter()
                    .map(|hit| json!({"source":id,"target":hit,"kind":"executed"})),
            );
        }
    }
    for ((from, to), widths) in navigation {
        let mut edge = json!({
            "source": format!("route:{from}"),
            "target": format!("route:{to}"),
            "kind": "linksTo",
            "observedWidths": widths,
        });
        if let Some(source) = report["screenDiscovery"]["sources"].get(to)
            && source["from"].as_str() == Some(from)
        {
            if let Some(origin) = source.get("kind").filter(|v| v.is_string()) {
                edge["origin"] = origin.clone();
            }
            if let Some(selector) = source.get("selector").filter(|v| v.is_string()) {
                edge["selector"] = selector.clone();
            }
        }
        links.push(edge);
    }
    for ((from, to), (widths, trigger_label)) in overlay_edges {
        let mut edge = json!({
            "source": format!("route:{from}"),
            "target": format!("route:{to}"),
            "kind": "linksTo",
            "origin": "overlay",
            "provenance": "verified",
            "replayVerified": true,
            "observedWidths": widths,
            "confirmedWidths": widths,
        });
        if let Some(label) = trigger_label {
            edge["action"] = json!({ "kind": "overlay", "selector": "", "label": label });
        }
        links.push(edge);
    }
    let nodes = graph["nodes"]
        .as_array_mut()
        .ok_or_else(|| failure(3, "missing nodes"))?;
    nodes.extend(extra);
    let ids: BTreeSet<_> = nodes.iter().map(|n| n["id"].to_string()).collect();
    if ids.len() != nodes.len() {
        return Err(failure(3, "duplicate graph node"));
    }
    let edges = graph["edges"]
        .as_array_mut()
        .ok_or_else(|| failure(3, "missing edges"))?;
    edges.extend(links);
    if edges
        .iter()
        .any(|e| !ids.contains(&e["source"].to_string()) || !ids.contains(&e["target"].to_string()))
    {
        return Err(failure(3, "unresolved graph endpoint"));
    }
    graph["overview"] = Value::Array(cells.iter().filter(|c| c["planIndex"] == 0).map(|c| {
        json!({"path":c["path"], "width":c["width"], "image":c["image"], "status":c["status"], "reason":c["reason"]})
    }).collect());
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_between_known_screens_use_observed_urls_and_widths() {
        let cell = |path, actual, width, links| {
            json!({
                "path": path, "scenario": path, "state": path, "width": width, "planIndex": 0,
                "observation": {"path": actual, "links": links}
            })
        };
        let report = json!({"screenDiscovery":{"mode":"router"},"cells":[
            cell("/", "/", 375, vec!["/a","/a","/","/absent"]),
            cell("/", "/", 1280, vec!["/a"]),
            cell("/a", "/a", 375, vec!["/"]),
            cell("/a", "/redirect", 1280, vec!["/"]),
        ]});
        let graph = with_cells(json!({"nodes":[],"edges":[]}), &report).unwrap();
        let links: Vec<_> = graph["edges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|edge| edge["kind"] == "linksTo")
            .collect();
        assert_eq!(links.len(), 2);
        assert_eq!(
            links[0],
            &json!({"kind":"linksTo","source":"route:/","target":"route:/a","observedWidths":[375,1280]})
        );
        assert_eq!(
            links[1],
            &json!({"kind":"linksTo","source":"route:/a","target":"route:/","observedWidths":[375]})
        );
    }

    #[test]
    fn links_carry_discovery_origin_and_selector_evidence_when_recorded() {
        let report = json!({
            "screenDiscovery": {
                "mode": "links",
                "sources": {
                    "/?q=demo": {"kind": "entry"},
                    "/": {"kind": "anchor", "from": "/?q=demo", "selector": "header a[href]"}
                }
            },
            "cells": [
                {"path": "/?q=demo", "scenario": "default", "state": "s0", "width": 375, "planIndex": 0, "status": "verified", "observation": {"path": "/?q=demo", "links": ["/"]}},
                {"path": "/?q=demo", "scenario": "default", "state": "s0", "width": 1280, "planIndex": 0, "status": "verified", "observation": {"path": "/?q=demo", "links": ["/"]}},
                {"path": "/", "scenario": "link-0", "state": "s1", "width": 375, "planIndex": 0, "status": "verified", "observation": {"path": "/", "links": []}},
                {"path": "/", "scenario": "link-0", "state": "s1", "width": 1280, "planIndex": 0, "status": "verified", "observation": {"path": "/", "links": []}}
            ]
        });
        let graph = with_cells(json!({"nodes":[],"edges":[]}), &report).unwrap();
        let links: Vec<_> = graph["edges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|edge| edge["kind"] == "linksTo")
            .collect();
        assert_eq!(
            links,
            vec![&json!({
                "kind": "linksTo",
                "source": "route:/?q=demo",
                "target": "route:/",
                "observedWidths": [375, 1280],
                "origin": "anchor",
                "selector": "header a[href]"
            })]
        );
    }

    #[test]
    fn run_without_screen_discovery_still_emits_links_to_edges_and_matches_normalized_queries() {
        let report = json!({
            "cells": [
                {
                    "path": "/",
                    "scenario": "default",
                    "state": "s0",
                    "width": 1280,
                    "planIndex": 0,
                    "status": "verified",
                    "observation": {
                        "path": "/?project=prj_1",
                        "links": ["/settings?project=prj_1", "/unknown"]
                    }
                },
                {
                    "path": "/settings",
                    "scenario": "case-1",
                    "state": "s1",
                    "width": 1280,
                    "planIndex": 0,
                    "status": "verified",
                    "observation": {
                        "path": "/settings",
                        "links": ["/"]
                    }
                }
            ]
        });
        let graph = with_cells(json!({"nodes":[],"edges":[]}), &report).unwrap();
        let links: Vec<_> = graph["edges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|edge| edge["kind"] == "linksTo")
            .collect();
        assert_eq!(
            links,
            vec![
                &json!({"kind":"linksTo","source":"route:/","target":"route:/settings","observedWidths":[1280]}),
                &json!({"kind":"linksTo","source":"route:/settings","target":"route:/","observedWidths":[1280]}),
            ]
        );
    }
}
