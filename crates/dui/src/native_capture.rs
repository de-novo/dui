//! Attach recorder-supplied pixels to declarations in an immutable native source map.
use crate::{
    Reply, analysis, archive, failure,
    replay::{Error, io, read},
};
use dui_core::model::{ProjectId, Screen, ScreenId, ScreenOrigin, State, Width};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::BufReader,
    path::{Component, Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recordings {
    source_snapshot: String,
    captures: Vec<Capture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Capture {
    screen_id: ScreenId,
    runtime: String,
    image: PathBuf,
    width: Width,
    observed_path: String,
}

fn invalid(message: impl std::fmt::Display) -> Error {
    failure(2, &message.to_string())
}

pub(crate) fn attach(source: &Path, manifest: &Path) -> Reply {
    let manifest = manifest.canonicalize().map_err(invalid)?;
    let image_root = manifest
        .parent()
        .ok_or_else(|| invalid("missing recording directory"))?;
    let recordings: Recordings =
        serde_json::from_reader(BufReader::new(fs::File::open(&manifest).map_err(invalid)?))
            .map_err(invalid)?;
    if recordings.captures.is_empty() {
        return Err(invalid("expected at least one recording"));
    }

    let source = source.canonicalize().map_err(invalid)?;
    let snapshot = read(&source.join("snapshot.json"))?;
    let project_root = snapshot["projectRoot"]
        .as_str()
        .ok_or_else(|| invalid("missing source project root"))?;
    let project = ProjectId::from_primary_root_bytes(project_root.as_bytes());
    let snapshot_id = source
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| invalid("missing source run ID"))?;
    if snapshot["projectId"] != project.as_str()
        || snapshot["id"] != snapshot_id
        || recordings.source_snapshot != snapshot_id
        || source
            != archive::home_dir()?
                .join("projects")
                .join(project.as_str())
                .join("runs")
                .join(snapshot_id)
    {
        return Err(invalid(
            "sourceSnapshot must identify the original local archive run",
        ));
    }

    let mut graph = read(&source.join("graph.json"))?;
    let mut report = read(&source.join("report.json"))?;
    if report["screenDiscovery"]["kind"] != "native-source"
        || ["attempted", "renderable", "unrenderable", "verified"]
            .iter()
            .any(|key| report["coverage"][key].as_u64() != Some(0))
        || graph["screenDiscovery"] != report["screenDiscovery"]
        || [
            &report["cells"],
            &graph["overview"],
            &graph["cells"],
            &graph["edges"],
        ]
        .iter()
        .any(|items| !items.as_array().is_some_and(Vec::is_empty))
    {
        return Err(invalid("expected a source-only native screen map"));
    }
    let nodes = graph["nodes"]
        .as_array()
        .ok_or_else(|| invalid("missing native declarations"))?;
    if report["screenDiscovery"]["declared"].as_u64() != Some(nodes.len() as u64) {
        return Err(invalid("source declaration count does not match graph"));
    }
    let mut declarations = BTreeMap::new();
    let mut label_counts = BTreeMap::new();
    for node in nodes {
        let declaration = serde_json::from_value(node["nativeSource"].clone()).map_err(invalid)?;
        let path = node["label"]
            .as_str()
            .ok_or_else(|| invalid("missing declaration path"))?;
        let screen = Screen::new(path, ScreenOrigin::Native { declaration }).map_err(invalid)?;
        if node["kind"] != "screen"
            || node["id"] != screen.id().as_str()
            || declarations
                .insert(screen.id().clone(), path.to_owned())
                .is_some()
        {
            return Err(invalid("invalid native declaration ID in source map"));
        }
        *label_counts.entry(path.to_owned()).or_insert(0_usize) += 1;
    }
    archive::publish(snapshot, |out| {
        fs::create_dir_all(out).map_err(io)?;
        let mut cells = Vec::new();
        let mut seen = BTreeSet::new();
        for capture in &recordings.captures {
            let path = declarations
                .get(&capture.screen_id)
                .ok_or_else(|| invalid(format!("unknown screenId {}", capture.screen_id)))?;
            if label_counts.get(path) != Some(&1) {
                return Err(invalid(format!("ambiguous declaration path {path}")));
            }
            if !seen.insert((&capture.screen_id, &capture.runtime, capture.width)) {
                return Err(invalid(format!(
                    "duplicate recording for {path} on {} at {} px",
                    capture.runtime, capture.width
                )));
            }
            cells.push(capture.import(path, image_root, out)?);
        }
        report["sourceSnapshot"] = json!(recordings.source_snapshot);
        report["cells"] = json!(cells);
        graph["overview"] = report["cells"].clone();
        graph["cells"] = report["cells"].clone();
        analysis::store(&out.join("graph.json"), &graph)?;
        analysis::store(&out.join("report.json"), &report)?;
        Ok(report)
    })
}

impl Capture {
    fn import(&self, path: &str, root: &Path, out: &Path) -> Reply {
        if !matches!(self.runtime.as_str(), "web" | "android" | "ios" | "desktop") {
            return Err(invalid("runtime must be web, android, ios or desktop"));
        }
        if !concrete_route(&self.observed_path, path) {
            return Err(invalid(
                "expected a concrete observedPath or the declared navigation key",
            ));
        }
        if self.image.is_absolute()
            || self
                .image
                .components()
                .any(|c| matches!(c, Component::ParentDir))
        {
            return Err(invalid("image must be relative to the recording directory"));
        }
        let source = root.join(&self.image).canonicalize().map_err(invalid)?;
        if !source.starts_with(root) || !source.is_file() {
            return Err(invalid(
                "image must be a file inside the recording directory",
            ));
        }
        let image = format!("{}-{}-{}.png", self.screen_id, self.runtime, self.width);
        let destination = out.join(&image);
        fs::copy(source, &destination).map_err(io)?;
        let (w, h) = png_dimensions(&destination)?;
        let inputs = BTreeMap::from([("observedPath".to_owned(), json!(self.observed_path))]);
        let state = State::new(
            self.screen_id.clone(),
            &self.runtime,
            None,
            inputs.clone(),
            0,
        )
        .map_err(invalid)?;
        Ok(json!({
            "screenId": self.screen_id, "path": path, "observedPath": self.observed_path,
            "runtime": self.runtime, "scenario": self.runtime, "state": state.id(), "input": inputs, "planIndex": 0,
            "width": self.width, "image": image, "imageWidth": w, "imageHeight": h,
            "status": "unverified", "reason": format!("Imported {} recording without automated verification", self.runtime),
            "observation": {"path": self.observed_path, "width": self.width}
        }))
    }
}

fn concrete_route(observed: &str, declaration: &str) -> bool {
    if observed.is_empty() || observed.trim() != observed || observed.chars().any(char::is_control)
    {
        return false;
    }
    let path = observed.split(['?', '#']).next().unwrap_or_default();
    (path.starts_with('/') && !path.starts_with("//") || observed == declaration)
        && !path.contains(['*', '[', ']', '{', '}', '\\'])
        && !path
            .split('/')
            .any(|part| part.starts_with(':') || matches!(part, ".." | "."))
}

fn png_dimensions(path: &Path) -> Result<(u32, u32), Error> {
    let decoder = png::Decoder::new(BufReader::new(fs::File::open(path).map_err(io)?));
    let mut reader = decoder.read_info().map_err(invalid)?;
    if reader.info().animation_control.is_some() {
        return Err(invalid("expected a still PNG capture"));
    }
    let size = (reader.info().width, reader.info().height);
    while reader.next_row().map_err(invalid)?.is_some() {}
    reader.finish().map_err(invalid)?;
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_concrete_routes_and_named_screens() {
        assert!(concrete_route("/h/a%2Fb?q[0]=b", "/h/[id]"));
        assert!(concrete_route("Project settings", "Project settings"));
        for pattern in ["/h/[id]", "/h/:id", "/h/*", "//host/path", "", "/a/../b"] {
            assert!(!concrete_route(pattern, "/h/[id]"));
        }
    }

    #[test]
    fn rejects_recordings_outside_the_manifest_directory() {
        let dir = tempfile::tempdir().expect("create test directory");
        let root = dir.path().join("recordings");
        fs::create_dir(&root).expect("create recording directory");
        let image = dir.path().join("other.png");
        fs::write(&image, []).expect("write external file");
        std::os::unix::fs::symlink(&image, root.join("link.png"))
            .expect("symlink to external file");
        for relative in ["../other.png", "link.png"] {
            let capture = Capture {
                screen_id: Screen::derive_id("/"),
                runtime: "ios".into(),
                image: relative.into(),
                width: Width::new(390).expect("valid width"),
                observed_path: "/".into(),
            };
            let (_, error) = capture
                .import("/", &root, dir.path())
                .expect_err("external image");
            assert!(
                error["reason"]
                    .as_str()
                    .is_some_and(|message| message.contains("recording directory"))
            );
        }
    }

    #[test]
    fn reads_pixels_independently_of_viewport_and_rejects_truncated_png() {
        let dir = tempfile::tempdir().expect("create recording directory");
        let source = dir.path().join("screen.png");
        let mut bytes = Vec::new();
        let mut writer = png::Encoder::new(&mut bytes, 2, 3)
            .write_header()
            .expect("PNG header");
        writer.write_image_data(&[127; 6]).expect("PNG pixels");
        writer.finish().expect("finish PNG");
        fs::write(&source, &bytes).expect("write PNG");
        assert_eq!(png_dimensions(&source).expect("valid PNG"), (2, 3));
        fs::write(&source, &bytes[..bytes.len() - 12]).expect("truncate PNG");
        assert!(png_dimensions(&source).is_err());
    }
}
