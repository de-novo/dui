//! Extract schema-v1 models from a TSX entry and its reachable imports.
mod class_view;
mod hash_routes;
mod query_views;
mod render;
mod routes;
mod style;
mod syntax;
use dui_core::source_model as m;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
use syntax::{File, children, field, first, unwrap_expr};
pub use syntax::{LoadStats, ParseCache};
use tree_sitter::Node as Ast;

type Files = BTreeMap<String, File>;
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
    extract_in_context_impl(project, entry, built_css, width, root_attribute, &mut cache)
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
    extract_in_context_impl(project, entry, built.as_deref(), None, None, cache)
}

fn attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    loop {
        let index = rest.find(name)?;
        let before = index
            .checked_sub(1)
            .and_then(|i| rest.as_bytes().get(i))
            .copied();
        if before.is_none_or(|c| c.is_ascii_whitespace()) {
            let after = &rest[index + name.len()..];
            if let Some(value) = after.strip_prefix('=') {
                let value = value.trim_start();
                if let Some(quoted) = value.strip_prefix('"') {
                    return quoted.split('"').next();
                }
                if let Some(quoted) = value.strip_prefix('\'') {
                    return quoted.split('\'').next();
                }
                return Some(value.split_ascii_whitespace().next()?.trim_end_matches('>'));
            }
        }
        rest = &rest[index + name.len()..];
    }
}

const MAX_FONT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TOTAL_FONT_BYTES: u64 = 12 * 1024 * 1024;

fn unquote_css(raw: &str) -> String {
    let raw = raw.trim();
    raw.strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| raw.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(raw)
        .trim()
        .to_owned()
}

fn parse_font_src(raw: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut rest = raw;
    while let Some(at) = rest.find("url(") {
        let after = &rest[at + 4..];
        let Some(end) = after.find(')') else { break };
        let url = unquote_css(&after[..end]);
        let mut format = None;
        let tail = after[end + 1..].trim_start();
        if let Some(args) = tail.strip_prefix("format(")
            && let Some(close) = args.find(')')
        {
            format = Some(unquote_css(&args[..close]));
        }
        if !url.is_empty() {
            out.push((url, format));
        }
        rest = &after[end + 1..];
    }
    out
}

