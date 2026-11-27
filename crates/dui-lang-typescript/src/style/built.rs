//! Resolve supported style declarations from an explicitly supplied, already bundled author stylesheet.
use super::{P, V, modules, shorthand, spacing, uniform_gap};
use crate::syntax::{File, children, first};
use dui_core::source_model as m;
use std::collections::BTreeMap;
use tree_sitter::Node as Ast;

// Each supported property uses the same value parser as a source stylesheet.
const DIRECT: &[(&str, P)] = &[
    ("gap", P::Gap),
    ("color", P::Color),
    ("background-color", P::Background),
    ("border-color", P::BorderColor),
    ("border-width", P::BorderWidth),
    ("border-radius", P::BorderRadius),
    ("opacity", P::Opacity),
    ("font-size", P::FontSize),
    ("font-weight", P::FontWeight),
    ("line-height", P::LineHeight),
    ("letter-spacing", P::LetterSpacing),
    ("width", P::Width),
    ("height", P::Height),
    ("min-width", P::MinWidth),
    ("max-width", P::MaxWidth),
    ("min-height", P::MinHeight),
    ("max-height", P::MaxHeight),
    ("flex-direction", P::Direction),
    ("align-items", P::AlignItems),
    ("justify-content", P::JustifyContent),
    ("flex-wrap", P::Wrap),
    ("flex-grow", P::Grow),
    ("padding-top", P::PaddingTop),
    ("padding-bottom", P::PaddingBottom),
    ("padding-inline-start", P::PaddingStart),
    ("padding-inline-end", P::PaddingEnd),
    ("margin-top", P::MarginTop),
    ("margin-bottom", P::MarginBottom),
    ("margin-inline-start", P::MarginStart),
    ("margin-inline-end", P::MarginEnd),
];

fn affected(name: &str) -> Vec<P> {
    if let Some(props) = spacing(name) {
        return props.to_vec();
    }
    if let Some((_, prop)) = DIRECT.iter().find(|(css, _)| *css == name) {
        return vec![*prop];
    }
    match name {
        "background" => vec![P::Background],
        "grid-gap" | "row-gap" | "column-gap" | "grid-row-gap" | "grid-column-gap" => vec![P::Gap],
        "border" | "border-top" | "border-bottom" | "border-left" | "border-right"
        | "border-inline" | "border-block" => vec![P::BorderWidth, P::BorderColor],
        "font" => vec![P::FontSize, P::LineHeight],
        "flex" => vec![P::Grow],
        "flex-flow" => vec![P::Direction, P::Wrap],
        "place-items" => vec![P::AlignItems],
        "place-content" => vec![P::JustifyContent],
        "padding-block" | "padding-block-start" | "padding-block-end" => {
            vec![P::PaddingTop, P::PaddingBottom]
        }
        "padding-left" | "padding-right" => vec![P::PaddingStart, P::PaddingEnd],
        "margin-block" | "margin-block-start" | "margin-block-end" => {
            vec![P::MarginTop, P::MarginBottom]
        }
        "margin-left" | "margin-right" => vec![P::MarginStart, P::MarginEnd],
        "inline-size" | "block-size" => vec![P::Width, P::Height],
        "min-inline-size" | "min-block-size" => vec![P::MinWidth, P::MinHeight],
        "max-inline-size" | "max-block-size" => vec![P::MaxWidth, P::MaxHeight],
        "all" => DIRECT.iter().map(|(_, p)| *p).collect(),
        n if n.starts_with("border-") && n.ends_with("-width") => vec![P::BorderWidth],
        n if n.starts_with("border-") && n.ends_with("-color") => vec![P::BorderColor],
        n if n.starts_with("border-") && n.ends_with("-radius") => vec![P::BorderRadius],
        _ => vec![],
    }
}

struct Rule<'a> {
    selectors: &'a str,
    layer: usize,
    uncertain: bool,
    name: &'a str,
    important: bool,
    value: &'a str,
    source: m::StyleSource,
}

