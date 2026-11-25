//! Build a Swift app for the iOS Simulator with an index store, then read its SwiftUI screens.
//! The build goes to dui-owned DerivedData, so mapping never modifies the project.
use super::{Error, Inventory, output};
use crate::{archive, replay::io};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) struct Project {
    container: Container,
    scheme: Option<String>,
}

enum Container {
    Workspace(PathBuf),
    Project(PathBuf),
    Package,
}

/// An Xcode workspace or project in the app root, or a Swift package.
pub(crate) fn detect(app: &Path, profile: &Value) -> Result<Option<Project>, Error> {
    let mut workspaces = Vec::new();
    let mut projects = Vec::new();
    for entry in fs::read_dir(app).map_err(io)? {
        let path = entry.map_err(io)?.path();
        match path.extension().and_then(|e| e.to_str()) {
            Some("xcworkspace") => workspaces.push(path),
            Some("xcodeproj") => projects.push(path),
            Some(_) | None => {}
        }
    }
    workspaces.sort();
    projects.sort();
    let container = if let Some(workspace) = workspaces.into_iter().next() {
        Container::Workspace(workspace)
    } else if let Some(project) = projects.into_iter().next() {
        Container::Project(project)
    } else if app.join("Package.swift").is_file() {
        Container::Package
    } else {
        return Ok(None);
    };
    Ok(Some(Project {
        container,
        scheme: profile["scheme"].as_str().map(str::to_owned),
    }))
}

fn limited(reason: String) -> Inventory {
    Inventory {
        screens: vec![],
        limitations: vec![reason],
    }
}

impl Project {
    fn container_args(&self) -> Vec<String> {
        match &self.container {
            Container::Workspace(path) => vec!["-workspace".into(), path.display().to_string()],
            Container::Project(path) => vec!["-project".into(), path.display().to_string()],
            Container::Package => vec![],
        }
    }

    pub(crate) fn discover(&self, app: &Path) -> Result<Inventory, Error> {
        if !cfg!(target_os = "macos") {
            return Ok(limited(
                "SwiftUI screen discovery needs macOS with Xcode.".into(),
            ));
        }
        let has_xcode = Command::new("xcrun")
            .args(["--find", "xcodebuild"])
            .output()
            .is_ok_and(|found| found.status.success());
        if !has_xcode {
            return Ok(limited(
                "SwiftUI screen discovery needs Xcode: xcrun cannot find xcodebuild.".into(),
            ));
        }
        let scheme = match &self.scheme {
            Some(scheme) => scheme.clone(),
            None => {
                let schemes = self.schemes(app)?;
                match schemes.as_slice() {
                    [only] => only.clone(),
                    [] | [_, _, ..] => {
                        return Ok(limited(format!(
                            "Set \"scheme\" in dui.json to one of: {}.",
                            schemes.join(", ")
                        )));
                    }
                }
            }
        };
        let mut digest = Sha256::new();
        digest.update(app.as_os_str().as_encoded_bytes());
        let derived = archive::home_dir()?
            .join("swift-index")
            .join(format!("{:x}", digest.finalize()));
        let build = Command::new("xcodebuild")
            .args(self.container_args())
            .args([
                "-scheme",
                &scheme,
                "-destination",
                "generic/platform=iOS Simulator",
            ])
            .arg("-derivedDataPath")
            .arg(&derived)
            .args([
                "-quiet",
                "build",
                "CODE_SIGNING_ALLOWED=NO",
                "COMPILER_INDEX_STORE_ENABLE=YES",
            ])
            .current_dir(app)
            .output()
            .map_err(io)?;
        let mut limitations = Vec::new();
        if !build.status.success() {
            let log = String::from_utf8_lossy(&build.stdout).into_owned()
                + &String::from_utf8_lossy(&build.stderr);
            let errors: Vec<&str> = log
                .lines()
                .filter(|line| line.contains("error:"))
                .take(3)
                .collect();
            limitations.push(format!(
                "xcodebuild -scheme {scheme} failed, so only files indexed before the failure were read: {}",
                errors.join(" | ")
            ));
        }
        let store = derived.join("Index.noindex").join("DataStore");
        if !store.is_dir() {
            limitations.push("The build produced no index store.".into());
            return Ok(Inventory {
                screens: vec![],
                limitations,
            });
        }
        let found = output(
            Command::new(tool()?)
                .arg(app)
                .arg(&store)
                .arg(index_store_library()?),
            "discover SwiftUI screens",
        )?;
        let mut inventory: Inventory = serde_json::from_slice(&found.stdout).map_err(io)?;
        limitations.append(&mut inventory.limitations);
        inventory.limitations = limitations;
        Ok(inventory)
    }

