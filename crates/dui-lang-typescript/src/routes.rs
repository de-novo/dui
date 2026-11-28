//! React Router object routes and statically declared navigation triggers.
//!
//! This module reads syntax only. It evaluates a deliberately small constant-expression
//! subset (literals, templates, object properties, and simple arrow functions) so route
//! configuration files can be read without running project code.
use crate::{
    Files, literal, symbol,
    syntax::{File, children, field, first, unwrap_expr},
};
use dui_core::source_model as m;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::Node as Ast;

const MAX_STATIC_DEPTH: usize = 24;

#[derive(Clone, Debug, PartialEq)]
enum StaticValue {
    String(String),
    Bool(bool),
    Number(f64),
    Null,
    Undefined,
    Unknown,
}

impl StaticValue {
    fn truthy(&self) -> Option<bool> {
        match self {
            Self::String(value) => Some(!value.is_empty()),
            Self::Bool(value) => Some(*value),
            Self::Number(value) => Some(*value != 0.0),
            Self::Null | Self::Undefined => Some(false),
            Self::Unknown => None,
        }
    }

    fn js_string(&self) -> Option<String> {
        match self {
            Self::String(value) => Some(value.clone()),
            Self::Bool(value) => Some(value.to_string()),
            Self::Number(value) => {
                if value.fract() == 0.0 {
                    Some(format!("{}", *value as i64))
                } else {
                    Some(value.to_string())
                }
            }
            Self::Null => Some("null".into()),
            Self::Undefined => Some("undefined".into()),
            Self::Unknown => None,
        }
    }
}

fn unwrap_static(mut n: Ast<'_>) -> Ast<'_> {
    loop {
        let next = match n.kind() {
            "parenthesized_expression"
            | "jsx_expression"
            | "as_expression"
            | "satisfies_expression"
            | "non_null_expression"
            | "await_expression" => first(n),
            _ => None,
        };
        let Some(child) = next else {
            return n;
        };
        n = child;
    }
}

fn descendants<'a>(root: Ast<'a>) -> Vec<Ast<'a>> {
    let mut out = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        out.push(node);
        let mut nested = children(node);
        nested.reverse();
        pending.extend(nested);
    }
    out
}

fn static_value(value: Value) -> StaticValue {
    match value {
        Value::String(value) => StaticValue::String(value),
        Value::Bool(value) => StaticValue::Bool(value),
        Value::Number(value) => value
            .as_f64()
            .map(StaticValue::Number)
            .unwrap_or(StaticValue::Unknown),
        Value::Null => StaticValue::Null,
        _ => StaticValue::Unknown,
    }
}

fn declaration_expression<'a>(file: &'a File, declaration: Ast<'a>, name: &str) -> Option<Ast<'a>> {
    if file.name(declaration, "name") != Some(name) {
        return None;
    }
    match declaration.kind() {
        "variable_declarator" => field(declaration, "value"),
        "function_declaration" => Some(declaration),
        _ => None,
    }
}

fn statement_declarations(statement: Ast<'_>) -> Vec<Ast<'_>> {
    let declaration = if statement.kind() == "export_statement" {
        field(statement, "declaration")
    } else {
        Some(statement)
    };
    declaration
        .map(|declaration| {
            if matches!(
                declaration.kind(),
                "lexical_declaration" | "ambient_declaration"
            ) {
                children(declaration)
            } else {
                vec![declaration]
            }
        })
        .unwrap_or_default()
}

fn scoped_expression<'a>(file: &'a File, at: Ast<'a>, name: &str) -> Option<Ast<'a>> {
    let mut scope = at.parent();
    while let Some(node) = scope {
        if matches!(node.kind(), "statement_block" | "program") {
            for statement in children(node) {
                for declaration in statement_declarations(statement) {
                    let temporal_dead_zone = declaration.kind() == "variable_declarator"
                        && declaration.start_byte() > at.start_byte();
                    if temporal_dead_zone {
                        continue;
                    }
                    if let Some(expression) = declaration_expression(file, declaration, name) {
                        return Some(expression);
                    }
                }
            }
        }
        scope = node.parent();
    }
    None
}

fn top_level_expression<'a>(file: &'a File, name: &str) -> Option<Ast<'a>> {
    file.declarations()
        .into_iter()
        .find_map(|declaration| declaration_expression(file, declaration, name))
}

fn identifier_expression_located<'a>(
    files: &'a Files,
    file: &'a File,
    at: Ast<'a>,
    name: &str,
    depth: usize,
) -> Option<(&'a File, Ast<'a>)> {
    if depth > MAX_STATIC_DEPTH {
        return None;
    }
    if let Some(expression) = scoped_expression(file, at, name) {
        return Some((file, expression));
    }
    let (module, exported) = file.bindings.get(name)?;
    let target = files.get(module)?;
    let local = target.exported(exported)?;
    top_level_expression(target, local).map(|expression| (target.as_ref(), expression))
}

