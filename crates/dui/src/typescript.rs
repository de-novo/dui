use crate::{Reply, failure, ports::TypeOracle, scan};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
pub struct TypeScript;
impl TypeOracle for TypeScript {
    fn facts(&self, project: &Path, spec: &Path) -> Reply {
        scan(project)?;
        let temp = tempfile::tempdir().map_err(|e| failure(3, &e.to_string()))?;
        let script = temp.path().join("facts.mjs");
        let rules = temp.path().join("rules.json");
        fs::write(
            &script,
            include_str!(concat!(env!("OUT_DIR"), "/typescript/facts.mjs")),
        )
        .and_then(|_| fs::write(&rules, include_str!("../../../rules/web.json")))
        .map_err(|e| failure(3, &e.to_string()))?;
        let version = Command::new("node")
            .arg("--version")
            .output()
            .map_err(|_| failure(4, "Node 26 required"))?;
        if !version.stdout.starts_with(b"v26.") {
            return Err((
                4,
                json!({"status":"unsupported","reason":"Node 26 required"}),
            ));
        }
        let output = Command::new("node")
            .arg(script)
            .arg(
                project
                    .canonicalize()
                    .map_err(|_| failure(2, "invalid project"))?,
            )
            .arg(spec)
            .arg(rules)
            .output()
            .map_err(|e| failure(3, &e.to_string()))?;
        if output.status.code() == Some(4) {
            let unsupported =
                serde_json::from_slice(&output.stderr).map_err(|e| failure(3, &e.to_string()))?;
            return Err((4, unsupported));
        }
        if !output.status.success() {
            return Err(failure(3, &String::from_utf8_lossy(&output.stderr)));
        }
        let mut result: Value =
            serde_json::from_slice(&output.stdout).map_err(|e| failure(3, &e.to_string()))?;
        if let Some(facts) = result["facts"].as_array_mut() {
            for f in facts.iter_mut().filter(|f| f["kind"] != "httpResponse") {
                if !f["shape"].is_null() {
                    f["representatives"] = json!(dui_core::discovery::representatives(&f["shape"]));
                }
            }
        }
        result["autoMocks"] = dui_core::mock::responses(
            result["facts"]
                .as_array()
                .ok_or_else(|| failure(3, "missing facts"))?,
        );
        Ok(result)
    }
}