pub(crate) struct Sheet<'a> {
    rules: BTreeMap<P, Vec<Rule<'a>>>,
    selectors: BTreeMap<&'a str, Vec<Selector<'a>>>,
    // Only a unique, unconditional document-root definition can be inherited here.
    vars: BTreeMap<&'a str, Option<&'a str>>,
}

struct Selector<'a> {
    text: &'a str,
    // A necessary type or class on the subject. Ancestors never qualify here.
    subject: Subject<'a>,
}

// Conditions necessary for a selector to apply to an intrinsic element.
// Keeping alternatives avoids counting :is() branches as simultaneous requirements.
enum Subject<'a> {
    Unknown,
    PseudoElement,
    Tag(&'a str),
    Class(String),
    Attribute(&'a str),
    Id(&'a str),
    All(Vec<Self>),
    Any(Vec<Self>),
}

impl Subject<'_> {
    fn excludes(&self, classes: Option<&str>, el: &m::Element, spread: bool) -> bool {
        match self {
            Self::Unknown => false,
            Self::PseudoElement => true,
            Self::Tag(tag) => !tag.eq_ignore_ascii_case(&el.tag),
            Self::Class(class) => {
                !spread && classes.is_some_and(|s| !s.split_ascii_whitespace().any(|v| v == class))
            }
            Self::Attribute(key) => !spread && !el.props.contains_key(*key),
            Self::Id(id) => {
                !spread
                    && match el.props.get("id") {
                        None => true,
                        Some(m::Expr::Literal { value }) => {
                            value.as_str().is_some_and(|v| v != *id)
                        }
                        _ => false,
                    }
            }
            Self::All(parts) => parts.iter().any(|p| p.excludes(classes, el, spread)),
            Self::Any(parts) => {
                !parts.is_empty() && parts.iter().all(|p| p.excludes(classes, el, spread))
            }
        }
    }
}

fn subject<'a>(file: &'a File, node: Ast<'_>) -> Subject<'a> {
    let parts = children(node);
    match node.kind() {
        "descendant_selector"
        | "child_selector"
        | "adjacent_sibling_selector"
        | "sibling_selector" => parts.last().map_or(Subject::Unknown, |n| subject(file, *n)),
        "tag_name" if ident(file.text(node)) => Subject::Tag(file.text(node)),
        "class_selector" | "id_selector" | "attribute_selector" | "pseudo_class_selector" => {
            let mut conditions = Vec::new();
            for n in parts {
                let raw = file.text(n);
                let condition = match (node.kind(), n.kind()) {
                    ("class_selector", "class_name") => {
                        let mut class = String::new();
                        let mut chars = raw.chars();
                        while let Some(c) = chars.next() {
                            if c == '\\' {
                                let Some(escaped) =
                                    chars.next().filter(|c| c.is_ascii_punctuation())
                                else {
                                    return Subject::Unknown;
                                };
                                class.push(escaped);
                            } else {
                                class.push(c);
                            }
                        }
                        Subject::Class(class)
                    }
                    ("attribute_selector", "attribute_name")
                        if raw.starts_with("data-") && ident(raw) =>
                    {
                        Subject::Attribute(raw)
                    }
                    ("id_selector", "id_name") if ident(raw) => Subject::Id(raw),
                    ("pseudo_class_selector", "arguments")
                        if n.prev_named_sibling()
                            .is_some_and(|name| matches!(file.text(name), "is" | "where")) =>
                    {
                        Subject::Any(children(n).into_iter().map(|p| subject(file, p)).collect())
                    }
                    (_, kind) if kind.ends_with("_selector") || kind == "tag_name" => {
                        subject(file, n)
                    }
                    _ => Subject::Unknown,
                };
                conditions.push(condition);
            }
            Subject::All(conditions)
        }
        "pseudo_element_selector"
            if parts.iter().any(|n| {
                n.kind() == "tag_name"
                    && (file.text(*n).starts_with("-webkit-")
                        || matches!(
                            file.text(*n),
                            "before"
                                | "after"
                                | "file-selector-button"
                                | "placeholder"
                                | "selection"
                                | "backdrop"
                                | "marker"
                        ))
            }) =>
        {
            Subject::PseudoElement
        }
        _ => Subject::Unknown,
    }
}
fn ident(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}

// The CSS grammar may mark a media range or an empty var() fallback as an error.
// Both remain inside an intact rule; conditions stay unknown and values are parsed from source.
fn structural_error(f: &File, root: Ast<'_>) -> bool {
    let mut pending = vec![root];
    while let Some(n) = pending.pop() {
        if !n.has_error() {
            continue;
        }
        if n.is_error() || n.is_missing() {
            let parent = n.parent().map(|p| p.kind());
            if n.is_error()
                && (parent == Some("parenthesized_query") && !f.text(n).contains(['{', '}', ';'])
                    || parent == Some("arguments") && f.text(n) == ",")
            {
                continue;
            }
            return true;
        }
        pending.extend(n.children(&mut n.walk()));
    }
    false
}

// Resolve a single pixel width constraint. Other media conditions keep their uncertainty.
fn width_query(raw: &str, width: u32) -> Option<bool> {
    // Minifiers also emit strict ranges as the negation of an inclusive range.
    let raw = raw.trim();
    let (query, negate) = raw
        .strip_prefix("not all and ")
        .map_or((raw, false), |q| (q.trim(), true));
    let feature = query.strip_prefix('(')?.strip_suffix(')')?.trim();
    let (op, length) = if let Some((name, length)) = feature.split_once(':') {
        let op = match name.trim() {
            "min-width" => ">=",
            "max-width" => "<=",
            "width" => "=",
            _ => return None,
        };
        (op, length)
    } else {
        let range = feature.strip_prefix("width")?.trim_start();
        [">=", "<=", ">", "<", "="]
            .into_iter()
            .find_map(|op| Some((op, range.strip_prefix(op)?)))?
    };
    let length = length.trim();
    let threshold = length
        .strip_suffix("px")
        .or_else(|| (length == "0").then_some(length))?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.)?;
    let width = f64::from(width);
    let matched = match op {
        ">=" => width >= threshold,
        "<=" => width <= threshold,
        ">" => width > threshold,
        "<" => width < threshold,
        _ => width == threshold,
    };
    Some(matched != negate)
}

