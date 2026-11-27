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
        for bind in children(pat)
            .into_iter()
            .filter(|p| p.kind() == "shorthand_property_identifier_pattern")
        {
            let name = file.text(bind);
            let prop = props.iter().find(|p| p.name == name);
            out.push(m::Slot {
                id: format!("{id}.{name}"),
                name: name.into(),
                kind: m::SlotKind::Prop,
                ty: prop.map(|p| p.ty.clone()).unwrap_or_else(unknown),
                optional: prop.is_some_and(|p| p.optional),
                initial: None,
                source: None,
                origin: m::Origin::Syntax,
                location: file.loc(bind),
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

pub fn extract(project: &Path, entry: &Path) -> Result<m::SourceModel, String> {
    extract_with_css(project, entry, None)
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
    extract_in_context_impl(project, entry, None, None, None, cache)
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
    let built = artifact
        .as_ref()
        .map(|p| style::built::Sheet::new(&files[p], width, root_attribute))
        .transpose()?;
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
            let slot = &signatures[&id];
            let mut cx = render::Context {
                file,
                files: &files,
                slots: slot,
                class_render: f.kind() == "method_definition",
                tailwind,
                built: built.as_ref(),
                modules: modules.bindings(file, f),
                signatures: &signatures,
                items: vec![],
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
    }
    m::validate(&model).map_err(|errs| format!("invalid source model: {errs:?}"))?;
    Ok((model, stats))
}
