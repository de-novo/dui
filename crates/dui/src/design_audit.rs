//! Explicit CLI opt-in to running project-owned checkers on archived evidence.
use crate::{Reply, audit, failure};
use std::{ffi::OsString, path::Path, process::Command};

pub fn command(args: &[OsString]) -> Reply {
    let usage = || {
        failure(
            2,
            "expected audit RUN_DIR [--goal PERCENT] [--design MANIFEST]",
        )
    };
    let (run, options) = args.split_first().ok_or_else(usage)?;
    let mut goal = None;
    let mut manifest = None;
    let mut pairs = options.chunks_exact(2);
    for pair in pairs.by_ref() {
        match pair[0].to_str() {
            Some("--goal") if goal.is_none() => {
                goal = Some(
                    pair[1]
                        .to_str()
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| failure(2, "invalid --goal percentage"))?,
                );
            }
            Some("--design") if manifest.is_none() => manifest = Some(&pair[1]),
            _ => return Err(usage()),
        }
    }
    if !pairs.remainder().is_empty() {
        return Err(usage());
    }
    let run = Path::new(run);
    let mut report = audit::audit_run(run, goal)?;
    let Some(manifest) = manifest else {
        return Ok(report);
    };
    let output = Command::new("node")
        .args([
            "--input-type=module",
            "--eval",
            include_str!("design_audit.mjs"),
            "--",
        ])
        .arg(manifest)
        .arg(run.join("report.json"))
        .output()
        .map_err(|e| failure(3, &format!("cannot start design audit: {e}")))?;
    let code = match output.status.code() {
        Some(0) => 0,
        Some(5) => 5,
        code => {
            return Err(failure(
                if code == Some(2) { 2 } else { 3 },
                String::from_utf8_lossy(&output.stderr).trim(),
            ));
        }
    };
    report["designAudit"] = serde_json::from_slice(&output.stdout)
        .map_err(|e| failure(3, &format!("invalid design audit report: {e}")))?;
    if code == 0 {
        Ok(report)
    } else {
        Err((code, report))
    }
}