fn object_value<'a>(file: &File, object: Ast<'a>, key: &str) -> Option<Ast<'a>> {
    children(object).into_iter().find_map(|child| {
        if child.kind() != "pair" {
            return None;
        }
        let name = field(child, "key").map(|key_node| {
            let text = file.text(key_node);
            text.strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(text)
                .to_owned()
        })?;
        (name == key)
            .then_some(child)
            .and_then(|pair| field(pair, "value"))
    })
}

fn member_parts(file: &File, node: Ast<'_>) -> Option<(String, Vec<String>)> {
    let mut object = node;
    let mut parts = Vec::new();
    while object.kind() == "member_expression" {
        if file.text(object).contains("?.") {
            return None;
        }
        parts.push(file.name(object, "property")?.to_owned());
        object = field(object, "object")?;
    }
    if object.kind() != "identifier" {
        return None;
    }
    parts.reverse();
    Some((file.text(object).to_owned(), parts))
}

fn resolve_object_path<'a>(
    files: &'a Files,
    file: &'a File,
    at: Ast<'a>,
    base: &str,
    parts: &[String],
    depth: usize,
) -> Option<(&'a File, Ast<'a>)> {
    if depth > MAX_STATIC_DEPTH {
        return None;
    }
    let (mut owner, mut expression) =
        identifier_expression_located(files, file, at, base, depth + 1)?;
    for part in parts {
        let mut target = unwrap_static(expression);
        if target.kind() == "identifier" {
            let (next_owner, next_expression) =
                identifier_expression_located(files, owner, target, owner.text(target), depth + 1)?;
            owner = next_owner;
            target = unwrap_static(next_expression);
        }
        if target.kind() != "object" {
            return None;
        }
        expression = object_value(owner, target, part)?;
    }
    Some((owner, expression))
}

fn parameter_name(file: &File, parameter: Ast<'_>) -> Option<String> {
    field(parameter, "name")
        .filter(|name| name.kind() == "identifier")
        .or_else(|| {
            children(parameter)
                .into_iter()
                .find(|child| child.kind() == "identifier")
        })
        .map(|name| file.text(name).to_owned())
}

type StaticEnv = BTreeMap<String, StaticValue>;

fn eval_function_call<'a>(
    files: &'a Files,
    file: &'a File,
    call_file: &'a File,
    function: Ast<'a>,
    arguments: Option<Ast<'a>>,
    env: &StaticEnv,
    depth: usize,
) -> StaticValue {
    if depth > MAX_STATIC_DEPTH {
        return StaticValue::Unknown;
    }
    let argument_values: Vec<StaticValue> = arguments
        .map(children)
        .unwrap_or_default()
        .into_iter()
        .map(|argument| eval_node(files, call_file, argument, env, depth + 1))
        .collect();
    let mut local = env.clone();
    let mut index = 0;
    if let Some(parameter) = field(function, "parameter")
        && let Some(name) = parameter_name(file, parameter)
    {
        local.insert(
            name,
            argument_values
                .first()
                .cloned()
                .unwrap_or(StaticValue::Undefined),
        );
    }
    if let Some(parameters) = field(function, "parameters") {
        for parameter in children(parameters).into_iter().filter(|parameter| {
            matches!(
                parameter.kind(),
                "required_parameter" | "optional_parameter"
            )
        }) {
            if let Some(name) = parameter_name(file, parameter) {
                local.insert(
                    name,
                    argument_values
                        .get(index)
                        .cloned()
                        .unwrap_or(StaticValue::Undefined),
                );
            }
            index += 1;
        }
    }
    let Some(body) = field(function, "body") else {
        return StaticValue::Unknown;
    };
    if body.kind() == "statement_block" {
        let statements = children(body);
        let returns: Vec<_> = statements
            .iter()
            .filter(|statement| statement.kind() == "return_statement")
            .collect();
        if returns.len() != 1 {
            return StaticValue::Unknown;
        }
        return first(*returns[0])
            .map(|expression| eval_node(files, file, expression, &local, depth + 1))
            .unwrap_or(StaticValue::Undefined);
    }
    eval_node(files, file, body, &local, depth + 1)
}

fn eval_call(
    files: &Files,
    file: &File,
    node: Ast<'_>,
    env: &StaticEnv,
    depth: usize,
) -> StaticValue {
    if depth > MAX_STATIC_DEPTH {
        return StaticValue::Unknown;
    }
    let Some(function_node) = field(node, "function").map(unwrap_static) else {
        return StaticValue::Unknown;
    };
    let function = match function_node.kind() {
        "member_expression" => {
            let Some((base, parts)) = member_parts(file, function_node) else {
                return StaticValue::Unknown;
            };
            resolve_object_path(files, file, function_node, &base, &parts, depth + 1)
        }
        "identifier" => identifier_expression_located(
            files,
            file,
            function_node,
            file.text(function_node),
            depth + 1,
        ),
        _ => None,
    };
    let Some((owner, function)) = function else {
        return StaticValue::Unknown;
    };
    let function = unwrap_static(function);
    if matches!(
        function.kind(),
        "arrow_function" | "function_expression" | "function_declaration"
    ) {
        eval_function_call(
            files,
            owner,
            file,
            function,
            field(node, "arguments"),
            env,
            depth + 1,
        )
    } else {
        StaticValue::Unknown
    }
}

