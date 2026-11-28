//! Extract schema-v1 models from a TSX entry and its reachable imports.
mod class_view;
mod fonts;
mod hash_routes;
mod query_views;
mod render;
mod root_classes;
mod routes;
mod style;
mod syntax;
use dui_core::source_model as m;
pub use fonts::cached_font_bytes;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use syntax::{File, children, field, first, unwrap_expr};
pub use syntax::{LoadStats, ParseCache};
use tree_sitter::Node as Ast;

type Files = BTreeMap<String, Arc<File>>;
fn symbol<'a>(files: &'a Files, file: &'a File, name: &'a str) -> Option<(&'a File, &'a str)> {
    if let Some((module, export)) = file.bindings.get(name) {
        let target = files.get(module)?;
        Some((target, target.exported(export)?))
    } else {
        Some((file, name))
    }
}
fn literal(file: &File, node: Ast<'_>) -> Option<Value> {
    let n = unwrap_expr(node);
    let text = file.text(n);
    match n.kind() {
        "string" if text.starts_with('\'') && text.ends_with('\'') && !text.contains('\\') => {
            Some(text[1..text.len() - 1].into())
        }
        "number" => serde_json::from_str(text)
            .ok()
            .or_else(|| serde_json::Number::from_f64(text.parse().ok()?).map(Value::Number)),
        "string" | "true" | "false" | "null" => serde_json::from_str(text).ok(),
        _ => None,
    }
}
fn unknown() -> m::TypeRef {
    m::TypeRef::Unknown { text: None }
}
fn type_ref(files: &Files, file: &File, node: Option<Ast<'_>>) -> m::TypeRef {
    let Some(n) = node.map(unwrap_expr) else {
        return unknown();
    };
    let text = file.text(n);
    match (n.kind(), text) {
        ("predefined_type", "boolean") => m::TypeRef::Boolean,
        ("predefined_type", "string") => m::TypeRef::String,
        ("predefined_type", "number") => m::TypeRef::Number,
        ("function_type", _) => m::TypeRef::Function,
        ("array_type", _) => m::TypeRef::Array {
            item: Box::new(type_ref(files, file, first(n))),
        },
        ("union_type", _) => {
            let vals: Option<Vec<_>> = children(n).into_iter().map(|c| literal(file, c)).collect();
            if let Some(values) = vals {
                return m::TypeRef::Enum { values };
            }
            m::TypeRef::Unknown {
                text: Some(text.into()),
            }
        }
        ("type_identifier", _) => {
            if let Some((f, name)) = symbol(files, file, text)
                && f.declarations().iter().any(|d| {
                    d.kind() == "interface_declaration" && f.name(*d, "name") == Some(name)
                })
            {
                return m::TypeRef::Object {
                    schema: format!("{}#{name}", f.path),
                };
            }
            m::TypeRef::Unknown {
                text: Some(text.into()),
            }
        }
        _ => m::TypeRef::Unknown {
            text: Some(text.into()),
        },
    }
}
fn fields(files: &Files, file: &File, body: Ast<'_>) -> Vec<m::Field> {
    children(body)
        .into_iter()
        .filter(|c| c.kind() == "property_signature")
        .filter_map(|c| {
            Some(m::Field {
                name: file.name(c, "name")?.into(),
                ty: type_ref(files, file, field(c, "type")),
                optional: c.children(&mut c.walk()).any(|n| n.kind() == "?"),
            })
        })
        .collect()
}
fn returned(f: Ast<'_>) -> Option<Ast<'_>> {
    if f.kind() == "method_definition" {
        return class_view::tail(f);
    }
    let body = field(f, "body")?;
    if body.kind() != "statement_block" {
        return Some(unwrap_expr(body));
    }
    let stmts = children(body);
    // A conditional early return changes the root. Keep such functions as placeholders.
    if stmts.iter().any(|n| {
        !matches!(
            n.kind(),
            "return_statement"
                | "lexical_declaration"
                | "variable_declaration"
                | "expression_statement"
                | "function_declaration"
        )
    }) || stmts
        .iter()
        .filter(|n| n.kind() == "return_statement")
        .count()
        != 1
    {
        return None;
    }
    stmts
        .into_iter()
        .find(|n| n.kind() == "return_statement")
        .and_then(first)
        .map(unwrap_expr)
}
fn slots(files: &Files, file: &File, f: Ast<'_>, id: &str) -> Vec<m::Slot> {
    if f.kind() == "method_definition" {
        return class_view::props(file, f, id);
    }
    let mut out = Vec::new();
    if let Some(param) = field(f, "parameters").and_then(first)
        && let Some(pat) = field(param, "pattern").filter(|p| p.kind() == "object_pattern")
    {
        let props = field(param, "type")
            .map(unwrap_expr)
            .map(|n| fields(files, file, n))
            .unwrap_or_default();
        for bind in children(pat) {
            if bind.kind() == "rest_pattern"
                && let Some(rest) = children(bind)
                    .into_iter()
                    .find(|n| n.kind() == "identifier")
            {
                let rest_name = file.text(rest);
                out.push(m::Slot {
                    id: format!("{id}...{rest_name}"),
                    name: format!("...{rest_name}"),
                    kind: m::SlotKind::Prop,
                    ty: m::TypeRef::Unknown {
                        text: Some("rest props".into()),
                    },
                    optional: true,
                    initial: None,
                    source: None,
                    origin: m::Origin::Syntax,
                    location: file.loc(rest),
                });
                continue;
            }
            let (name_node, default) = match bind.kind() {
                "shorthand_property_identifier_pattern" => (Some(bind), None),
                "object_assignment_pattern" => (field(bind, "left"), field(bind, "right")),
                "pair_pattern" => {
                    let value = field(bind, "value");
                    if value.is_some_and(|v| v.kind() == "assignment_pattern") {
                        (
                            field(value.unwrap(), "left"),
                            field(value.unwrap(), "right"),
                        )
                    } else {
                        (value, None)
                    }
                }
                _ => (None, None),
            };
            let Some(name_node) = name_node.filter(|n| {
                matches!(
                    n.kind(),
                    "identifier" | "shorthand_property_identifier_pattern"
                )
            }) else {
                continue;
            };
            let name = file.text(name_node);
            let prop = props.iter().find(|p| p.name == name);
            let mut ty = prop.map(|p| p.ty.clone()).unwrap_or_else(unknown);
            if name == "children" && matches!(ty, m::TypeRef::Unknown { .. } | m::TypeRef::Node) {
                ty = m::TypeRef::Node;
            }
            out.push(m::Slot {
                id: format!("{id}.{name}"),
                name: name.into(),
                kind: m::SlotKind::Prop,
                ty,
                optional: prop.is_some_and(|p| p.optional) || default.is_some(),
                initial: default.map(|n| {
                    literal(file, n)
                        .map(|value| m::Expr::Literal { value })
                        .unwrap_or_else(|| m::Expr::Opaque {
                            text: file.text(n).into(),
                            reads: vec![],
                        })
                }),
                source: None,
                origin: m::Origin::Syntax,
                location: file.loc(name_node),
            });
        }
    }
    let Some(body) = field(f, "body") else {
        return out;
    };
    for var in children(body)
        .into_iter()
        .filter(|d| matches!(d.kind(), "lexical_declaration" | "variable_declaration"))
        .flat_map(children)
    {
        let (Some(pat), Some(call)) = (
            field(var, "name"),
            field(var, "value").filter(|c| c.kind() == "call_expression"),
        ) else {
            continue;
        };
        let Some(callee) = file.name(call, "function") else {
            continue;
        };
        let local = file
            .bindings
            .get(callee)
            .is_some_and(|(module, name)| module == "react" && name == "useState");
        let binding = if local && pat.kind() == "array_pattern" {
            pat.child(1).filter(|c| c.kind() == "identifier")
        } else if !local && pat.kind() == "identifier" {
            Some(pat)
        } else {
            None
        };
        let Some(bind) = binding else { continue };
        let name = file.text(bind);
        let arg = field(call, "arguments").and_then(first);
        let initial = if local {
            arg.map(|n| {
                literal(file, n)
                    .map(|value| m::Expr::Literal { value })
                    .unwrap_or_else(|| m::Expr::Opaque {
                        text: file.text(n).into(),
                        reads: vec![],
                    })
            })
        } else {
            None
        };
        let mut ty = type_ref(files, file, field(var, "type"));
        if local {
            ty = if let Some(t) = field(call, "type_arguments").and_then(first) {
                type_ref(files, file, Some(t))
            } else {
                match arg.and_then(|n| literal(file, n)) {
                    Some(Value::Bool(_)) => m::TypeRef::Boolean,
                    Some(Value::String(_)) => m::TypeRef::String,
                    Some(Value::Number(_)) => m::TypeRef::Number,
                    _ => unknown(),
                }
            };
        } else if field(var, "type").is_none()
            && let Some((target, name)) = symbol(files, file, callee)
            && let Some(decl) = target.declarations().into_iter().find(|n| {
                matches!(n.kind(), "function_signature" | "function_declaration")
                    && target.name(*n, "name") == Some(name)
            })
        {
            ty = type_ref(files, target, field(decl, "return_type"));
        }
        out.push(m::Slot {
            id: format!("{id}.{name}"),
            name: name.into(),
            kind: if local {
                m::SlotKind::Local
            } else {
                m::SlotKind::External
            },
            ty,
            optional: false,
            initial,
            source: (!local).then(|| file.text(call).into()),
            origin: m::Origin::Syntax,
            location: file.loc(var),
        });
    }
    out
}

fn stylesheet_order(files: &Files, entry: &str) -> Vec<String> {
    fn visit(
        files: &Files,
        path: &str,
        active: &mut std::collections::BTreeSet<String>,
        seen: &mut std::collections::BTreeSet<String>,
        out: &mut Vec<String>,
    ) {
        if !active.insert(path.to_owned()) {
            return;
        }
        if seen.insert(path.to_owned())
            && let Some(file) = files.get(path)
        {
            for import in &file.ordered_imports {
                visit(files, import, active, seen, out);
            }
            if path.ends_with(".css") && !out.iter().any(|item| item == path) {
                out.push(path.to_owned());
            }
        }
        active.remove(path);
    }
    let mut out = Vec::new();
    visit(
        files,
        entry,
        &mut std::collections::BTreeSet::new(),
        &mut std::collections::BTreeSet::new(),
        &mut out,
    );
    out
}

/// Immutable literal bindings visible to a straightforward component body. This is deliberately
/// narrower than JavaScript scope: only module constants and constants directly preceding the
/// component return participate, so nested shadowing cannot make an unrelated value look proven.
fn env_literal(file: &File, node: Ast<'_>, slots: &[m::Slot]) -> Option<Value> {
    let node = crate::syntax::unwrap_expr(node);
    if let Some(value) = render::static_literal(file, node) {
        return Some(value);
    }
    if node.kind() == "identifier" {
        return slots
            .iter()
            .find(|slot| slot.name == file.text(node))
            .and_then(|slot| slot.initial.as_ref())
            .and_then(|expr| match expr {
                m::Expr::Literal { value } => Some(value.clone()),
                _ => None,
            });
    }
    if node.kind() == "ternary_expression" {
        let condition = env_literal(file, crate::syntax::field(node, "condition")?, slots)?;
        let truthy = match condition {
            Value::Null => false,
            Value::Bool(value) => value,
            Value::Number(value) => value.as_f64().is_some_and(|v| v != 0.),
            Value::String(value) => !value.is_empty(),
            Value::Array(_) | Value::Object(_) => true,
        };
        return env_literal(
            file,
            crate::syntax::field(node, if truthy { "consequence" } else { "alternative" })?,
            slots,
        );
    }
    None
}

fn constant_env(file: &File, function: Ast<'_>, slots: &[m::Slot]) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for node in children(file.tree.root_node()) {
        let declarators: Vec<_> = if node.kind() == "export_statement" {
            field(node, "declaration").into_iter().collect()
        } else {
            vec![node]
        };
        for declaration in declarators {
            if matches!(
                declaration.kind(),
                "lexical_declaration" | "variable_declaration"
            ) {
                for declarator in children(declaration) {
                    if file.text(declaration).trim_start().starts_with("const")
                        && let Some(name_node) =
                            field(declarator, "name").filter(|n| n.kind() == "identifier")
                        && let Some(value) = field(declarator, "value")
                            .and_then(|value| env_literal(file, value, slots))
                    {
                        out.insert(file.text(name_node).into(), value);
                    }
                }
            }
        }
    }
    if let Some(body) = field(function, "body").filter(|body| body.kind() == "statement_block") {
        for statement in children(body) {
            if statement.kind() == "return_statement" {
                break;
            }
            if matches!(
                statement.kind(),
                "lexical_declaration" | "variable_declaration"
            ) {
                for declarator in children(statement) {
                    if file.text(statement).trim_start().starts_with("const")
                        && let Some(name_node) =
                            field(declarator, "name").filter(|n| n.kind() == "identifier")
                        && let Some(value) = field(declarator, "value")
                            .and_then(|value| env_literal(file, value, slots))
                    {
                        out.insert(file.text(name_node).into(), value);
                    }
                }
            }
        }
    }
    out
}

