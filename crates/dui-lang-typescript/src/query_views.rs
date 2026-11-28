//! Literal URL-query view selection and same-page query links.
//!
//! This module is deliberately conservative: a query value becomes a screen only when a
//! literal `URLSearchParams` equality controls a branch whose JSX resolves to one model
//! component. Runtime gates and imperative history writes remain opaque.
use crate::{
    Ast, Files, literal, m, symbol,
    syntax::{File, children, field, first, unwrap_expr},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct QueryEq {
    param: String,
    value: String,
}

#[derive(Default)]
struct QueryScope {
    search_params: BTreeSet<String>,
    reads: BTreeMap<String, String>,
    aliases: BTreeMap<String, QueryEq>,
}

fn string(file: &File, node: Ast<'_>) -> Option<String> {
    literal(file, node).and_then(|value| value.as_str().map(str::to_owned))
}

fn search_params_constructor(file: &File, node: Ast<'_>) -> bool {
    let node = unwrap_expr(node);
    node.kind() == "new_expression"
        && field(node, "constructor").is_some_and(|n| file.text(n) == "URLSearchParams")
}

fn query_get(file: &File, node: Ast<'_>, scope: &QueryScope) -> Option<String> {
    let node = unwrap_expr(node);
    if node.kind() == "identifier" {
        return scope.reads.get(file.text(node)).cloned();
    }
    if node.kind() != "call_expression" {
        return None;
    }
    let function = field(node, "function")?;
    if function.kind() != "member_expression" || file.name(function, "property") != Some("get") {
        return None;
    }
    let object = unwrap_expr(field(function, "object")?);
    let search_params = search_params_constructor(file, object)
        || (object.kind() == "identifier" && scope.search_params.contains(file.text(object)));
    if !search_params {
        return None;
    }
    field(node, "arguments")
        .and_then(first)
        .and_then(|argument| string(file, argument))
}

fn equality(file: &File, node: Ast<'_>, scope: &QueryScope) -> Option<QueryEq> {
    let node = unwrap_expr(node);
    if node.kind() != "binary_expression" || file.name(node, "operator") != Some("===") {
        return None;
    }
    let left = field(node, "left")?;
    let right = field(node, "right")?;
    for (read, value) in [(left, right), (right, left)] {
        if let Some(param) = query_get(file, read, scope)
            && let Some(value) = string(file, value)
        {
            return Some(QueryEq { param, value });
        }
    }
    None
}

fn scope(file: &File, body: Ast<'_>) -> QueryScope {
    let mut out = QueryScope::default();
    for declaration in children(body)
        .into_iter()
        .filter(|n| matches!(n.kind(), "lexical_declaration" | "variable_declaration"))
        .flat_map(children)
    {
        let (Some(name), Some(value)) =
            (file.name(declaration, "name"), field(declaration, "value"))
        else {
            continue;
        };
        let value = unwrap_expr(value);
        if search_params_constructor(file, value) {
            out.search_params.insert(name.into());
        }
        if let Some(param) = query_get(file, value, &out) {
            out.reads.insert(name.into(), param);
        }
        if let Some(eq) = equality(file, value, &out) {
            out.aliases.insert(name.into(), eq);
        }
    }
    out
}

fn condition_query(file: &File, node: Ast<'_>, scope: &QueryScope) -> Option<(QueryEq, bool)> {
    let node = unwrap_expr(node);
    if let Some(eq) = equality(file, node, scope) {
        return Some((eq, false));
    }
    if node.kind() == "identifier"
        && let Some(eq) = scope.aliases.get(file.text(node))
    {
        return Some((eq.clone(), false));
    }
    if node.kind() == "binary_expression" && file.name(node, "operator") == Some("&&") {
        let left = field(node, "left").and_then(|n| condition_query(file, n, scope));
        let right = field(node, "right").and_then(|n| condition_query(file, n, scope));
        if let Some((eq, _)) = left.or(right) {
            // The query equality is necessary, but the rest of the conjunction is runtime state.
            return Some((eq, true));
        }
    }
    None
}

fn import_call<'a>(file: &'a File, node: Ast<'a>) -> Option<(String, Ast<'a>)> {
    if node.kind() != "call_expression"
        || field(node, "function").is_none_or(|f| f.kind() != "import")
    {
        return None;
    }
    let request = field(node, "arguments").and_then(first)?;
    Some((string(file, request)?, node))
}

fn find_import<'a>(file: &'a File, node: Ast<'a>) -> Option<(String, Ast<'a>)> {
    if let Some(found) = import_call(file, node) {
        return Some(found);
    }
    children(node)
        .into_iter()
        .find_map(|child| find_import(file, child))
}

