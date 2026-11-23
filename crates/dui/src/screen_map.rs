use crate::{Reply, archive, failure, replay::io};
use std::{fs, path::Path, process::Command};

pub fn capture(project: &Path) -> Reply {
    let project = project.canonicalize().map_err(io)?;
    let work = tempfile::tempdir().map_err(io)?;
    let manifest = work.path().join("map.json");
    discover(&project, &manifest)?;
    archive::capture(
        &project,
        &manifest,
        None,
        archive::Capture::Run { force: false },
    )
}

pub(crate) fn discover(project: &Path, manifest: &Path) -> Result<(), (u8, serde_json::Value)> {
    let work = tempfile::tempdir().map_err(io)?;
    let discover = work.path().join("discover.mjs");
    fs::write(
        &discover,
        include_str!(concat!(env!("OUT_DIR"), "/vite/discover.mjs")),
    )
    .map_err(io)?;
    fs::write(
        work.path().join("parser.mjs"),
        include_str!(concat!(env!("OUT_DIR"), "/vite/parser.mjs")),
    )
    .map_err(io)?;
    let result = Command::new("node")
        .arg(discover)
        .arg(project)
        .arg(include_str!("../../../rules/web.json"))
        .output()
        .map_err(io)?;
    if !result.status.success() {
        return Err(failure(3, &String::from_utf8_lossy(&result.stderr)));
    }
    fs::write(manifest, result.stdout).map_err(io)
}