fn eval_member(
    files: &Files,
    file: &File,
    node: Ast<'_>,
    env: &StaticEnv,
    depth: usize,
) -> StaticValue {
    if depth > MAX_STATIC_DEPTH {
        return StaticValue::Unknown;
    }
    let Some((base, parts)) = member_parts(file, node) else {
        return StaticValue::Unknown;
    };
    resolve_object_path(files, file, node, &base, &parts, depth + 1)
        .map(|(owner, expression)| eval_node(files, owner, expression, env, depth + 1))
        .unwrap_or(StaticValue::Unknown)
}

fn eval_identifier(
    files: &Files,
    file: &File,
    node: Ast<'_>,
    env: &StaticEnv,
    depth: usize,
) -> StaticValue {
    let name = file.text(node);
    if let Some(value) = env.get(name) {
        return value.clone();
    }
    if depth > MAX_STATIC_DEPTH {
        return StaticValue::Unknown;
    }
    identifier_expression_located(files, file, node, name, depth + 1)
        .map(|(owner, expression)| eval_node(files, owner, expression, env, depth + 1))
        .unwrap_or(StaticValue::Unknown)
}

fn eval_template(
    files: &Files,
    file: &File,
    node: Ast<'_>,
    env: &StaticEnv,
    depth: usize,
) -> StaticValue {
    if depth > MAX_STATIC_DEPTH {
        return StaticValue::Unknown;
    }
    let mut out = String::new();
    for child in children(node) {
        match child.kind() {
            "string_fragment" => out.push_str(file.text(child)),
            "template_substitution" => {
                let Some(expression) = first(child) else {
                    return StaticValue::Unknown;
                };
                let value = eval_node(files, file, expression, env, depth + 1);
                let Some(text) = value.js_string() else {
                    return StaticValue::Unknown;
                };
                out.push_str(&text);
            }
            "escape_sequence" => return StaticValue::Unknown,
            _ => {}
        }
    }
    StaticValue::String(out)
}

fn eval_node(
    files: &Files,
    file: &File,
    node: Ast<'_>,
    env: &StaticEnv,
    depth: usize,
) -> StaticValue {
    if depth > MAX_STATIC_DEPTH {
        return StaticValue::Unknown;
    }
    let node = unwrap_static(node);
    if let Some(value) = literal(file, node) {
        return static_value(value);
    }
    match node.kind() {
        "identifier" => eval_identifier(files, file, node, env, depth + 1),
        "member_expression" => eval_member(files, file, node, env, depth + 1),
        "call_expression" => eval_call(files, file, node, env, depth + 1),
        "template_string" => eval_template(files, file, node, env, depth + 1),
        "ternary_expression" => {
            let Some(condition) = field(node, "condition") else {
                return StaticValue::Unknown;
            };
            let Some(truthy) = eval_node(files, file, condition, env, depth + 1).truthy() else {
                return StaticValue::Unknown;
            };
            let branch = if truthy {
                field(node, "consequence")
            } else {
                field(node, "alternative")
            };
            branch
                .map(|branch| eval_node(files, file, branch, env, depth + 1))
                .unwrap_or(StaticValue::Unknown)
        }
        "binary_expression" if file.name(node, "operator") == Some("+") => {
            let (Some(left), Some(right)) = (field(node, "left"), field(node, "right")) else {
                return StaticValue::Unknown;
            };
            let left = eval_node(files, file, left, env, depth + 1);
            let right = eval_node(files, file, right, env, depth + 1);
            match (left, right) {
                (StaticValue::String(left), StaticValue::String(right)) => {
                    StaticValue::String(left + &right)
                }
                (StaticValue::Number(left), StaticValue::Number(right)) => {
                    StaticValue::Number(left + right)
                }
                (left, right) => match (left.js_string(), right.js_string()) {
                    (Some(left), Some(right)) => StaticValue::String(left + &right),
                    _ => StaticValue::Unknown,
                },
            }
        }
        _ => StaticValue::Unknown,
    }
}

fn static_string(files: &Files, file: &File, node: Ast<'_>) -> Option<String> {
    match eval_node(files, file, node, &StaticEnv::new(), 0) {
        StaticValue::String(value) => Some(value),
        _ => None,
    }
}

fn string_literal(file: &File, node: Ast<'_>) -> Option<String> {
    match literal(file, unwrap_static(node)) {
        Some(Value::String(value)) => Some(value),
        _ => None,
    }
}

struct RouteCandidate {
    route: String,
    component: String,
    location: m::Location,
    depth: usize,
}

struct RouteScan<'a> {
    files: &'a Files,
    components: &'a BTreeSet<String>,
    candidates: Vec<RouteCandidate>,
    diagnostics: Vec<m::Diagnostic>,
}

fn pair_value<'a>(file: &File, object: Ast<'a>, key: &str) -> Option<Ast<'a>> {
    object_value(file, object, key)
}

fn join_route(parent: &str, child: &str) -> String {
    let joined = if child.starts_with('/') {
        child.to_owned()
    } else if child.is_empty() {
        parent.to_owned()
    } else if parent.is_empty() || parent == "/" {
        format!("/{child}")
    } else {
        format!("{parent}/{child}")
    };
    normalize_route(&joined)
}

