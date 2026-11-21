//! Pure locale rules: RTL detection, cross-locale missing translation key diffs,
//! and deterministic per-locale report summaries.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const RTL_PRIMARY_SUBTAGS: &[&str] = &[
    "ar", "arc", "ckb", "dv", "fa", "ha", "he", "iw", "khw", "ks", "ku", "ps", "sd", "ug", "ur",
    "yi",
];

/// Returns `true` if the BCP-47 locale tag's primary language subtag is right-to-left.
pub fn is_rtl_locale(locale: &str) -> bool {
    let primary = locale
        .trim()
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    RTL_PRIMARY_SUBTAGS.contains(&primary.as_str())
}

/// Flattens a nested JSON translation object into sorted dot-separated leaf keys.
pub fn flatten_message_keys(value: &Value) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    collect_keys(value, "", &mut keys);
    keys
}

fn collect_keys(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let next = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                collect_keys(v, &next, out);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            if !prefix.is_empty() {
                out.insert(prefix.to_owned());
            }
        }
    }
}

/// Computes keys missing in each locale relative to the union of keys across all locales.
pub fn missing_translation_keys(
    catalogs: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<String, Vec<String>> {
    let all_keys: BTreeSet<&String> = catalogs.values().flat_map(|s| s.iter()).collect();
    let mut missing = BTreeMap::new();
    for (locale, keys) in catalogs {
        let absent: Vec<String> = all_keys
            .iter()
            .filter(|k| !keys.contains(**k))
            .map(|k| (*k).clone())
            .collect();
        missing.insert(locale.clone(), absent);
    }
    missing
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleSummary {
    pub cells: usize,
    pub verified: usize,
    pub unverified: usize,
    pub unrenderable: usize,
    pub missing_keys: Vec<String>,
    pub overflow_count: usize,
    pub rtl_mismatches: usize,
}

/// Groups captured report cells by locale and summarizes verification and locale-audit issues.
pub fn summarize_by_locale(
    cells: &[Value],
    missing_by_locale: &BTreeMap<String, Vec<String>>,
) -> Value {
    let mut groups: BTreeMap<String, LocaleSummary> = BTreeMap::new();
    for (locale, missing) in missing_by_locale {
        groups.insert(
            locale.clone(),
            LocaleSummary {
                missing_keys: missing.clone(),
                ..LocaleSummary::default()
            },
        );
    }
    for cell in cells {
        let Some(locale) = cell["locale"].as_str() else {
            continue;
        };
        let entry = groups.entry(locale.to_owned()).or_default();
        entry.cells += 1;
        match cell["status"].as_str() {
            Some("verified") => entry.verified += 1,
            Some("unrenderable") => entry.unrenderable += 1,
            _ => entry.unverified += 1,
        }
        if let Some(audit) = cell.get("localeAudit").filter(|v| v.is_object()) {
            if let Some(overflows) = audit["overflows"].as_array() {
                entry.overflow_count += overflows.len();
            }
            if audit["rtlMismatch"].is_object() {
                entry.rtl_mismatches += 1;
            }
            if let Some(runtime_missing) = audit["missingKeys"].as_array() {
                let mut merged: BTreeSet<String> = entry.missing_keys.iter().cloned().collect();
                for key in runtime_missing.iter().filter_map(Value::as_str) {
                    merged.insert(key.to_owned());
                }
                entry.missing_keys = merged.into_iter().collect();
            }
        }
    }
    json!(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rtl_locales_from_bcp47_tags() {
        assert!(is_rtl_locale("ar"));
        assert!(is_rtl_locale("ar-SA"));
        assert!(is_rtl_locale("he_IL"));
        assert!(is_rtl_locale("fa"));
        assert!(!is_rtl_locale("en"));
        assert!(!is_rtl_locale("en-US"));
        assert!(!is_rtl_locale("ko"));
        assert!(!is_rtl_locale("zh-CN"));
    }

    #[test]
    fn missing_keys_are_reported_only_for_the_locale_missing_them() {
        let en = flatten_message_keys(
            &json!({"nav":{"home":"Home","settings":"Settings"},"save":"Save"}),
        );
        let de = flatten_message_keys(&json!({"nav":{"home":"Start"},"save":"Speichern"}));
        let catalogs = BTreeMap::from([("en".into(), en), ("de".into(), de)]);
        let missing = missing_translation_keys(&catalogs);
        assert_eq!(missing["en"], Vec::<String>::new());
        assert_eq!(missing["de"], vec!["nav.settings".to_owned()]);
    }

    #[test]
    fn summarizes_cells_and_issues_by_locale() {
        let cells = vec![
            json!({"locale":"en","status":"verified","localeAudit":{"overflows":[],"missingKeys":[]}}),
            json!({"locale":"de","status":"verified","localeAudit":{"overflows":[{"selector":"button"}],"missingKeys":["subtitle"]}}),
            json!({"locale":"ar","status":"verified","localeAudit":{"overflows":[],"rtlMismatch":{"expected":"rtl","observed":"ltr"}}}),
        ];
        let summary = summarize_by_locale(&cells, &BTreeMap::new());
        assert_eq!(summary["en"]["cells"], 1);
        assert_eq!(summary["en"]["overflowCount"], 0);
        assert_eq!(summary["de"]["overflowCount"], 1);
        assert_eq!(summary["de"]["missingKeys"], json!(["subtitle"]));
        assert_eq!(summary["ar"]["rtlMismatches"], 1);
    }
}
