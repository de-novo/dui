//! Explicit runtime inputs for source-model cards. These values are reviewer-supplied
//! fixtures, not source facts: the frozen model stays unchanged and the archive stores the
//! inputs beside it for the viewer to pass through its renderer boundary.
use crate::failure;
use dui_core::source_model::SourceModel;
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub(crate) const ARCHIVE_FILE: &str = "source-inputs.json";

fn invalid(reason: impl AsRef<str>) -> (u8, Value) {
    failure(2, &format!("invalid sourceInputs: {}", reason.as_ref()))
}

fn object<'a>(value: &'a Value, context: &str) -> Result<&'a Map<String, Value>, (u8, Value)> {
    value
        .as_object()
        .ok_or_else(|| invalid(format!("{context} must be an object")))
}

fn reject_unknown(
    object: &Map<String, Value>,
    allowed: &[&str],
    context: &str,
) -> Result<(), (u8, Value)> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(invalid(format!("unknown field {context}.{key}")));
        }
    }
    Ok(())
}

fn normalized(value: &Value) -> Result<Value, (u8, Value)> {
    let root = object(value, "sourceInputs")?;
    reject_unknown(root, &["defaultState", "states"], "sourceInputs")?;
    let states_value = root
        .get("states")
        .ok_or_else(|| invalid("states is required"))?;
    let states_object = object(states_value, "sourceInputs.states")?;
    if states_object.is_empty() {
        return Err(invalid("states must name at least one state"));
    }
    let default_state = match root.get("defaultState") {
        None => None,
        Some(value) => {
            let state = value
                .as_str()
                .filter(|state| !state.is_empty())
                .ok_or_else(|| invalid("defaultState must be a non-empty string"))?;
            if !states_object.contains_key(state) {
                return Err(invalid(format!("defaultState {state} has no state entry")));
            }
            Some(state.to_owned())
        }
    };
    let mut states = Map::new();
    for (state, raw) in states_object {
        if state.is_empty() {
            return Err(invalid("state names must be non-empty"));
        }
        let entry = object(raw, &format!("sourceInputs.states.{state}"))?;
        reject_unknown(
            entry,
            &["slots", "expressions"],
            &format!("sourceInputs.states.{state}"),
        )?;
        if !entry.contains_key("slots") && !entry.contains_key("expressions") {
            return Err(invalid(format!(
                "state {state} must define slots or expressions"
            )));
        }
        let mut clean = Map::new();
        for field in ["slots", "expressions"] {
            if let Some(raw_values) = entry.get(field) {
                let values = object(raw_values, &format!("sourceInputs.states.{state}.{field}"))?;
                for key in values.keys() {
                    if key.is_empty() {
                        return Err(invalid(format!("state {state} has an empty {field} key")));
                    }
                }
                clean.insert(field.into(), raw_values.clone());
            }
        }
        states.insert(state.clone(), Value::Object(clean));
    }
    let mut clean = Map::new();
    if let Some(state) = default_state {
        clean.insert("defaultState".into(), json!(state));
    }
    clean.insert("states".into(), Value::Object(states));
    Ok(Value::Object(clean))
}

fn collect_opaque(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if object.get("kind").and_then(Value::as_str) == Some("opaque")
                && let Some(text) = object.get("text").and_then(Value::as_str)
            {
                out.insert(text.to_owned());
            }
            for child in object.values() {
                collect_opaque(child, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_opaque(child, out);
            }
        }
        _ => {}
    }
}

fn validate_references(model: &SourceModel, inputs: &Value) -> Result<(), (u8, Value)> {
    let slots: BTreeSet<&str> = model
        .components
        .iter()
        .flat_map(|component| component.slots.iter().map(|slot| slot.id.as_str()))
        .collect();
    let model_json = serde_json::to_value(model).map_err(|e| failure(3, &e.to_string()))?;
    let mut opaque = BTreeSet::new();
    collect_opaque(&model_json, &mut opaque);
    let mut unknown_slots = Vec::new();
    let mut unknown_expressions = Vec::new();
    for (state, entry) in inputs["states"]
        .as_object()
        .map(|states| states.iter().collect::<Vec<_>>())
        .unwrap_or_default()
    {
        for key in entry["slots"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|v| v.0)
        {
            if !slots.contains(key.as_str()) {
                unknown_slots.push(format!("{state}:{key}"));
            }
        }
        for key in entry["expressions"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|v| v.0)
        {
            if !opaque.contains(key.as_str()) {
                unknown_expressions.push(format!("{state}:{key}"));
            }
        }
    }
    if !unknown_slots.is_empty() {
        unknown_slots.sort();
        unknown_slots.truncate(8);
        return Err(invalid(format!(
            "slot keys do not exist in the extracted model: {}",
            unknown_slots.join(", ")
        )));
    }
    if !unknown_expressions.is_empty() {
        unknown_expressions.sort();
        unknown_expressions.truncate(8);
        return Err(invalid(format!(
            "expression keys do not exist as opaque source in the extracted model: {}",
            unknown_expressions.join(", ")
        )));
    }
    Ok(())
}

fn input_file(project: &Path, raw: &str) -> Result<PathBuf, (u8, Value)> {
    if raw.is_empty() || raw.starts_with('/') || raw.contains('\\') {
        return Err(invalid("file path must be relative to the project"));
    }
    let root = project
        .canonicalize()
        .map_err(|e| failure(2, &format!("cannot resolve project: {e}")))?;
    let path = root
        .join(raw)
        .canonicalize()
        .map_err(|e| invalid(format!("{raw}: {e}")))?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err(invalid("file path must stay inside the project"));
    }
    Ok(path)
}

/// Read, normalize, and validate the project-supplied source renderer inputs for one
/// extraction. A malformed or unmatched input is a CLI error rather than a silent placeholder
/// because the value was explicit reviewer evidence.
pub(crate) fn configured(
    project: &Path,
    model: &SourceModel,
) -> Result<Option<Value>, (u8, Value)> {
    let manifest_path = project.join("dui.json");
    let Ok(manifest_bytes) = fs::read(&manifest_path) else {
        return Ok(None);
    };
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| failure(2, &format!("invalid dui.json: {e}")))?;
    let Some(configured) = manifest.get("sourceInputs") else {
        return Ok(None);
    };
    let raw = if let Some(file) = configured.as_str() {
        let path = input_file(project, file)?;
        serde_json::from_slice(&fs::read(&path).map_err(|e| failure(2, &e.to_string()))?)
            .map_err(|e| invalid(format!("{}: {e}", path.display())))?
    } else {
        configured.clone()
    };
    let normalized = normalized(&raw)?;
    validate_references(model, &normalized)?;
    Ok(Some(normalized))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_rejects_typos_and_bad_default_states() {
        assert!(normalized(&json!({"states":{"populated":{"slots":{"a":1}}}})).is_ok());
        assert!(normalized(&json!({"states":{"populated":{"slot":{"a":1}}}})).is_err());
        assert!(
            normalized(&json!({"defaultState":"missing","states":{"populated":{"slots":{"a":1}}}}))
                .is_err()
        );
    }
}