fn discover_built_css(project: &Path) -> Option<std::path::PathBuf> {
    for directory in ["dist", "build"] {
        let root = project.join(directory);
        if !root.is_dir() {
            continue;
        }
        let mut pending = vec![root];
        let mut found = Vec::new();
        while let Some(dir) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                    pending.push(path);
                } else if path.extension().is_some_and(|ext| ext == "css") {
                    found.push(path);
                }
            }
        }
        found.sort();
        if let [path] = found.as_slice() {
            return path.strip_prefix(project).ok().map(|p| p.to_path_buf());
        }
    }
    None
}

pub fn extract(project: &Path, entry: &Path) -> Result<m::SourceModel, String> {
    let built = discover_built_css(project);
    extract_with_css(project, entry, built.as_deref())
}

pub fn extract_with_css(
    project: &Path,
    entry: &Path,
    built_css: Option<&Path>,
) -> Result<m::SourceModel, String> {
    extract_at_width(project, entry, built_css, None)
}

pub fn extract_at_width(
    project: &Path,
    entry: &Path,
    built_css: Option<&Path>,
    width: Option<u32>,
) -> Result<m::SourceModel, String> {
    extract_in_context(project, entry, built_css, width, None)
}

pub fn extract_in_context(
    project: &Path,
    entry: &Path,
    built_css: Option<&Path>,
    width: Option<u32>,
    root_attribute: Option<(&str, &str)>,
) -> Result<m::SourceModel, String> {
    let mut cache = ParseCache::default();
    extract_in_context_impl(
        project,
        entry,
        built_css,
        width,
        root_attribute,
        &mut cache,
        true,
    )
    .map(|(model, _)| model)
}

