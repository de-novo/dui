use crate::{Reply, failure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub fn with_cells(mut graph: Value, report: &Value) -> Reply {
    let cells = report["cells"]
        .as_array()
        .ok_or_else(|| failure(3, "missing cells"))?;
    let mut extra = Vec::new();
    let mut links = Vec::new();
    let mut known = BTreeSet::new();
    let screen_paths: BTreeSet<_> = cells.iter().filter_map(|c| c["path"].as_str()).collect();
    let mut navigation = BTreeMap::<(&str, &str), BTreeSet<u64>>::new();
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
        if report.get("screenDiscovery").is_some()
            && let Some(from) = cell["observation"]["path"].as_str()
            && screen_paths.contains(from)
            && let Some(width) = cell["width"].as_u64()
            && let Some(anchors) = cell["observation"]["links"].as_array()
        {
            for target in anchors.iter().filter_map(Value::as_str) {
                if from != target && screen_paths.contains(target) {
                    navigation.entry((from, target)).or_default().insert(width);
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
        links.push(json!({"source":format!("route:{from}"),"target":format!("route:{to}"),"kind":"linksTo","observedWidths":widths}));
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
pub fn write(dir: &Path, graph: &Value) -> Result<(), (u8, Value)> {
    let data = graph.to_string().replace('<', "\\u003c");
    fs::write(
        dir.join("graph.html"),
        include_str!("graph.html")
            .replace(
                "/*VIEW*/",
                include_str!(concat!(env!("OUT_DIR"), "/browser/graph.js")),
            )
            .replace("/*GRAPH*/", &data),
    )
    .map_err(|e| failure(3, &e.to_string()))
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
}
