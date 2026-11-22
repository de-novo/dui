use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FillTermination {
    Exhausted,
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

fn is_unbounded_primitive(shape: &Value) -> bool {
    match shape["kind"].as_str() {
        Some("string" | "number") => true,
        Some("union") => shape["members"]
            .as_array()
            .is_some_and(|members| members.iter().any(is_unbounded_primitive)),
        _ => false,
    }
}

fn truthy_representatives(shape: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    match shape["kind"].as_str() {
        Some("string") => {
            out.push(json!(""));
            out.push(json!("sample"));
        }
        Some("number") => {
            out.push(json!(0));
            out.push(json!(1));
        }
        Some("literal") => out.push(shape["value"].clone()),
        Some("boolean") => {
            out.push(json!(false));
            out.push(json!(true));
        }
        Some("union") => {
            for m in shape["members"].as_array().into_iter().flatten() {
                for v in truthy_representatives(m) {
                    if !out.contains(&v) {
                        out.push(v);
                    }
                }
            }
        }
        _ => {}
    }
    out
}

fn slot_values_for_planning(
    slot: &Value,
    predicate_ops: &BTreeMap<(String, u64), BTreeSet<String>>,
) -> Vec<Value> {
    let shape = &slot["shape"];
    if !is_unbounded_primitive(shape) {
        return representatives(shape);
    }
    let ops = slot["file"]
        .as_str()
        .zip(slot["binding"].as_u64())
        .and_then(|(file, binding)| predicate_ops.get(&(file.to_owned(), binding)));
    let Some(ops) = ops else {
        return vec![];
    };
    if ops.len() == 1 && ops.contains("truthy") {
        truthy_representatives(shape)
    } else {
        representatives(shape)
    }
}

/// Deterministic greedy pairwise covering-array builder for owners with 3+ slots.
/// Covers 100% of value pairs `(slot_i = v_i, slot_j = v_j)` in O(D^2 * log N) rows
/// instead of emitting O(N^2 * D^2) isolated 2-slot rows.
fn greedy_covering_pairs(slots: &[(String, Vec<Value>)]) -> Vec<BTreeMap<String, Value>> {
    let n = slots.len();
    let mut uncovered: BTreeSet<(usize, usize, usize, usize)> = BTreeSet::new();
    for i in 0..n {
        for j in (i + 1)..n {
            for vi in 0..slots[i].1.len() {
                for vj in 0..slots[j].1.len() {
                    uncovered.insert((i, vi, j, vj));
                }
            }
        }
    }
    let mut rows = Vec::new();
    while let Some(&(seed_i, seed_vi, seed_j, seed_vj)) = uncovered.iter().next() {
        let mut assignment: BTreeMap<usize, usize> = BTreeMap::new();
        assignment.insert(seed_i, seed_vi);
        assignment.insert(seed_j, seed_vj);
        for (slot_idx, (_, slot_vals)) in slots.iter().enumerate() {
            if assignment.contains_key(&slot_idx) || slot_vals.is_empty() {
                continue;
            }
            let mut best_val = 0usize;
            let mut best_gain = 0usize;
            for v_idx in 0..slot_vals.len() {
                let gain = assignment
                    .iter()
                    .filter(|&(&other_idx, &other_v)| {
                        let pair = if slot_idx < other_idx {
                            (slot_idx, v_idx, other_idx, other_v)
                        } else {
                            (other_idx, other_v, slot_idx, v_idx)
                        };
                        uncovered.contains(&pair)
                    })
                    .count();
                if gain > best_gain {
                    best_gain = gain;
                    best_val = v_idx;
                }
            }
            if best_gain > 0 {
                assignment.insert(slot_idx, best_val);
            }
        }
        let assigned_vec: Vec<(usize, usize)> = assignment.iter().map(|(&k, &v)| (k, v)).collect();
        for a in 0..assigned_vec.len() {
            for b in (a + 1)..assigned_vec.len() {
                let (i, vi) = assigned_vec[a];
                let (j, vj) = assigned_vec[b];
                uncovered.remove(&(i, vi, j, vj));
            }
        }
        let row: BTreeMap<String, Value> = assignment
            .into_iter()
            .map(|(slot_idx, v_idx)| (slots[slot_idx].0.clone(), slots[slot_idx].1[v_idx].clone()))
            .collect();
        rows.push(row);
    }
    rows
}

// Each row covers either one value or one pair (using a greedy covering array for 3+ slots).
// Independent owners are planned separately.
pub fn candidates(facts: &[Value], active: &BTreeSet<String>) -> Vec<BTreeMap<String, Value>> {
    let mut predicate_ops: BTreeMap<(String, u64), BTreeSet<String>> = BTreeMap::new();
    for f in facts.iter().filter(|f| f["kind"] == "branch") {
        if let (Some(file), Some(binding), Some(op)) = (
            f["file"].as_str(),
            f["predicate"]["binding"].as_u64(),
            f["predicate"]["operator"].as_str(),
        ) {
            predicate_ops
                .entry((file.to_owned(), binding))
                .or_default()
                .insert(op.to_owned());
        }
    }
    let mut groups = BTreeMap::new();
    for f in facts.iter().filter(|f| {
        f["kind"] == "slot"
            && active.contains(&key(f))
            && !f["init"].is_null()
            && !f["after"].is_null()
    }) {
        let values = slot_values_for_planning(f, &predicate_ops);
        if values.is_empty() {
            continue;
        }
        groups
            .entry((f["file"].to_string(), f["owner"].to_string()))
            .or_insert_with(Vec::new)
            .push((key(f), values));
    }
    let mut singles = Vec::new();
    let mut pairs = Vec::new();
    for slots in groups.values_mut() {
        slots.sort_by(|a, b| a.0.cmp(&b.0));
        for (slot_key, values) in slots.iter() {
            for v in values {
                singles.push(BTreeMap::from([(slot_key.clone(), v.clone())]));
            }
        }
        if slots.len() == 2 {
            for v in &slots[1].1 {
                for option in &slots[0].1 {
                    pairs.push(BTreeMap::from([
                        (slots[1].0.clone(), v.clone()),
                        (slots[0].0.clone(), option.clone()),
                    ]));
                }
            }
        } else if slots.len() >= 3 {
            pairs.extend(greedy_covering_pairs(slots));
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

fn is_dynamic_id_segment(seg: &str) -> bool {
    if seg.is_empty() {
        return false;
    }
    if seg.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }
    let is_uuid = seg.len() == 36
        && seg.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        });
    if is_uuid {
        return true;
    }
    seg.len() >= 12
        && seg.bytes().all(|b| b.is_ascii_hexdigit())
        && seg.bytes().any(|b| b.is_ascii_digit())
}

/// Normalize a route path into a canonical pattern for loop-free BFS discovery.
/// Collapses numeric, UUID, and hex-ID path segments into `:id` so infinite dynamic
/// lists (`/orders/1`, `/orders/2`, ...) visit a single representative screen while
/// preserving distinct static paths and sorted query parameters.
pub fn route_pattern(path: &str) -> String {
    let without_hash = path.split('#').next().unwrap_or(path);
    let (raw_pathname, raw_query) = match without_hash.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (without_hash, None),
    };
    let segments: Vec<&str> = raw_pathname
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|seg| {
            if is_dynamic_id_segment(seg) {
                ":id"
            } else {
                seg
            }
        })
        .collect();
    let normalized_path = if segments.is_empty() {
        "/".to_owned()
    } else {
        format!("/{}", segments.join("/"))
    };
    if let Some(q) = raw_query.filter(|q| !q.is_empty()) {
        let mut pairs: Vec<&str> = q.split('&').filter(|p| !p.is_empty()).collect();
        pairs.sort_unstable();
        format!("{normalized_path}?{}", pairs.join("&"))
    } else {
        normalized_path
    }
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

    #[test]
    fn route_pattern_collapses_dynamic_ids_while_keeping_static_routes_and_queries_distinct() {
        assert_eq!(route_pattern("/orders/101"), "/orders/:id");
        assert_eq!(route_pattern("/orders/99999/"), "/orders/:id");
        assert_eq!(
            route_pattern("/items/550e8400-e29b-41d4-a716-446655440000/edit"),
            "/items/:id/edit"
        );
        assert_eq!(route_pattern("/blobs/abcdef1234567890"), "/blobs/:id");
        assert_eq!(
            route_pattern("/settings/api-keys#top"),
            "/settings/api-keys"
        );
        assert_eq!(route_pattern("/?b=2&a=1"), "/?a=1&b=2");
        assert_ne!(
            route_pattern("/?tab=details"),
            route_pattern("/?tab=billing")
        );
    }

    #[test]
    fn candidates_prune_unreferenced_unbounded_primitives_while_keeping_finite_and_branch_bound_slots()
     {
        let mut finite = slot(1, "View");
        finite["shape"] = json!({"kind": "boolean"});
        let mut internal_str = slot(10, "View");
        internal_str["binding"] = json!(100);
        internal_str["shape"] = json!({"kind": "string"});
        let mut branch_str = slot(20, "View");
        branch_str["binding"] = json!(200);
        branch_str["shape"] = json!({"kind": "string"});
        let branch = json!({
            "file": "screen.tsx",
            "owner": "View",
            "start": 50,
            "end": 60,
            "kind": "branch",
            "predicate": {"binding": 200, "operator": "truthy"}
        });
        let facts = vec![
            finite.clone(),
            internal_str.clone(),
            branch_str.clone(),
            branch,
        ];
        let active: BTreeSet<String> = [key(&finite), key(&internal_str), key(&branch_str)]
            .into_iter()
            .collect();
        let planned = candidates(&facts, &active);
        assert!(planned.iter().any(|row| row.contains_key(&key(&finite))));
        assert!(
            planned
                .iter()
                .any(|row| row.contains_key(&key(&branch_str)))
        );
        assert!(
            planned
                .iter()
                .all(|row| !row.contains_key(&key(&internal_str))),
            "unreferenced unbounded string slot must be pruned"
        );
        // Truthy-only string slot uses 2 minimal values ("" and "sample"), not 3.
        let str_vals: BTreeSet<String> = planned
            .iter()
            .filter_map(|row| row.get(&key(&branch_str)).and_then(Value::as_str))
            .map(str::to_owned)
            .collect();
        assert_eq!(str_vals, BTreeSet::from(["".into(), "sample".into()]));
    }

    #[test]
    fn covering_array_covers_every_pair_with_far_fewer_rows_than_quadratic_pairs() {
        let slots: Vec<Value> = (0..5)
            .map(|i| {
                let mut s = slot(i * 10 + 1, "BigView");
                s["shape"] = json!({"kind": "boolean"});
                s
            })
            .collect();
        let active: BTreeSet<String> = slots.iter().map(key).collect();
        let planned = candidates(&slots, &active);
        let keys: Vec<String> = slots.iter().map(key).collect();
        // Verify 100% of all C(5,2) * 4 = 40 pairs are covered by `planned`.
        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                for vi in [json!(false), json!(true)] {
                    for vj in [json!(false), json!(true)] {
                        assert!(
                            planned.iter().any(|row| row.get(&keys[i]) == Some(&vi)
                                && row.get(&keys[j]) == Some(&vj)),
                            "missing pair ({i}={vi}, {j}={vj})"
                        );
                    }
                }
            }
        }
        // Quadratic enumeration would emit 10 singles + 40 pairs = 50 rows;
        // greedy covering array packs all 40 pairs into <= 8 rows (<= 18 total).
        assert!(
            planned.len() <= 18,
            "expected covering array compression <= 18 rows, got {}",
            planned.len()
        );
    }
}
