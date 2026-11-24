//! Compare recorded verification outcomes independently of pixels and render trees.
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct Scope<'a> {
    path: &'a str,
    scenario: (&'static str, &'a str),
    width: u64,
    locale: Option<&'a str>,
}

struct Cell<'a> {
    scope: Scope<'a>,
    raw: &'a Value,
}

fn cells(report: &Value) -> Result<Vec<Cell<'_>>, &'static str> {
    let names: BTreeMap<_, _> = report["scenarios"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| Some((r["scenario"].as_str()?, r["name"].as_str()?)))
        .collect();
    let rows = report["cells"]
        .as_array()
        .ok_or("report.json missing cells array")?;
    Ok(rows
        .iter()
        .map(|raw| {
            let path = raw["path"]
                .as_str()
                .or(report["path"].as_str())
                .unwrap_or("/");
            let id = raw["scenario"].as_str().unwrap_or("default");
            let scenario = if id.starts_with("case-")
                && let Some(name) = raw["caseName"].as_str().or_else(|| names.get(id).copied())
            {
                ("named", name)
            } else if (report["discoveredRoutes"].is_array() || raw["linkOrigin"].is_object())
                && id
                    .strip_prefix("link-")
                    .is_some_and(|n| n.parse::<usize>().is_ok())
            {
                ("linked", "")
            } else {
                ("id", id)
            };
            let width = raw["width"].as_u64().unwrap_or_default();
            Cell {
                scope: Scope {
                    path,
                    scenario,
                    width,
                    locale: raw["locale"].as_str(),
                },
                raw,
            }
        })
        .collect())
}