/// Extract with a caller-owned parse cache. Unchanged files reuse their trees; changed files are
/// the only files parsed again. Import declarations are re-read against the filesystem without
/// re-parsing so added and deleted imports stay correct.
pub fn extract_incremental(
    project: &Path,
    entry: &Path,
    cache: &mut ParseCache,
) -> Result<(m::SourceModel, LoadStats), String> {
    let built = discover_built_css(project);
    extract_in_context_impl(project, entry, built.as_deref(), None, None, cache, true)
}

/// Extract through a watcher that can prove whether module-resolution structure changed.
///
/// Pass `dependency_structure_changed` when any source file was added or removed, or when a
/// module-resolution configuration file changed. When it is false, unchanged parse trees keep
/// their previously scanned imports; files whose bytes changed are still parsed and scanned again.
pub fn extract_incremental_watched(
    project: &Path,
    entry: &Path,
    cache: &mut ParseCache,
    dependency_structure_changed: bool,
) -> Result<(m::SourceModel, LoadStats), String> {
    let built = discover_built_css(project);
    extract_in_context_impl(
        project,
        entry,
        built.as_deref(),
        None,
        None,
        cache,
        dependency_structure_changed,
    )
}

fn timing_enabled() -> bool {
    std::env::var_os("DUI_HOT_TIMING").is_some()
}