fn normalize_route(path: &str) -> String {
    let mut normalized = if path.is_empty() {
        "/".to_owned()
    } else if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    };
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

fn is_router_factory(file: &File, call: Ast<'_>) -> bool {
    let Some(function) = field(call, "function").map(unwrap_static) else {
        return false;
    };
    if function.kind() != "identifier" {
        return false;
    }
    let local = file.text(function);
    let Some((module, exported)) = file.bindings.get(local) else {
        return false;
    };
    matches!(
        module.as_str(),
        "react-router" | "react-router-dom" | "react-router/dom"
    ) && matches!(
        exported.as_str(),
        "createBrowserRouter" | "createHashRouter" | "createMemoryRouter"
    )
}

fn array_node<'a>(
    files: &'a Files,
    file: &'a File,
    node: Ast<'a>,
    depth: usize,
) -> Option<(&'a File, Ast<'a>)> {
    if depth > MAX_STATIC_DEPTH {
        return None;
    }
    let node = unwrap_static(node);
    if node.kind() == "array" {
        return Some((file, node));
    }
    if node.kind() == "identifier" {
        return identifier_expression_located(files, file, node, file.text(node), depth + 1)
            .and_then(|(owner, expression)| array_node(files, owner, expression, depth + 1));
    }
    None
}

impl<'a> RouteScan<'a> {
    fn component_id(&self, file: &File, name: &str) -> Option<String> {
        let (target, exported) = symbol(self.files, file, name)?;
        let id = format!("{}#{exported}", target.path);
        self.components.contains(&id).then_some(id)
    }

    fn lazy_component(&self, file: &'a File, lazy: Ast<'a>) -> Option<String> {
        descendants(lazy).into_iter().find_map(|node| {
            if node.kind() != "call_expression"
                || field(node, "function").is_none_or(|f| f.kind() != "import")
            {
                return None;
            }
            let request = field(node, "arguments")
                .and_then(first)
                .and_then(|argument| string_literal(file, argument))?;
            let module = file.dynamic_imports.get(&request)?;
            let target = self.files.get(module)?;
            let exported = target.exported("default")?;
            let id = format!("{}#{exported}", target.path);
            self.components.contains(&id).then_some(id)
        })
    }

    fn jsx_component(&self, file: &'a File, jsx: Ast<'a>) -> Option<String> {
        let jsx = unwrap_static(jsx);
        if !matches!(jsx.kind(), "jsx_element" | "jsx_self_closing_element") {
            return None;
        }
        descendants(jsx).into_iter().find_map(|node| {
            if !matches!(node.kind(), "jsx_element" | "jsx_self_closing_element") {
                return None;
            }
            let open = field(node, "open_tag").unwrap_or(node);
            let name = file.name(open, "name")?;
            self.component_id(file, name)
        })
    }

    fn object_component(&self, file: &'a File, object: Ast<'a>) -> Option<String> {
        if let Some(lazy) = pair_value(file, object, "lazy")
            && let Some(component) = self.lazy_component(file, lazy)
        {
            return Some(component);
        }
        if let Some(element) = pair_value(file, object, "element")
            && let Some(component) = self.jsx_component(file, element)
        {
            return Some(component);
        }
        if let Some(component) = pair_value(file, object, "Component")
            && unwrap_static(component).kind() == "identifier"
        {
            return self.component_id(file, file.text(unwrap_static(component)));
        }
        None
    }

    fn visit_array(&mut self, file: &'a File, array: Ast<'a>, parent: &str, depth: usize) {
        for child in children(array) {
            if child.kind() == "object" {
                self.visit_object(file, child, parent, depth);
            }
        }
    }

    fn visit_object(&mut self, file: &'a File, object: Ast<'a>, parent: &str, depth: usize) {
        let path_node = pair_value(file, object, "path");
        let path = path_node.and_then(|node| static_string(self.files, file, node));
        let location = path_node
            .map(|node| file.loc(node))
            .unwrap_or_else(|| file.loc(object));
        if path_node.is_some() && path.is_none() {
            self.diagnostics.push(m::Diagnostic {
                code: "unresolved-route".into(),
                message: "Route path must be a statically resolvable string".into(),
                location: Some(location.clone()),
            });
        }
        let index = pair_value(file, object, "index")
            .is_some_and(|node| unwrap_static(node).kind() == "true");
        let mut base = parent.to_owned();
        let mut declares_screen = false;
        if let Some(path) = path {
            if path == "*" || path.ends_with("/*") {
                self.diagnostics.push(m::Diagnostic {
                    code: "unsupported-route".into(),
                    message: "Wildcard routes are not named screens".into(),
                    location: Some(location),
                });
                return;
            }
            base = join_route(parent, &path);
            declares_screen = true;
        } else if index {
            base = normalize_route(parent);
            declares_screen = true;
        }
        if declares_screen && let Some(component) = self.object_component(file, object) {
            self.candidates.push(RouteCandidate {
                route: base.clone(),
                component,
                location,
                depth,
            });
        }
        if let Some(children_node) = pair_value(file, object, "children")
            && let Some((owner, children_array)) = array_node(self.files, file, children_node, 0)
        {
            self.visit_array(owner, children_array, &base, depth + 1);
        }
    }
}

