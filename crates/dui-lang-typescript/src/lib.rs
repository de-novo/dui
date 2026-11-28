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
fn object_type_fields(files: &Files, file: &File, node: Ast<'_>) -> Vec<m::Field> {
    let n = unwrap_expr(node);
    if matches!(n.kind(), "object_type" | "interface_body") {
        return fields(files, file, n);
    }
    if n.kind() == "type_identifier"
        && let Some((target, name)) = symbol(files, file, file.text(n))
    {
        for declaration in target.declarations() {
            if declaration.kind() == "interface_declaration"
                && target.name(declaration, "name") == Some(name)
                && let Some(body) = field(declaration, "body")
            {
                return fields(files, target, body);
            }
            if declaration.kind() == "type_alias_declaration"
                && target.name(declaration, "name") == Some(name)
                && let Some(value) = field(declaration, "value")
            {
                return object_type_fields(files, target, value);
            }
        }
    }
    Vec::new()
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
    if let Some(param) = field(f, "parameters").and_then(first)
        && let Some(pattern) = field(param, "pattern").filter(|p| p.kind() == "identifier")
        && let Some(ty) = field(param, "type").map(unwrap_expr)
    {
        for prop in object_type_fields(files, file, ty) {
            out.push(m::Slot {
                id: format!("{id}.{}", prop.name),
                name: prop.name.clone(),
                kind: m::SlotKind::Prop,
                ty: prop.ty,
                optional: prop.optional,
                initial: None,
                source: None,
                origin: m::Origin::Syntax,
                location: file.loc(pattern),
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
        let (Some(pat), Some(value)) = (field(var, "name"), field(var, "value")) else {
            continue;
        };
        if value.kind() != "call_expression" {
            if pat.kind() == "identifier"
                && !matches!(
                    value.kind(),
                    "arrow_function" | "function_expression" | "object" | "array" | "template_string"
                )
            {
                let name = file.text(pat);
                out.push(m::Slot {
                    id: format!("{id}.{name}"),
                    name: name.into(),
                    kind: m::SlotKind::External,
                    ty: unknown(),
                    optional: true,
                    initial: None,
                    source: Some(file.text(value).into()),
                    origin: m::Origin::Syntax,
                    location: file.loc(var),
                });
            }
            continue;
        }
        let call = value;
        let Some(callee) = file.name(call, "function") else {
            continue;
        };
        let local = file
            .bindings
            .get(callee)
            .is_some_and(|(module, name)| module == "react" && name == "useState");
        let query = !local
            && field(call, "function").is_some_and(|function| {
                let text = file.text(function);
                text.ends_with("useQuery")
                    || text.ends_with("useInfiniteQuery")
                    || text.ends_with("useSWR")
            });
        if !local && pat.kind() == "object_pattern" {
            for bind in children(pat) {
                let name_node = match bind.kind() {
                    "shorthand_property_identifier_pattern" => Some(bind),
                    "pair_pattern" => field(bind, "value").filter(|value| {
                        matches!(
                            value.kind(),
                            "identifier" | "shorthand_property_identifier_pattern"
                        )
                    }),
                    "object_assignment_pattern" => field(bind, "left").filter(|value| {
                        matches!(
                            value.kind(),
                            "identifier" | "shorthand_property_identifier_pattern"
                        )
                    }),
                    _ => None,
                };
                let Some(name_node) = name_node else { continue };
                let name = file.text(name_node);
                out.push(m::Slot {
                    id: format!("{id}.{name}"),
                    name: name.into(),
                    kind: if query {
                        m::SlotKind::Query
                    } else {
                        m::SlotKind::External
                    },
                    ty: unknown(),
                    optional: false,
                    initial: None,
                    source: Some(file.text(call).into()),
                    origin: m::Origin::Syntax,
                    location: file.loc(name_node),
                });
            }
            continue;
        }
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
            } else if query {
                m::SlotKind::Query
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

fn return_expression(statement: Ast<'_>) -> Option<Ast<'_>> {
    if statement.kind() != "return_statement" {
        return None;
    }
    first(statement).map(unwrap_expr)
}

fn conditional_return(statement: Ast<'_>) -> Option<(Ast<'_>, Ast<'_>)> {
    if statement.kind() != "if_statement" || field(statement, "alternative").is_some() {
        return None;
    }
    let condition = field(statement, "condition")?;
    let consequence = field(statement, "consequence")?;
    if consequence.kind() == "return_statement" || consequence.kind() == "statement_block" {
        return Some((condition, consequence));
    }
    None
}

fn condition_shape<'a>(
    file: &'a File,
    condition: Ast<'a>,
) -> (
    Ast<'a>,
    m::Match,
    Option<Ast<'a>>,
    Option<Ast<'a>>,
    Option<&'a str>,
) {
    let condition = unwrap_expr(condition);
    if condition.kind() == "binary_expression" {
        let operator = file.name(condition, "operator");
        if matches!(operator, Some("&&" | "||")) {
            return (
                condition,
                m::Match::Truthy(true),
                field(condition, "left"),
                field(condition, "right"),
                operator,
            );
        }
        if operator == Some("===")
            && let (Some(left), Some(value)) = (
                field(condition, "left"),
                field(condition, "right").and_then(|right| literal(file, right)),
            )
        {
            return (left, m::Match::Equals(value), None, None, None);
        }
    }
    if condition.kind() == "unary_expression"
        && file.name(condition, "operator") == Some("!")
        && let Some(argument) = field(condition, "argument")
    {
        return (argument, m::Match::Truthy(false), None, None, None);
    }
    (condition, m::Match::Truthy(true), None, None, None)
}

fn conditional_node(
    cx: &mut render::Context<'_>,
    condition: Ast<'_>,
    yes: m::Node,
    otherwise: m::Node,
    id: String,
) -> m::Node {
    let location = cx.file.loc(condition);
    let (on, when, _left, _right, _operator) = condition_shape(cx.file, condition);
    let (truthy, fallback) = (yes, otherwise);
    m::Node::Branch(m::Branch {
        id,
        on: cx.expr(on),
        arms: vec![
            m::Arm {
                when,
                children: vec![truthy],
            },
            m::Arm {
                when: m::Match::Otherwise,
                children: vec![fallback],
            },
        ],
        location,
    })
}

fn consequence_node(
    cx: &mut render::Context<'_>,
    consequence: Ast<'_>,
    id: String,
) -> Option<m::Node> {
    if consequence.kind() == "return_statement" {
        return return_expression(consequence).map(|content| cx.node(content, id));
    }
    if consequence.kind() == "statement_block" {
        return statements_root(cx, children(consequence), id);
    }
    None
}

fn statements_root(cx: &mut render::Context<'_>, statements: Vec<Ast<'_>>, id: String) -> Option<m::Node> {
    let mut conditional = Vec::new();
    let mut final_return = None;
    for statement in &statements {
        match statement.kind() {
            "lexical_declaration"
            | "variable_declaration"
            | "expression_statement"
            | "function_declaration" => {}
            "return_statement" => {
                if final_return.is_some() {
                    return None;
                }
                final_return = return_expression(*statement);
            }
            "if_statement" if final_return.is_none() => {
                if let Some(early) = conditional_return(*statement) {
                    conditional.push(early);
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }
    let final_return = final_return?;
    let final_id = if conditional.is_empty() {
        id.clone()
    } else {
        format!("{id}.final")
    };
    let mut root = cx.node(final_return, final_id);
    for (index, (condition, consequence)) in conditional.iter().rev().enumerate() {
        let branch_id = format!("{id}.return{index}");
        let yes = consequence_node(cx, *consequence, format!("{branch_id}.content"))?;
        root = conditional_node(cx, *condition, yes, root, branch_id);
    }
    Some(root)
}

fn component_root(cx: &mut render::Context<'_>, f: Ast<'_>, id: String) -> m::Node {
    if f.kind() == "method_definition" {
        return crate::returned(f)
            .map(|n| cx.node(n, id.clone()))
            .unwrap_or_else(|| {
                m::Node::Placeholder(m::Placeholder {
                    id,
                    reason: "Unsupported component body".into(),
                    location: cx.file.loc(f),
                })
            });
    }
    let Some(body) = field(f, "body") else {
        return m::Node::Placeholder(m::Placeholder {
            id,
            reason: "Unsupported component body".into(),
            location: cx.file.loc(f),
        });
    };
    if body.kind() != "statement_block" {
        return cx.node(unwrap_expr(body), id);
    }
    statements_root(cx, children(body), id.clone()).unwrap_or_else(|| {
        crate::returned(f)
            .map(|n| cx.node(n, id.clone()))
            .unwrap_or_else(|| {
                m::Node::Placeholder(m::Placeholder {
                    id,
                    reason: "Unsupported component body".into(),
                    location: cx.file.loc(f),
                })
            })
    })
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
    let root_classes = root_classes::default_document_classes(&files);
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
            let root = component_root(&mut cx, f, format!("{id}@0"));
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
        model.document_style = sheet.document_style(&root_classes);
    } else {
        model.document_style = source_sheets
            .iter()
            .rev()
            .find_map(|sheet| sheet.document_style(&root_classes));
    }
    let resources = fonts::document_resources(project, artifact.as_deref().map(Path::new));
    if let Some(html) = resources.html_file.clone()
        && !model.files.iter().any(|file| file.path == html.path)
    {
        model.files.push(html);
    }
    let intake = fonts::font_faces(
        project,
        &files,
        artifact.as_deref(),
        &resources,
        &mut model.diagnostics,
    );
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
    m::validate(&model).map_err(|errs| format!("invalid source model: {errs:?}"))?;
    Ok((model, stats))
}