fn timing_wall_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

fn log_extract_phase(phase: &str, start: Instant) {
    if timing_enabled() {
        eprintln!(
            "{{\"kind\":\"extract-phase\",\"phase\":\"{}\",\"ms\":{:.3},\"wallMs\":{}}}",
            phase,
            start.elapsed().as_secs_f64() * 1000.0,
            timing_wall_ms()
        );
    }
}

fn log_render_file(path: &str, cached: bool, start: Instant) {
    if timing_enabled() {
        eprintln!(
            "{{\"kind\":\"render-file\",\"path\":\"{}\",\"cached\":{},\"ms\":{:.3},\"wallMs\":{}}}",
            path.replace('\\', "\\").replace('"', "\""),
            cached,
            start.elapsed().as_secs_f64() * 1000.0,
            timing_wall_ms()
        );
    }
}

fn hash_part(hasher: &mut Sha256, part: &str) {
    hasher.update(part.len().to_string());
    hasher.update([0]);
    hasher.update(part.as_bytes());
    hasher.update([0]);
}

fn dependency_closures(files: &Files) -> BTreeMap<String, Vec<String>> {
    let paths: Vec<String> = files.keys().cloned().collect();
    let indexes: BTreeMap<&str, usize> = paths
        .iter()
        .enumerate()
        .map(|(index, path)| (path.as_str(), index))
        .collect();
    let words = paths.len().div_ceil(64);
    let mut reachability = vec![vec![0u64; words]; paths.len()];
    for (index, path) in paths.iter().enumerate() {
        reachability[index][index / 64] |= 1u64 << (index % 64);
        let Some(file) = files.get(path) else {
            continue;
        };
        for dependency in &file.imports {
            if let Some(dependency) = indexes.get(dependency.as_str()) {
                reachability[index][dependency / 64] |= 1u64 << (dependency % 64);
            }
        }
    }
    for pivot in 0..paths.len() {
        let pivot_row = reachability[pivot].clone();
        for row in &mut reachability {
            if row[pivot / 64] & (1u64 << (pivot % 64)) == 0 {
                continue;
            }
            for (cell, pivot_cell) in row.iter_mut().zip(&pivot_row) {
                *cell |= pivot_cell;
            }
        }
    }
    paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let closure = paths
                .iter()
                .enumerate()
                .filter(|(dependency, _)| {
                    reachability[index][dependency / 64] & (1u64 << (dependency % 64)) != 0
                })
                .map(|(_, dependency)| dependency.clone())
                .collect();
            (path.clone(), closure)
        })
        .collect()
}

