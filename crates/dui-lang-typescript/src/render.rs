//! Lower syntax into the model's node tree without running expressions or handlers.
use crate::{
    Files, literal, style, symbol,
    syntax::{File, children, field, first, unwrap_expr},
    unknown,
};
use dui_core::source_model as m;
use std::collections::BTreeMap;
use tree_sitter::Node as Ast;

pub struct Context<'a> {
    pub file: &'a File,
    pub files: &'a Files,
    pub slots: &'a [m::Slot],
    pub class_render: bool,
    pub signatures: &'a BTreeMap<String, Vec<m::Slot>>,
    pub modules: style::modules::Bindings<'a>,
    pub tailwind: bool,
    pub built: Option<&'a style::built::Sheet<'a>>,
    pub sources: &'a [style::built::Sheet<'a>],
    pub items: Vec<(String, String)>,
    pub constants: BTreeMap<String, serde_json::Value>,
    pub model: &'a mut m::SourceModel,
}
fn attr<'a>(f: &File, tag: Ast<'a>, key: &str) -> Option<Ast<'a>> {
    children(tag)
        .into_iter()
        .filter(|n| n.kind() == "jsx_attribute")
        .find(|n| first(*n).is_some_and(|k| f.text(k) == key))
        .and_then(|n| children(n).get(1).copied())
}
fn tag(n: Ast<'_>) -> Ast<'_> {
    field(n, "open_tag").unwrap_or(n)
}

fn object_key(file: &File, key: Ast<'_>) -> Option<String> {
    let raw = file.text(key);
    match key.kind() {
        "property_identifier" | "private_property_identifier" | "number" => Some(raw.into()),
        "string" => serde_json::from_str::<String>(raw).ok().or_else(|| {
            raw.strip_prefix('\'')
                .and_then(|v| v.strip_suffix('\''))
                .map(str::to_owned)
        }),
        _ => None,
    }
}

/// Parse a closed literal tree. Objects may spread other closed literal objects; duplicate keys
/// follow JavaScript's last-write-wins object semantics. No calls, arithmetic or references run.
pub(crate) fn static_literal(file: &File, node: Ast<'_>) -> Option<serde_json::Value> {
    let n = unwrap_expr(node);
    if let Some(value) = crate::literal(file, n) {
        return Some(value);
    }
    match n.kind() {
        "array" => {
            let mut out = Vec::new();
            for child in children(n) {
                if child.kind() == "spread_element" {
                    let value = static_literal(file, first(child)?)?;
                    out.extend(value.as_array()?.iter().cloned());
                } else {
                    out.push(static_literal(file, child)?);
                }
            }
            Some(out.into())
        }
        "object" => {
            let mut out = serde_json::Map::new();
            for child in children(n) {
                match child.kind() {
                    "pair" => {
                        let key = field(child, "key").and_then(|key| object_key(file, key))?;
                        let value = field(child, "value").and_then(|v| static_literal(file, v))?;
                        out.insert(key, value);
                    }
                    "spread_element" => {
                        let value = static_literal(file, first(child)?)?;
                        for (key, value) in value.as_object()? {
                            out.insert(key.clone(), value.clone());
                        }
                    }
                    "shorthand_property_identifier" => return None,
                    _ => return None,
                }
            }
            Some(out.into())
        }
        "unary_expression" if file.name(n, "operator") == Some("-") => {
            let value = static_literal(file, field(n, "argument")?)?;
            value.as_f64().map(|v| (-v).into())
        }
        _ => None,
    }
}

fn static_path(value: &serde_json::Value, path: &[String]) -> Option<serde_json::Value> {
    let mut current = value;
    for key in path {
        current = current.get(key)?;
    }
    Some(current.clone())
}

fn truthy_json(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(value) => value.as_f64().is_some_and(|v| v != 0.),
        serde_json::Value::String(value) => !value.is_empty(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => true,
    }
}

fn plus_json(left: serde_json::Value, right: serde_json::Value) -> Option<serde_json::Value> {
    if left.is_string() || right.is_string() {
        let text = |value: serde_json::Value| -> Option<String> {
            match value {
                serde_json::Value::String(value) => Some(value),
                serde_json::Value::Number(value) => Some(value.to_string()),
                serde_json::Value::Bool(value) => Some(value.to_string()),
                _ => None,
            }
        };
        return Some((text(left)? + &text(right)?).into());
    }
    Some((left.as_f64()? + right.as_f64()?).into())
}
fn object_pair<'a>(file: &File, object: Ast<'a>, key: &str) -> Option<Ast<'a>> {
    pair_entries(file, object)
        .into_iter()
        .find_map(|(name, value)| (name == key).then_some(value))
}