// Only pair keys that identify a single remaining cell on each side.
fn index<'a>(
    cells: &[Cell<'a>],
    matched: &[bool],
    by_input: bool,
) -> BTreeMap<(Scope<'a>, String), Option<usize>> {
    let mut keys = BTreeMap::new();
    for (i, cell) in cells.iter().enumerate().filter(|(i, _)| !matched[*i]) {
        if cell.scope.width == 0 {
            continue;
        }
        let identity = if by_input {
            cell.raw.get("input").map(Value::to_string)
        } else {
            cell.raw["state"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        if let Some(key) = identity {
            keys.entry((cell.scope, key))
                .and_modify(|v| *v = None)
                .or_insert(Some(i));
        }
    }
    keys
}

pub fn compare(before: &Value, after: &Value) -> Result<Value, &'static str> {
    let base = cells(before)?;
    let head = cells(after)?;
    let mut base_matched = vec![false; base.len()];
    let mut head_matched = vec![false; head.len()];
    let mut pairs = Vec::new();
    // Existing archives carry state IDs. A change in mocks can change that ID
    // while leaving the logical scenario and its injected input identical.
    for by_input in [false, true] {
        let heads = index(&head, &head_matched, by_input);
        for (key, base_index) in index(&base, &base_matched, by_input) {
            if let (Some(b), Some(Some(h))) = (base_index, heads.get(&key)) {
                if let (Some(old), Some(new)) =
                    (base[b].raw.get("input"), head[*h].raw.get("input"))
                    && old != new
                {
                    continue;
                }
                pairs.push((b, *h));
                base_matched[b] = true;
                head_matched[*h] = true;
            }
        }
    }
    let matched = pairs.len();
    let mut comparable = 0;
    let mut regressions = Vec::new();
    for (b, h) in pairs {
        let (prior, current) = (base[b].raw, head[h].raw);
        let mut checks = Vec::new();
        for (check, old, new) in [
            ("capture", &prior["status"], &current["status"]),
            (
                "screen",
                &prior["observation"]["screenEvidence"]["screenStatus"],
                &current["observation"]["screenEvidence"]["screenStatus"],
            ),
        ] {
            let known =
                |s: &Value| matches!(s.as_str(), Some("verified" | "unverified" | "unrenderable"));
            if known(old) && known(new) {
                comparable += 1;
                if old == "verified" && new != "verified" {
                    let mut change = json!({"check":check, "before":old,"after":new});
                    if check == "screen" {
                        change["evidence"] = current["observation"]["screenEvidence"].clone();
                    } else {
                        change["reason"] = current["reason"].clone();
                    }
                    checks.push(change);
                }
            }
        }
        if !checks.is_empty() {
            regressions.push(json!({"path":head[h].scope.path,"scenario":current["scenario"],"state":current["state"],
                "beforeState":prior["state"],"width":head[h].scope.width,"locale":head[h].scope.locale,"checks":checks}));
        }
    }
    let verdict = if !regressions.is_empty() {
        "regressions"
    } else if comparable == 0 {
        "inconclusive"
    } else {
        "no-regressions"
    };
    Ok(
        json!({"verdict":verdict,"regressedCells":regressions.len(),"matchedCells":matched,"comparedChecks":comparable,
        "unmatchedBaseCells":base.len()-matched,"unmatchedHeadCells":head.len()-matched,"regressions":regressions}),
    )
}

pub fn markdown(report: &Value) -> String {
    let verification = &report["verification"];
    let mut out = format!(
        "## dui verification regressions\n\n{} regressed cells across {} compared checks. {} base and {} head cells were unmatched.\n",
        verification["regressedCells"],
        verification["comparedChecks"],
        verification["unmatchedBaseCells"],
        verification["unmatchedHeadCells"]
    );
    if verification["verdict"] == "inconclusive" {
        out.push_str("\nInconclusive: the archives contain no comparable verification checks.\n");
    }
    let table_text = |v: &Value| {
        v.as_str()
            .unwrap_or("—")
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('|', "&#124;")
            .replace('`', "&#96;")
            .replace(['\r', '\n'], " ")
    };
    if let Some(rows) = verification["regressions"]
        .as_array()
        .filter(|r| !r.is_empty())
    {
        out.push_str("\n| Screen | Scenario | State | Width | Locale | Check | Before → after | Head evidence |\n|---|---|---|---:|---|---|---|---|\n");
        for row in rows {
            for check in row["checks"].as_array().into_iter().flatten() {
                let evidence = &check["evidence"];
                let detail = if evidence.is_object() {
                    format!(
                        "HTTP {} · path <code>{}</code> · selector <code>{}</code> · visible {}",
                        evidence["documentStatus"]
                            .as_u64()
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "unknown".into()),
                        table_text(&evidence["observedPath"]),
                        table_text(&evidence["selector"]),
                        evidence["selectorVisible"]
                            .as_bool()
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "unknown".into())
                    )
                } else {
                    table_text(&check["reason"])
                };
                out.push_str(&format!("| <code>{}</code> | <code>{}</code> | <code>{}</code> | {}px | {} | {} | {} → {} | {} |\n",
                    table_text(&row["path"]),table_text(&row["scenario"]),table_text(&row["state"]),row["width"],
                    table_text(&row["locale"]),table_text(&check["check"]),table_text(&check["before"]),table_text(&check["after"]),detail));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(state: &str, capture: &str, screen: Option<&str>) -> Value {
        json!({"path":"/inbox","scenario":"default","state":state,"input":{"tab":state},
            "width":800,"locale":"en","status":capture,"image":"same.png",
            "observation":{"screenEvidence":{"screenStatus":screen}}})
    }

    #[test]
    fn only_newly_failed_checks_count_and_a_cell_is_counted_once() {
        let base = json!({"cells":[
            cell("capture","verified",None), cell("screen","verified",Some("verified")),
            cell("both","verified",Some("verified")), cell("old","unverified",Some("unverified")),
            cell("recovered","unverified",None), cell("legacy","verified",None),
            cell("lost","verified",Some("verified")),cell("removed","verified",None)]});
        let head = json!({"cells":[
            cell("added","unverified",Some("unverified")),cell("old","unrenderable",Some("unverified")),
            cell("recovered","verified",None),cell("screen","verified",Some("unverified")),
            cell("capture","unrenderable",None),cell("both","unverified",Some("unverified")),
            cell("legacy","verified",Some("unverified")),cell("lost","verified",None)]});
        let result = compare(&base, &head).expect("compare");
        assert_eq!(result["verdict"], "regressions");
        assert_eq!(result["regressedCells"], 3);
        assert_eq!(result["comparedChecks"], 10);
        assert_eq!(result["unmatchedBaseCells"], 1);
        assert_eq!(result["unmatchedHeadCells"], 1);
        let rows = result["regressions"].as_array().expect("regressions");
        assert_eq!(rows[0]["state"], "both");
        assert_eq!(rows[0]["checks"].as_array().expect("checks").len(), 2);
        assert_eq!(rows[1]["checks"][0]["check"], "capture");
        assert_eq!(rows[2]["checks"][0]["check"], "screen");
    }

    #[test]
    fn pairs_unique_logical_inputs_and_never_guesses_ambiguous_or_different_dimensions() {
        let base = cell("a", "verified", Some("verified"));
        let mut head = cell("a", "unverified", Some("unverified"));
        head["state"] = json!("new-hash");
        let compare_cells = |base: Value, head: Value| {
            compare(&json!({"cells":base}), &json!({"cells":head})).expect("compare")
        };
        assert_eq!(
            compare_cells(json!([base]), json!([head]))["regressedCells"],
            1
        );
        let mut conflicting = base.clone();
        conflicting["input"] = json!({"tab":"different"});
        assert_eq!(
            compare_cells(json!([base]), json!([conflicting]))["matchedCells"],
            0
        );
        let mut other = head.clone();
        other["state"] = json!("ambiguous");
        assert_eq!(
            compare_cells(json!([base]), json!([head, other]))["matchedCells"],
            0
        );
        for (field, value) in [
            ("locale", json!("ko")),
            ("width", json!(390)),
            ("scenario", json!("case-2")),
            ("path", json!("/cart")),
            ("input", json!({"tab":"b"})),
        ] {
            let mut different = head.clone();
            different[field] = value;
            assert_eq!(
                compare_cells(json!([base]), json!([different]))["matchedCells"],
                0
            );
        }
    }
}
