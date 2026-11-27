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
    // Only a unique, unconditional document-root definition can be inherited here.
    vars: BTreeMap<&'a str, Option<&'a str>>,
}

fn ident(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}

impl<'a> Sheet<'a> {
    pub(crate) fn new(f: &'a File) -> Result<Self, String> {
        let root = f.tree.root_node();
        if root.has_error() {
            return Err(format!("Cannot parse built CSS: {}", f.path));
        }
        let mut sheet = Self {
            rules: BTreeMap::new(),
            vars: BTreeMap::new(),
        };
        let mut layers = Vec::new();
        let mut stack = vec![(root, "", usize::MAX, false)];
        while let Some((node, selector, layer, uncertain)) = stack.pop() {
            let parts = children(node);
            match node.kind() {
                "import_statement" => {
                    return Err("Built CSS must include its imports in one bundle".into());
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
            self.apply_property(property, classes, el);
        }
    }

    fn apply_property(&self, property: P, classes: Option<&str>, el: &mut m::Element) {
        el.style
            .retain(|d| d.property != property || d.from.syntax == m::StyleSyntax::Inline);
        let Some(rules) = self.rules.get(&property) else {
            return;
        };
        let mut best = None;
        let mut rank = None;
        let mut unknown: Option<&Rule<'_>> = None;
        for rule in rules {
            for sel in rule.selectors.split(',').map(str::trim) {
                match specificity(sel, classes, el) {
                    Some(None) => {}
                    Some(Some(s)) if !rule.uncertain && !rule.important => {
                        if rank.is_none_or(|r| (rule.layer, s) >= r) {
                            rank = Some((rule.layer, s));
                            best = Some(rule);
                        }
                    }
                    _ if unknown.is_none_or(|prev| {
                        (rule.important, rule.layer) >= (prev.important, prev.layer)
                    }) =>
                    {
                        unknown = Some(rule)
                    }
                    _ => {}
                }
            }
        }
        // Unknown selectors can only displace a normal winner in the same or a higher layer.
        let unknown =
            unknown.filter(|r| r.important || rank.is_none_or(|(layer, _)| r.layer >= layer));
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

// Outer None means the selector may match, but its outcome is unknown.
fn specificity(sel: &str, classes: Option<&str>, el: &m::Element) -> Option<Option<u8>> {
    let (matches, weight) = if let Some(class) = sel.strip_prefix('.').filter(|c| ident(c)) {
        (classes?.split_ascii_whitespace().any(|v| v == class), 2)
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
        (present, 2)
    } else if sel == "*" {
        (true, 0)
    } else if ident(sel) {
        (sel.eq_ignore_ascii_case(&el.tag), 1)
    } else {
        return None;
    };
    Some(matches.then_some(weight))
}