fn discover_object_routes(files: &Files, model: &mut m::SourceModel) {
    let components: BTreeSet<String> = model
        .components
        .iter()
        .map(|component| component.id.clone())
        .collect();
    let mut scan = RouteScan {
        files,
        components: &components,
        candidates: Vec::new(),
        diagnostics: Vec::new(),
    };
    for file in files.values().filter(|file| !file.path.ends_with(".css")) {
        if ![
            "createBrowserRouter",
            "createHashRouter",
            "createMemoryRouter",
        ]
        .iter()
        .any(|needle| file.src.contains(needle))
        {
            continue;
        }
        for node in descendants(file.tree.root_node()) {
            if node.kind() != "call_expression" || !is_router_factory(file, node) {
                continue;
            }
            let Some(argument) = field(node, "arguments").and_then(first) else {
                continue;
            };
            if let Some((owner, array)) = array_node(files, file, argument, 0) {
                scan.visit_array(owner, array, "", 0);
            }
        }
    }
    let mut by_route: BTreeMap<String, Vec<RouteCandidate>> = BTreeMap::new();
    for candidate in scan.candidates {
        by_route
            .entry(candidate.route.clone())
            .or_default()
            .push(candidate);
    }
    for (route, candidates) in by_route {
        let mut chosen_index = 0;
        for (index, candidate) in candidates.iter().enumerate() {
            if candidate.depth > candidates[chosen_index].depth {
                chosen_index = index;
            }
        }
        let chosen = &candidates[chosen_index];
        for (index, duplicate) in candidates.iter().enumerate() {
            if index != chosen_index {
                scan.diagnostics.push(m::Diagnostic {
                    code: "duplicate-route".into(),
                    message: format!(
                        "{route}: using the deepest declaration; parent layout composition is not represented"
                    ),
                    location: Some(duplicate.location.clone()),
                });
            }
        }
        if model
            .screens
            .iter()
            .any(|screen| screen.route.as_deref() == Some(route.as_str()))
        {
            continue;
        }
        let name = chosen
            .component
            .rsplit_once('#')
            .map(|(_, name)| name)
            .unwrap_or(chosen.component.as_str());
        model.screens.push(m::Screen {
            id: format!("route:{route}"),
            name: name.into(),
            route: Some(route),
            component: chosen.component.clone(),
            origin: m::Origin::Syntax,
            location: chosen.location.clone(),
        });
    }
    model.diagnostics.extend(scan.diagnostics);
}

fn open_tag(node: Ast<'_>) -> Ast<'_> {
    field(node, "open_tag").unwrap_or(node)
}

fn jsx_attr<'a>(file: &File, open: Ast<'a>, key: &str) -> Option<Ast<'a>> {
    children(open)
        .into_iter()
        .filter(|node| node.kind() == "jsx_attribute")
        .find(|node| first(*node).is_some_and(|name| file.text(name) == key))
        .and_then(|node| children(node).get(1).copied())
}

fn jsx_at<'a>(file: &'a File, location: &m::Location) -> Option<Ast<'a>> {
    descendants(file.tree.root_node()).into_iter().find(|node| {
        matches!(node.kind(), "jsx_element" | "jsx_self_closing_element")
            && node.start_byte() == location.start as usize
            && node.end_byte() == location.end as usize
    })
}

fn internal_route(route: &str) -> bool {
    route.starts_with('/') && !route.starts_with("//")
}

