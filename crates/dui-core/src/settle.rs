//! When a captured screen has stopped changing.
//!
//! The browser samples the page about every 100 ms. A screen is stable after
//! [`STABLE_SAMPLES`] identical samples in a row. Regions that change on their own do not
//! hold it back:
//! - nodes the page marks as busy (`volatile`: `aria-busy`, progress bars, timers and endlessly
//!   animated spinners);
//! - nodes whose text or rectangle changed in [`CHURN_SAMPLES`] consecutive samples, such as a
//!   ticking clock.
//!
//! Ignored nodes are recorded. When the time budget runs out first, [`Settle::unsettled`]
//! names what was still changing.

use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const STABLE_SAMPLES: u32 = 4;
pub const CHURN_SAMPLES: u32 = 3;
/// Default `settleTimeoutMs`.
pub const DEFAULT_TIMEOUT_MS: u64 = 10_000;

#[derive(Default)]
pub struct Settle {
    /// The two samples around the most recent change.
    changed: Option<(Value, Value)>,
    previous: Option<Value>,
    churn: BTreeMap<String, u32>,
    ignored: BTreeMap<String, &'static str>,
    stable: u32,
}

fn nodes(sample: &Value) -> BTreeMap<&str, &Value> {
    sample["renderTree"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|node| Some((node["id"].as_str()?, node)))
        .collect()
}

impl Settle {
    /// Record one sample; true once the screen counts as stable at `width`.
    pub fn sample(&mut self, sample: &Value, width: u32) -> bool {
        for id in sample["volatile"].as_array().into_iter().flatten() {
            if let Some(id) = id.as_str() {
                self.ignored.entry(id.to_owned()).or_insert("busy");
            }
        }
        if let Some(previous) = &self.previous {
            let before = nodes(previous);
            for (id, node) in nodes(sample) {
                let changed = before
                    .get(id)
                    .is_some_and(|old| old["text"] != node["text"] || old["rect"] != node["rect"]);
                let count = self.churn.entry(id.to_owned()).or_insert(0);
                *count = if changed { *count + 1 } else { 0 };
                if *count >= CHURN_SAMPLES {
                    self.ignored
                        .entry(id.to_owned())
                        .or_insert("changes every sample");
                }
            }
        }
        let same = self
            .previous
            .as_ref()
            .is_some_and(|previous| self.key(previous) == self.key(sample));
        self.stable = if same && sample["ready"] == true && sample["width"] == width {
            self.stable + 1
        } else {
            0
        };
        if let Some(previous) = self.previous.replace(sample.clone())
            && !same
        {
            self.changed = Some((previous, sample.clone()));
        }
        self.stable >= STABLE_SAMPLES
    }

    /// The sample without ignored nodes, their text, or the raw `volatile` list.
    fn key(&self, sample: &Value) -> Value {
        let mut key = sample.clone();
        if let Some(fields) = key.as_object_mut() {
            fields.remove("volatile");
        }
        let mut dropped = Vec::new();
        if let Some(tree) = key["renderTree"].as_array_mut() {
            tree.retain(|node| {
                let ignored = node["id"]
                    .as_str()
                    .is_some_and(|id| self.ignored.contains_key(id));
                if ignored && let Some(text) = node["text"].as_str().filter(|t| !t.is_empty()) {
                    dropped.push(text.to_owned());
                }
                !ignored
            });
        }
        if let Some(text) = key["text"].as_str() {
            let mut text = text.to_owned();
            for part in &dropped {
                text = text.replace(part.as_str(), "");
            }
            key["text"] = json!(text);
        }
        key
    }

    /// Nodes left out of the comparison, with why: `[{"node", "reason"}]`.
    pub fn ignored(&self) -> Vec<Value> {
        self.ignored
            .iter()
            .map(|(id, reason)| json!({"node": id, "reason": reason}))
            .collect()
    }

    /// What differed in the most recent change: nodes added, removed or changed, and other
    /// observation fields. At most `limit` entries.
    pub fn unsettled(&self, limit: usize) -> Vec<Value> {
        let Some((older, newer)) = &self.changed else {
            return Vec::new();
        };
        let (before, after) = (self.key(older), self.key(newer));
        let (old, new) = (nodes(&before), nodes(&after));
        let mut found = Vec::new();
        for (id, node) in &new {
            match old.get(id) {
                None => found.push(json!({"node": id, "change": "added"})),
                Some(prior) if prior["text"] != node["text"] => {
                    found.push(json!({"node": id, "change": "text"}))
                }
                Some(prior) if prior != node => found.push(json!({"node": id, "change": "rect"})),
                Some(_) => {}
            }
        }
        for id in old.keys().filter(|id| !new.contains_key(*id)) {
            found.push(json!({"node": id, "change": "removed"}));
        }
        let fields: BTreeSet<&String> = after
            .as_object()
            .into_iter()
            .flat_map(|o| o.keys())
            .chain(before.as_object().into_iter().flat_map(|o| o.keys()))
            .filter(|k| *k != "renderTree")
            .collect();
        for field in fields {
            if before[field.as_str()] != after[field.as_str()] {
                found.push(json!({"field": field}));
            }
        }
        found.truncate(limit);
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(clock: &str, spinner_x: u32, rows: usize) -> Value {
        let mut tree = vec![
            json!({"id":"h1#0","text":"Orders","rect":{"x":0,"y":0}}),
            json!({"id":"span#0","text":clock,"rect":{"x":0,"y":40}}),
            json!({"id":"svg#0","text":"","rect":{"x":spinner_x,"y":80}}),
        ];
        for row in 0..rows {
            tree.push(json!({"id":format!("li#{row}"),"text":format!("row {row}"),"rect":{"x":0,"y":120}}));
        }
        json!({"ready":true,"width":375,"text":format!("Orders {clock}"),"renderTree":tree,"volatile":["svg#0"]})
    }

    #[test]
    fn a_ticking_clock_and_a_spinner_do_not_hold_the_screen_back() {
        let mut settle = Settle::default();
        let stable_at = (0..20u32)
            .position(|tick| settle.sample(&sample(&format!("12:00:0{tick}"), tick * 7, 3), 375))
            .expect("settles");
        assert!(stable_at < 10, "settled after {stable_at} samples");
        assert_eq!(
            settle.ignored(),
            [
                json!({"node":"span#0","reason":"changes every sample"}),
                json!({"node":"svg#0","reason":"busy"}),
            ]
        );
    }

    #[test]
    fn a_list_that_keeps_growing_never_settles_and_names_the_new_node() {
        let mut settle = Settle::default();
        for rows in 0..20 {
            assert!(!settle.sample(&sample("12:00:00", 0, rows), 375));
        }
        let still = settle.unsettled(5);
        assert!(
            still.contains(&json!({"node":"li#18","change":"added"})),
            "{still:?}"
        );
    }
}
