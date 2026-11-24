//! Discover route declarations without starting an app or inventing capture cells.
use crate::{Reply, analysis, archive, failure, replay::io};
use dui_core::model::{Screen, ScreenOrigin, capture::NativeSource};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

type Error = (u8, Value);

pub(crate) enum Adapter {
    JavaScript,
    Dart,
}

pub(crate) fn target(project: &Path) -> Result<Option<(PathBuf, Adapter)>, Error> {
    let profile: Value = if project.join("dui.json").is_file() {
        crate::replay::read(&project.join("dui.json"))?
    } else {
        Value::Null
    };
    if ["host", "path", "surface"]
        .iter()
        .any(|key| profile[key].is_string())
    {
        return Ok(None);
    }
    let app = project.join(profile["appRoot"].as_str().unwrap_or("."));
    let package_path = app.join("package.json");
    if package_path.is_file() {
        let package = crate::replay::read(&package_path)?;
        let is_native = ["dependencies", "devDependencies", "peerDependencies"]
            .iter()
            .any(|key| {
                package[key].as_object().is_some_and(|deps| {
                    deps.keys().any(|name| {
                        matches!(name.as_str(), "react-native" | "expo-router")
                            || name.starts_with("@react-navigation/")
                    })
                })
            });
        if is_native || package["main"].as_str() == Some("expo-router/entry") {
            return Ok(Some((app, Adapter::JavaScript)));
        }
    }
    Ok(app
        .join("pubspec.yaml")
        .is_file()
        .then_some((app, Adapter::Dart)))
}

#[derive(Deserialize)]
struct Route {
    path: String,
    #[serde(flatten)]
    declaration: NativeSource,
}

#[derive(Deserialize)]
pub(crate) struct Inventory {
    screens: Vec<Route>,
    limitations: Vec<String>,
}

fn output(command: &mut Command, operation: &str) -> Result<Output, Error> {
    let result = command
        .output()
        .map_err(|e| failure(3, &format!("{operation}: {e}")))?;
    if !result.status.success() {
        return Err(failure(
            3,
            &format!(
                "{operation}: {}",
                String::from_utf8_lossy(&result.stderr).trim()
            ),
        ));
    }
    Ok(result)
}

impl Adapter {
    pub(crate) fn discover(&self, app: &Path) -> Result<Inventory, Error> {
        let result = match self {
            Self::JavaScript => {
                let temp = tempfile::tempdir().map_err(io)?;
                let script = temp.path().canonicalize().map_err(io)?.join("discover.mjs");
                fs::write(
                    &script,
                    include_bytes!("../../../adapters/native-map/discover-js.mjs"),
                )
                .map_err(io)?;
                output(
                    Command::new("node").arg(script).arg(app),
                    "discover native JavaScript routes",
                )?
            }
            Self::Dart => {
                let tool = dart_tool()?;
                output(
                    Command::new("dart")
                        .arg(tool.join("bin/discover.dart"))
                        .arg(app)
                        .current_dir(&tool),
                    "discover Flutter routes",
                )?
            }
        };
        serde_json::from_slice(&result.stdout).map_err(io)
    }
}

/// Keep the locked analyzer in dui's cache so mapping an app never modifies its packages.
fn dart_tool() -> Result<PathBuf, Error> {
    let version = output(Command::new("dart").arg("--version"), "locate Dart SDK")?;
    let files = [
        (
            "pubspec.yaml",
            include_str!("../../../adapters/native-map/flutter/pubspec.yaml"),
        ),
        (
            "pubspec.lock",
            include_str!("../../../adapters/native-map/flutter/pubspec.lock"),
        ),
        (
            "bin/discover.dart",
            include_str!("../../../adapters/native-map/flutter/bin/discover.dart"),
        ),
    ];
    let mut digest = Sha256::new();
    digest.update(&version.stdout);
    digest.update(&version.stderr);
    for (_, contents) in files {
        digest.update(contents.as_bytes());
    }
    let tool = archive::home_dir()?
        .join("tools")
        .join("flutter-map")
        .join(format!("{:x}", digest.finalize()));
    fs::create_dir_all(tool.join("bin")).map_err(io)?;
    let lock = fs::File::options()
        .write(true)
        .create(true)
        .truncate(false)
        .open(tool.join("prepare.lock"))
        .map_err(io)?;
    lock.lock().map_err(io)?;
    for (name, contents) in files {
        let file = tool.join(name);
        if !file.is_file() {
            fs::write(file, contents).map_err(io)?;
        }
    }
    // Pub validates both the lockfile and the active package cache. The offline
    // path also works after the first install on a disconnected machine.
    let offline = Command::new("dart")
        .args(["pub", "get", "--offline", "--enforce-lockfile"])
        .current_dir(&tool)
        .output()
        .map_err(io)?;
    if !offline.status.success() {
        output(
            Command::new("dart")
                .args(["pub", "get", "--enforce-lockfile"])
                .current_dir(&tool),
            "prepare locked Dart analyzer",
        )?;
    }
    Ok(tool)
}

impl Inventory {
    pub(crate) fn archive(&self, out: &Path) -> Reply {
        fs::create_dir_all(out).map_err(io)?;
        let mut nodes = Vec::new();
        let mut sources = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for route in &self.screens {
            let source = &route.declaration;
            let screen = Screen::new(
                &route.path,
                ScreenOrigin::Native {
                    declaration: source.clone(),
                },
            )
            .map_err(io)?;
            if !ids.insert(screen.id().clone()) {
                continue;
            }
            sources.insert(&source.source);
            nodes.push(json!({"id":screen.id(), "kind":"screen", "label":screen.path(), "nativeSource":source}));
        }
        let discovery =
            json!({"kind":"native-source", "declared":nodes.len(), "limitations":self.limitations});
        let graph = json!({"nodes":nodes, "overview":[], "cells":[], "edges":[], "screenDiscovery":discovery});
        let report = json!({"screenDiscovery":discovery, "sourceFiles":sources, "cells":[],
            "coverage":{"attempted":0,"renderable":0,"unrenderable":0,"verified":0}});
        analysis::store(&out.join("graph.json"), &graph)?;
        analysis::store(&out.join("report.json"), &report)?;
        Ok(report)
    }
}
