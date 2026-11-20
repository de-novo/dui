//! Deterministic JSON examples from compiler-extracted response contracts.
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

fn sample(shape: &Value, field: &str, row: usize, budget: &mut usize) -> Option<Value> {
    *budget = budget.checked_sub(1)?;
    match shape["kind"].as_str()? {
        "literal" => Some(shape["value"].clone()),
        "number" => Some(json!(row)),
        "boolean" => Some(json!(true)),
        "string" => {
            let name = field.to_lowercase();
            let value = match name.as_str() {
                "email" => format!("person{row}@example.invalid"),
                "date" => format!("2026-01-{row:02}"),
                "image" | "imageurl" | "cover" | "coverurl" | "thumbnail" | "thumb" | "src" => {
                    let color = ["436c78", "c77753", "839968"][(row - 1) % 3];
                    format!(
                        "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 120 90'%3E%3Crect width='120' height='90' fill='%23{color}'/%3E%3Ctext x='12' y='66' font-size='42' fill='white'%3E{row}%3C/text%3E%3C/svg%3E"
                    )
                }
                "id" => format!("sample-{row}"),
                _ => format!("{field} {row}"),
            };
            Some(json!(value))
        }
        "union" => shape["members"]
            .as_array()?
            .iter()
            .filter(|m| m["kind"] != "literal" || !m["value"].is_null())
            .find_map(|member| sample(member, field, row, budget))
            .or_else(|| {
                shape["members"]
                    .as_array()?
                    .iter()
                    .any(|m| m["kind"] == "literal" && m["value"].is_null())
                    .then_some(Value::Null)
            }),
        "array" => (1..=3)
            .map(|i| sample(&shape["item"], field, i, budget))
            .collect::<Option<Vec<_>>>()
            .map(Value::Array),
        "object" => {
            let mut record = Map::new();
            for prop in shape["fields"].as_array()? {
                let name = prop["name"].as_str()?;
                record.insert(name.into(), sample(&prop["shape"], name, row, budget)?);
            }
            Some(Value::Object(record))
        }
        _ => None,
    }
}

pub fn responses(facts: &[Value]) -> Value {
    let mut paths = BTreeMap::new();
    for f in facts.iter().filter(|f| f["kind"] == "httpResponse") {
        if let Some(path) = f["path"].as_str() {
            let shape = paths.entry(path).or_insert(Some(&f["shape"]));
            if *shape != Some(&f["shape"]) {
                *shape = None;
            }
        }
    }
    let mut generated = Map::new();
    let mut unresolved = Vec::new();
    for (path, shape) in paths {
        if let Some(payload) = shape.and_then(|s| sample(s, "sample", 1, &mut 4096)) {
            generated.insert(path.into(), payload);
        } else {
            unresolved.push(path);
        }
    }
    json!({"generated":generated,"unresolved":unresolved})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflicting_or_unknown_contracts_never_replace_an_endpoint() {
        let string = json!({"kind":"string"});
        let number = json!({"kind":"number"});
        let facts = vec![
            json!({"kind":"httpResponse","path":"/one","shape":string}),
            json!({"kind":"httpResponse","path":"/one","shape":number}),
            json!({"kind":"httpResponse","path":"/two","shape":{"kind":"object","fields":[{"name":"required","shape":{"kind":"unknown"}}]}}),
            json!({"kind":"httpResponse","path":"/three","shape":{"kind":"array","item":number}}),
        ];
        let result = responses(&facts);
        assert_eq!(result["generated"], json!({"/three":[1,2,3]}));
        assert_eq!(result["unresolved"], json!(["/one", "/two"]));
    }
}
