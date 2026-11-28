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
    path::{Path, PathBuf},
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use syntax::{File, children, field, first, unwrap_expr};
pub use syntax::{LoadStats, ParseCache};
use tree_sitter::Node as Ast;

type Files = BTreeMap<String, Arc<File>>;
fn resolve_relative_module(files: &Files, from: &str, request: &str) -> Option<String> {
    if !request.starts_with('.') {
        return None;
    }
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in request.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    let base = parts.join("/");
    let candidates = [
        base.clone(),
        format!("{base}.tsx"),
        format!("{base}.ts"),
        format!("{base}.jsx"),
        format!("{base}.js"),
        format!("{base}/index.tsx"),
        format!("{base}/index.ts"),
    ];
    candidates
        .into_iter()
        .find(|candidate| files.contains_key(candidate))
}
fn export_text(text: &str) -> &str {
    text.strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .or_else(|| {
            text.strip_prefix('\'')
                .and_then(|text| text.strip_suffix('\''))
        })
        .unwrap_or(text)
}
fn reexported_symbol<'a>(
    files: &'a Files,
    file: &'a File,
    name: &'a str,
    depth: usize,
) -> Option<(&'a File, &'a str)> {
    if depth > 8 {
        return None;
    }
    for statement in children(file.tree.root_node())
        .into_iter()
        .filter(|node| node.kind() == "export_statement")
    {
        let Some(source) = field(statement, "source") else {
            continue;
        };
        let request = export_text(file.text(source));
        let Some(module) = resolve_relative_module(files, &file.path, request) else {
            continue;
        };
        let target = files.get(&module)?;
        let clause = children(statement)
            .into_iter()
            .find(|node| node.kind() == "export_clause");
        if let Some(clause) = clause {
            for specifier in children(clause) {
                if specifier.kind() != "export_specifier" {
                    continue;
                }
                let Some(local) = field(specifier, "name") else {
                    continue;
                };
                let exported = field(specifier, "alias")
                    .map(|alias| export_text(file.text(alias)))
                    .unwrap_or_else(|| export_text(file.text(local)));
                if exported == name {
                    let local = export_text(file.text(local));
                    if let Some(export) = target.exported(local) {
                        return Some((target, export));
                    }
                    return reexported_symbol(files, target, local, depth + 1);
                }
            }
        } else {
            if let Some(export) = target.exported(name) {
                return Some((target, export));
            }
            if let Some(found) = reexported_symbol(files, target, name, depth + 1) {
                return Some(found);
            }
        }
    }
    None
}
fn symbol<'a>(files: &'a Files, file: &'a File, name: &'a str) -> Option<(&'a File, &'a str)> {
    if let Some((module, export)) = file.bindings.get(name) {
        let target = files.get(module)?;
        if let Some(export) = target.exported(export) {
            return Some((target, export));
        }
        return reexported_symbol(files, target, export, 0);
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
    if matches!(
        n.kind(),
        "intersection_type" | "union_type" | "parenthesized_type"
    ) {
        return children(n)
            .into_iter()
            .flat_map(|part| object_type_fields(files, file, part))
            .collect();
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

/// Lower a deterministic component-body derivation that uses only modeled slots. This is
/// intentionally narrower than JavaScript evaluation: no calls, mutation, arithmetic or effects.
fn derived_append_key(expr: m::Expr, key: String) -> m::Expr {
    match expr {
        m::Expr::Slot { slot, mut path } => {
            path.push(key);
            m::Expr::Slot { slot, path }
        }
        object => m::Expr::Index {
            object: Box::new(object),
            key: Box::new(m::Expr::Literal { value: key.into() }),
        },
    }
}

fn derived_chain(
    file: &File,
    slots: &[m::Slot],
    props_object: Option<&str>,
    mut n: Ast<'_>,
    depth: usize,
) -> Option<m::Expr> {
    if depth >= 24 {
        return None;
    }
    enum Segment<'a> {
        Static(String),
        Key(Ast<'a>),
    }
    let mut segments = Vec::new();
    while matches!(n.kind(), "member_expression" | "subscript_expression") {
        if segments.len() >= 24 {
            return None;
        }
        if n.kind() == "member_expression" {
            segments.push(Segment::Static(file.name(n, "property")?.into()));
        } else {
            let index = field(n, "index")?;
            let static_key = literal(file, index).and_then(|value| match value {
                Value::String(value) => Some(value),
                Value::Number(value) => Some(value.to_string()),
                _ => None,
            });
            if let Some(key) = static_key {
                segments.push(Segment::Static(key));
            } else {
                segments.push(Segment::Key(index));
            }
        }
        n = field(n, "object")?;
    }
    if !matches!(n.kind(), "identifier" | "shorthand_property_identifier") {
        return None;
    }
    segments.reverse();
    let name = file.text(n);
    let slot_name = if Some(name) == props_object {
        let Some(Segment::Static(first)) = segments.first() else {
            return None;
        };
        let first = first.clone();
        segments.drain(..1);
        first
    } else {
        name.into()
    };
    let slot = slots.iter().find(|slot| slot.name == slot_name)?;
    let mut expr = m::Expr::Slot {
        slot: slot.id.clone(),
        path: vec![],
    };
    for segment in segments {
        match segment {
            Segment::Static(key) => expr = derived_append_key(expr, key),
            Segment::Key(node) => {
                let key = derived_expr(file, slots, props_object, node, depth + 1)?;
                expr = m::Expr::Index {
                    object: Box::new(expr),
                    key: Box::new(key),
                };
            }
        }
    }
    Some(expr)
}

fn derived_expr(
    file: &File,
    slots: &[m::Slot],
    props_object: Option<&str>,
    node: Ast<'_>,
    depth: usize,
) -> Option<m::Expr> {
    if depth >= 24 {
        return None;
    }
    let n = unwrap_expr(node);
    if let Some(value) = literal(file, n) {
        return Some(m::Expr::Literal { value });
    }
    if let Some(read) = derived_chain(file, slots, props_object, n, depth) {
        return Some(read);
    }
    if n.kind() == "unary_expression" && file.name(n, "operator") == Some("!") {
        return Some(m::Expr::Unary {
            operator: "!".into(),
            value: Box::new(derived_expr(
                file,
                slots,
                props_object,
                field(n, "argument")?,
                depth + 1,
            )?),
        });
    }
    if n.kind() == "binary_expression" {
        let operator = file.name(n, "operator")?;
        if matches!(
            operator,
            "&&" | "||" | "??" | "===" | "!==" | ">" | ">=" | "<" | "<="
        ) {
            return Some(m::Expr::Binary {
                operator: operator.into(),
                left: Box::new(derived_expr(
                    file,
                    slots,
                    props_object,
                    field(n, "left")?,
                    depth + 1,
                )?),
                right: Box::new(derived_expr(
                    file,
                    slots,
                    props_object,
                    field(n, "right")?,
                    depth + 1,
                )?),
            });
        }
    }
    None
}

fn default_expr(
    file: &File,
    slots: &[m::Slot],
    props_object: Option<&str>,
    node: Ast<'_>,
) -> m::Expr {
    if let Some(value) = render::static_literal(file, node) {
        return m::Expr::Literal { value };
    }
    if let Some(expr) = derived_expr(file, slots, props_object, node, 0) {
        return expr;
    }
    m::Expr::Opaque {
        text: file.text(node).into(),
        reads: vec![],
    }
}

fn destructured_props<'a>(
    file: &'a File,
    pattern: Ast<'a>,
) -> Vec<(String, String, Option<Ast<'a>>, Ast<'a>)> {
    children(pattern)
        .into_iter()
        .filter_map(|bind| match bind.kind() {
            "shorthand_property_identifier_pattern" => {
                let name = file.text(bind).to_owned();
                Some((name.clone(), name, None, bind))
            }
            "object_assignment_pattern" => {
                let left = field(bind, "left")?;
                if left.kind() != "shorthand_property_identifier_pattern" {
                    return None;
                }
                let name = file.text(left).to_owned();
                Some((name.clone(), name, field(bind, "right"), bind))
            }
            "pair_pattern" => {
                let prop = field(bind, "key").and_then(|key| render::object_key(file, key))?;
                let value = field(bind, "value")?;
                if value.kind() == "assignment_pattern" {
                    let left = field(value, "left")?;
                    if left.kind() != "identifier" {
                        return None;
                    }
                    Some((prop, file.text(left).into(), field(value, "right"), bind))
                } else if matches!(
                    value.kind(),
                    "identifier" | "shorthand_property_identifier_pattern"
                ) {
                    Some((prop, file.text(value).into(), None, bind))
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect()
}

fn slots(files: &Files, file: &File, f: Ast<'_>, id: &str) -> Vec<m::Slot> {
    if f.kind() == "method_definition" {
        return class_view::props(file, f, id);
    }
    let mut out = Vec::new();
    let props_object = props_object_name(file, f);
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
                if !out.iter().any(|slot| slot.name == "children") {
                    out.push(m::Slot {
                        id: format!("{id}.children"),
                        name: "children".into(),
                        kind: m::SlotKind::Prop,
                        ty: m::TypeRef::Node,
                        optional: true,
                        initial: None,
                        source: None,
                        origin: m::Origin::Syntax,
                        location: file.loc(rest),
                    });
                }
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
            if name == "children"
                && matches!(ty, m::TypeRef::Unknown { ref text, .. } if !text.as_deref().is_some_and(|text| text.contains("=>")))
            {
                ty = m::TypeRef::Node;
            }
            out.push(m::Slot {
                id: format!("{id}.{name}"),
                name: name.into(),
                kind: m::SlotKind::Prop,
                ty,
                optional: prop.is_some_and(|p| p.optional) || default.is_some(),
                initial: default.map(|n| default_expr(file, &out, props_object, n)),
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
            let ty = if prop.name == "children"
                && matches!(prop.ty, m::TypeRef::Unknown { ref text, .. } if !text.as_deref().is_some_and(|text| text.contains("=>")))
            {
                m::TypeRef::Node
            } else {
                prop.ty
            };
            out.push(m::Slot {
                id: format!("{id}.{}", prop.name),
                name: prop.name.clone(),
                kind: m::SlotKind::Prop,
                ty,
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
            if pat.kind() == "object_pattern"
                && value.kind() == "identifier"
                && Some(file.text(value)) == props_object
            {
                for (prop, local, default, bind) in destructured_props(file, pat) {
                    let initial = default.map(|n| default_expr(file, &out, props_object, n));
                    let prop_id = format!("{id}.{prop}");
                    if let Some(slot) = out
                        .iter_mut()
                        .find(|slot| slot.kind == m::SlotKind::Prop && slot.name == prop)
                    {
                        if initial.is_some() {
                            slot.initial = initial.clone();
                            slot.optional = true;
                        }
                    } else {
                        out.push(m::Slot {
                            id: prop_id.clone(),
                            name: prop.clone(),
                            kind: m::SlotKind::Prop,
                            ty: unknown(),
                            optional: initial.is_some(),
                            initial: initial.clone(),
                            source: None,
                            origin: m::Origin::Syntax,
                            location: file.loc(bind),
                        });
                    }
                    if local != prop && !out.iter().any(|slot| slot.name == local) {
                        out.push(m::Slot {
                            id: format!("{id}.{local}"),
                            name: local,
                            kind: m::SlotKind::External,
                            ty: unknown(),
                            optional: true,
                            initial: Some(m::Expr::Slot {
                                slot: prop_id,
                                path: vec![],
                            }),
                            source: Some(file.text(bind).into()),
                            origin: m::Origin::Syntax,
                            location: file.loc(bind),
                        });
                    }
                }
                continue;
            }
            if pat.kind() == "identifier"
                && !matches!(
                    value.kind(),
                    "arrow_function"
                        | "function_expression"
                        | "object"
                        | "array"
                        | "template_string"
                )
            {
                let name = file.text(pat);
                out.push(m::Slot {
                    id: format!("{id}.{name}"),
                    name: name.into(),
                    kind: m::SlotKind::External,
                    ty: unknown(),
                    optional: true,
                    initial: derived_expr(file, &out, props_object, value, 0),
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
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|slot| seen.insert((slot.id.clone(), slot.name.clone())));
    out
}

fn props_object_name<'a>(file: &'a File, f: Ast<'_>) -> Option<&'a str> {
    let param = field(f, "parameters").and_then(first)?;
    field(param, "pattern")
        .filter(|pattern| pattern.kind() == "identifier")
        .map(|pattern| file.text(pattern))
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
    if consequence.kind() == "return_statement"
        || consequence.kind() == "statement_block" && contains_return(consequence)
    {
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

fn contains_return(node: Ast<'_>) -> bool {
    node.kind() == "return_statement"
        || !matches!(
            node.kind(),
            "arrow_function" | "function_expression" | "function_declaration" | "method_definition"
        ) && children(node).into_iter().any(contains_return)
}

fn statements_root(
    cx: &mut render::Context<'_>,
    statements: Vec<Ast<'_>>,
    id: String,
) -> Option<m::Node> {
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
                } else if contains_return(*statement) {
                    return None;
                }
            }
            "switch_statement" if final_return.is_none() && conditional.is_empty() => {
                return switch_root(cx, *statement, id);
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

fn switch_root(cx: &mut render::Context<'_>, statement: Ast<'_>, id: String) -> Option<m::Node> {
    let value = crate::syntax::field(statement, "value")?;
    let body = crate::syntax::field(statement, "body")?;
    let discriminant = cx.expr(value);
    let mut cases = Vec::new();
    let mut fallback = Vec::new();
    for case in children(body) {
        if case.kind() == "switch_default" {
            fallback = vec![statements_root(
                cx,
                children(case),
                format!("{id}.default"),
            )?];
        } else if case.kind() == "switch_case" {
            let case_value = crate::syntax::field(case, "value")?;
            let statements = children(case)
                .into_iter()
                .filter(|child| *child != case_value)
                .collect();
            let content = statements_root(cx, statements, format!("{id}.case{}", cases.len()))?;
            cases.push((case_value, content));
        }
    }
    let mut otherwise = m::Node::Fragment(m::Fragment {
        children: fallback,
        id: format!("{id}.otherwise"),
        location: cx.file.loc(statement),
    });
    let count = cases.len();
    for (index, (case_value, content)) in cases.into_iter().rev().enumerate() {
        otherwise = m::Node::Branch(m::Branch {
            id: format!("{id}.switch{}", count - index - 1),
            on: m::Expr::Binary {
                operator: "===".into(),
                left: Box::new(discriminant.clone()),
                right: Box::new(cx.expr(case_value)),
            },
            arms: vec![
                m::Arm {
                    when: m::Match::Truthy(true),
                    children: vec![content],
                },
                m::Arm {
                    when: m::Match::Otherwise,
                    children: vec![otherwise],
                },
            ],
            location: cx.file.loc(case_value),
        });
    }
    Some(otherwise)
}

fn component_root(cx: &mut render::Context<'_>, f: Ast<'_>, id: String) -> m::Node {
    if f.kind() == "method_definition" {
        let body = field(f, "body").filter(|body| body.kind() == "statement_block");
        return body
            .and_then(|body| statements_root(cx, children(body), id.clone()))
            .or_else(|| crate::returned(f).map(|n| cx.node(n, id.clone())))
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

fn merge_unique<T>(target: &mut Vec<T>, incoming: Vec<T>, key: impl Fn(&T) -> String) {
    for value in incoming {
        let value_key = key(&value);
        if !target.iter().any(|existing| key(existing) == value_key) {
            target.push(value);
        }
    }
}

fn merge_models(target: &mut m::SourceModel, incoming: m::SourceModel) {
    merge_unique(&mut target.files, incoming.files, |file| file.path.clone());
    merge_unique(&mut target.screens, incoming.screens, |screen| {
        screen.id.clone()
    });
    merge_unique(&mut target.components, incoming.components, |component| {
        component.id.clone()
    });
    merge_unique(&mut target.item_schemas, incoming.item_schemas, |schema| {
        schema.id.clone()
    });
    merge_unique(&mut target.tokens, incoming.tokens, |token| {
        token.id.clone()
    });
    merge_unique(&mut target.edges, incoming.edges, |edge| edge.id.clone());
    if target.document_style.is_none() {
        target.document_style = incoming.document_style;
    }
    for diagnostic in incoming.diagnostics {
        if !target.diagnostics.contains(&diagnostic) {
            target.diagnostics.push(diagnostic);
        }
    }
}

/// Extract several independent TSX roots into one deterministic model. The roots share one parse
/// and render cache, and duplicate shared components are kept once under their stable file#name id.
pub fn extract_entries_incremental(
    project: &Path,
    entries: &[PathBuf],
    cache: &mut ParseCache,
    dependency_structure_changed: bool,
) -> Result<(m::SourceModel, LoadStats), String> {
    let Some(first) = entries.first() else {
        return Err("expected at least one TSX entry".into());
    };
    let built = discover_built_css(project);
    let (mut merged, mut stats) = extract_in_context_impl(
        project,
        first,
        built.as_deref(),
        None,
        None,
        cache,
        dependency_structure_changed,
    )?;
    for entry in &entries[1..] {
        let (model, next) = extract_in_context_impl(
            project,
            entry,
            built.as_deref(),
            None,
            None,
            cache,
            dependency_structure_changed,
        )?;
        merge_models(&mut merged, model);
        stats.parsed.extend(next.parsed);
        stats.reused.extend(next.reused);
    }
    merged.files.sort_by(|a, b| a.path.cmp(&b.path));
    merged.screens.sort_by(|a, b| a.id.cmp(&b.id));
    merged.components.sort_by(|a, b| a.id.cmp(&b.id));
    merged.item_schemas.sort_by(|a, b| a.id.cmp(&b.id));
    merged.tokens.sort_by(|a, b| a.id.cmp(&b.id));
    merged.edges.sort_by(|a, b| a.id.cmp(&b.id));
    stats.parsed.sort();
    stats.parsed.dedup();
    stats.reused.sort();
    stats.reused.dedup();
    m::validate(&merged).map_err(|errs| format!("invalid merged source model: {errs:?}"))?;
    Ok((merged, stats))
}

pub fn extract_entries(project: &Path, entries: &[PathBuf]) -> Result<m::SourceModel, String> {
    let mut cache = ParseCache::default();
    extract_entries_incremental(project, entries, &mut cache, true).map(|(model, _)| model)
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
                props_object: props_object_name(file, f),
                class_render: f.kind() == "method_definition",
                tailwind,
                built,
                sources: &source_sheets,
                modules: modules.bindings(file, f),
                signatures: &signatures,
                items: vec![],
                bindings: vec![],
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
        let exported: Vec<_> = files[&entry]
            .exported_names()
            .into_iter()
            .filter_map(|exported| candidates.iter().find(|c| c.name == exported).copied())
            .collect();
        let root = default
            .and_then(|n| candidates.iter().find(|c| c.name == n).copied())
            .or(match candidates.as_slice() {
                [root] => Some(*root),
                _ => None,
            })
            .or(match exported.as_slice() {
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