fn link_key(file: &File, open: Ast<'_>) -> Option<&'static str> {
    let name = file.name(open, "name")?;
    if name == "a" {
        return Some("href");
    }
    let (module, exported) = file.bindings.get(name)?;
    (matches!(
        module.as_str(),
        "react-router" | "react-router-dom" | "react-router/dom"
    ) && matches!(exported.as_str(), "Link" | "NavLink"))
    .then_some("to")
}

fn resolve_link_targets(files: &Files, model: &mut m::SourceModel) {
    for edge in &mut model.edges {
        if edge.kind != m::EdgeKind::Link || !matches!(edge.to, m::EdgeTarget::Opaque { .. }) {
            continue;
        }
        let Some(file) = files.get(&edge.location.file) else {
            continue;
        };
        let Some(jsx) = jsx_at(file, &edge.location) else {
            continue;
        };
        let open = open_tag(jsx);
        let Some(key) = link_key(file, open) else {
            continue;
        };
        let Some(target) = jsx_attr(file, open, key) else {
            continue;
        };
        if let Some(route) = static_string(files, file, target)
            && internal_route(&route)
        {
            edge.to = m::EdgeTarget::Route { route };
        }
    }
}

fn react_router_export(file: &File, local: &str, exported_name: &str) -> bool {
    let Some((module, exported)) = file.bindings.get(local) else {
        return false;
    };
    matches!(
        module.as_str(),
        "react-router" | "react-router-dom" | "react-router/dom"
    ) && exported == exported_name
}

fn declaration_list(body: Ast<'_>) -> Vec<Ast<'_>> {
    children(body)
        .into_iter()
        .filter(|statement| {
            matches!(
                statement.kind(),
                "lexical_declaration" | "variable_declaration"
            )
        })
        .flat_map(children)
        .collect()
}

fn navigate_names(file: &File, function: Ast<'_>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(body) = field(function, "body") else {
        return names;
    };
    if body.kind() != "statement_block" {
        return names;
    }
    for variable in declaration_list(body) {
        if variable.kind() != "variable_declarator" {
            continue;
        }
        let Some(name) = file.name(variable, "name") else {
            continue;
        };
        let Some(call) = field(variable, "value")
            .map(unwrap_expr)
            .filter(|value| value.kind() == "call_expression")
        else {
            continue;
        };
        let Some(callee) = field(call, "function").map(unwrap_expr) else {
            continue;
        };
        if callee.kind() == "identifier"
            && react_router_export(file, file.text(callee), "useNavigate")
        {
            names.insert(name.to_owned());
        }
    }
    names
}

fn handler_function<'a>(file: &'a File, component: Ast<'a>, name: &str) -> Option<Ast<'a>> {
    let body = field(component, "body")?;
    if body.kind() != "statement_block" {
        return None;
    }
    for variable in declaration_list(body) {
        if variable.kind() == "variable_declarator"
            && file.name(variable, "name") == Some(name)
            && let Some(value) = field(variable, "value").map(unwrap_expr)
            && matches!(value.kind(), "arrow_function" | "function_expression")
        {
            return Some(value);
        }
    }
    children(body).into_iter().find(|statement| {
        statement.kind() == "function_declaration" && file.name(*statement, "name") == Some(name)
    })
}

fn navigate_calls<'a>(file: &File, root: Ast<'a>, names: &BTreeSet<String>) -> Vec<Ast<'a>> {
    descendants(root)
        .into_iter()
        .filter(|node| {
            node.kind() == "call_expression"
                && field(*node, "function")
                    .map(unwrap_expr)
                    .is_some_and(|function| {
                        function.kind() == "identifier" && names.contains(file.text(function))
                    })
        })
        .collect()
}

fn edge_text(file: &File, expression: Option<Ast<'_>>) -> m::Expr {
    expression
        .and_then(|expression| literal(file, unwrap_expr(expression)))
        .map(|value| m::Expr::Literal { value })
        .unwrap_or_else(|| m::Expr::Opaque {
            text: expression
                .map(|expression| file.text(expression).into())
                .unwrap_or_default(),
            reads: vec![],
        })
}

fn discover_navigate_edges(files: &Files, model: &mut m::SourceModel) {
    let mut additions = Vec::new();
    for component in &model.components {
        let Some((path, name)) = component.id.rsplit_once('#') else {
            continue;
        };
        let Some(file) = files.get(path) else {
            continue;
        };
        if !file.src.contains("useNavigate") {
            continue;
        }
        let Some((_, function)) = file.functions().into_iter().find(|(n, _)| *n == name) else {
            continue;
        };
        let navigate = navigate_names(file, function);
        if navigate.is_empty() {
            continue;
        }
        let mut nodes = vec![&component.root];
        while let Some(node) = nodes.pop() {
            nodes.extend(node.children());
            if !matches!(
                node,
                m::Node::Element(_) | m::Node::Instance(_) | m::Node::Placeholder(_)
            ) {
                continue;
            }
            let location = node.location();
            if location.file != file.path {
                continue;
            }
            let Some(jsx) = jsx_at(file, location) else {
                continue;
            };
            let open = open_tag(jsx);
            let Some(trigger) = jsx_attr(file, open, "onClick").map(unwrap_expr) else {
                continue;
            };
            let handler = match trigger.kind() {
                "identifier" => handler_function(file, function, file.text(trigger)),
                "arrow_function" | "function_expression" => Some(trigger),
                _ => None,
            };
            let Some(handler) = handler else {
                continue;
            };
            for (index, call) in navigate_calls(file, handler, &navigate)
                .into_iter()
                .enumerate()
            {
                let argument = field(call, "arguments").and_then(first);
                let to = argument
                    .and_then(|argument| static_string(files, file, argument))
                    .filter(|route| internal_route(route))
                    .map(|route| m::EdgeTarget::Route { route })
                    .unwrap_or_else(|| m::EdgeTarget::Opaque {
                        expr: edge_text(file, argument),
                    });
                additions.push(m::Edge {
                    id: if index == 0 {
                        format!("{}:navigate", node.id())
                    } else {
                        format!("{}:navigate.{}", node.id(), index + 1)
                    },
                    from: node.id().into(),
                    kind: m::EdgeKind::Navigate,
                    to,
                    origin: m::Origin::Syntax,
                    location: file.loc(call),
                });
            }
        }
    }
    let existing: BTreeSet<String> = model.edges.iter().map(|edge| edge.id.clone()).collect();
    additions.retain(|edge| !existing.contains(&edge.id));
    model.edges.extend(additions);
}

struct TanstackRoute {
    local: String,
    module: String,
    path: String,
    parent: String,
    location: m::Location,
}

