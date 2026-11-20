//! Decide whether a serialized edit may reach the renderer.
use serde_json::Value;

pub enum Decision<'a, T> {
    Replay(&'a T),
    Reject(&'static str),
    Write { revision: u64 },
}

pub fn decide<'a, T>(
    current: u64,
    request: &Value,
    receipt: Option<&'a (Value, T)>,
) -> Decision<'a, T> {
    if let Some((original, result)) = receipt {
        return if original == request {
            Decision::Replay(result)
        } else {
            Decision::Reject("request-id-reused")
        };
    }
    if request["expectedRevision"].as_u64() != Some(current) {
        return Decision::Reject("stale-revision");
    }
    match current.checked_add(1) {
        Some(revision) => Decision::Write { revision },
        None => Decision::Reject("revision-exhausted"),
    }
}
