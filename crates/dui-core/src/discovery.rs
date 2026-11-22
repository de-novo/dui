use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FillTermination {
    Exhausted,
    BudgetLimit,
    NoImprovement,
}

pub fn key(f: &Value) -> String {
    format!(
        "{}#{}:{}:{}",
        f["file"].as_str().unwrap_or(""),
        f["start"],
        f["end"],
        f["kind"].as_str().unwrap_or("")
    )
}

pub fn representatives(shape: &Value) -> Vec<Value> {
    match shape["kind"].as_str() {
        Some("literal") => vec![shape["value"].clone()],
        Some("boolean") => vec![json!(false), json!(true)],
        Some("string") => vec![json!(""), json!("sample"), json!("x".repeat(128))],
        Some("number") => vec![json!(-1), json!(0), json!(1)],
        Some("array") => {
            let mut options = vec![json!([])];
            if let Some(item) = representatives(&shape["item"]).first() {
                options.push(json!([item]));
                options.push(json!([item, item, item]));
            }
            options
        }
        Some("union") => shape["members"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(representatives)
            .collect(),
        _ => vec![],
    }
}

// Each row covers either one value or one pair. Independent owners are planned separately.
pub fn candidates(facts: &[Value], active: &BTreeSet<String>) -> Vec<BTreeMap<String, Value>> {
    let mut groups = BTreeMap::new();
    for f in facts.iter().filter(|f| {
        f["kind"] == "slot"
            && active.contains(&key(f))
            && !f["init"].is_null()
            && !f["after"].is_null()
    }) {
        groups
            .entry((f["file"].to_string(), f["owner"].to_string()))
            .or_insert_with(Vec::new)
            .push((key(f), representatives(&f["shape"])));
    }
    let mut singles = Vec::new();
    let mut pairs = Vec::new();
    for slots in groups.values_mut() {
        slots.sort_by(|a, b| a.0.cmp(&b.0));
        for (i, (key, values)) in slots.iter().enumerate() {
            for v in values {
                singles.push(BTreeMap::from([(key.clone(), v.clone())]));
                for (other, options) in &slots[..i] {
                    for option in options {
                        pairs.push(BTreeMap::from([
                            (key.clone(), v.clone()),
                            (other.clone(), option.clone()),
                        ]));
                    }
                }
            }
        }
    }
    singles.extend(pairs);
    singles
}

fn value_to_query_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn literal_values_from_shape(shape: &Value, out: &mut BTreeSet<String>) {
    match shape["kind"].as_str() {
        Some("literal") => {
            if let Some(s) = value_to_query_string(&shape["value"]) {
                out.insert(s);
            }
        }
        Some("boolean") => {
            out.insert("false".into());
            out.insert("true".into());
        }
        Some("union") => {
            for m in shape["members"].as_array().into_iter().flatten() {
                literal_values_from_shape(m, out);
            }
        }
        _ => {}
    }
}

/// Framework-agnostic deterministic URL query parameter state planner.
/// Extracts candidate values from `searchParam` compiler facts (both typed unions/booleans
/// and literal equality predicates bound to the same `URLSearchParams.get(...)` declaration).
pub fn query_candidates(facts: &[Value]) -> Vec<BTreeMap<String, String>> {
    let mut by_name: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for f in facts.iter().filter(|f| f["kind"] == "searchParam") {
        let Some(name) = f["label"].as_str().filter(|s| !s.is_empty()) else {
            continue;
        };
        let values = by_name.entry(name.to_owned()).or_default();
        if f["presence"] == json!(true) {
            values.insert("1".into());
            continue;
        }
        literal_values_from_shape(&f["shape"], values);
        // Bind untyped `URLSearchParams.get("key")` variables to branch equality predicates.
        if f["binding"].is_number() {
            for b in facts.iter().filter(|b| {
                b["kind"] == "branch"
                    && b["file"] == f["file"]
                    && b["owner"] == f["owner"]
                    && b["predicate"]["binding"] == f["binding"]
            }) {
                if matches!(
                    b["predicate"]["operator"].as_str(),
                    Some("equals" | "eq" | "neq")
                ) && let Some(s) = value_to_query_string(&b["predicate"]["value"])
                {
                    values.insert(s);
                }
            }
        }
    }

    let params: Vec<(String, Vec<String>)> = by_name
        .into_iter()
        .filter(|(_, vals)| !vals.is_empty())
        .map(|(k, vals)| (k, vals.into_iter().collect()))
        .collect();

    let mut singles = Vec::new();
    let mut pairs = Vec::new();
    for (i, (key, vals)) in params.iter().enumerate() {
        for v in vals {
            singles.push(BTreeMap::from([(key.clone(), v.clone())]));
            for (other_k, other_vals) in &params[..i] {
                for ov in other_vals {
                    pairs.push(BTreeMap::from([
                        (key.clone(), v.clone()),
                        (other_k.clone(), ov.clone()),
                    ]));
                }
            }
        }
    }
    singles.extend(pairs);
    singles
}

/// Build a canonical URL path by merging sorted query parameters onto `base_path`.
pub fn with_query(base_path: &str, query: &BTreeMap<String, String>) -> String {
    if query.is_empty() {
        return base_path.to_owned();
    }
    let (pathname, mut merged) = crate::verification::parse_url_parts(base_path);
    for (k, v) in query {
        merged.insert(k.clone(), v.clone());
    }
    let qs = merged
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{pathname}?{qs}")
}

pub fn graph(facts: &[Value], hits: &BTreeSet<String>) -> Value {
    let mut nodes = BTreeMap::new();
    let mut edges = BTreeSet::new();
    for f in facts {
        let owner = format!(
            "{}::{}",
            f["file"].as_str().unwrap_or(""),
            f["owner"].as_str().unwrap_or("")
        );
        nodes.insert(
            owner.clone(),
            json!({"id":owner,"kind":"function","label":f["owner"],"file":f["file"]}),
        );
        let k = key(f);
        if f["kind"] == "renders" {
            let target = format!(
                "{}::{}",
                f["target"].as_str().unwrap_or(""),
                f["label"].as_str().unwrap_or("")
            );
            nodes.entry(target.clone()).or_insert_with(
                || json!({"id":target,"kind":"function","label":f["label"],"file":f["target"]}),
            );
            edges.insert((owner, target, "renders"));
            continue;
        }
        nodes.insert(
            k.clone(),
            json!({"id":k,"kind":f["kind"],"label":f["label"],"file":f["file"],"line":f["line"]}),
        );
        edges.insert((owner, k.clone(), "contains"));
        if f["kind"] == "branch" {
            for truth in [false, true] {
                let id = format!("{k}:{truth}");
                nodes.insert(id.clone(),json!({"id":id,"kind":"outcome","label":truth.to_string(),"observed":hits.contains(&id)}));
                edges.insert((k.clone(), id, "outcome"));
            }
        }
    }
    json!({"nodes":nodes.into_values().collect::<Vec<_>>(),"edges":edges.into_iter().map(|(source,target,kind)|json!({"source":source,"target":target,"kind":kind})).collect::<Vec<_>>()})
}

// Bind simple predicates to the compiler-discovered slot in the same declaration.
pub fn assertions(facts: &[Value], row: &BTreeMap<String, Value>, spec: &Value) -> Value {
    let mut checks = Vec::new();
    for (id, value) in row {
        let Some(slot) = facts.iter().find(|f| f["kind"] == "slot" && key(f) == *id) else {
            continue;
        };
        let mut matching_branches: Vec<&Value> = facts
            .iter()
            .filter(|f| {
                f["kind"] == "branch"
                    && f["owner"] == slot["owner"]
                    && f["file"] == slot["file"]
                    && slot["binding"].is_number()
                    && f["predicate"]["binding"] == slot["binding"]
            })
            .collect();
        matching_branches.sort_by_key(|b| key(b));
        for branch in matching_branches {
            let predicate = &branch["predicate"];
            let truth = match predicate["operator"].as_str() {
                Some("equals") => value == &predicate["value"],
                Some("truthy") if value.is_boolean() => value == true,
                _ => continue,
            };
            let (shown, hidden) = if truth {
                ("textWhenTrue", "textWhenFalse")
            } else {
                ("textWhenFalse", "textWhenTrue")
            };
            checks.push(json!({"slot":id,"hit":format!("{}:{truth}",key(branch)),"shown":branch[shown],"hidden":if branch[shown]!=branch[hidden]{&branch[hidden]}else{&Value::Null}}));
        }
        for rule in spec["slotAssertions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| {
                r["file"] == slot["file"]
                    && r["owner"] == slot["owner"]
                    && r["label"] == slot["label"]
                    && &r["value"] == value
            })
        {
            let mut check = json!({"slot":id,"selector":rule["selector"],"present":rule.get("present").cloned().unwrap_or(json!(true))});
            if let Some(mask) = rule.get("absentWhen") {
                let mut related = facts.iter().filter(|other| {
                    other["kind"] == "slot"
                        && other["owner"] == slot["owner"]
                        && other["file"] == slot["file"]
                        && other["label"] == mask["slot"]
                });
                let source = related.next();
                // A missing or ambiguous reference must fail its assertion at capture.
                let mask_id = source.filter(|_| related.next().is_none()).map(key);
                check["absentWhen"] = json!({"slot":mask_id,"value":mask["value"]});
            }
            checks.push(check);
        }
    }
    json!(checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(at: u32, owner: &str) -> Value {
        json!({"file":"screen.tsx","owner":owner,"start":at,"end":at+1,"kind":"slot","init":[at,at+1],"after":at+2,"shape":{"kind":"boolean"}})
    }

    #[test]
    fn pairwise_covers_each_pair_without_crossing_owner_boundaries() {
        let facts = [slot(2, "List"), slot(6, "List"), slot(9, "Menu")];
        let active = facts.iter().map(key).collect();
        let rows = candidates(&facts, &active);
        assert_eq!(rows, candidates(&facts, &active));
        let (a, b, c) = (key(&facts[0]), key(&facts[1]), key(&facts[2]));
        for x in [false, true] {
            for y in [false, true] {
                assert!(
                    rows.iter()
                        .any(|r| r.get(&a) == Some(&json!(x)) && r.get(&b) == Some(&json!(y)))
                );
            }
        }
        assert!(rows.iter().all(|r| !r.contains_key(&c) || r.len() == 1));
    }

    #[test]
    fn nested_facts_at_same_start_have_distinct_ids() {
        let mut inner = slot(4, "Grid");
        let outer = inner.clone();
        inner["end"] = json!(10);
        assert_ne!(key(&outer), key(&inner));
    }

    #[test]
    fn predicates_bind_to_the_compiler_declaration() {
        let mut s = slot(2, "Grid");
        s["binding"] = json!(23);
        let mut branch = json!({"file":"screen.tsx","owner":"Grid","start":30,"end":44,"kind":"branch","predicate":{"binding":24,"operator":"truthy"}});
        let row = BTreeMap::from([(key(&s), json!(false))]);
        assert_eq!(
            assertions(&[s.clone(), branch.clone()], &row, &json!({})),
            json!([])
        );
        branch["predicate"]["binding"] = json!(23);
        assert_eq!(
            assertions(&[s, branch], &row, &json!({}))[0]["hit"],
            "screen.tsx#30:44:branch:false"
        );
    }

    #[test]
    fn candidates_graph_and_assertions_are_invariant_to_fact_order() {
        let mut a = slot(2, "List");
        a["binding"] = json!(10);
        let mut b = slot(6, "List");
        b["binding"] = json!(11);
        let br1 = json!({"file":"screen.tsx","owner":"List","start":20,"end":30,"kind":"branch","predicate":{"binding":10,"operator":"truthy"},"textWhenTrue":"On","textWhenFalse":"Off"});
        let br2 = json!({"file":"screen.tsx","owner":"List","start":40,"end":50,"kind":"branch","predicate":{"binding":10,"operator":"truthy"},"textWhenTrue":"Open","textWhenFalse":"Closed"});
        let forward = vec![a.clone(), b.clone(), br1.clone(), br2.clone()];
        let reversed: Vec<Value> = forward.iter().rev().cloned().collect();
        let active: BTreeSet<String> = [key(&a), key(&b)].into_iter().collect();
        let hits = BTreeSet::from([format!("{}:true", key(&br1))]);
        assert_eq!(
            candidates(&forward, &active),
            candidates(&reversed, &active)
        );
        assert_eq!(graph(&forward, &hits), graph(&reversed, &hits));
        let row = BTreeMap::from([(key(&a), json!(true))]);
        assert_eq!(
            assertions(&forward, &row, &json!({})),
            assertions(&reversed, &row, &json!({}))
        );
    }

    #[test]
    fn query_candidates_plan_standard_url_search_params_and_typed_unions() {
        let facts = vec![
            json!({
                "file": "app.tsx",
                "owner": "App",
                "start": 10,
                "end": 40,
                "kind": "searchParam",
                "label": "tab",
                "presence": false,
                "binding": 10,
                "shape": {"kind": "string"}
            }),
            json!({
                "file": "app.tsx",
                "owner": "App",
                "start": 50,
                "end": 80,
                "kind": "searchParam",
                "label": "compact",
                "presence": true,
                "binding": 50,
                "shape": {"kind": "boolean"}
            }),
            json!({
                "file": "app.tsx",
                "owner": "App",
                "start": 100,
                "end": 120,
                "kind": "branch",
                "predicate": {"operator": "equals", "binding": 10, "value": "details"}
            }),
            json!({
                "file": "app.tsx",
                "owner": "App",
                "start": 130,
                "end": 150,
                "kind": "branch",
                "predicate": {"operator": "equals", "binding": 10, "value": "billing"}
            }),
        ];
        let planned = query_candidates(&facts);
        let urls: Vec<String> = planned.iter().map(|q| with_query("/", q)).collect();
        assert!(urls.contains(&"/?tab=details".to_owned()));
        assert!(urls.contains(&"/?tab=billing".to_owned()));
        assert!(urls.contains(&"/?compact=1".to_owned()));
        assert!(urls.contains(&"/?compact=1&tab=details".to_owned()));
    }
}
