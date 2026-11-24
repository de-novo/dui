//! Resolve a captured state observation within one immutable cell.
use crate::discovery::key;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub state: String,
    pub width: u32,
    pub slot: Option<String>,
    pub instance: Option<String>,
    // Presence matters: JSON null is a proposed value too.
    #[serde(default, deserialize_with = "present")]
    pub value: Option<Value>,
}

fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

fn supported(shape: &Value) -> bool {
    match shape["kind"].as_str() {
        Some("boolean" | "literal") => true,
        Some("union") => shape["members"]
            .as_array()
            .is_some_and(|m| !m.is_empty() && m.iter().all(supported)),
        _ => false,
    }
}

fn accepts(shape: &Value, value: &Value) -> bool {
    match shape["kind"].as_str() {
        Some("boolean") => value.is_boolean(),
        Some("literal") => shape.get("value") == Some(value),
        Some("union") => shape["members"]
            .as_array()
            .is_some_and(|m| m.iter().any(|s| accepts(s, value))),
        _ => false,
    }
}

fn collect_options(shape: &Value, out: &mut Vec<Value>) -> bool {
    match shape["kind"].as_str() {
        Some("boolean") => {
            for value in [Value::Bool(false), Value::Bool(true)] {
                if !out.contains(&value) {
                    out.push(value);
                }
            }
            true
        }
        Some("literal") => {
            let Some(value) = shape.get("value") else {
                return false;
            };
            if !out.contains(value) {
                out.push(value.clone());
            }
            true
        }
        Some("union") => shape["members"].as_array().is_some_and(|members| {
            !members.is_empty() && members.iter().all(|m| collect_options(m, out))
        }),
        _ => false,
    }
}

pub fn inspect(facts: &[Value], cells: &[Value], query: &Selection) -> Result<Value, &'static str> {
    if query.slot.is_none() && (query.instance.is_some() || query.value.is_some()) {
        return Err("slot-required");
    }
    let mut matches = cells
        .iter()
        .filter(|c| c["state"] == query.state && c["width"] == query.width);
    let cell = matches.next().ok_or("cell-not-found")?;
    if matches.next().is_some() {
        return Err("ambiguous-cell");
    }
    let mut reply = json!({
        "state": query.state, "width": query.width, "observation": "archive", "controls": [],
        "status": cell["status"], "reason": cell["reason"], "captured": cell["image"].is_string()
    });
    if let Some(evidence) = cell["observation"].get("screenEvidence") {
        reply["screenEvidence"] = evidence.clone();
    }
    if query.slot.is_none() && !cell["observation"]["instances"].is_object() {
        reply["controlsAvailable"] = Value::Bool(false);
        return Ok(reply);
    }
    let instances = cell["observation"]["instances"]
        .as_object()
        .ok_or("instance-evidence-unavailable")?;
    let mut controls = Vec::new();
    for fact in facts.iter().filter(|f| f["kind"] == "slot") {
        let id = key(fact);
        if query.slot.as_ref().is_some_and(|s| s != &id) {
            continue;
        }
        let entries = instances.get(&id).and_then(Value::as_object);
        let mut add = |instance: Option<&String>, entry: &Value| {
            let reason = if instance.is_none() {
                Some("not-observed")
            } else if !supported(&fact["shape"]) {
                Some("unsupported-type")
            } else if !entry
                .get("value")
                .is_some_and(|v| accepts(&fact["shape"], v))
            {
                Some("value-unconfirmed")
            } else if entry["setter"] != true {
                Some("no-setter")
            } else {
                None
            };
            let mut control = json!({"slot":id,"owner":fact["owner"],"instance":instance,"source":{"file":fact["file"],"start":fact["start"],"end":fact["end"]},"label":fact["label"],"type":fact["type"],"editable":reason.is_none(),"reason":reason});
            let mut allowed = Vec::new();
            if collect_options(&fact["shape"], &mut allowed) {
                control["options"] = json!(allowed);
            }
            if let Some(v) = entry.get("value") {
                control["value"] = v.clone();
            }
            controls.push(control);
        };
        if let Some(entries) = entries.filter(|e| !e.is_empty()) {
            if query.slot.is_some() && query.instance.is_none() && entries.len() > 1 {
                return Err("ambiguous-instance");
            }
            for (instance, entry) in entries {
                if query.instance.as_ref().is_none_or(|i| i == instance) {
                    add(Some(instance), entry);
                }
            }
        } else if query.instance.is_none() {
            add(None, &Value::Null);
        }
        if let Some(value) = &query.value {
            let control = controls.last().ok_or("instance-not-found")?;
            if control["editable"] != true {
                return Err("read-only");
            }
            if !accepts(&fact["shape"], value) {
                return Err("invalid-value");
            }
        }
    }
    if query.slot.is_some() && controls.is_empty() {
        return Err("slot-or-instance-not-found");
    }
    reply["controls"] = json!(controls);
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_typed_options_for_boolean_and_literal_union_controls() {
        let facts = vec![
            json!({"kind":"slot","file":"app.tsx","start":1,"end":2,"owner":"Menu","label":"open","type":"boolean","shape":{"kind":"boolean"}}),
            json!({"kind":"slot","file":"app.tsx","start":3,"end":4,"owner":"Menu","label":"mode","type":"\"view\" | \"edit\"","shape":{"kind":"union","members":[{"kind":"literal","value":"view"},{"kind":"literal","value":"edit"}]}}),
            json!({"kind":"slot","file":"app.tsx","start":5,"end":6,"owner":"Menu","label":"meta","type":"unknown","shape":{"kind":"unknown"}}),
        ];
        let cells = vec![json!({
            "state": "s1",
            "width": 1280,
            "observation": {
                "instances": {
                    key(&facts[0]): { "i1": { "value": false, "setter": true } },
                    key(&facts[1]): { "i1": { "value": "view", "setter": true } },
                    key(&facts[2]): { "i1": { "value": null, "setter": false } },
                }
            }
        })];
        let query = Selection {
            state: "s1".into(),
            width: 1280,
            slot: None,
            instance: None,
            value: None,
        };
        let result = inspect(&facts, &cells, &query).unwrap();
        let controls = result["controls"].as_array().unwrap();
        assert_eq!(controls[0]["options"], json!([false, true]));
        assert_eq!(controls[1]["options"], json!(["view", "edit"]));
        assert!(controls[2].get("options").is_none());
    }
}