fn tanstack_updates(files: &Files) -> Vec<TanstackRoute> {
    let mut routes = Vec::new();
    for file in files.values().filter(|file| !file.path.ends_with(".css")) {
        for declaration in file.declarations() {
            if declaration.kind() != "variable_declarator" {
                continue;
            }
            let Some(local) = file
                .name(declaration, "name")
                .filter(|name| {
                    !name.is_empty()
                        && name
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
                })
                .map(str::to_owned)
            else {
                continue;
            };
            let Some(call) = field(declaration, "value").map(unwrap_static) else {
                continue;
            };
            if call.kind() != "call_expression" {
                continue;
            }
            let Some(function) = field(call, "function").map(unwrap_static) else {
                continue;
            };
            if function.kind() != "member_expression"
                || file.name(function, "property") != Some("update")
            {
                continue;
            }
            let Some(receiver) = field(function, "object").map(unwrap_static) else {
                continue;
            };
            if receiver.kind() != "identifier" {
                continue;
            }
            let Some((module, exported)) = file.bindings.get(file.text(receiver)) else {
                continue;
            };
            if exported != "Route" {
                continue;
            }
            let Some(options) = field(call, "arguments")
                .and_then(first)
                .map(unwrap_static)
                .filter(|node| node.kind() == "object")
            else {
                continue;
            };
            let (Some(path), Some(parent_node)) = (
                object_value(file, options, "path")
                    .and_then(|node| static_string(files, file, node)),
                object_value(file, options, "getParentRoute"),
            ) else {
                continue;
            };
            let parent = descendants(parent_node)
                .into_iter()
                .find(|node| node.kind() == "identifier" && file.text(*node) != "getParentRoute")
                .map(|node| file.text(node).to_owned())
                .unwrap_or_default();
            routes.push(TanstackRoute {
                local,
                module: module.clone(),
                path,
                parent,
                location: file.loc(options),
            });
        }
    }
    routes
}

fn tanstack_component(files: &Files, scan: &RouteScan<'_>, path: &str) -> Option<String> {
    let file = files.get(path)?;
    for node in descendants(file.tree.root_node()) {
        if node.kind() != "object" {
            continue;
        }
        let Some(component) = object_value(file, node, "component").map(unwrap_static) else {
            continue;
        };
        if component.kind() == "identifier"
            && let Some(id) = scan.component_id(file, file.text(component))
        {
            return Some(id);
        }
    }
    None
}

fn tanstack_full_path(
    local: &str,
    by_local: &BTreeMap<String, TanstackRoute>,
    memo: &mut BTreeMap<String, Option<String>>,
    active: &mut BTreeSet<String>,
) -> Option<String> {
    if let Some(path) = memo.get(local) {
        return path.clone();
    }
    if !active.insert(local.to_owned()) {
        return None;
    }
    let route = by_local.get(local)?;
    let parent = if route.parent.to_ascii_lowercase().contains("rootroute") {
        Some(String::new())
    } else if by_local.contains_key(&route.parent) {
        tanstack_full_path(&route.parent, by_local, memo, active)
    } else {
        Some(String::new())
    }?;
    let child = route.path.trim_matches('/');
    let full = if child.is_empty() {
        if parent.is_empty() {
            "/".into()
        } else {
            parent
        }
    } else if parent.is_empty() {
        normalize_route(&route.path)
    } else {
        normalize_route(&format!("{}/{child}", parent.trim_end_matches('/')))
    };
    active.remove(local);
    memo.insert(local.to_owned(), Some(full.clone()));
    Some(full)
}

fn discover_tanstack_routes(files: &Files, model: &mut m::SourceModel) {
    let updates = tanstack_updates(files);
    if updates.is_empty() {
        return;
    }
    let components: BTreeSet<String> = model
        .components
        .iter()
        .map(|component| component.id.clone())
        .collect();
    let scan = RouteScan {
        files,
        components: &components,
        candidates: Vec::new(),
        diagnostics: Vec::new(),
    };
    let by_local: BTreeMap<_, _> = updates
        .into_iter()
        .map(|route| (route.local.clone(), route))
        .collect();
    let mut memo = BTreeMap::new();
    for local in by_local.keys().cloned().collect::<Vec<_>>() {
        let mut active = BTreeSet::new();
        tanstack_full_path(&local, &by_local, &mut memo, &mut active);
    }
    for (local, route) in &by_local {
        let Some(full) = memo.get(local).cloned().flatten() else {
            continue;
        };
        if full.contains('$') || full.contains('*') {
            continue;
        }
        let Some(component) = tanstack_component(files, &scan, &route.module) else {
            continue;
        };
        if model
            .screens
            .iter()
            .any(|screen| screen.route.as_deref() == Some(full.as_str()))
        {
            continue;
        }
        let name = component
            .rsplit_once('#')
            .map(|(_, name)| name)
            .unwrap_or(component.as_str());
        model.screens.push(m::Screen {
            id: format!("route:{full}"),
            name: name.into(),
            route: Some(full),
            component,
            origin: m::Origin::Syntax,
            location: route.location.clone(),
        });
    }
}