fn pair_entries<'a>(file: &File, object: Ast<'a>) -> Vec<(String, Ast<'a>)> {
    children(object)
        .into_iter()
        .filter(|node| node.kind() == "pair")
        .filter_map(|node| {
            Some((
                object_key(file, field(node, "key")?)?,
                field(node, "value")?,
            ))
        })
        .collect()
}

fn find_declarator<'a>(file: &File, root: Ast<'a>, name: &str) -> Option<Ast<'a>> {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.kind() == "variable_declarator"
            && field(node, "name").is_some_and(|n| file.text(n) == name)
        {
            return Some(node);
        }
        pending.extend(children(node));
    }
    None
}

impl Context<'_> {
    fn component(&self, name: &str) -> Option<String> {
        let (file, export) = symbol(self.files, self.file, name)?;
        let id = format!("{}#{export}", file.path);
        self.signatures.contains_key(&id).then_some(id)
    }
    fn binding(&self, mut n: Ast<'_>) -> Option<m::Expr> {
        let mut path = Vec::new();
        while n.kind() == "member_expression" {
            path.push(self.file.name(n, "property")?.into());
            n = field(n, "object")?;
        }
        if self.class_render && n.kind() == "this" {
            path.reverse();
            if let [props, name, rest @ ..] = path.as_slice()
                && props == "props"
                && let Some(slot) = self.slots.iter().find(|s| &s.name == name)
            {
                return Some(m::Expr::Slot {
                    slot: slot.id.clone(),
                    path: rest.to_vec(),
                });
            }
            return None;
        }
        if n.kind() != "identifier" {
            return None;
        }
        path.reverse();
        let name = self.file.text(n);
        if let Some((_, list)) = self.items.iter().rev().find(|(item, _)| item == name) {
            Some(m::Expr::Item {
                list: list.clone(),
                path,
            })
        } else if self.class_render {
            None
        } else {
            self.slots
                .iter()
                .find(|s| s.name == name)
                .map(|s| m::Expr::Slot {
                    slot: s.id.clone(),
                    path,
                })
        }
    }
    /// Evaluate only syntax-proven immutable bindings and closed boolean/string/class-helper
    /// expressions. Anything outside this bounded subset remains opaque rather than executed.
    fn static_value(&self, node: Ast<'_>) -> Option<serde_json::Value> {
        let n = unwrap_expr(node);
        if let Some(value) = static_literal(self.file, n) {
            return Some(value);
        }
        if n.kind() == "identifier" || n.kind() == "member_expression" {
            let mut base = n;
            let mut path = Vec::new();
            while base.kind() == "member_expression" {
                path.push(self.file.name(base, "property")?.to_owned());
                base = field(base, "object")?;
            }
            if base.kind() == "identifier" {
                path.reverse();
                return self
                    .constants
                    .get(self.file.text(base))
                    .and_then(|value| static_path(value, &path));
            }
        }
        match n.kind() {
            "ternary_expression" => {
                let condition = self.static_value(field(n, "condition")?)?;
                if truthy_json(&condition) {
                    self.static_value(field(n, "consequence")?)
                } else {
                    self.static_value(field(n, "alternative")?)
                }
            }
            "binary_expression" => {
                let left = self.static_value(field(n, "left")?)?;
                let operator = self.file.name(n, "operator")?;
                match operator {
                    "+" => plus_json(left, self.static_value(field(n, "right")?)?),
                    "&&" => {
                        if truthy_json(&left) {
                            self.static_value(field(n, "right")?)
                        } else {
                            Some(left)
                        }
                    }
                    "||" => {
                        if truthy_json(&left) {
                            Some(left)
                        } else {
                            self.static_value(field(n, "right")?)
                        }
                    }
                    "===" => Some((self.static_value(field(n, "right")?) == Some(left)).into()),
                    "!==" => Some((self.static_value(field(n, "right")?) != Some(left)).into()),
                    _ => None,
                }
            }
            "unary_expression" if self.file.name(n, "operator") == Some("!") => {
                Some((!truthy_json(&self.static_value(field(n, "argument")?)?)).into())
            }
            "call_expression" => {
                let function = field(n, "function")?;
                let local = if function.kind() == "identifier" {
                    self.file.text(function)
                } else {
                    return None;
                };
                if self.file.bindings.get(local).is_none()
                    && let Some(factory) =
                        find_declarator(self.file, self.file.tree.root_node(), local)
                            .and_then(|node| field(node, "value"))
                            .map(unwrap_expr)
                            .filter(|value| value.kind() == "call_expression")
                    && let Some(arguments) = field(factory, "arguments")
                {
                    let base = first(arguments)
                        .and_then(|v| literal(self.file, v))
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default();
                    let config = children(arguments)
                        .get(1)
                        .copied()
                        .filter(|v| v.kind() == "object")?;
                    let variants = object_pair(self.file, config, "variants")
                        .filter(|v| v.kind() == "object")?;
                    let defaults = object_pair(self.file, config, "defaultVariants")
                        .filter(|v| v.kind() == "object");
                    let options = field(n, "arguments")
                        .and_then(first)
                        .filter(|v| v.kind() == "object");
                    let mut supplied = BTreeMap::new();
                    if let Some(options) = options {
                        for (key, value) in pair_entries(self.file, options) {
                            let value = if value.kind() == "identifier" {
                                self.slots
                                    .iter()
                                    .find(|slot| slot.name == self.file.text(value))
                                    .and_then(|slot| slot.initial.as_ref())
                                    .and_then(|expr| match expr {
                                        m::Expr::Literal { value } => Some(value.clone()),
                                        _ => None,
                                    })
                            } else {
                                self.static_value(value)
                            };
                            if let Some(value) = value {
                                supplied.insert(key, value);
                            }
                        }
                    }
                    let mut out = base;
                    for (group, choices) in pair_entries(self.file, variants) {
                        let selected = supplied
                            .get(&group)
                            .and_then(|v| v.as_str().map(str::to_owned))
                            .or_else(|| {
                                defaults
                                    .and_then(|d| object_pair(self.file, d, &group))
                                    .and_then(|v| literal(self.file, v))
                                    .and_then(|v| v.as_str().map(str::to_owned))
                            });
                        if let Some(selected) = selected
                            && let Some(classes) = object_pair(self.file, choices, &selected)
                                .and_then(|v| literal(self.file, v))
                                .and_then(|v| v.as_str().map(str::to_owned))
                            && !classes.is_empty()
                        {
                            out.push(' ');
                            out.push_str(&classes);
                        }
                    }
                    return Some(out.into());
                }
                let (_, export) = self.file.bindings.get(local)?;
                if !matches!(
                    (export.as_str(), local),
                    ("default", "clsx" | "classnames" | "cn") | ("cn", "cn")
                ) {
                    return None;
                }
                let mut out = String::new();
                for argument in children(field(n, "arguments")?) {
                    let Some(value) = self.static_value(argument) else {
                        continue;
                    };
                    let mut push = |text: String| {
                        if !text.is_empty() {
                            if !out.is_empty() {
                                out.push(' ');
                            }
                            out.push_str(&text);
                        }
                    };
                    match value {
                        serde_json::Value::String(text) => push(text),
                        serde_json::Value::Array(values) => {
                            for value in values {
                                if let Some(text) = value.as_str() {
                                    push(text.to_owned());
                                }
                            }
                        }
                        serde_json::Value::Object(values) => {
                            for (key, enabled) in values {
                                if truthy_json(&enabled) {
                                    push(key);
                                }
                            }
                        }
                        serde_json::Value::Number(number) => push(number.to_string()),
                        _ => {}
                    }
                }
                Some(out.into())
            }
            _ => None,
        }
    }
    fn expr(&self, node: Ast<'_>) -> m::Expr {
        let n = unwrap_expr(node);
        if let Some(value) = literal(self.file, n) {
            return m::Expr::Literal { value };
        }
        if let Some(read) = self.binding(n) {
            return read;
        }
        if let Some(value) = self.static_value(n) {
            return m::Expr::Literal { value };
        }
        let mut reads = vec![];
        let mut pending = children(n);
        pending.reverse();
        while let Some(c) = pending.pop() {
            if let Some(read) = self.binding(c) {
                if !reads.contains(&read) {
                    reads.push(read)
                }
            } else if !matches!(c.kind(), "arrow_function" | "function_expression") {
                pending.extend(children(c).into_iter().rev())
            }
        }
        m::Expr::Opaque {
            text: self.file.text(n).into(),
            reads,
        }
    }
    fn placeholder(&self, n: Ast<'_>, id: String, reason: &str) -> m::Node {
        m::Node::Placeholder(m::Placeholder {
            id,
            reason: reason.into(),
            location: self.file.loc(n),
        })
    }
    pub fn node(&mut self, node: Ast<'_>, id: String) -> m::Node {
        let n = unwrap_expr(node);
        if n.has_error() {
            return self.placeholder(n, id, "Incomplete TSX syntax");
        }
        if id.bytes().filter(|b| *b == b'.').count() > 128 {
            return self.placeholder(n, id, "Nesting limit reached");
        }
        let loc = self.file.loc(n);
        if n.kind() == "identifier"
            && node.kind() == "jsx_expression"
            && let Some((condition, content)) = crate::class_view::conditional_local(self.file, n)
        {
            let on = self.expr(condition);
            return m::Node::Branch(m::Branch {
                on,
                arms: vec![
                    m::Arm {
                        when: m::Match::Truthy(true),
                        children: vec![self.node(content, format!("{id}.0.0"))],
                    },
                    m::Arm {
                        when: m::Match::Otherwise,
                        children: vec![],
                    },
                ],
                id,
                location: loc,
            });
        }
        match n.kind() {
            "jsx_element" | "jsx_self_closing_element" => self.element(n, id),
            "ternary_expression" | "binary_expression"
                if n.kind() == "ternary_expression"
                    || self.file.name(n, "operator") == Some("&&") =>
            {
                let cond = field(n, "condition").or_else(|| field(n, "left"));
                let yes = field(n, "consequence").or_else(|| field(n, "right"));
                let (Some(cond), Some(yes)) = (cond, yes) else {
                    return self.placeholder(n, id, "Incomplete branch");
                };
                let (on, when) = match (
                    self.file.name(cond, "operator"),
                    field(cond, "left"),
                    field(cond, "right").and_then(|v| literal(self.file, v)),
                ) {
                    (Some("==="), Some(left), Some(value)) => {
                        (self.expr(left), m::Match::Equals(value))
                    }
                    _ => (self.expr(cond), m::Match::Truthy(true)),
                };
                let mut arms = vec![m::Arm {
                    when,
                    children: vec![self.node(yes, format!("{id}.0.0"))],
                }];
                arms.push(m::Arm {
                    when: m::Match::Otherwise,
                    children: field(n, "alternative")
                        .map(|c| self.node(c, format!("{id}.1.0")))
                        .into_iter()
                        .collect(),
                });
                m::Node::Branch(m::Branch {
                    id,
                    on,
                    arms,
                    location: loc,
                })
            }
            "call_expression"
                if field(n, "function").and_then(|f| self.file.name(f, "property"))
                    == Some("map") =>
            {
                self.list(n, id)
            }
            "jsx_text" => m::Node::Text(m::Text {
                id,
                parts: vec![m::Expr::Literal {
                    value: self.file.text(n).into(),
                }],
                location: loc,
            }),
            _ => m::Node::Text(m::Text {
                id,
                parts: vec![self.expr(n)],
                location: loc,
            }),
        }
    }
    fn list(&mut self, n: Ast<'_>, id: String) -> m::Node {
        let source = field(n, "function").and_then(|f| field(f, "object"));
        let callback = field(n, "arguments").and_then(first);
        let Some((source, callback)) = source
            .zip(callback)
            .filter(|(_, c)| c.kind() == "arrow_function")
        else {
            return self.placeholder(n, id, "Unsupported map callback");
        };
        let param = field(callback, "parameter").or_else(|| {
            field(callback, "parameters")
                .and_then(first)
                .and_then(|p| field(p, "pattern"))
        });
        let Some((param, body)) = param
            .zip(crate::returned(callback))
            .filter(|(p, _)| p.kind() == "identifier")
        else {
            return self.placeholder(n, id, "Unsupported map template");
        };
        let item = self.file.text(param).to_owned();
        let source = self.expr(source);
        let ty = if let m::Expr::Slot { slot, path } = &source
            && path.is_empty()
            && let Some(m::TypeRef::Array { item }) =
                self.slots.iter().find(|s| &s.id == slot).map(|s| &s.ty)
        {
            (**item).clone()
        } else {
            unknown()
        };
        self.items.push((item.clone(), id.clone()));
        let key = attr(self.file, tag(body), "key").map(|n| self.expr(n));
        let child = self.node(body, format!("{id}.0"));
        self.items.pop();
        m::Node::List(m::List {
            id,
            source,
            item,
            item_type: ty,
            key,
            children: vec![child],
            location: self.file.loc(n),
        })
    }
    fn child_nodes(&mut self, n: Ast<'_>, id: &str) -> Vec<m::Node> {
        let mut nodes = Vec::new();
        for c in children(n).into_iter().filter(|c| {
            !(matches!(c.kind(), "jsx_opening_element" | "jsx_closing_element")
                || c.kind() == "jsx_expression" && first(*c).is_none())
        }) {
            if n.kind() == "jsx_self_closing_element" {
                break;
            }
            nodes.push(self.node(c, format!("{id}.{}", nodes.len())));
        }
        nodes
    }
    fn element(&mut self, n: Ast<'_>, id: String) -> m::Node {
        let open = tag(n);
        let Some(name) = self.file.name(open, "name") else {
            return m::Node::Fragment(m::Fragment {
                children: self.child_nodes(n, &id),
                id,
                location: self.file.loc(n),
            });
        };
        let resolved_name =
            if name.starts_with(char::is_uppercase) && self.component(name).is_none() {
                self.constants
                    .get(name)
                    .and_then(|value| value.as_str())
                    .unwrap_or(name)
            } else {
                name
            };
        let name = resolved_name;
        let component = self.component(name);
        let binding = self.file.bindings.get(name);
        let router = binding
            .filter(|(module, _)| matches!(module.as_str(), "react-router" | "react-router-dom"))
            .map(|(_, symbol)| symbol.as_str());
        let location = self.file.loc(n);
        if router == Some("Route")
            && let Some(route) = attr(self.file, open, "path")
                .and_then(|n| literal(self.file, n))
                .and_then(|v| v.as_str().map(str::to_owned))
                .filter(|s| s.starts_with('/'))
            && let Some(target) = attr(self.file, open, "element")
                .and_then(|v| self.file.name(tag(unwrap_expr(v)), "name"))
            && let Some(component) = self.component(target)
        {
            if self
                .model
                .screens
                .iter()
                .any(|s| s.route.as_ref() == Some(&route))
            {
                self.model.diagnostics.push(m::Diagnostic {
                    code: "duplicate-route".into(),
                    message: route,
                    location: Some(location.clone()),
                })
            } else {
                self.model.screens.push(m::Screen {
                    id: format!("route:{route}"),
                    name: target.into(),
                    route: Some(route),
                    component,
                    origin: m::Origin::Syntax,
                    location: location.clone(),
                });
            }
        } else if router == Some("Route") {
            self.model.diagnostics.push(m::Diagnostic {
                code: "unresolved-route".into(),
                message: "Route requires a literal absolute path and a statically resolved element component".into(),
                location: Some(location.clone()),
            });
        }
        if name == "a" || matches!(router, Some("Link" | "NavLink")) {
            let key = if name == "a" { "href" } else { "to" };
            if let Some(v) = attr(self.file, open, key) {
                let expr = self.expr(v);
                let to = if let m::Expr::Literal { value } = &expr
                    && let Some(route) = value
                        .as_str()
                        .filter(|s| s.starts_with('/') && !s.starts_with("//"))
                {
                    m::EdgeTarget::Route {
                        route: route.into(),
                    }
                } else {
                    m::EdgeTarget::Opaque { expr }
                };
                self.model.edges.push(m::Edge {
                    id: format!("{id}:link"),
                    from: id.clone(),
                    kind: m::EdgeKind::Link,
                    to,
                    origin: m::Origin::Syntax,
                    location: location.clone(),
                });
            }
        }
        let mut el = m::Element {
            id: id.clone(),
            tag: name.into(),
            module: binding
                .filter(|(module, _)| !module.starts_with('.') && !self.files.contains_key(module))
                .map(|(module, _)| module.clone()),
            props: BTreeMap::new(),
            style: vec![],
            unresolved_style: vec![],
            children: vec![],
            location: location.clone(),
        };
        if name.starts_with(char::is_uppercase) && component.is_none() && el.module.is_none() {
            return self.placeholder(n, id, "Unresolved component");
        }
        for attribute in open.children_by_field_name("attribute", &mut open.walk()) {
            if attribute.kind() != "jsx_attribute" {
                let spread = first(attribute).filter(|n| n.kind() == "spread_element");
                let object = spread
                    .and_then(first)
                    .and_then(|value| self.static_value(value))
                    .and_then(|value| value.as_object().cloned());
                if let Some(object) = object {
                    for (key, value) in object {
                        if let Some(target) = &component
                            && !self.signatures[target].iter().any(|s| {
                                s.kind == m::SlotKind::Prop
                                    && (s.name == key || s.name.starts_with("..."))
                            })
                        {
                            return self.placeholder(n, id, "Unsupported component prop");
                        }
                        el.props.insert(key, m::Expr::Literal { value });
                    }
                    continue;
                }
                if spread
                    .and_then(first)
                    .filter(|value| value.kind() == "identifier")
                    .and_then(|value| {
                        let wanted = format!("...{}", self.file.text(value));
                        self.slots
                            .iter()
                            .find(|slot| slot.name == wanted)
                            .map(|slot| {
                                (
                                    value,
                                    m::Expr::Slot {
                                        slot: slot.id.clone(),
                                        path: vec![],
                                    },
                                )
                            })
                    })
                    .is_some_and(|(value, expr)| {
                        el.props
                            .insert(format!("...{}", self.file.text(value)), expr);
                        true
                    })
                {
                    continue;
                }
                return self.placeholder(n, id, "Unsupported spread attributes");
            }
            let parts = children(attribute);
            let Some(k) = parts.first() else { continue };
            let key = self.file.text(*k);
            if component.is_none() && matches!(key, "className" | "style") {
                if key == "className"
                    && let Some(value) = parts.get(1)
                    && self.modules.apply(self.file, *value, &mut el)
                {
                    continue;
                }
                style::attribute(
                    self.file,
                    attribute,
                    &self.model.tokens,
                    self.tailwind,
                    &mut el,
                );
                continue;
            }
            if let Some(target) = &component
                && !self.signatures[target].iter().any(|s| {
                    s.kind == m::SlotKind::Prop && (s.name == key || s.name.starts_with("..."))
                })
            {
                return self.placeholder(n, id, "Unsupported component prop");
            }
            let expr = parts
                .get(1)
                .map(|v| self.expr(*v))
                .unwrap_or(m::Expr::Literal { value: true.into() });
            el.props.insert(key.into(), expr);
        }
        let class_text = attr(self.file, open, "className").map_or_else(
            || Some(String::new()),
            |value| match self.expr(value) {
                m::Expr::Literal { value } => value.as_str().map(str::to_owned),
                _ => None,
            },
        );
        if name.starts_with(char::is_lowercase) {
            if let Some(sheet) = self.built {
                sheet.apply_classes(self.file, open, &mut el, class_text.as_deref());
            } else {
                for sheet in self.sources {
                    sheet.apply_classes(self.file, open, &mut el, class_text.as_deref());
                }
            }
            if class_text.is_some()
                && let Some(attribute) = attr(self.file, open, "className")
                && !attribute.is_error()
            {
                let source = self.file.loc(attribute.parent().unwrap_or(attribute));
                el.unresolved_style.retain(|item| {
                    !(item.syntax == m::StyleSyntax::Class && item.location == source)
                });
            }
        }
        // Inline declarations have higher precedence regardless of JSX attribute order.
        el.style
            .sort_by_key(|decl| decl.from.syntax == m::StyleSyntax::Inline);
        el.children = self.child_nodes(n, &id);
        if let Some(component) = component {
            m::Node::Instance(m::Instance {
                id,
                component,
                props: el.props,
                children: el.children,
                location,
            })
        } else {
            m::Node::Element(el)
        }
    }
}