fn parameter_name(file: &File, arrow: Ast<'_>) -> Option<String> {
    let parameter = field(arrow, "parameter").or_else(|| {
        field(arrow, "parameters")
            .and_then(first)
            .and_then(|p| field(p, "pattern"))
    })?;
    (parameter.kind() == "identifier").then(|| file.text(parameter).into())
}

fn lazy_export(file: &File, import: Ast<'_>) -> String {
    let mut export = "default".to_owned();
    let Some(member) = import
        .parent()
        .filter(|p| p.kind() == "member_expression" && file.name(*p, "property") == Some("then"))
    else {
        return export;
    };
    let Some(call) = member.parent().filter(|p| p.kind() == "call_expression") else {
        return export;
    };
    let Some(callback) = field(call, "arguments")
        .and_then(first)
        .filter(|n| n.kind() == "arrow_function")
    else {
        return export;
    };
    let Some(parameter) = parameter_name(file, callback) else {
        return export;
    };
    let mut pending = field(callback, "body").into_iter().collect::<Vec<_>>();
    while let Some(node) = pending.pop() {
        if node.kind() == "member_expression"
            && field(node, "object").is_some_and(|object| file.text(object) == parameter)
            && let Some(property) = file.name(node, "property")
        {
            export = property.into();
            break;
        }
        pending.extend(children(node));
    }
    export
}

/// Literal dynamic-import aliases used as components. Static imports still win.
fn dynamic_bindings(file: &File) -> BTreeMap<String, (String, String)> {
    fn walk<'a>(file: &'a File, node: Ast<'a>, out: &mut BTreeMap<String, (String, String)>) {
        if node.kind() == "variable_declarator"
            && let Some(value) = field(node, "value")
            && let Some((request, import)) = find_import(file, value)
            && let Some(module) = file.dynamic_imports.get(&request)
        {
            match field(node, "name") {
                Some(pattern) if pattern.kind() == "object_pattern" => {
                    for child in children(pattern) {
                        if child.kind() == "shorthand_property_identifier_pattern" {
                            let name = file.text(child).to_owned();
                            out.entry(name.clone()).or_insert((module.clone(), name));
                        }
                    }
                }
                Some(name) if name.kind() == "identifier" => {
                    let function = field(value, "function");
                    let lazy = function.is_some_and(|f| {
                        file.text(f) == "lazy"
                            && file.bindings.get("lazy").is_some_and(|(module, export)| {
                                module == "react" && export == "lazy"
                            })
                    });
                    if lazy {
                        out.entry(file.text(name).into())
                            .or_insert((module.clone(), lazy_export(file, import)));
                    }
                }
                _ => {}
            }
        }
        for child in children(node) {
            walk(file, child, out);
        }
    }
    let mut out = BTreeMap::new();
    walk(file, file.tree.root_node(), &mut out);
    out
}

fn tag_name<'a>(file: &'a File, node: Ast<'_>) -> Option<&'a str> {
    let open = field(node, "open_tag").unwrap_or(node);
    file.name(open, "name")
}

fn uppercase_jsx<'a>(file: &'a File, node: Ast<'a>) -> Vec<Ast<'a>> {
    fn walk<'a>(file: &'a File, node: Ast<'a>, root: bool, out: &mut Vec<Ast<'a>>) {
        if !root
            && matches!(
                node.kind(),
                "arrow_function" | "function_expression" | "function_declaration"
            )
        {
            return;
        }
        if matches!(node.kind(), "jsx_element" | "jsx_self_closing_element")
            && tag_name(file, node).is_some_and(|name| name.starts_with(char::is_uppercase))
        {
            out.push(node);
        }
        for child in children(node) {
            walk(file, child, false, out);
        }
    }
    let mut out = Vec::new();
    walk(file, node, true, &mut out);
    out
}