fn dependency_content_hash(files: &Files, closure: &[String]) -> String {
    let mut hasher = Sha256::new();
    for dependency in closure {
        hash_part(&mut hasher, dependency);
        if let Some(file) = files.get(dependency) {
            hash_part(&mut hasher, &file.sha256);
        }
    }
    format!("{:x}", hasher.finalize())
}

fn semantic_fingerprint(
    files: &Files,
    dependency_hash: &str,
    entry: &str,
    artifact: Option<&str>,
    sheet_order: &[String],
    width: Option<u32>,
    root_attribute: Option<(&str, &str)>,
    theme_classes: style::built::ThemeClasses,
    tailwind: bool,
) -> String {
    let mut hasher = Sha256::new();
    hash_part(&mut hasher, "semantic-v1");
    hash_part(&mut hasher, entry);
    hash_part(&mut hasher, artifact.unwrap_or(""));
    hash_part(
        &mut hasher,
        &width.map_or(String::new(), |width| width.to_string()),
    );
    if let Some((key, value)) = root_attribute {
        hash_part(&mut hasher, key);
        hash_part(&mut hasher, value);
    }
    hash_part(&mut hasher, if theme_classes.dark { "dark" } else { "" });
    hash_part(&mut hasher, if theme_classes.light { "light" } else { "" });
    hash_part(&mut hasher, if tailwind { "tailwind" } else { "" });
    // Global stylesheet order affects the cascade of every component. Module stylesheets are
    // included conservatively because their extracted theme tokens are currently global.
    for path in files
        .values()
        .filter(|file| file.path.ends_with(".css"))
        .map(|file| &file.path)
    {
        hash_part(&mut hasher, path);
        hash_part(&mut hasher, &files[path].sha256);
    }
    for path in sheet_order {
        hash_part(&mut hasher, path);
    }
    hash_part(&mut hasher, dependency_hash);
    format!("{:x}", hasher.finalize())
}

fn font_intake_key(
    files: &Files,
    artifact: Option<&str>,
    resources: &fonts::DocumentResources,
) -> String {
    let mut hasher = Sha256::new();
    hash_part(&mut hasher, "font-intake-v1");
    hash_part(&mut hasher, artifact.unwrap_or(""));
    for file in files.values().filter(|file| file.path.ends_with(".css")) {
        hash_part(&mut hasher, &file.path);
        hash_part(&mut hasher, &file.sha256);
    }
    if let Some(html) = &resources.html_file {
        hash_part(&mut hasher, &html.path);
        hash_part(&mut hasher, &html.sha256);
    }
    for url in &resources.external_stylesheets {
        hash_part(&mut hasher, url);
    }
    for (path, location) in &resources.local_stylesheets {
        hash_part(&mut hasher, path);
        hash_part(&mut hasher, &location.start.to_string());
        hash_part(&mut hasher, &location.end.to_string());
    }
    format!("{:x}", hasher.finalize())
}

fn cached_font_inputs_current(project: &Path, intake: &fonts::FontIntake) -> bool {
    intake.extra_files.iter().all(|file| {
        fs::read(project.join(&file.path))
            .ok()
            .is_some_and(|bytes| format!("{:x}", Sha256::digest(&bytes)) == file.sha256)
    }) && intake.faces.iter().all(|face| {
        face.sources.iter().all(|source| match source {
            m::FontSource::Asset { path, sha256, .. } => fs::read(project.join(path))
                .ok()
                .is_some_and(|bytes| format!("{:x}", Sha256::digest(&bytes)) == *sha256),
            _ => true,
        })
    })
}

