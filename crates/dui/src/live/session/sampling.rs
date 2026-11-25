//! Adaptively probe a bounded integer interval on the current mounted screen.
use super::{Reply, Session, field, invalid, io};
use crate::live::{MAX_LAYOUT_SAMPLES, MAX_LAYOUT_SIZE};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn bounds(r: &Value, key: &str, limit: u64) -> Result<u64, (u8, Value)> {
    r[key]
        .as_u64()
        .filter(|n| (1..=limit).contains(n))
        .ok_or_else(|| invalid("invalid-sample-range"))
}

// Comparing sibling alignment, visibility and text lines avoids treating every fluid pixel as a breakpoint.
fn signature(tree: &Value) -> Value {
    let mut previous = BTreeMap::new();
    let mut items = Vec::new();
    for node in tree.as_array().into_iter().flatten() {
        let rect = &node["rect"];
        let parent = node["attributes"]["data-dui-dom-parent"]
            .as_str()
            .or(node["parentId"].as_str());
        let prior = previous.insert(parent, rect);
        items.push(json!([
            node["id"],
            node["textLines"],
            prior.map(|s| s["y"] == rect["y"]),
            prior.map(|s| s["x"] == rect["x"])
        ]));
    }
    json!(items)
}

impl Session {
    pub(super) fn sample_layout(&mut self, r: &Value) -> Reply {
        let axis = field(r, "axis")?;
        if !matches!(axis, "width" | "height") {
            return Err(invalid("invalid-sample-axis"));
        }
        let min = bounds(r, "min", MAX_LAYOUT_SIZE)?;
        let max = bounds(r, "max", MAX_LAYOUT_SIZE)?;
        let budget = bounds(r, "budget", MAX_LAYOUT_SAMPLES)?;
        if min >= max || budget < 2 {
            return Err(invalid("invalid-sample-range"));
        }
        let original = self.layout(None)?;
        if r.get("container").is_none()
            && original[axis].as_u64().is_none_or(|n| n > MAX_LAYOUT_SIZE)
        {
            return Err(invalid("original-viewport-outside-sample-range"));
        }
        let mut change = json!({});
        let saved = if let Some(container) = r.get("container") {
            let id = container
                .as_u64()
                .ok_or_else(|| invalid("invalid-container"))?;
            change["container"] = json!(id);
            let saved = self.eval_web(&format!(
                "globalThis.__duiRuntime.containerStyle({id},{})",
                json!(axis)
            ))?;
            if saved.is_null() {
                return Err(invalid("container-unavailable"));
            }
            Some(saved)
        } else {
            None
        };
        let mut observations = BTreeMap::new();
        let result = (|| -> Reply {
            let mut size = min;
            loop {
                change[axis] = json!(size);
                let observed = self.layout(Some(&change))?;
                // Include measured geometry at each probe without serializing form contents.
                let geometry:Vec<_>=observed["renderTree"].as_array().into_iter().flatten().map(|node|json!({"id":node["id"],"rect":node["rect"],"textLines":node["textLines"]})).collect();
                let sample = json!({"size":size,"viewport":{"width":observed["width"],"height":observed["height"]},"containers":observed["containers"],
                    "scrollWidth":observed["scrollWidth"],"scrollHeight":observed["scrollHeight"],"elements":geometry});
                observations.insert(size, (signature(&observed["renderTree"]), sample));
                if observations.len() == 1 {
                    size = max;
                    continue;
                }
                if observations.len() >= budget as usize {
                    break;
                }
                // Refine observed changes first; spend the remaining budget exploring wider gaps.
                let next = observations
                    .iter()
                    .zip(observations.iter().skip(1))
                    .filter(|((a, _), (b, _))| *b - *a > 1)
                    .max_by_key(|((a, left), (b, right))| (left.0 != right.0, *b - *a))
                    .map(|((a, _), (b, _))| a + (b - a) / 2);
                let Some(mid) = next else { break };
                size = mid;
            }
            let mut transitions = Vec::new();
            let mut gaps = Vec::new();
            for ((a, left), (b, right)) in observations.iter().zip(observations.iter().skip(1)) {
                if b - a > 1 {
                    gaps.push([a + 1, b - 1]);
                }
                if left.0 != right.0 {
                    transitions.push([*a, *b]);
                }
            }
            Ok(
                json!({"session":self.id,"sourceCell":self.cell,"path":original["path"],"renderer":self.environment["renderer"],
                "axis":axis,"container":r["container"],"min":min,"max":max,"samples":observations.values().map(|(_,v)|v).collect::<Vec<_>>(),
                "observedChanges":transitions,"unresolvedIntervals":gaps}),
            )
        })();
        if let Some(style) = saved {
            let restored = self.eval_web(&format!(
                "globalThis.__duiRuntime.restoreContainerStyle({style})"
            ))?;
            if restored != true {
                return Err(io("could-not-restore-sampled-container"));
            }
            self.layout(None)?;
        } else {
            change[axis] = original[axis].clone();
            self.layout(Some(&change))
                .map_err(|_| io("could-not-restore-sampled-viewport"))?;
        }
        result
    }
}