/// Pick view leaves inside linear wrapper chains. Branching uppercase JSX is ambiguous.
fn view_leaves<'a>(file: &'a File, node: Ast<'a>) -> Vec<Ast<'a>> {
    let nodes = uppercase_jsx(file, node);
    if nodes.is_empty() {
        return Vec::new();
    }
    let parent = |index: usize| {
        let node = nodes[index];
        nodes
            .iter()
            .enumerate()
            .filter(|(other, candidate)| {
                *other != index
                    && candidate.start_byte() < node.start_byte()
                    && candidate.end_byte() > node.end_byte()
            })
            .min_by_key(|(_, candidate)| candidate.end_byte() - candidate.start_byte())
            .map(|(other, _)| other)
    };
    let children_of = |index: usize| {
        (0..nodes.len())
            .filter(|child| parent(*child) == Some(index))
            .collect::<Vec<_>>()
    };
    fn chain_leaf(index: usize, children_of: &impl Fn(usize) -> Vec<usize>) -> Option<usize> {
        match children_of(index).as_slice() {
            [] => Some(index),
            [child] => chain_leaf(*child, children_of),
            _ => None,
        }
    }
    let mut leaves = Vec::new();
    for root in (0..nodes.len()).filter(|index| parent(*index).is_none()) {
        let Some(leaf) = chain_leaf(root, &children_of) else {
            return Vec::new();
        };
        if !leaves.contains(&leaf) {
            leaves.push(leaf);
        }
    }
    leaves.into_iter().map(|index| nodes[index]).collect()
}

fn resolve_component(
    files: &Files,
    file: &File,
    model: &m::SourceModel,
    name: &str,
) -> Option<String> {
    let dynamic = dynamic_bindings(file);
    let (target, export) = if let Some((module, export)) = dynamic.get(name) {
        (files.get(module)?.as_ref(), export.as_str())
    } else {
        symbol(files, file, name)?
    };
    let id = format!("{}#{export}", target.path);
    model.components.iter().any(|c| c.id == id).then_some(id)
}

fn view_target(
    files: &Files,
    file: &File,
    model: &m::SourceModel,
    branch: Ast<'_>,
) -> Option<(String, m::Location)> {
    let leaves = view_leaves(file, branch);
    if leaves.is_empty() {
        return None;
    }
    let mut found = BTreeMap::new();
    for leaf in leaves {
        let name = tag_name(file, leaf)?;
        let id = resolve_component(files, file, model, name)?;
        found.insert(id, file.loc(leaf));
    }
    if found.len() == 1 {
        found.into_iter().next()
    } else {
        None
    }
}

fn simple_part(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
}

fn diagnostic(model: &mut m::SourceModel, code: &str, message: &str, location: m::Location) {
    if !model
        .diagnostics
        .iter()
        .any(|d| d.code == code && d.location.as_ref() == Some(&location))
    {
        model.diagnostics.push(m::Diagnostic {
            code: code.into(),
            message: message.into(),
            location: Some(location),
        });
    }
}

fn add_screen(model: &mut m::SourceModel, route: String, component: String, location: m::Location) {
    if let Some(existing) = model
        .screens
        .iter_mut()
        .find(|screen| screen.route.as_deref() == Some(route.as_str()))
    {
        if existing.component != component {
            diagnostic(
                model,
                "conflicting-query-view",
                "The same query destination selects different components",
                location,
            );
        }
        return;
    }
    if route == "/"
        && let Some(existing) = model
            .screens
            .iter_mut()
            .find(|screen| screen.route.is_none() && screen.component == component)
    {
        existing.id = "route:/".into();
        existing.route = Some(route);
        existing.location = location;
        return;
    }
    let name = model
        .components
        .iter()
        .find(|c| c.id == component)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| component.clone());
    model.screens.push(m::Screen {
        id: format!("route:{route}"),
        name,
        route: Some(route),
        component,
        origin: m::Origin::Syntax,
        location,
    });
}

fn add_query_screen(
    model: &mut m::SourceModel,
    eq: &QueryEq,
    target: (String, m::Location),
    gated: bool,
) {
    let (component, location) = target;
    if !(simple_part(&eq.param) && simple_part(&eq.value)) {
        diagnostic(
            model,
            "unresolved-query-view",
            "Query view names need simple literal parameter and value text",
            location,
        );
        return;
    }
    if gated {
        diagnostic(
            model,
            "runtime-query-gate",
            "The query-selected view also depends on runtime values",
            location.clone(),
        );
    }
    add_screen(
        model,
        format!("/?{}={}", eq.param, eq.value),
        component,
        location,
    );
}

fn function_named<'a>(file: &'a File, name: &str) -> Option<Ast<'a>> {
    file.functions()
        .into_iter()
        .find_map(|(n, f)| (n == name).then_some(f))
}