/// Adds React Router object-route screens and imperative navigation edges, then improves
/// declarative link targets that use the same constant-expression subset.
pub(crate) fn discover(files: &Files, model: &mut m::SourceModel) {
    discover_object_routes(files, model);
    discover_tanstack_routes(files, model);
    resolve_link_targets(files, model);
    discover_navigate_edges(files, model);
}

#[cfg(test)]
mod tests {
    use dui_core::source_model as m;
    use std::{fs, path::Path};

    fn write(root: &Path, path: &str, text: &str) {
        let file = root.join(path);
        fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
        fs::write(file, text).expect("write");
    }

    #[test]
    fn object_routes_and_navigation_edges_use_statically_declared_syntax() {
        let project = tempfile::tempdir().expect("tempdir");
        let root = project.path();
        write(
            root,
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["./src/*"]}}}"#,
        );
        write(
            root,
            "src/main.tsx",
            r#"import { createRouter } from "./router";
createRouter();
"#,
        );
        write(
            root,
            "src/config/paths.ts",
            r#"export const paths = {
  home: { path: "/", getHref: () => "/" },
  login: {
    path: "/login",
    getHref: (next?: string) => `/login${next ? `?next=${next}` : ""}`,
  },
  app: {
    root: { path: "/app", getHref: () => "/app" },
    dashboard: { path: "", getHref: () => "/app" },
    users: { path: "users", getHref: () => "/app/users" },
  },
} as const;
"#,
        );
        write(
            root,
            "src/router.tsx",
            r#"import { createBrowserRouter } from "react-router";
import { paths } from "@/config/paths";
import Shell from "./shell";

export const createRouter = () => createBrowserRouter([
  { path: paths.home.path, lazy: () => import("./routes/home").then((m) => m) },
  { path: paths.login.path, lazy: () => import("./routes/login").then((m) => m) },
  {
    path: paths.app.root.path,
    element: <Shell />,
    children: [
      { path: paths.app.dashboard.path, lazy: () => import("./routes/dashboard").then((m) => m) },
      { path: paths.app.users.path, lazy: () => import("./routes/users").then((m) => m) },
      { path: "*", lazy: () => import("./routes/missing").then((m) => m) },
    ],
  },
]);
"#,
        );
        write(
            root,
            "src/shell.tsx",
            "export default function Shell() { return <main />; }\n",
        );
        write(
            root,
            "src/button.tsx",
            r#"import * as React from "react";
const Button = React.forwardRef(function Button(props, ref) {
  return <button ref={ref} {...props} />;
});
export { Button };
"#,
        );
        write(
            root,
            "src/routes/home.tsx",
            r#"import { NavLink, useNavigate } from "react-router";
import { paths } from "@/config/paths";
import { Button } from "../button";

const HomeRoute = () => {
  const go = useNavigate();
  const start = () => {
    if (ready) {
      go(paths.app.dashboard.getHref());
    } else {
      go(paths.login.getHref());
    }
  };
  return <>
    <Button onClick={start}>Start</Button>
    <NavLink to={paths.login.getHref()}>Sign in</NavLink>
  </>;
};
export default HomeRoute;
"#,
        );
        for (file, name) in [
            ("login", "LoginRoute"),
            ("dashboard", "DashboardRoute"),
            ("users", "UsersRoute"),
            ("missing", "MissingRoute"),
        ] {
            write(
                root,
                &format!("src/routes/{file}.tsx"),
                &format!("export default function {name}() {{ return <main />; }}\n"),
            );
        }

        let model = crate::extract(root, Path::new("src/main.tsx")).expect("extract");
        let routes: Vec<_> = model
            .screens
            .iter()
            .filter_map(|screen| screen.route.as_deref())
            .collect();
        assert_eq!(routes, ["/", "/app", "/app/users", "/login"]);
        let app = model
            .screens
            .iter()
            .find(|screen| screen.route.as_deref() == Some("/app"))
            .expect("app screen");
        assert!(app.component.ends_with("dashboard.tsx#DashboardRoute"));
        assert!(
            model
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "duplicate-route")
        );
        assert!(
            model
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "unsupported-route")
        );

        let navigate: Vec<_> = model
            .edges
            .iter()
            .filter(|edge| edge.kind == m::EdgeKind::Navigate)
            .collect();
        assert_eq!(navigate.len(), 2);
        let destinations: Vec<_> = navigate
            .iter()
            .map(|edge| match &edge.to {
                m::EdgeTarget::Screen { screen } => screen.as_str(),
                _ => "unresolved",
            })
            .collect();
        assert_eq!(destinations, ["route:/app", "route:/login"]);
        assert!(
            model.edges.iter().any(|edge| {
                edge.kind == m::EdgeKind::Link
                    && matches!(&edge.to, m::EdgeTarget::Screen { screen } if screen == "route:/login")
            }),
            "NavLink target resolves to the login screen"
        );
        let relations = model.screen_edges();
        for edge in &navigate {
            assert!(
                relations
                    .iter()
                    .any(|relation| relation.from == "route:/" && relation.edge == edge.id),
                "landing relation for {}",
                edge.id
            );
        }
    }
}