impl<'a> Sheet<'a> {
    pub(crate) fn new(f: &'a File, width: Option<u32>) -> Result<Self, String> {
        let root = f.tree.root_node();
        if structural_error(f, root) {
            return Err(format!("Cannot parse built CSS: {}", f.path));
        }
        let mut sheet = Self {
            rules: BTreeMap::new(),
            vars: BTreeMap::new(),
            selectors: BTreeMap::from([(
                "",
                vec![Selector {
                    text: "",
                    subject: Subject::Unknown,
                }],
            )]),
        };
        let mut layers = Vec::new();
        let mut stack = vec![(root, "", usize::MAX, false)];
        while let Some((node, selector, layer, uncertain)) = stack.pop() {
            let parts = children(node);
            match node.kind() {
                "import_statement" => {
                    return Err("Built CSS must include its imports in one bundle".into());
                }
                "media_statement" => {
                    if let [query, body] = parts.as_slice() {
                        let matches = width.and_then(|w| width_query(f.text(*query), w));
                        if matches != Some(false) {
                            stack.push((*body, selector, layer, uncertain || matches.is_none()));
                        }
                    } else {
                        stack.extend(parts.into_iter().rev().map(|p| (p, selector, layer, true)));
                    }
                }
                "declaration" => {
                    let Some(prop) = parts.first().map(|p| f.text(*p)) else {
                        continue;
                    };
                    let Some((_, raw)) = f.text(node).split_once(':') else {
                        continue;
                    };
                    let value = raw.trim_end_matches(';').trim();
                    let important = parts.iter().any(|p| p.kind() == "important");
                    if prop.starts_with("--") {
                        let root = selector.split(',').map(str::trim).any(|s| s == ":root")
                            && selector
                                .split(',')
                                .map(str::trim)
                                .all(|s| matches!(s, ":root" | ":host"));
                        let definition = (root && !uncertain && !important).then_some(value);
                        sheet
                            .vars
                            .entry(prop)
                            .and_modify(|v| *v = None)
                            .or_insert(definition);
                    } else {
                        for property in affected(prop) {
                            sheet.rules.entry(property).or_default().push(Rule {
                                selectors: selector,
                                layer,
                                uncertain,
                                important,
                                name: prop,
                                value,
                                source: m::StyleSource {
                                    syntax: m::StyleSyntax::Stylesheet,
                                    text: f.text(node).into(),
                                    location: f.loc(node),
                                },
                            });
                        }
                    }
                }
                "rule_set" => {
                    if let [selectors, body] = parts.as_slice() {
                        sheet
                            .selectors
                            .entry(f.text(*selectors))
                            .or_insert_with(|| {
                                children(*selectors)
                                    .into_iter()
                                    .map(|node| Selector {
                                        text: f.text(node),
                                        subject: subject(f, node),
                                    })
                                    .collect()
                            });
                        stack.push((
                            *body,
                            f.text(*selectors),
                            layer,
                            uncertain || !selector.is_empty(),
                        ));
                    }
                }
                "at_rule" if parts.first().is_some_and(|p| f.text(*p) == "@layer") => {
                    let body = parts.iter().find(|p| p.kind() == "block");
                    let end = body.map_or(node.end_byte(), |p| p.start_byte());
                    let names = f.src[parts[0].end_byte()..end].trim_end_matches(';').trim();
                    let mut rank = 0;
                    for name in names.split(',').map(str::trim) {
                        if !ident(name) || layer != usize::MAX {
                            return Err(
                                "Nested and anonymous cascade layers in built CSS are unsupported"
                                    .into(),
                            );
                        }
                        rank = layers
                            .iter()
                            .position(|entry| *entry == name)
                            .unwrap_or(layers.len());
                        if rank == layers.len() {
                            layers.push(name);
                        }
                    }
                    if let Some(body) = body {
                        stack.push((*body, selector, rank, uncertain));
                    }
                }
                _ => {
                    let unknown = uncertain || !matches!(node.kind(), "stylesheet" | "block");
                    stack.extend(
                        parts
                            .into_iter()
                            .rev()
                            .map(|p| (p, selector, layer, unknown)),
                    );
                }
            }
        }
        Ok(sheet)
    }

