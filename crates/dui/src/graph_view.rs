use crate::{Reply, failure};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};

pub fn with_cells(mut graph: Value, report: &Value) -> Reply {
    let cells = report["cells"]
        .as_array()
        .ok_or_else(|| failure(3, "missing cells"))?;
    let mut extra = Vec::new();
    let mut links = Vec::new();
    let mut known = BTreeSet::new();
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
            extra.push(json!({"id":route,"kind":"screen","label":path}));
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
