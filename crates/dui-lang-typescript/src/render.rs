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
    pub signatures: &'a BTreeMap<String, Vec<m::Slot>>,
    pub modules: style::modules::Bindings<'a>,
    pub tailwind: bool,
    pub built: Option<&'a style::built::Sheet<'a>>,
    pub items: Vec<(String, String)>,
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
    fn expr(&self, node: Ast<'_>) -> m::Expr {
        let n = unwrap_expr(node);
        if let Some(value) = literal(self.file, n) {
            return m::Expr::Literal { value };
        }
        if let Some(read) = self.binding(n) {
            return read;
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
        }
        if name == "a" || router == Some("Link") {
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
                && !self.signatures[target]
                    .iter()
                    .any(|s| s.kind == m::SlotKind::Prop && s.name == key)
            {
                return self.placeholder(n, id, "Unsupported component prop");
            }
            let expr = parts
                .get(1)
                .map(|v| self.expr(*v))
                .unwrap_or(m::Expr::Literal { value: true.into() });
            el.props.insert(key.into(), expr);
        }
        if let Some(sheet) = self.built
            && name.starts_with(char::is_lowercase)
        {
            sheet.apply(self.file, open, &mut el);
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