    fn value(&self, property: P, rule: &Rule<'_>) -> Option<V> {
        let raw = rule.value;
        let raw = if let Some(name) = raw.strip_prefix("var(").and_then(|s| s.strip_suffix(')')) {
            self.vars.get(name.trim()).copied().flatten()?
        } else {
            raw
        };
        if rule.name == "flex-flow" {
            let mut direction = None;
            let mut wrap = None;
            for word in raw.split_ascii_whitespace() {
                let slot = match word {
                    "row" | "column" | "row-reverse" | "column-reverse" => &mut direction,
                    "wrap" | "nowrap" | "wrap-reverse" => &mut wrap,
                    _ => return None,
                };
                if slot.replace(word).is_some() {
                    return None;
                }
            }
            let (name, value) = if property == P::Direction {
                ("flex-direction", direction.unwrap_or("row"))
            } else {
                ("flex-wrap", wrap.unwrap_or("nowrap"))
            };
            return modules::declaration(name, value).map(|(_, v)| v);
        }
        if rule.name == "border" {
            let mut width = None;
            let mut color = None;
            let mut style = None;
            for word in raw.split_ascii_whitespace() {
                if let Some(c) = super::color(word) {
                    if color.replace(c).is_some() {
                        return None;
                    }
                } else if matches!(
                    word,
                    "solid"
                        | "dashed"
                        | "dotted"
                        | "double"
                        | "groove"
                        | "ridge"
                        | "inset"
                        | "outset"
                        | "none"
                        | "hidden"
                ) {
                    if style.replace(word).is_some() {
                        return None;
                    }
                } else {
                    let v = super::size_value(P::BorderWidth, word)?;
                    if width.replace(v).is_some() {
                        return None;
                    }
                }
            }
            return if property == P::BorderColor {
                color
            } else {
                width
            };
        }
        let name = match rule.name {
            "grid-gap" => "gap",
            "background" => "background-color",
            name => name,
        };
        if let Some(sides) = shorthand(name, raw) {
            return sides
                .into_iter()
                .find(|(p, _)| *p == property)
                .map(|(_, v)| v);
        }
        if name != "gap" || !raw.starts_with("calc(") {
            let (props, value) = modules::declaration(name, raw)?;
            return props.contains(&property).then_some(value);
        }
        let calc = raw.strip_prefix("calc(")?.strip_suffix(')')?;
        let (variable, factor) = calc.split_once('*')?;
        let factor = factor.trim().parse::<f64>().ok()?;
        let name = variable
            .trim()
            .strip_prefix("var(")?
            .strip_suffix(')')?
            .trim();
        let V::Length { value, unit } = uniform_gap(self.vars.get(name).copied().flatten()?)?
        else {
            return None;
        };
        let value = value * factor;
        (value.is_finite() && value >= 0.).then_some(V::Length { value, unit })
    }

