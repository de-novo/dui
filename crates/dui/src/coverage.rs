//! Shared report coverage counts for planned and captured cells.

use serde_json::{Value, json};

/// Coverage counts for one report's cells. `renderable` is the legacy planned-renderable
/// count kept for archive compatibility; `capturedRenderable` is the explicit denominator
/// for verification coverage. `attempted` remains as the legacy alias of `captured`.
pub(crate) fn base(cells: &[Value]) -> Value {
    let planned = cells.len();
    let verified = cells.iter().filter(|c| c["status"] == "verified").count();
    let unrenderable = cells
        .iter()
        .filter(|c| c["status"] == "unrenderable")
        .count();
    let not_captured = cells
        .iter()
        .filter(|c| {
            c["status"] == "unverified"
                && c["reason"]
                    .as_str()
                    .is_some_and(dui_core::model::capture::is_uncaptured_reason)
        })
        .count();
    let renderable = planned.saturating_sub(unrenderable);
    let captured = planned.saturating_sub(unrenderable + not_captured);
    json!({
        "planned": planned,
        "captured": captured,
        "verified": verified,
        "capturedRenderable": captured,
        "renderable": renderable,
        "attempted": captured,
        "unrenderable": unrenderable,
    })
}