fn percent_decode_path(raw: &str) -> Option<String> {
    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            out.push((hex(bytes[index + 1])? << 4) | hex(bytes[index + 2])?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn normalize_font_path(css_path: &str, url: &str) -> Option<String> {
    let clean = url.split(['?', '#']).next()?;
    let clean = percent_decode_path(clean)?;
    if clean.is_empty() || clean.contains('\\') || clean.contains('\0') {
        return None;
    }
    let mut parts = Vec::new();
    if clean.starts_with('/') {
        let public_root = css_path
            .split('/')
            .next()
            .filter(|part| matches!(*part, "dist" | "build"));
        if let Some(root) = public_root {
            parts.push(root);
        }
    } else if let Some(parent) = Path::new(css_path).parent() {
        for part in parent.iter() {
            parts.push(part.to_str()?);
        }
    }
    for part in clean.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part if part.bytes().all(|b| b.is_ascii_graphic() || b >= 0x80) => parts.push(part),
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn font_magic_matches(extension: &str, bytes: &[u8]) -> bool {
    match extension {
        "woff2" => bytes.starts_with(b"wOF2"),
        "woff" => bytes.starts_with(b"wOFF"),
        "ttf" => bytes.starts_with(&[0, 1, 0, 0]) || bytes.starts_with(b"true"),
        "otf" => bytes.starts_with(b"OTTO"),
        _ => false,
    }
}

fn font_diagnostic(
    diagnostics: &mut Vec<m::Diagnostic>,
    location: &m::Location,
    message: impl Into<String>,
) {
    diagnostics.push(m::Diagnostic {
        code: "unsupported-font-resource".into(),
        message: message.into(),
        location: Some(location.clone()),
    });
}

fn local_font_source(
    project: &Path,
    project_root: &Path,
    css_path: &str,
    url: &str,
    format: Option<String>,
    location: &m::Location,
    diagnostics: &mut Vec<m::Diagnostic>,
    cache: &mut BTreeMap<String, Option<m::FontSource>>,
    total_bytes: &mut u64,
) -> Option<m::FontSource> {
    let path = normalize_font_path(css_path, url)?;
    if let Some(cached) = cache.get(&path) {
        return cached.clone();
    }
    let mut invalid = |reason: &str, diagnostics: &mut Vec<m::Diagnostic>| {
        font_diagnostic(diagnostics, location, format!("{path}: {reason}"));
        cache.insert(path.clone(), None);
        None
    };
    let Some(extension) = path.rsplit_once('.').map(|(_, ext)| ext.to_owned()) else {
        return invalid("font path has no extension", diagnostics);
    };
    if !matches!(extension.as_str(), "woff2" | "woff" | "ttf" | "otf") {
        return invalid("font extension is not woff2, woff, ttf or otf", diagnostics);
    }
    let full = project.join(&path);
    let Ok(full) = full.canonicalize() else {
        return invalid("font file is unavailable", diagnostics);
    };
    if !full.starts_with(project_root) || !full.is_file() {
        return invalid(
            "font path escapes the project or is not a file",
            diagnostics,
        );
    }
    let Ok(bytes) = std::fs::read(&full) else {
        return invalid("font file could not be read", diagnostics);
    };
    let len = bytes.len() as u64;
    if len == 0 || len > MAX_FONT_BYTES {
        return invalid("font file exceeds the bounded local font size", diagnostics);
    }
    if !font_magic_matches(&extension, &bytes) {
        return invalid("font bytes do not match their extension", diagnostics);
    }
    if *total_bytes + len > MAX_TOTAL_FONT_BYTES {
        return invalid(
            "project font assets exceed the bounded total size",
            diagnostics,
        );
    }
    *total_bytes += len;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let source = m::FontSource::Asset {
        path: path.clone(),
        asset: format!("asset-{sha256}.{extension}"),
        sha256,
        bytes: len,
        format,
    };
    cache.insert(path, Some(source.clone()));
    Some(source)
}

fn font_faces(
    project: &Path,
    files: &Files,
    artifact: Option<&str>,
    diagnostics: &mut Vec<m::Diagnostic>,
) -> Vec<m::FontFace> {
    let Ok(project_root) = project.canonicalize() else {
        return vec![];
    };
    let paths: Vec<&str> = if let Some(path) = artifact {
        vec![path]
    } else {
        files
            .keys()
            .filter(|path| path.ends_with(".css") && !path.ends_with(".module.css"))
            .map(String::as_str)
            .collect()
    };
    let mut faces = Vec::new();
    let mut cache = BTreeMap::new();
    let mut total_bytes = 0u64;
    for path in paths {
        let Some(file) = files.get(path) else {
            continue;
        };
        let mut stack = vec![file.tree.root_node()];
        while let Some(node) = stack.pop() {
            if node.kind() == "at_rule"
                && children(node)
                    .first()
                    .is_some_and(|part| file.text(*part) == "@font-face")
            {
                let parts = children(node);
                let location = file.loc(node);
                let mut family = None;
                let mut src = None;
                let mut descriptors: BTreeMap<String, String> = BTreeMap::new();
                for declaration in parts
                    .iter()
                    .flat_map(|part| children(*part))
                    .filter(|part| part.kind() == "declaration")
                {
                    let text = file.text(declaration);
                    let Some((name, value)) = text.split_once(':') else {
                        continue;
                    };
                    let value = value.trim().trim_end_matches(';').trim();
                    match name.trim() {
                        "font-family" => family = Some(unquote_css(value)),
                        "src" => src = Some(value.to_owned()),
                        "font-style" | "font-weight" | "font-stretch" | "font-display" => {
                            descriptors.insert(
                                name.trim().replace("font-", ""),
                                value.split_whitespace().collect::<Vec<_>>().join(" "),
                            );
                        }
                        _ => {}
                    }
                }
                let (Some(family), Some(src)) = (family.filter(|v| !v.is_empty()), src) else {
                    continue;
                };
                let mut sources = Vec::new();
                for (url, format) in parse_font_src(&src) {
                    if url.starts_with("https://")
                        || url.starts_with("http://")
                        || url.starts_with("//")
                    {
                        sources.push(m::FontSource::External { url, format });
                    } else if url.starts_with("data:") {
                        font_diagnostic(
                            diagnostics,
                            &location,
                            format!("{family}: embedded data font URLs are not extracted"),
                        );
                    } else if let Some(source) = local_font_source(
                        project,
                        &project_root,
                        path,
                        &url,
                        format,
                        &location,
                        diagnostics,
                        &mut cache,
                        &mut total_bytes,
                    ) {
                        if !sources.contains(&source) {
                            sources.push(source);
                        }
                    }
                }
                if sources.is_empty() {
                    continue;
                }
                faces.push(m::FontFace {
                    id: format!("{path}#font-face-{}", node.start_byte()),
                    family,
                    style: descriptors.get("style").cloned(),
                    weight: descriptors.get("weight").cloned(),
                    stretch: descriptors.get("stretch").cloned(),
                    display: descriptors.get("display").cloned(),
                    sources,
                    origin: m::Origin::Syntax,
                    location,
                });
                continue;
            }
            stack.extend(children(node));
        }
    }
    faces.sort_by(|a, b| a.id.cmp(&b.id));
    faces.dedup_by(|a, b| a.id == b.id);
    faces
}

fn external_stylesheets(project: &Path, artifact: Option<&Path>) -> Vec<String> {
    let html = artifact
        .and_then(|path| {
            if path.starts_with("dist/") {
                Some(project.join("dist/index.html"))
            } else {
                None
            }
        })
        .filter(|path| path.is_file())
        .or_else(|| {
            let path = project.join("index.html");
            path.is_file().then_some(path)
        });
    let Some(html) = html.and_then(|path| std::fs::read_to_string(path).ok()) else {
        return vec![];
    };
    let mut out = Vec::new();
    for part in html.split('<').skip(1) {
        let Some(tag) = part.split('>').next() else {
            continue;
        };
        let Some(name) = tag.split_ascii_whitespace().next() else {
            continue;
        };
        if !name.eq_ignore_ascii_case("link") {
            continue;
        }
        let rel = attr_value(tag, "rel").unwrap_or_default();
        let href = attr_value(tag, "href").unwrap_or_default();
        if rel
            .split_ascii_whitespace()
            .any(|v| v.eq_ignore_ascii_case("stylesheet"))
            && (href.starts_with("https://")
                || href.starts_with("http://")
                || href.starts_with("//"))
            && !out.iter().any(|v| v == href)
        {
            out.push(href.to_owned());
        }
    }
    out
}

fn extract_in_context_impl(
    project: &Path,
    entry: &Path,
    built_css: Option<&Path>,
    width: Option<u32>,
    root_attribute: Option<(&str, &str)>,
    cache: &mut ParseCache,
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
    let mut diagnostics = Vec::new();
    let (
        syntax::Sources {
            entry,
            artifact,
            files,
        },
        stats,
    ) = syntax::load_cached(project, entry, built_css, &mut diagnostics, cache)?;
    let theme_classes = style::built::ThemeClasses {
        dark: true,
        light: true,
    };
    let built = artifact
        .as_ref()
        .map(|p| style::built::Sheet::new(&files[p], width, root_attribute, theme_classes))
        .transpose()?;
    let mut sheet_diagnostics = Vec::new();
    let source_sheets: Vec<_> = stylesheet_order(&files, &entry)
        .into_iter()
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
            sha256: format!("{:x}", Sha256::digest(file.src.as_bytes())),
            imports: file.imports.iter().cloned().collect(),
        });
        if css {
            model.tokens.extend(style::theme(file));
            continue;
        }
        for n in file
            .declarations()
            .into_iter()
            .filter(|n| n.kind() == "interface_declaration")
        {
            if let (Some(name), Some(body)) = (file.name(n, "name"), field(n, "body")) {
                model.item_schemas.push(m::ItemSchema {
                    id: format!("{}#{name}", file.path),
                    name: name.into(),
                    fields: fields(&files, file, body),
                    origin: m::Origin::Syntax,
                    location: file.loc(n),
                });
            }
        }
        if !file.path.ends_with(".tsx") {
            continue;
        }
        for (name, f) in file.functions() {
            let id = format!("{}#{name}", file.path);
            signatures.insert(id.clone(), slots(&files, file, f, &id));
        }
    }
    let tailwind = files.values().any(|f| {
        f.path.ends_with(".css")
            && children(f.tree.root_node()).into_iter().any(|n| {
                n.kind() == "import_statement"
                    && first(n)
                        .is_some_and(|p| matches!(f.text(p), "\"tailwindcss\"" | "'tailwindcss'"))
            })
    });
    let modules = style::modules::Modules::new(&files);
    for file in files.values().filter(|f| f.path.ends_with(".tsx")) {
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
                built: built.as_ref(),
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
    }
    routes::discover(&files, &mut model);
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
    if let Some(sheet) = &built {
        model.document_style = sheet.document_style();
    } else {
        model.document_style = source_sheets
            .iter()
            .rev()
            .find_map(|sheet| sheet.document_style());
    }
    let external = external_stylesheets(project, artifact.as_deref().map(Path::new));
    let fonts = font_faces(project, &files, artifact.as_deref(), &mut model.diagnostics);
    if !external.is_empty() || !fonts.is_empty() {
        let document = model
            .document_style
            .get_or_insert_with(|| m::DocumentStyle {
                root: None,
                body: None,
                external_stylesheets: vec![],
                font_faces: vec![],
            });
        document.external_stylesheets = external;
        document.font_faces = fonts;
    }
    m::validate(&model).map_err(|errs| format!("invalid source model: {errs:?}"))?;
    Ok((model, stats))
}