    pub(crate) fn apply(&self, file: &File, open: Ast<'_>, el: &mut m::Element) {
        let classes = open
            .children_by_field_name("attribute", &mut open.walk())
            .find(|a| first(*a).is_some_and(|k| file.text(k) == "className"))
            .map(|a| children(a).get(1).and_then(|v| crate::literal(file, *v)));
        let classes = match &classes {
            None => Some(""),
            Some(v) => v.as_ref().and_then(|v| v.as_str()),
        };
        for &(_, property) in DIRECT {
            self.apply_property(property, classes, el, file, open);
        }
    }

    fn apply_property(
        &self,
        property: P,
        classes: Option<&str>,
        el: &mut m::Element,
        file: &File,
        open: Ast<'_>,
    ) {
        el.style
            .retain(|d| d.property != property || d.from.syntax == m::StyleSyntax::Inline);
        let Some(rules) = self.rules.get(&property) else {
            return;
        };
        let mut best = None;
        let mut rank = None;
        let mut unknown: Option<&Rule<'_>> = None;
        let spread = children(open).iter().any(|n| n.kind() == "jsx_expression");
        let mut unknown_rank = None;
        for (order, rule) in rules.iter().enumerate() {
            for selector in &self.selectors[rule.selectors] {
                if selector.subject.excludes(classes, el, spread) {
                    continue;
                }
                match contextual_specificity(selector.text, classes, el, file, open) {
                    Some(None) => {}
                    Some(Some(s)) if !rule.uncertain && !rule.important => {
                        if rank.is_none_or(|r| (rule.layer, s, order) >= r) {
                            rank = Some((rule.layer, s, order));
                            best = Some(rule);
                        }
                    }
                    outcome => {
                        let weight = outcome
                            .flatten()
                            .or_else(|| simple_selector_weight(selector.text))
                            .unwrap_or(u32::MAX);
                        let candidate = (rule.important, (rule.layer, weight, order));
                        if unknown_rank.is_none_or(|r| candidate >= r) {
                            unknown_rank = Some(candidate);
                            unknown = Some(rule);
                        }
                    }
                }
            }
        }
        // Unknown selectors can only displace a normal winner in the same or a higher layer.
        let unknown = unknown.filter(|_| {
            unknown_rank.is_some_and(|(important, candidate)| important || Some(candidate) >= rank)
        });
        if !unknown.is_some_and(|r| r.important) && el.style.iter().any(|d| d.property == property)
        {
            return;
        }
        let Some(rule) = unknown.or(best) else { return };
        el.style.retain(|d| d.property != property);
        let opaque_inline = el
            .unresolved_style
            .iter()
            .any(|d| d.syntax == m::StyleSyntax::Inline);
        let value = (unknown.is_none() && !opaque_inline)
            .then(|| self.value(property, rule))
            .flatten()
            .unwrap_or_else(|| V::Unresolved {
                text: rule.source.text.clone(),
                reason: "Cannot determine style value from built CSS".into(),
            });
        el.style.push(m::StyleDeclaration {
            property,
            value,
            when: None,
            from: rule.source.clone(),
        });
    }
}

