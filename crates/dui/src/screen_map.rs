use crate::{Reply, archive, failure, replay::io};
use std::{fs, path::Path, process::Command};

pub fn capture(project: &Path) -> Reply {
    let project = project.canonicalize().map_err(io)?;
    let work = tempfile::tempdir().map_err(io)?;
    let discover = work.path().join("discover.mjs");
    fs::write(
        &discover,
        include_str!(concat!(env!("OUT_DIR"), "/vite/discover.mjs")),
    )
    .map_err(io)?;
    let result = Command::new("node")
        .arg(discover)
        .arg(&project)
        .arg(include_str!("../../../rules/web.json"))
        .output()
        .map_err(io)?;
    if !result.status.success() {
        return Err(failure(3, &String::from_utf8_lossy(&result.stderr)));
    }
    let manifest = work.path().join("map.json");
    fs::write(&manifest, result.stdout).map_err(io)?;
    archive::capture(&project, &manifest, None, archive::Capture::Run)
}