fn built_sheet_key(
    file: &File,
    width: Option<u32>,
    root_attribute: Option<(&str, &str)>,
    theme_classes: style::built::ThemeClasses,
) -> String {
    let mut hasher = Sha256::new();
    hash_part(&mut hasher, "built-sheet-v1");
    hash_part(&mut hasher, &file.sha256);
    hash_part(
        &mut hasher,
        &width.map_or(String::new(), |width| width.to_string()),
    );
    if let Some((key, value)) = root_attribute {
        hash_part(&mut hasher, key);
        hash_part(&mut hasher, value);
    }
    hash_part(&mut hasher, if theme_classes.dark { "dark" } else { "" });
    hash_part(&mut hasher, if theme_classes.light { "light" } else { "" });
    format!("{:x}", hasher.finalize())
}

fn extract_in_context_impl(
    project: &Path,
    entry: &Path,
    built_css: Option<&Path>,
    width: Option<u32>,
    root_attribute: Option<(&str, &str)>,
    cache: &mut ParseCache,
    refresh_imports: bool,
) -> Result<(m::SourceModel, LoadStats), String> {
    if let Some((name, value)) = root_attribute
        && (built_css.is_none()
            || !name.starts_with("data-")
            || name.len() == 5
            || [name, value].iter().any(|s| {
                s.is_empty()
                    || !s
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
            }))
    {
        return Err(
            "root attribute requires built CSS and data-NAME=VALUE with simple identifiers".into(),
        );
    }
    if width.is_some() && (built_css.is_none() || width == Some(0)) {
        return Err("viewport width requires built CSS and a positive width in CSS pixels".into());
    }
    let extract_start = Instant::now();
    let mut diagnostics = Vec::new();
    let load_cached_start = Instant::now();
    let (
        syntax::Sources {
            entry,
            artifact,
            files,
        },
        stats,
    ) = syntax::load_cached(
        project,
        entry,
        built_css,
        &mut diagnostics,
        cache,
        refresh_imports,
    )?;
    log_extract_phase("load_cached", load_cached_start);
    let built_sheet_start = Instant::now();
    let theme_classes = style::built::ThemeClasses {
        dark: true,
        light: true,
    };
    let built_sheet = artifact
        .as_ref()
        .map(|path| {
            let file = files[path].clone();
            let key = built_sheet_key(&file, width, root_attribute, theme_classes);
            if let Some(cached) = cache
                .built_sheet
                .as_ref()
                .filter(|cached| cached.key == key)
            {
                return Ok::<Arc<syntax::CachedSheet>, String>(cached.sheet.clone());
            }
            let sheet = syntax::CachedSheetTryBuilder {
                file,
                sheet_builder: |file| {
                    style::built::Sheet::new(file, width, root_attribute, theme_classes)
                },
            }
            .try_build()
            .map(std::sync::Arc::new)?;
            cache.built_sheet = Some(syntax::CachedBuiltSheet {
                key,
                sheet: sheet.clone(),
            });
            Ok(sheet)
        })
        .transpose()?;
    let built = built_sheet.as_ref().map(|sheet| sheet.borrow_sheet());
    log_extract_phase("built_sheet_parse", built_sheet_start);
    let source_sheets_start = Instant::now();
    let mut sheet_diagnostics = Vec::new();
    let sheet_order = stylesheet_order(&files, &entry);
    let source_sheets: Vec<_> = sheet_order
        .iter()
        .cloned()
        .filter(|path| {
            path.ends_with(".css")
                && !path.ends_with(".module.css")
                && artifact.as_ref() != Some(path)
        })
        .filter_map(|path| {
            let file = files.get(&path)?;
            match style::built::Sheet::source(file) {
                Ok(sheet) => Some(sheet),
                Err(reason) => {
                    sheet_diagnostics.push(m::Diagnostic {
                        code: "unsupported-stylesheet".into(),
                        message: reason,
                        location: None,
                    });
                    None
                }
            }
        })
        .collect();
    log_extract_phase("source_sheet_parse", source_sheets_start);
    let model_scan_start = Instant::now();
    let mut model = m::SourceModel {
        schema_version: m::SCHEMA_VERSION,
        extractor: m::Extractor {
            name: "dui-lang-typescript".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        files: vec![],
        screens: vec![],
        components: vec![],
        document_style: None,
        item_schemas: vec![],
        tokens: vec![],
        edges: vec![],
        diagnostics,
    };
    model.diagnostics.extend(sheet_diagnostics);
    let dependency_closures = dependency_closures(&files);
    let dependency_hashes: BTreeMap<String, String> = dependency_closures
        .iter()
        .map(|(path, closure)| (path.clone(), dependency_content_hash(&files, closure)))
        .collect();
    let mut signatures = BTreeMap::new();
    for file in files.values() {
        let css = file.path.ends_with(".css");
        model.files.push(m::SourceFile {
            path: file.path.clone(),
            language: if css {
                m::Language::Css
            } else {
                m::Language::Typescript
            },
            sha256: file.sha256.clone(),
            imports: file.imports.iter().cloned().collect(),
        });
        let dependency_hash = dependency_hashes[&file.path].clone();
        let facts = if let Some((key, facts)) = cache.facts.get(&file.path) {
            (*key == dependency_hash).then(|| (key.clone(), facts.clone()))
        } else {
            None
        };
        let (_, facts) = facts.unwrap_or_else(|| {
            let mut tokens = Vec::new();
            let mut item_schemas = Vec::new();
            let mut file_signatures = Vec::new();
            if css {
                tokens.extend(style::theme(file));
            } else {
                for n in file
                    .declarations()
                    .into_iter()
                    .filter(|n| n.kind() == "interface_declaration")
                {
                    if let (Some(name), Some(body)) = (file.name(n, "name"), field(n, "body")) {
                        item_schemas.push(m::ItemSchema {
                            id: format!("{}#{name}", file.path),
                            name: name.into(),
                            fields: fields(&files, file, body),
                            origin: m::Origin::Syntax,
                            location: file.loc(n),
                        });
                    }
                }
                if file.path.ends_with(".tsx") {
                    for (name, f) in file.functions() {
                        let id = format!("{}#{name}", file.path);
                        file_signatures.push((id.clone(), slots(&files, file, f, &id)));
                    }
                }
            }
            let facts = Arc::new(syntax::FileFacts {
                tokens,
                item_schemas,
                signatures: file_signatures,
            });
            cache
                .facts
                .insert(file.path.clone(), (dependency_hash, facts.clone()));
            (String::new(), facts)
        });
        model.tokens.extend(facts.tokens.iter().cloned());
        model
            .item_schemas
            .extend(facts.item_schemas.iter().cloned());
        for (id, slots) in &facts.signatures {
            signatures.insert(id.clone(), slots.clone());
        }
    }
    log_extract_phase("model_files_tokens_schemas_signatures", model_scan_start);
    let tailwind_start = Instant::now();
    let root_classes = root_classes::default_document_classes(&files);
    let tailwind = files.values().any(|f| {
        f.path.ends_with(".css")
            && children(f.tree.root_node()).into_iter().any(|n| {
                n.kind() == "import_statement"
                    && first(n)
                        .is_some_and(|p| matches!(f.text(p), "\"tailwindcss\"" | "'tailwindcss'"))
            })
    });
    log_extract_phase("tailwind_detection", tailwind_start);
    let modules_start = Instant::now();
    let modules = style::modules::Modules::new(&files);
    log_extract_phase("module_css_parse", modules_start);
    let component_render_start = Instant::now();
    for file in files.values().filter(|f| f.path.ends_with(".tsx")) {
        let file_render_start = Instant::now();
        let fingerprint = semantic_fingerprint(
            &files,
            &dependency_hashes[&file.path],
            &entry,
            artifact.as_deref(),
            &sheet_order,
            width,
            root_attribute,
            theme_classes,
            tailwind,
        );
        if let Some(cached) = cache
            .rendered
            .get(&file.path)
            .filter(|cached| cached.cacheable && cached.fingerprint == fingerprint)
        {
            model.components.extend(cached.components.iter().cloned());
            model.edges.extend(cached.edges.iter().cloned());
            log_render_file(&file.path, true, file_render_start);
            continue;
        }
        let component_start = model.components.len();
        let screen_start = model.screens.len();
        let edge_start = model.edges.len();
        let diagnostic_start = model.diagnostics.len();
        for (name, f) in file.functions() {
            let id = format!("{}#{name}", file.path);
            let mut slot_values = signatures[&id].clone();
            render::apply_variant_defaults(file, f, &mut slot_values);
            let slot = &slot_values;
            let mut cx = render::Context {
                file,
                files: &files,
                slots: slot,
                class_render: f.kind() == "method_definition",
                tailwind,
                built,
                sources: &source_sheets,
                modules: modules.bindings(file, f),
                signatures: &signatures,
                items: vec![],
                constants: constant_env(file, f, slot),
                model: &mut model,
            };
            let root = match returned(f) {
                Some(n) => cx.node(n, format!("{id}@0")),
                None => m::Node::Placeholder(m::Placeholder {
                    id: format!("{id}@0"),
                    reason: "Unsupported component body".into(),
                    location: file.loc(f),
                }),
            };
            model.components.push(m::Component {
                id,
                name: name.into(),
                location: file.loc(f),
                slots: slot.clone(),
                root,
            });
        }
        let components = model.components[component_start..].to_vec();
        let screens = model.screens[screen_start..].to_vec();
        let edges = model.edges[edge_start..].to_vec();
        let diagnostics = model.diagnostics[diagnostic_start..].to_vec();
        cache.rendered.insert(
            file.path.clone(),
            syntax::RenderedFile {
                fingerprint,
                cacheable: screens.is_empty() && diagnostics.is_empty(),
                components,
                edges,
            },
        );
        log_render_file(&file.path, false, file_render_start);
    }
    log_extract_phase("component_model_render", component_render_start);
    let route_discovery_start = Instant::now();
    routes::discover(&files, &mut model);
    log_extract_phase("route_discovery", route_discovery_start);
    let route_postprocess_start = Instant::now();
    if model.screens.is_empty() {
        let candidates: Vec<_> = model
            .components
            .iter()
            .filter(|c| c.location.file == entry)
            .collect();
        let default = files[&entry].exported("default");
        let root = default
            .and_then(|n| candidates.iter().find(|c| c.name == n).copied())
            .or(match candidates.as_slice() {
                [root] => Some(*root),
                _ => None,
            });
        if let Some(c) = root {
            model.screens.push(m::Screen {
                id: c.id.clone(),
                name: c.name.clone(),
                route: None,
                component: c.id.clone(),
                origin: m::Origin::Syntax,
                location: c.location.clone(),
            })
        }
    }
    model.components.sort_by(|a, b| a.id.cmp(&b.id));
    hash_routes::apply(&files[&entry], &mut model);
    query_views::apply(&files, &entry, &mut model);
    for e in &mut model.edges {
        if let m::EdgeTarget::Route { route } = &e.to
            && model
                .screens
                .iter()
                .any(|s| s.route.as_ref() == Some(route))
        {
            e.to = m::EdgeTarget::Screen {
                screen: format!("route:{route}"),
            }
        }
    }
    model.screens.sort_by(|a, b| a.id.cmp(&b.id));
    model.edges.sort_by(|a, b| a.id.cmp(&b.id));
    model.item_schemas.sort_by(|a, b| a.id.cmp(&b.id));
    model.tokens.sort_by(|a, b| a.id.cmp(&b.id));
    log_extract_phase("route_postprocess", route_postprocess_start);
    let document_style_start = Instant::now();
    if let Some(sheet) = &built {
        model.document_style = sheet.document_style(&root_classes);
    } else {
        model.document_style = source_sheets
            .iter()
            .rev()
            .find_map(|sheet| sheet.document_style(&root_classes));
    }
    log_extract_phase("document_style", document_style_start);
    let external_stylesheet_start = Instant::now();
    let resources = fonts::document_resources(project, artifact.as_deref().map(Path::new));
    log_extract_phase("external_stylesheet_scan", external_stylesheet_start);
    let font_face_start = Instant::now();
    if let Some(html) = resources.html_file.clone()
        && !model.files.iter().any(|file| file.path == html.path)
    {
        model.files.push(html);
    }
    let font_key = font_intake_key(&files, artifact.as_deref(), &resources);
    let intake = if let Some(cached) = cache.fonts.as_ref().filter(|cached| {
        cached.key == font_key && cached_font_inputs_current(project, &cached.intake)
    }) {
        cached.intake.clone()
    } else {
        let diagnostic_start = model.diagnostics.len();
        let intake = fonts::font_faces(
            project,
            &files,
            artifact.as_deref(),
            &resources,
            &mut model.diagnostics,
        );
        if model.diagnostics.len() == diagnostic_start {
            cache.fonts = Some(syntax::CachedFontIntake {
                key: font_key,
                intake: intake.clone(),
            });
        } else {
            cache.fonts = None;
        }
        intake
    };
    for file in intake.extra_files {
        if !model
            .files
            .iter()
            .any(|existing| existing.path == file.path)
        {
            model.files.push(file);
        }
    }
    model.files.sort_by(|a, b| a.path.cmp(&b.path));
    if !resources.external_stylesheets.is_empty() || !intake.faces.is_empty() {
        let document = model
            .document_style
            .get_or_insert_with(|| m::DocumentStyle {
                root: None,
                body: None,
                external_stylesheets: vec![],
                font_faces: vec![],
            });
        document.external_stylesheets = resources.external_stylesheets;
        document.font_faces = intake.faces;
    }
    log_extract_phase("font_face_scan", font_face_start);
    let validate_start = Instant::now();
    m::validate(&model).map_err(|errs| format!("invalid source model: {errs:?}"))?;
    log_extract_phase("validate_model", validate_start);
    log_extract_phase("extract_total", extract_start);
    Ok((model, stats))
}