    fn schemes(&self, app: &Path) -> Result<Vec<String>, Error> {
        let listed = output(
            Command::new("xcodebuild")
                .args(self.container_args())
                .args(["-list", "-json"])
                .current_dir(app),
            "list Xcode schemes",
        )?;
        let list: Value = serde_json::from_slice(&listed.stdout).map_err(io)?;
        let schemes = ["workspace", "project"]
            .iter()
            .find_map(|key| list[key]["schemes"].as_array())
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| name.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        Ok(schemes)
    }
}

/// The toolchain's index store reader, from the same Xcode that built the app.
fn index_store_library() -> Result<PathBuf, Error> {
    let swift = output(
        Command::new("xcrun").args(["--find", "swift"]),
        "locate the Swift toolchain",
    )?;
    let swift = PathBuf::from(String::from_utf8_lossy(&swift.stdout).trim());
    let library = swift
        .parent()
        .and_then(Path::parent)
        .map(|toolchain| toolchain.join("lib").join("libIndexStore.dylib"))
        .filter(|library| library.is_file());
    library.ok_or_else(|| crate::failure(3, "libIndexStore.dylib not found next to swift"))
}

/// Build the locked discovery tool once per toolchain and source version, in dui's cache.
fn tool() -> Result<PathBuf, Error> {
    let version = output(
        Command::new("xcrun").args(["swift", "--version"]),
        "locate Swift",
    )?;
    let files = [
        (
            "Package.swift",
            include_str!("../../../../adapters/native-map/swift/Package.swift"),
        ),
        (
            "Package.resolved",
            include_str!("../../../../adapters/native-map/swift/Package.resolved"),
        ),
        (
            "Sources/DuiSwiftDiscover/Discover.swift",
            include_str!(
                "../../../../adapters/native-map/swift/Sources/DuiSwiftDiscover/Discover.swift"
            ),
        ),
        (
            "Sources/dui-swift-discover/main.swift",
            include_str!(
                "../../../../adapters/native-map/swift/Sources/dui-swift-discover/main.swift"
            ),
        ),
    ];
    let mut digest = Sha256::new();
    digest.update(&version.stdout);
    for (name, contents) in files {
        digest.update(name.as_bytes());
        digest.update(contents.as_bytes());
    }
    let tool = archive::home_dir()?
        .join("tools")
        .join("swift-map")
        .join(format!("{:x}", digest.finalize()));
    fs::create_dir_all(&tool).map_err(io)?;
    let lock = fs::File::options()
        .write(true)
        .create(true)
        .truncate(false)
        .open(tool.join("prepare.lock"))
        .map_err(io)?;
    lock.lock().map_err(io)?;
    for (name, contents) in files {
        let file = tool.join(name);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(io)?;
        }
        if !file.is_file() {
            fs::write(file, contents).map_err(io)?;
        }
    }
    let binary = tool
        .join(".build")
        .join("release")
        .join("dui-swift-discover");
    if !binary.is_file() {
        output(
            Command::new("xcrun")
                .args([
                    "swift",
                    "build",
                    "-c",
                    "release",
                    "--force-resolved-versions",
                ])
                .arg("--package-path")
                .arg(&tool),
            "build the SwiftUI discovery tool",
        )?;
    }
    Ok(binary)
}