fn nested_query_screens(files: &Files, model: &mut m::SourceModel, component: &str) {
    let Some(component_model) = model.components.iter().find(|c| c.id == component) else {
        return;
    };
    let Some(file) = files.get(&component_model.location.file) else {
        return;
    };
    let Some(function) = function_named(file, &component_model.name) else {
        return;
    };
    let Some(body) = field(function, "body") else {
        return;
    };
    let query_scope = scope(file, body);
    let mut ternaries = Vec::new();
    fn walk<'a>(node: Ast<'a>, root: bool, out: &mut Vec<Ast<'a>>) {
        if !root
            && matches!(
                node.kind(),
                "arrow_function" | "function_expression" | "function_declaration"
            )
        {
            return;
        }
        if node.kind() == "ternary_expression" {
            out.push(node);
        }
        for child in children(node) {
            walk(child, false, out);
        }
    }
    walk(body, true, &mut ternaries);
    let mut additions = Vec::new();
    for ternary in ternaries {
        let Some((eq, gated)) = field(ternary, "condition")
            .and_then(|condition| condition_query(file, condition, &query_scope))
        else {
            continue;
        };
        if let Some(branch) = field(ternary, "consequence")
            && let Some(target) = view_target(files, file, model, branch)
        {
            additions.push((eq, target, gated));
        } else {
            diagnostic(
                model,
                "unresolved-query-view",
                "A literal query comparison does not select one statically resolved component",
                file.loc(ternary),
            );
        }
    }
    for (eq, target, gated) in additions {
        add_query_screen(model, &eq, target, gated);
    }
}

fn query_route(value: &str) -> Option<String> {
    if value == "?" {
        return Some("/".into());
    }
    let query = value.strip_prefix('?')?;
    if query.is_empty() || query.contains(['#', ' ', '\t', '\n', '\r']) {
        return None;
    }
    Some(format!("/?{query}"))
}

fn resolve_query_links(model: &mut m::SourceModel) {
    let screens: BTreeMap<_, _> = model
        .screens
        .iter()
        .filter_map(|screen| {
            screen
                .route
                .as_ref()
                .map(|route| (route.clone(), screen.id.clone()))
        })
        .collect();
    let screen_routes: BTreeMap<_, _> = model
        .screens
        .iter()
        .map(|screen| (screen.id.as_str(), screen.route.as_deref().unwrap_or("/")))
        .collect();
    let mut edge_paths: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    for relation in model.screen_edges() {
        if let Some(route) = screen_routes.get(relation.from.as_str()) {
            edge_paths
                .entry(relation.edge.clone())
                .or_default()
                .insert(route.split('?').next().unwrap_or("/"));
        }
    }
    for edge in &mut model.edges {
        if !edge_paths
            .get(edge.id.as_str())
            .is_some_and(|paths| paths.contains("/"))
        {
            continue;
        }
        let m::EdgeTarget::Opaque {
            expr: m::Expr::Literal { value },
        } = &edge.to
        else {
            continue;
        };
        let Some(target) = value.as_str().and_then(query_route) else {
            continue;
        };
        if let Some(screen) = screens.get(&target) {
            edge.to = m::EdgeTarget::Screen {
                screen: screen.clone(),
            };
        }
    }
}

/// Add corroborated query views and resolve same-page query links after component extraction.
pub(crate) fn apply(files: &Files, entry: &str, model: &mut m::SourceModel) {
    let Some(file) = files.get(entry) else {
        return;
    };
    let program_scope = scope(file, file.tree.root_node());
    let mut default = None;
    let mut positives = Vec::new();
    for statement in children(file.tree.root_node())
        .into_iter()
        .filter(|n| n.kind() == "if_statement")
    {
        let Some((eq, gated)) = field(statement, "condition")
            .and_then(|condition| condition_query(file, condition, &program_scope))
        else {
            continue;
        };
        if let Some(branch) = field(statement, "consequence") {
            if let Some(target) = view_target(files, file, model, branch) {
                positives.push((eq, target, gated));
            } else {
                diagnostic(
                    model,
                    "unresolved-query-view",
                    "A literal query comparison does not select one statically resolved component",
                    file.loc(statement),
                );
            }
        }
        if let Some(branch) = field(statement, "alternative")
            && let Some(target) = view_target(files, file, model, branch)
            && default.is_none()
        {
            default = Some(target);
        }
    }
    for (eq, target, gated) in positives {
        add_query_screen(model, &eq, target, gated);
    }
    let has_default = default.is_some();
    let host = default.or_else(|| {
        model
            .screens
            .iter()
            .find(|screen| screen.route.is_none())
            .map(|screen| (screen.component.clone(), screen.location.clone()))
    });
    if let Some((component, location)) = host {
        if has_default {
            add_screen(model, "/".into(), component.clone(), location);
        }
        nested_query_screens(files, model, &component);
    }
    resolve_query_links(model);
}
