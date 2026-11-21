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
}
