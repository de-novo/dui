use dui_ports::{Reply, failure};
use serde_json::Value;
use std::{fs, path::Path};

pub fn archive(directory: &Path, selection: Value) -> Reply {
    let query = serde_json::from_value(selection)
        .map_err(|e| failure(2, &format!("invalid selection: {e}")))?;
    let read = |file| -> Result<Value, (u8, Value)> {
        let path = directory.join(file);
        let bytes = fs::read(&path).map_err(|e| failure(2, &format!("{}: {e}", path.display())))?;
        serde_json::from_slice(&bytes).map_err(|e| failure(2, &e.to_string()))
    };
    let facts = read("facts.json")?;
    let report = read("report.json")?;
    dui_core::inspect::inspect(
        facts["facts"]
            .as_array()
            .ok_or_else(|| failure(2, "invalid facts"))?,
        report["cells"]
            .as_array()
            .ok_or_else(|| failure(2, "invalid report"))?,
        &query,
    )
    .map_err(|reason| failure(2, reason))
}
