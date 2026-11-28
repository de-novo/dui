use dui_lang_typescript::{ParseCache, extract, extract_incremental};
use std::{fs, path::Path};

fn write(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
    fs::write(file, text).expect("write");
}

fn fixture(root: &Path) {
    write(
        root,
        "src/main.tsx",
        r#"import { Child } from "./child";
export default function App() {
  return <main><Child /></main>;
}
"#,
    );
    write(
        root,
        "src/child.tsx",
        r#"export function Child() {
  return <p>before</p>;
}
"#,
    );
}

#[test]
fn dependency_edits_match_a_fresh_extraction() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    fixture(root);
    let entry = root.join("src/main.tsx");
    let mut cache = ParseCache::default();
    let (initial, stats) = extract_incremental(root, &entry, &mut cache).expect("initial");
    assert!(stats.parsed.iter().any(|path| path == "src/child.tsx"));
    write(
        root,
        "src/child.tsx",
        r#"export function Child() {
  return <p>after dependency edit</p>;
}
"#,
    );
    let (incremental, _) = extract_incremental(root, &entry, &mut cache).expect("incremental");
    let fresh = extract(root, &entry).expect("fresh");
    assert_eq!(incremental, fresh);
    assert_ne!(incremental, initial);
}

#[test]
fn newly_added_imports_match_a_fresh_extraction() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    fixture(root);
    let entry = root.join("src/main.tsx");
    let mut cache = ParseCache::default();
    extract_incremental(root, &entry, &mut cache).expect("initial");
    write(
        root,
        "src/main.tsx",
        r#"import { Child } from "./child";
import { Added } from "./added";
export default function App() {
  return <main><Child /><Added /></main>;
}
"#,
    );
    write(
        root,
        "src/added.tsx",
        r#"export function Added() {
  return <span>new file visible</span>;
}
"#,
    );
    let (incremental, _) = extract_incremental(root, &entry, &mut cache).expect("incremental");
    let fresh = extract(root, &entry).expect("fresh");
    assert_eq!(incremental, fresh);
}

#[test]
fn global_css_edits_match_a_fresh_extraction() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    fixture(root);
    write(root, "src/index.css", "p { color: red; }\n");
    write(
        root,
        "src/main.tsx",
        r#"import "./index.css";
import { Child } from "./child";
export default function App() {
  return <main><Child /></main>;
}
"#,
    );
    let entry = root.join("src/main.tsx");
    let mut cache = ParseCache::default();
    extract_incremental(root, &entry, &mut cache).expect("initial");
    write(root, "src/index.css", "p { color: blue; }\n");
    let (incremental, _) = extract_incremental(root, &entry, &mut cache).expect("incremental");
    let fresh = extract(root, &entry).expect("fresh");
    assert_eq!(incremental, fresh);
}

#[test]
fn unchanged_files_are_reused_without_reparsing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    fixture(root);
    let entry = root.join("src/main.tsx");
    let mut cache = ParseCache::default();
    let (initial, _) = extract_incremental(root, &entry, &mut cache).expect("initial");
    let (again, stats) = extract_incremental(root, &entry, &mut cache).expect("again");
    assert_eq!(again, initial);
    assert!(
        stats.parsed.is_empty(),
        "unexpected parses: {:?}",
        stats.parsed
    );
    assert!(stats.reused.iter().any(|path| path == "src/main.tsx"));
    assert!(stats.reused.iter().any(|path| path == "src/child.tsx"));
}

#[test]
fn rapid_same_size_edits_match_a_fresh_extraction() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    fixture(root);
    let entry = root.join("src/main.tsx");
    let mut cache = ParseCache::default();
    extract_incremental(root, &entry, &mut cache).expect("initial");
    for marker in ["aaaa", "bbbb", "cccc", "dddd", "eeee", "ffff"] {
        write(
            root,
            "src/child.tsx",
            &format!(
                r#"export function Child() {{
  return <p>{marker}</p>;
}}
"#
            ),
        );
        let (incremental, stats) =
            extract_incremental(root, &entry, &mut cache).expect("incremental");
        let fresh = extract(root, &entry).expect("fresh");
        assert_eq!(incremental, fresh, "same-size edit {marker} was stale");
        assert!(
            stats.parsed.iter().any(|path| path == "src/child.tsx"),
            "same-size edit {marker} did not reparse: {:?}",
            stats.parsed
        );
    }
}

#[test]
fn dynamic_imports_are_discovered_and_tracked() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    write(
        root,
        "src/main.tsx",
        r#"const Child = React.lazy(() => import /* deferred */ ( "./child" ));
export default function App() {
  return <main><Child /></main>;
}
"#,
    );
    write(
        root,
        "src/child.tsx",
        r#"export function Child() {
  return <p>lazy before</p>;
}
"#,
    );
    let entry = root.join("src/main.tsx");
    let mut cache = ParseCache::default();
    let (initial, _) = extract_incremental(root, &entry, &mut cache).expect("initial");
    let main = initial
        .files
        .iter()
        .find(|file| file.path == "src/main.tsx")
        .expect("main file");
    assert!(main.imports.iter().any(|path| path == "src/child.tsx"));
    write(
        root,
        "src/child.tsx",
        r#"export function Child() {
  return <p>lazy after</p>;
}
"#,
    );
    let (incremental, _) = extract_incremental(root, &entry, &mut cache).expect("incremental");
    let fresh = extract(root, &entry).expect("fresh");
    assert_eq!(incremental, fresh);
}

#[test]
fn dependency_prop_signature_edits_invalidate_dependent_renders() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    write(
        root,
        "src/main.tsx",
        r#"import { Child } from "./child";
export default function App() {
  return <main><Child label="visible" /></main>;
}
"#,
    );
    write(
        root,
        "src/child.tsx",
        r#"export function Child({ label }: { label: string }) {
  return <p>{label}</p>;
}
"#,
    );
    let entry = root.join("src/main.tsx");
    let mut cache = ParseCache::default();
    let (initial, _) = extract_incremental(root, &entry, &mut cache).expect("initial");
    write(
        root,
        "src/child.tsx",
        r#"export function Child({ text }: { text: string }) {
  return <p>{text}</p>;
}
"#,
    );
    let (incremental, _) = extract_incremental(root, &entry, &mut cache).expect("incremental");
    let fresh = extract(root, &entry).expect("fresh");
    assert_eq!(incremental, fresh);
    assert_ne!(incremental, initial);
}

#[test]
fn bare_project_imports_resolve_without_external_package_diagnostics() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    write(
        root,
        "src/main.tsx",
        r#"import React from 'react';
import { Child } from 'src/child';
export default function App() {
  return <main><Child /></main>;
}
"#,
    );
    write(
        root,
        "src/child.tsx",
        r#"export function Child() {
  return <p>root relative</p>;
}
"#,
    );
    let model = extract(root, &root.join("src/main.tsx")).expect("extract");
    let main = model
        .files
        .iter()
        .find(|file| file.path == "src/main.tsx")
        .expect("main file");
    assert!(main.imports.iter().any(|path| path == "src/child.tsx"));
    assert!(
        !model
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unresolved-import"
                && diagnostic.message == "react"),
        "external package import became a project diagnostic: {:?}",
        model.diagnostics
    );
}
