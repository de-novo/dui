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
    pub props_object: Option<&'a str>,
    pub class_render: bool,
    pub signatures: &'a BTreeMap<String, Vec<m::Slot>>,
    pub modules: style::modules::Bindings<'a>,
    pub tailwind: bool,
    pub built: Option<&'a style::built::Sheet<'a>>,
    pub sources: &'a [style::built::Sheet<'a>],
    pub items: Vec<(String, String)>,
    pub bindings: Vec<std::collections::BTreeSet<String>>,
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

pub(crate) fn object_key(file: &File, key: Ast<'_>) -> Option<String> {
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

fn object_entries<'a>(file: &File, object: Ast<'a>) -> Vec<(String, Ast<'a>)> {
    children(object)
        .into_iter()
        .filter_map(|node| {
            if node.kind() == "pair" {
                Some((
                    object_key(file, field(node, "key")?)?,
                    field(node, "value")?,
                ))
            } else if node.kind() == "shorthand_property_identifier" {
                Some((file.text(node).to_owned(), node))
            } else {
                None
            }
        })
        .collect()
}

fn strip_type_assertion(mut n: Ast<'_>) -> Ast<'_> {
    while matches!(n.kind(), "as_expression" | "satisfies_expression") {
        let Some(child) = first(n) else { break };
        n = child;
    }
    n
}

fn static_number(value: &serde_json::Value) -> Option<f64> {
    value.as_f64()
}

fn static_bool(value: &serde_json::Value) -> bool {
    truthy_json(value)
}

fn eval_static(
    files: &Files,
    file: &File,
    node: Ast<'_>,
    depth: usize,
) -> Option<serde_json::Value> {
    if depth >= 16 {
        return None;
    }
    let n = strip_type_assertion(unwrap_expr(node));
    if let Some(value) = literal(file, n) {
        return Some(value);
    }
    match n.kind() {
        "array" => {
            let mut out = Vec::new();
            for child in children(n) {
                if child.kind() == "spread_element" {
                    let value = eval_static(files, file, first(child)?, depth + 1)?;
                    out.extend(value.as_array()?.iter().cloned());
                } else {
                    out.push(eval_static(files, file, child, depth + 1)?);
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
                        let value = eval_static(files, file, field(child, "value")?, depth + 1)?;
                        out.insert(key, value);
                    }
                    "spread_element" => {
                        let value = eval_static(files, file, first(child)?, depth + 1)?;
                        for (key, value) in value.as_object()? {
                            out.insert(key.clone(), value.clone());
                        }
                    }
                    "shorthand_property_identifier" => {
                        let key = file.text(child).to_owned();
                        let value = eval_static(files, file, child, depth + 1)?;
                        out.insert(key, value);
                    }
                    _ => return None,
                }
            }
            Some(out.into())
        }
        "identifier" | "shorthand_property_identifier" => {
            let name = file.text(n);
            if let Some(declarator) = find_declarator(file, file.tree.root_node(), name)
                && let Some(value) = field(declarator, "value")
            {
                return eval_static(files, file, value, depth + 1);
            }
            let (target, export) = symbol(files, file, name)?;
            let declarator = find_declarator(target, target.tree.root_node(), export)?;
            eval_static(files, target, field(declarator, "value")?, depth + 1)
        }
        "member_expression" => {
            let object = eval_static(files, file, field(n, "object")?, depth + 1)?;
            let property = file.name(n, "property")?;
            object.get(property).cloned()
        }
        "subscript_expression" => {
            let object = eval_static(files, file, field(n, "object")?, depth + 1)?;
            let index = eval_static(files, file, field(n, "index")?, depth + 1)?;
            if let Some(key) = index.as_str() {
                object.get(key).cloned()
            } else if let Some(index) = index.as_u64() {
                object.get(index as usize).cloned()
            } else {
                None
            }
        }
        "unary_expression" => {
            let value = eval_static(files, file, field(n, "argument")?, depth + 1)?;
            match file.name(n, "operator")? {
                "!" => Some((!static_bool(&value)).into()),
                "-" => static_number(&value).map(|value| (-value).into()),
                "+" => static_number(&value).map(|value| value.into()),
                _ => None,
            }
        }
        "binary_expression" => {
            let left = eval_static(files, file, field(n, "left")?, depth + 1)?;
            let operator = file.name(n, "operator")?;
            if operator == "&&" {
                return if static_bool(&left) {
                    eval_static(files, file, field(n, "right")?, depth + 1)
                } else {
                    Some(left)
                };
            }
            if operator == "||" {
                return if static_bool(&left) {
                    Some(left)
                } else {
                    eval_static(files, file, field(n, "right")?, depth + 1)
                };
            }
            if operator == "??" {
                return if left.is_null() {
                    eval_static(files, file, field(n, "right")?, depth + 1)
                } else {
                    Some(left)
                };
            }
            let right = eval_static(files, file, field(n, "right")?, depth + 1)?;
            match operator {
                "+" => plus_json(left, right),
                "-" | "*" | "/" | "%" => {
                    let a = static_number(&left)?;
                    let b = static_number(&right)?;
                    let value = match operator {
                        "-" => a - b,
                        "*" => a * b,
                        "/" => a / b,
                        _ => a % b,
                    };
                    serde_json::Number::from_f64(value).map(serde_json::Value::Number)
                }
                "===" => Some((left == right).into()),
                "!==" => Some((left != right).into()),
                ">" | ">=" | "<" | "<=" => {
                    let a = static_number(&left)?;
                    let b = static_number(&right)?;
                    Some(
                        (match operator {
                            ">" => a > b,
                            ">=" => a >= b,
                            "<" => a < b,
                            _ => a <= b,
                        })
                        .into(),
                    )
                }
                _ => None,
            }
        }
        "ternary_expression" => {
            let condition = eval_static(files, file, field(n, "condition")?, depth + 1)?;
            eval_static(
                files,
                file,
                field(
                    n,
                    if static_bool(&condition) {
                        "consequence"
                    } else {
                        "alternative"
                    },
                )?,
                depth + 1,
            )
        }
        _ => None,
    }
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

type VariantConfig = (
    String,
    Vec<(String, Vec<(String, String)>)>,
    BTreeMap<String, String>,
);

fn cva_config(file: &File, local: &str) -> Option<VariantConfig> {
    if file.bindings.get(local).is_some() {
        return None;
    }
    let factory = find_declarator(file, file.tree.root_node(), local)
        .and_then(|node| field(node, "value"))
        .map(unwrap_expr)
        .filter(|value| value.kind() == "call_expression")?;
    let arguments = field(factory, "arguments")?;
    let base = first(arguments)
        .and_then(|v| literal(file, v))
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    let config = children(arguments)
        .get(1)
        .copied()
        .filter(|v| v.kind() == "object")?;
    let variants_node = object_pair(file, config, "variants").filter(|v| v.kind() == "object")?;
    let mut variants = Vec::new();
    for (group, choices_node) in pair_entries(file, variants_node) {
        if choices_node.kind() != "object" {
            continue;
        }
        let mut choices = Vec::new();
        for (choice, value) in pair_entries(file, choices_node) {
            if let Some(classes) = literal(file, value).and_then(|v| v.as_str().map(str::to_owned))
            {
                choices.push((choice, classes));
            }
        }
        variants.push((group, choices));
    }
    let mut defaults = BTreeMap::new();
    if let Some(node) =
        object_pair(file, config, "defaultVariants").filter(|v| v.kind() == "object")
    {
        for (group, value) in pair_entries(file, node) {
            if let Some(choice) = literal(file, value).and_then(|v| v.as_str().map(str::to_owned)) {
                defaults.insert(group, choice);
            }
        }
    }
    Some((base, variants, defaults))
}

fn join_classes(parts: impl IntoIterator<Item = String>) -> String {
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Add a variant equality to a stylesheet condition. Width, color-scheme and interaction
/// constraints survive; a second expression cannot be represented as a conjunction in schema v1.
fn variant_condition(
    base: Option<&m::Condition>,
    on: &m::Expr,
    choice: &str,
) -> Option<m::Condition> {
    let mut condition = base.cloned().unwrap_or(m::Condition {
        min_width: None,
        max_width: None,
        interaction: None,
        color_scheme: None,
        expr: None,
        equals: None,
    });
    if condition.expr.is_some() || condition.equals.is_some() {
        return None;
    }
    condition.expr = Some(on.clone());
    condition.equals = Some(choice.into());
    Some(condition)
}

/// cva's defaultVariants are runtime defaults of the props read by the call. Preserve
/// them as prop defaults so an omitted caller prop and an explicit default value draw alike.
pub(crate) fn apply_variant_defaults(file: &File, root: Ast<'_>, slots: &mut [m::Slot]) {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        pending.extend(children(node));
        if node.kind() != "call_expression" {
            continue;
        }
        let Some(function) = field(node, "function").filter(|f| f.kind() == "identifier") else {
            continue;
        };
        let local = file.text(function);
        let Some((_, _, defaults)) = cva_config(file, local) else {
            continue;
        };
        let Some(options) = field(node, "arguments")
            .and_then(first)
            .filter(|v| v.kind() == "object")
        else {
            continue;
        };
        for (group, value) in object_entries(file, options) {
            let Some(choice) = defaults.get(&group) else {
                continue;
            };
            let value = unwrap_expr(value);
            if !matches!(value.kind(), "identifier" | "shorthand_property_identifier") {
                continue;
            }
            let name = file.text(value);
            if let Some(slot) = slots
                .iter_mut()
                .find(|slot| slot.name == name && slot.initial.is_none())
            {
                slot.initial = Some(m::Expr::Literal {
                    value: choice.clone().into(),
                });
            }
        }
    }
}

struct VariantClasses {
    base: String,
    on: m::Expr,
    cases: Vec<(String, String)>,
}

fn push_direct_reads(expr: &m::Expr, reads: &mut Vec<m::Expr>) {
    match expr {
        m::Expr::Slot { .. } | m::Expr::Item { .. } | m::Expr::Binding { .. } => {
            if !reads.contains(expr) {
                reads.push(expr.clone());
            }
        }
        m::Expr::Index { object, key } => {
            push_direct_reads(object, reads);
            push_direct_reads(key, reads);
        }
        m::Expr::Object { properties } => {
            for value in properties.values() {
                push_direct_reads(value, reads);
            }
        }
        m::Expr::Unary { value, .. } => push_direct_reads(value, reads),
        m::Expr::Binary { left, right, .. } => {
            push_direct_reads(left, reads);
            push_direct_reads(right, reads);
        }
        m::Expr::Literal { .. } | m::Expr::Opaque { .. } => {}
    }
}

impl Context<'_> {
    fn component(&self, name: &str) -> Option<String> {
        let (file, export) = symbol(self.files, self.file, name)?;
        let id = format!("{}#{export}", file.path);
        self.signatures.contains_key(&id).then_some(id)
    }
    fn variant_classes(&self, node: Ast<'_>) -> Option<VariantClasses> {
        let mut pending = vec![unwrap_expr(node)];
        let mut invocation = None;
        while let Some(current) = pending.pop() {
            if current.kind() == "call_expression"
                && field(current, "function").is_some_and(|f| {
                    f.kind() == "identifier" && cva_config(self.file, self.file.text(f)).is_some()
                })
            {
                invocation = Some(current);
                break;
            }
            pending.extend(children(current));
        }
        let invocation = invocation?;
        let local = field(invocation, "function").map(|f| self.file.text(f))?;
        let (base, variants, defaults) = cva_config(self.file, local)?;
        let options = field(invocation, "arguments")
            .and_then(first)
            .filter(|v| v.kind() == "object");
        let mut selected: BTreeMap<String, String> = BTreeMap::new();
        let mut dynamic: Vec<(String, m::Expr)> = Vec::new();
        for (group, _) in &variants {
            let supplied = options.and_then(|object| {
                object_entries(self.file, object)
                    .into_iter()
                    .find(|(key, _)| key == group)
                    .map(|(_, value)| value)
            });
            let supplied_expr = supplied.and_then(|value| {
                let value = unwrap_expr(value);
                self.binding(value).and_then(|expr| match &expr {
                    m::Expr::Slot { path, .. } if path.is_empty() => Some(expr),
                    _ => None,
                })
            });
            if let Some(expr) = supplied_expr {
                dynamic.push((group.clone(), expr));
                continue;
            }
            let choice = supplied
                .and_then(|value| literal(self.file, unwrap_expr(value)))
                .and_then(|value| value.as_str().map(str::to_owned))
                .or_else(|| defaults.get(group).cloned());
            if let Some(choice) = choice {
                selected.insert(group.clone(), choice.clone());
            }
        }
        let (dynamic_group, on) = dynamic.first()?;
        for (group, _) in dynamic.iter().skip(1) {
            if let Some(choice) = defaults.get(group) {
                selected.insert(group.clone(), choice.clone());
            }
        }
        let dynamic_group = dynamic_group.clone();
        let choices = variants
            .iter()
            .find(|(group, _)| group == &dynamic_group)?
            .1
            .clone();
        let mut cases = Vec::new();
        for (choice, classes) in choices {
            let mut parts = vec![base.clone()];
            for (group, group_choices) in &variants {
                if group == &dynamic_group {
                    parts.push(classes.clone());
                } else if let Some(wanted) = selected.get(group) {
                    parts.push(
                        group_choices
                            .iter()
                            .find(|(choice, _)| choice == wanted)
                            .map(|(_, classes)| classes.clone())
                            .unwrap_or_default(),
                    );
                }
            }
            cases.push((choice, join_classes(parts)));
        }
        Some(VariantClasses {
            base,
            on: on.clone(),
            cases,
        })
    }

    fn append_static_key(expr: m::Expr, key: String) -> m::Expr {
        match expr {
            m::Expr::Slot { slot, mut path } => {
                path.push(key);
                m::Expr::Slot { slot, path }
            }
            m::Expr::Item { list, mut path } => {
                path.push(key);
                m::Expr::Item { list, path }
            }
            m::Expr::Binding { name, mut path } => {
                path.push(key);
                m::Expr::Binding { name, path }
            }
            object => m::Expr::Index {
                object: Box::new(object),
                key: Box::new(m::Expr::Literal { value: key.into() }),
            },
        }
    }

    fn binding(&self, n: Ast<'_>) -> Option<m::Expr> {
        self.binding_at(n, 0)
    }

    /// Lower a bounded member/subscript chain. Static properties remain field paths on direct
    /// reads; computed keys become explicit indexed expressions. Unsupported or deeply nested
    /// chains return `None` so the caller records opaque source instead of guessing a value.
    fn binding_at(&self, mut n: Ast<'_>, depth: usize) -> Option<m::Expr> {
        const EXPRESSION_DEPTH_LIMIT: usize = 24;
        if depth >= EXPRESSION_DEPTH_LIMIT {
            return None;
        }
        enum Segment<'a> {
            Static(String),
            Key(Ast<'a>),
        }
        let mut segments = Vec::new();
        while matches!(n.kind(), "member_expression" | "subscript_expression") {
            if segments.len() >= EXPRESSION_DEPTH_LIMIT {
                return None;
            }
            if n.kind() == "member_expression" {
                segments.push(Segment::Static(self.file.name(n, "property")?.into()));
            } else {
                let index = field(n, "index")?;
                let static_key = literal(self.file, index).and_then(|value| match value {
                    serde_json::Value::String(value) => Some(value),
                    serde_json::Value::Number(value) => Some(value.to_string()),
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
        segments.reverse();
        let mut expr = if self.class_render && n.kind() == "this" {
            if !matches!(segments.first(), Some(Segment::Static(props)) if props == "props") {
                return None;
            }
            let Some(Segment::Static(name)) = segments.get(1) else {
                return None;
            };
            let slot = self.slots.iter().find(|s| &s.name == name)?;
            segments.drain(..2);
            m::Expr::Slot {
                slot: slot.id.clone(),
                path: vec![],
            }
        } else if matches!(n.kind(), "identifier" | "shorthand_property_identifier") {
            let name = self.file.text(n);
            if Some(name) == self.props_object {
                let Some(Segment::Static(first)) = segments.first() else {
                    return None;
                };
                let slot = self.slots.iter().find(|s| &s.name == first)?;
                segments.drain(..1);
                m::Expr::Slot {
                    slot: slot.id.clone(),
                    path: vec![],
                }
            } else if self.bindings.iter().rev().any(|scope| scope.contains(name)) {
                m::Expr::Binding {
                    name: name.into(),
                    path: vec![],
                }
            } else if let Some((_, list)) = self.items.iter().rev().find(|(item, _)| item == name) {
                m::Expr::Item {
                    list: list.clone(),
                    path: vec![],
                }
            } else if self.class_render {
                return None;
            } else {
                let slot = self.slots.iter().find(|s| s.name == name)?;
                m::Expr::Slot {
                    slot: slot.id.clone(),
                    path: vec![],
                }
            }
        } else {
            return None;
        };
        for segment in segments {
            match segment {
                Segment::Static(key) => expr = Self::append_static_key(expr, key),
                Segment::Key(key_node) => {
                    let key = self.structured_operand_at(key_node, depth + 1)?;
                    expr = m::Expr::Index {
                        object: Box::new(expr),
                        key: Box::new(key),
                    };
                }
            }
        }
        Some(expr)
    }
    /// Evaluate only syntax-proven immutable bindings and closed boolean/string/class-helper
    /// expressions. Anything outside this bounded subset remains opaque rather than executed.
    fn static_value(&self, node: Ast<'_>) -> Option<serde_json::Value> {
        let n = unwrap_expr(node);
        if let Some(value) = eval_static(self.files, self.file, n, 0) {
            return Some(value);
        }
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
    fn structured_operand_at(&self, node: Ast<'_>, depth: usize) -> Option<m::Expr> {
        if depth >= 24 {
            return None;
        }
        let n = unwrap_expr(node);
        if let Some(value) = literal(self.file, n) {
            return Some(m::Expr::Literal { value });
        }
        if let Some(read) = self.binding_at(n, depth) {
            return Some(read);
        }
        if let Some(value) = self.static_value(n) {
            return Some(m::Expr::Literal { value });
        }
        self.structured_expr_at(n, depth)
    }
    fn structured_expr(&self, node: Ast<'_>) -> Option<m::Expr> {
        self.structured_expr_at(node, 0)
    }
    fn structured_expr_at(&self, node: Ast<'_>, depth: usize) -> Option<m::Expr> {
        if depth >= 24 {
            return None;
        }
        let n = unwrap_expr(node);
        if n.kind() == "object" {
            let mut properties = BTreeMap::new();
            for child in children(n) {
                match child.kind() {
                    "pair" => {
                        let key = field(child, "key").and_then(|key| object_key(self.file, key))?;
                        let value =
                            self.structured_operand_at(field(child, "value")?, depth + 1)?;
                        properties.insert(key, value);
                    }
                    "shorthand_property_identifier" => {
                        let key = self.file.text(child).to_owned();
                        let value = self.binding_at(child, depth + 1)?;
                        properties.insert(key, value);
                    }
                    _ => return None,
                }
            }
            return Some(m::Expr::Object { properties });
        }
        if n.kind() == "unary_expression" && self.file.name(n, "operator") == Some("!") {
            return Some(m::Expr::Unary {
                operator: "!".into(),
                value: Box::new(self.structured_operand_at(field(n, "argument")?, depth + 1)?),
            });
        }
        if n.kind() == "binary_expression" {
            let operator = self.file.name(n, "operator")?;
            if matches!(
                operator,
                "&&" | "||" | "??" | "===" | "!==" | ">" | ">=" | "<" | "<="
            ) {
                return Some(m::Expr::Binary {
                    operator: operator.into(),
                    left: Box::new(self.structured_operand_at(field(n, "left")?, depth + 1)?),
                    right: Box::new(self.structured_operand_at(field(n, "right")?, depth + 1)?),
                });
            }
        }
        None
    }
    pub(crate) fn expr(&self, node: Ast<'_>) -> m::Expr {
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
        if let Some(structured) = self.structured_expr(n) {
            return structured;
        }
        let mut reads = vec![];
        let mut pending = children(n);
        pending.reverse();
        while let Some(c) = pending.pop() {
            if let Some(read) = self.binding(c) {
                push_direct_reads(&read, &mut reads);
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
        if let m::Expr::Slot { slot, path } = self.expr(n)
            && path.is_empty()
            && let Some(slot) = self
                .slots
                .iter()
                .find(|candidate| candidate.id == slot && candidate.ty == m::TypeRef::Node)
        {
            return m::Node::Outlet(m::Outlet {
                id,
                slot: slot.id.clone(),
                location: loc,
            });
        }
        if n.kind() == "call_expression" && self.is_react_create_element(n) {
            return self.create_element(n, id);
        }
        if n.kind() == "call_expression"
            && field(n, "function").is_some_and(|function| {
                function.kind() == "identifier"
                    && self
                        .slots
                        .iter()
                        .any(|slot| slot.name == self.file.text(function))
            })
        {
            let function = field(n, "function").unwrap();
            let slot = self
                .slots
                .iter()
                .find(|slot| slot.name == self.file.text(function))
                .unwrap();
            let arguments = field(n, "arguments")
                .map(children)
                .unwrap_or_default()
                .into_iter()
                .map(|argument| self.expr(argument))
                .collect();
            return m::Node::Invoke(m::Invoke {
                id,
                callee: m::Expr::Slot {
                    slot: slot.id.clone(),
                    path: vec![],
                },
                arguments,
                location: loc,
            });
        }
        if matches!(n.kind(), "arrow_function" | "function_expression") {
            return self.render_function(n, id);
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
    fn parameter_nodes<'a>(&self, f: Ast<'a>) -> Vec<Ast<'a>> {
        if let Some(parameter) = field(f, "parameter") {
            return vec![parameter];
        }
        field(f, "parameters")
            .map(children)
            .unwrap_or_default()
            .into_iter()
            .filter(|parameter| {
                matches!(
                    parameter.kind(),
                    "required_parameter" | "optional_parameter" | "identifier" | "object_pattern"
                )
            })
            .collect()
    }
    fn pattern_node<'a>(&self, parameter: Ast<'a>) -> Ast<'a> {
        field(parameter, "pattern")
            .or_else(|| field(parameter, "name"))
            .unwrap_or(parameter)
    }
    fn binding_pattern(&self, parameter: Ast<'_>) -> Option<m::BindingPattern> {
        let pattern = self.pattern_node(parameter);
        if pattern.kind() == "identifier" {
            return Some(m::BindingPattern::Identifier {
                name: self.file.text(pattern).into(),
            });
        }
        if pattern.kind() != "object_pattern" {
            return None;
        }
        let mut properties = BTreeMap::new();
        for binding in children(pattern) {
            match binding.kind() {
                "shorthand_property_identifier_pattern" => {
                    let name = self.file.text(binding);
                    properties.insert(name.into(), name.into());
                }
                "pair_pattern" | "object_assignment_pattern" => {
                    let key = field(binding, "key")
                        .and_then(|key| object_key(self.file, key))
                        .or_else(|| first(binding).map(|key| self.file.text(key).to_owned()))?;
                    let value = field(binding, "value").or_else(|| field(binding, "right"))?;
                    let value = if value.kind() == "assignment_pattern" {
                        field(value, "left")?
                    } else {
                        value
                    };
                    if !matches!(
                        value.kind(),
                        "identifier" | "shorthand_property_identifier_pattern"
                    ) {
                        return None;
                    }
                    properties.insert(key, self.file.text(value).into());
                }
                _ => return None,
            }
        }
        (!properties.is_empty()).then_some(m::BindingPattern::Object { properties })
    }
    fn render_function(&mut self, n: Ast<'_>, id: String) -> m::Node {
        let parameters = self.parameter_nodes(n);
        let mut patterns = Vec::new();
        for parameter in parameters {
            let Some(pattern) = self.binding_pattern(parameter) else {
                return self.placeholder(n, id, "Unsupported render function parameters");
            };
            patterns.push(pattern);
        }
        let names = patterns
            .iter()
            .flat_map(|pattern| match pattern {
                m::BindingPattern::Identifier { name } => vec![name.clone()],
                m::BindingPattern::Object { properties } => properties.values().cloned().collect(),
            })
            .collect::<std::collections::BTreeSet<_>>();
        let Some(body) = crate::returned(n) else {
            return self.placeholder(n, id, "Unsupported render function body");
        };
        self.bindings.push(names);
        let child = self.node(body, format!("{id}.0"));
        self.bindings.pop();
        m::Node::RenderFunction(m::RenderFunction {
            id,
            parameters: patterns,
            children: vec![child],
            location: self.file.loc(n),
        })
    }
    fn is_react_create_element(&self, n: Ast<'_>) -> bool {
        field(n, "function").is_some_and(|function| {
            function.kind() == "member_expression"
                && self.file.name(function, "property") == Some("createElement")
                && field(function, "object").is_some_and(|object| self.file.text(object) == "React")
        })
    }
    fn unique_declarator<'a>(file: &'a File, name: &str) -> Option<Ast<'a>> {
        let mut pending = vec![file.tree.root_node()];
        let mut found = None;
        while let Some(node) = pending.pop() {
            if node.kind() == "variable_declarator"
                && field(node, "name").is_some_and(|n| file.text(n) == name)
            {
                if found.is_some() {
                    return None;
                }
                found = Some(node);
            }
            pending.extend(children(node));
        }
        found
    }
    fn expression_child(&mut self, node: Ast<'_>, id: String, depth: usize) -> m::Node {
        let n = unwrap_expr(node);
        if depth < 8 && n.kind() == "identifier" {
            let name = self.file.text(n);
            if let Some(initial) = Self::unique_declarator(self.file, name)
                .and_then(|declarator| field(declarator, "value"))
                .filter(|value| value.id() != n.id())
            {
                return self.expression_child(initial, id, depth + 1);
            }
        }
        self.node(n, id)
    }
    fn create_element(&mut self, n: Ast<'_>, id: String) -> m::Node {
        let arguments = field(n, "arguments").map(children).unwrap_or_default();
        let Some(tag) = arguments.first() else {
            return self.placeholder(n, id, "Incomplete createElement");
        };
        let mut props = BTreeMap::new();
        if let Some(object) = arguments.get(1).map(|v| unwrap_expr(*v)) {
            if object.kind() == "object" {
                for (key, value) in object_entries(self.file, object) {
                    props.insert(key, self.expr(value));
                }
            } else if !matches!(object.kind(), "null") {
                return self.placeholder(n, id, "Unsupported createElement props");
            }
        }
        let children = arguments
            .iter()
            .skip(2)
            .enumerate()
            .map(|(index, child)| self.expression_child(*child, format!("{id}.{index}"), 0))
            .collect();
        m::Node::CreateElement(m::CreateElement {
            id,
            tag: self.expr(*tag),
            props,
            children,
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
        if name.ends_with(".Provider") {
            let mut children = self.child_nodes(n, &id);
            if children.is_empty()
                && let Some(slot) = self
                    .slots
                    .iter()
                    .find(|slot| slot.kind == m::SlotKind::Prop && slot.name == "children")
            {
                children.push(m::Node::Outlet(m::Outlet {
                    id: format!("{id}.children"),
                    slot: slot.id.clone(),
                    location: location.clone(),
                }));
            }
            return m::Node::Fragment(m::Fragment {
                children,
                id,
                location,
            });
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
                    .is_some_and(|value| {
                        if self.file.text(value) == "props" {
                            for slot in self
                                .slots
                                .iter()
                                .filter(|slot| slot.kind == m::SlotKind::Prop)
                            {
                                let accepted = component.as_ref().is_none_or(|target| {
                                    self.signatures[target].iter().any(|target_slot| {
                                        target_slot.kind == m::SlotKind::Prop
                                            && (*target_slot.name == *slot.name
                                                || target_slot.name.starts_with("..."))
                                    })
                                });
                                if accepted {
                                    el.props.insert(
                                        slot.name.clone().into(),
                                        m::Expr::Slot {
                                            slot: slot.id.clone(),
                                            path: vec![],
                                        },
                                    );
                                }
                            }
                            return true;
                        }
                        let wanted = format!("...{}", self.file.text(value));
                        self.slots
                            .iter()
                            .find(|slot| slot.name == wanted)
                            .is_some_and(|slot| {
                                el.props.insert(
                                    wanted.clone(),
                                    m::Expr::Slot {
                                        slot: slot.id.clone(),
                                        path: vec![],
                                    },
                                );
                                true
                            })
                    })
                {
                    continue;
                }
                return self.placeholder(n, id, "Unsupported spread attributes");
            }
            let parts = children(attribute);
            let Some(k) = parts.first() else { continue };
            let key = self.file.text(*k);
            // React keys identify list instances; they are metadata rather than component inputs.
            if key == "key" {
                continue;
            }
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
        let variant_classes =
            attr(self.file, open, "className").and_then(|v| self.variant_classes(v));
        let class_text = if let Some(variant) = &variant_classes {
            Some(variant.base.clone())
        } else {
            attr(self.file, open, "className").map_or_else(
                || Some(String::new()),
                |value| match self.expr(value) {
                    m::Expr::Literal { value } => value.as_str().map(str::to_owned),
                    _ => None,
                },
            )
        };
        if name.starts_with(char::is_lowercase) {
            let sheets: Vec<&style::built::Sheet<'_>> = if let Some(sheet) = self.built {
                vec![sheet]
            } else {
                self.sources.iter().collect()
            };
            for sheet in &sheets {
                sheet.apply_classes(self.file, open, &mut el, class_text.as_deref());
            }
            if let Some(variant) = variant_classes {
                for (choice, classes) in variant.cases {
                    let mut conditional = el.clone();
                    conditional
                        .unresolved_style
                        .retain(|item| item.syntax == m::StyleSyntax::Inline);
                    for sheet in &sheets {
                        sheet.apply_classes(self.file, open, &mut conditional, Some(&classes));
                    }
                    for mut declaration in conditional.style {
                        if el.style.iter().any(|existing| *existing == declaration) {
                            continue;
                        }
                        let Some(when) =
                            variant_condition(declaration.when.as_ref(), &variant.on, &choice)
                        else {
                            continue;
                        };
                        declaration.when = Some(when);
                        el.style.push(declaration);
                    }
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
