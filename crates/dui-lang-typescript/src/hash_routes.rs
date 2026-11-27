//! Corroborate a class screen's literal hash links with its initialized route table.
use crate::{Ast, children, field, first, literal, m, syntax::File};
use std::collections::{BTreeMap, BTreeSet};

fn route_table<'a>(file: &File, method: Ast<'a>) -> Option<Ast<'a>> {
    let body = children(field(method, "body")?);
    for decl in body
        .iter()
        .copied()
        .filter(|n| matches!(n.kind(), "variable_declaration" | "lexical_declaration"))
        .flat_map(children)
    {
        let Some(call) = field(decl, "value").filter(|n| n.kind() == "call_expression") else {
            continue;
        };
        if file.name(call, "function") != Some("Router") {
            continue;
        }
        let Some(name) = file.name(decl, "name") else {
            continue;
        };
        let initialized = body
            .iter()
            .filter_map(|s| first(*s))
            .filter(|s| s.kind() == "call_expression" && s.start_byte() > call.end_byte())
            .any(|s| {
                field(s, "function").is_some_and(|f| {
                    f.kind() == "member_expression"
                        && file.name(f, "object") == Some(name)
                        && file.name(f, "property") == Some("init")
                })
            });
        if initialized
            && let Some(args) = field(call, "arguments")
            && let [table] = children(args).as_slice()
            && table.kind() == "object"
        {
            return Some(*table);
        }
    }
    None
}

pub fn apply(file: &File, model: &mut m::SourceModel) {
    let [root] = model.screens.as_slice() else {
        return;
    };
    if root.route.is_some() || root.location.file != file.path {
        return;
    }
    // An ambient browser Router or a director import may provide the factory.
    // Ordinary project functions named Router do not declare hash routes.
    let ambient = children(file.tree.root_node())
        .into_iter()
        .filter(|n| n.kind() == "ambient_declaration")
        .flat_map(children)
        .filter(|n| matches!(n.kind(), "variable_declaration" | "lexical_declaration"))
        .flat_map(children)
        .any(|v| file.name(v, "name") == Some("Router"));
    if !ambient
        && !file
            .bindings
            .get("Router")
            .is_some_and(|(m, e)| m == "director" && matches!(e.as_str(), "Router" | "default"))
    {
        return;
    }
    let Some(class) = file.declarations().into_iter().find(|n| {
        n.kind() == "class_declaration" && file.name(*n, "name") == Some(root.name.as_str())
    }) else {
        return;
    };
    let Some(table) = field(class, "body")
        .into_iter()
        .flat_map(children)
        .filter(|m| file.name(*m, "name") == Some("componentDidMount"))
        .find_map(|m| route_table(file, m))
    else {
        return;
    };
    let owned: BTreeSet<_> = model.screen_edges().into_iter().map(|r| r.edge).collect();
    let links: BTreeSet<_> = model
        .edges
        .iter()
        .filter(|e| owned.contains(&e.id))
        .filter_map(hash)
        .collect();
    let mut routes = BTreeMap::new();
    for pair in children(table) {
        let Some(key) = field(pair, "key") else {
            return;
        };
        let Some(path) = literal(file, key).and_then(|v| v.as_str().map(str::to_owned)) else {
            return;
        };
        if !links.contains(path.as_str())
            || !path.starts_with('/')
            || path.starts_with("//")
            || routes.contains_key(&path)
        {
            return;
        }
        let route = format!("/#{path}");
        routes.insert(
            path,
            m::Screen {
                id: format!("route:{route}"),
                route: Some(route),
                location: file.loc(key),
                ..root.clone()
            },
        );
    }
    if routes.is_empty() {
        return;
    }
    for e in &mut model.edges {
        if owned.contains(&e.id)
            && let Some(target) = hash(e).and_then(|p| routes.get(p))
        {
            e.to = m::EdgeTarget::Screen {
                screen: target.id.clone(),
            };
        }
    }
    model.screens = routes.into_values().collect();
}
fn hash(edge: &m::Edge) -> Option<&str> {
    if let m::EdgeTarget::Opaque {
        expr: m::Expr::Literal { value },
    } = &edge.to
    {
        value.as_str()?.strip_prefix('#')
    } else {
        None
    }
}