// Matching an ancestor can be uncertain while its selector weight is still exact.
fn simple_selector_weight(sel: &str) -> Option<u32> {
    sel.split(|c: char| c == '>' || c.is_ascii_whitespace())
        .filter(|s| !s.is_empty())
        .try_fold(0u32, |sum, token| {
            let weight = if token == "*" {
                0
            } else if token.strip_prefix('.').is_some_and(ident) {
                1024
            } else if ident(token) {
                1
            } else {
                return None;
            };
            sum.checked_add(weight)
        })
}

// Walk only intrinsic JSX ancestors. A component or expression boundary can add
// wrappers, so failure to prove a match beyond that boundary stays unknown.
fn contextual_specificity(
    sel: &str,
    classes: Option<&str>,
    el: &m::Element,
    file: &File,
    open: Ast<'_>,
) -> Option<Option<u32>> {
    if let Some(result) = specificity(sel, classes, el) {
        return Some(result);
    }
    let (ancestor, target, direct) = if let Some((a, b)) = sel.split_once('>') {
        (a.trim(), b.trim(), true)
    } else {
        let mut tokens = sel.split_ascii_whitespace();
        let pair = (tokens.next()?, tokens.next()?, false);
        if tokens.next().is_some() {
            return None;
        }
        pair
    };
    let target_weight = match specificity(target, classes, el)? {
        Some(w) => w,
        None => return Some(None),
    };
    let class = ancestor.strip_prefix('.');
    if !ident(class.unwrap_or(ancestor)) {
        return None;
    }
    let mut parent = if open.kind() == "jsx_opening_element" {
        open.parent()?.parent()?
    } else {
        open.parent()?
    };
    loop {
        if parent.kind() != "jsx_element" {
            return None;
        }
        let tag = parent.child_by_field_name("open_tag")?;
        let name = file.text(tag.child_by_field_name("name")?);
        if !name.starts_with(|c: char| c.is_ascii_lowercase()) || !ident(name) {
            return None;
        }
        let matched = if let Some(class) = class {
            if children(tag).iter().any(|n| n.kind() == "jsx_expression") {
                return None;
            }
            match children(tag).into_iter().find(|a| {
                a.kind() == "jsx_attribute"
                    && first(*a).is_some_and(|k| file.text(k) == "className")
            }) {
                Some(a) => crate::literal(file, *children(a).get(1)?)?
                    .as_str()?
                    .split_ascii_whitespace()
                    .any(|s| s == class),
                None => false,
            }
        } else {
            name.eq_ignore_ascii_case(ancestor)
        };
        if matched {
            return Some(Some(target_weight + if class.is_some() { 1024 } else { 1 }));
        }
        if direct {
            return Some(None);
        }
        parent = parent.parent()?;
    }
}

// Outer None means the selector may match, but its outcome is unknown.
fn specificity(sel: &str, classes: Option<&str>, el: &m::Element) -> Option<Option<u32>> {
    let (matches, weight) = if let Some(class) = sel.strip_prefix('.').filter(|c| ident(c)) {
        (classes?.split_ascii_whitespace().any(|v| v == class), 1024)
    } else if let Some(attr) = sel
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .filter(|s| s.starts_with("data-") && ident(s))
    {
        let present = match el.props.get(attr) {
            None => false,
            Some(m::Expr::Literal { value }) => !value.is_null(),
            Some(_) => return None,
        };
        (present, 1024)
    } else if sel == "*" {
        (true, 0)
    } else if ident(sel) {
        (sel.eq_ignore_ascii_case(&el.tag), 1)
    } else {
        return None;
    };
    Some(matches.then_some(weight))
}
