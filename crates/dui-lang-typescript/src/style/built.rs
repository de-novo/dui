//! Resolve supported style declarations from an explicitly supplied, already bundled author stylesheet.
use super::{P, V, modules, shorthand, spacing, uniform_gap};
use crate::syntax::{File, children, first};
use dui_core::source_model as m;
use std::collections::BTreeMap;
use tree_sitter::Node as Ast;

// Each supported property uses the same value parser as a source stylesheet.
const DIRECT: &[(&str, P)] = &[
    ("order", P::Order),
    ("text-align", P::TextAlign),
    ("border-collapse", P::BorderCollapse),
    ("vertical-align", P::VerticalAlign),
    ("text-transform", P::TextTransform),
    ("white-space", P::WhiteSpace),
    ("backdrop-filter", P::BackdropBlur),
    ("border-style", P::BorderStyle),
    ("border-top-width", P::BorderTopWidth),
    ("border-top-color", P::BorderTopColor),
    ("border-top-style", P::BorderTopStyle),
    ("border-right-width", P::BorderRightWidth),
    ("border-right-color", P::BorderRightColor),
    ("border-right-style", P::BorderRightStyle),
    ("border-bottom-width", P::BorderBottomWidth),
    ("border-bottom-color", P::BorderBottomColor),
    ("border-bottom-style", P::BorderBottomStyle),
    ("border-left-width", P::BorderLeftWidth),
    ("border-left-color", P::BorderLeftColor),
    ("border-left-style", P::BorderLeftStyle),
    ("position", P::Position),
    ("top", P::Top),
    ("right", P::Right),
    ("bottom", P::Bottom),
    ("left", P::Left),
    ("z-index", P::ZIndex),
    ("background-image", P::DotRadius),
    ("background-image", P::DotColor),
    ("background-size", P::TileWidth),
    ("background-size", P::TileHeight),
    ("box-shadow", P::ShadowX),
    ("box-shadow", P::ShadowY),
    ("box-shadow", P::ShadowBlur),
    ("box-shadow", P::ShadowSpread),
    ("box-shadow", P::ShadowColor),
    ("border-top-left-radius", P::BorderTopLeftRadius),
    ("border-top-right-radius", P::BorderTopRightRadius),
    ("border-bottom-right-radius", P::BorderBottomRightRadius),
    ("border-bottom-left-radius", P::BorderBottomLeftRadius),
    ("display", P::Display),
    ("grid-template-columns", P::GridColumnMinimum),
    ("overflow-x", P::OverflowX),
    ("gap", P::Gap),
    ("row-gap", P::RowGap),
    ("column-gap", P::ColumnGap),
    ("flex-shrink", P::Shrink),
    ("flex-basis", P::FlexBasis),
    ("color", P::Color),
    ("background-color", P::Background),
    ("border-color", P::BorderColor),
    ("border-width", P::BorderWidth),
    ("border-radius", P::BorderRadius),
    ("opacity", P::Opacity),
    ("font-family", P::FontFamily),
    ("text-decoration-line", P::TextDecorationLine),
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

fn flex_basis(raw: &str) -> Option<V> {
    if matches!(
        raw,
        "auto" | "min-content" | "max-content" | "content" | "fit-content"
    ) {
        return Some(V::Keyword { value: raw.into() });
    }
    super::size_value(P::Width, raw).filter(|v| matches!(v, V::Length { .. }))
}

const BORDER_STYLES: &[&str] = &[
    "none", "hidden", "dotted", "dashed", "solid", "double", "groove", "ridge", "inset", "outset",
];
const EDGES: &[(&str, [P; 3])] = &[
    (
        "border-top",
        [P::BorderTopWidth, P::BorderTopColor, P::BorderTopStyle],
    ),
    (
        "border-right",
        [
            P::BorderRightWidth,
            P::BorderRightColor,
            P::BorderRightStyle,
        ],
    ),
    (
        "border-bottom",
        [
            P::BorderBottomWidth,
            P::BorderBottomColor,
            P::BorderBottomStyle,
        ],
    ),
    (
        "border-left",
        [P::BorderLeftWidth, P::BorderLeftColor, P::BorderLeftStyle],
    ),
];
fn edge_properties(name: &str) -> Option<Vec<P>> {
    let general = [P::BorderWidth, P::BorderColor, P::BorderStyle];
    let part = match name {
        "border" => None,
        "border-width" => Some(0),
        "border-color" => Some(1),
        "border-style" => Some(2),
        _ => {
            for (side, props) in EDGES {
                if name == *side {
                    return Some(general.into_iter().chain(*props).collect());
                }
                if let Some(suffix) = name.strip_prefix(side) {
                    let i = match suffix {
                        "-width" => 0,
                        "-color" => 1,
                        "-style" => 2,
                        _ => continue,
                    };
                    return Some(vec![general[i], props[i]]);
                }
            }
            return None;
        }
    };
    Some(match part {
        None => general
            .into_iter()
            .chain(EDGES.iter().flat_map(|(_, props)| *props))
            .collect(),
        Some(i) => std::iter::once(general[i])
            .chain(EDGES.iter().map(|(_, props)| props[i]))
            .collect(),
    })
}

fn alpha_mix(raw: &str, vars: &BTreeMap<&str, Option<&str>>) -> Option<V> {
    let mut args = raw
        .strip_prefix("color-mix(")?
        .strip_suffix(')')?
        .split(',')
        .map(str::trim);
    if args.next()? != "in srgb" {
        return None;
    }
    let (base, weight) = args.next()?.rsplit_once(' ')?;
    if args.next()? != "transparent" || args.next().is_some() {
        return None;
    }
    let fraction = weight.strip_suffix('%')?.parse::<f64>().ok()? / 100.;
    if !(0.0..=1.0).contains(&fraction) {
        return None;
    }
    let base = if let Some(key) = base.strip_prefix("var(").and_then(|v| v.strip_suffix(')')) {
        vars.get(key)?.as_ref()?
    } else {
        base
    };
    let V::Color { value } = super::color(base)? else {
        return None;
    };
    let alpha = if value.len() == 9 {
        u8::from_str_radix(&value[7..], 16).ok()?
    } else {
        255
    };
    Some(V::Color {
        value: format!(
            "{}{:02x}",
            &value[..7],
            (f64::from(alpha) * fraction).round() as u8
        ),
    })
}
fn four_sides(raw: &str) -> Option<[&str; 4]> {
    let tokens: Vec<_> = raw.split_ascii_whitespace().collect();
    match tokens.as_slice() {
        [a] => Some([a, a, a, a]),
        [a, b] => Some([a, b, a, b]),
        [a, b, c] => Some([a, b, c, b]),
        [a, b, c, d] => Some([a, b, c, d]),
        _ => None,
    }
}
fn affected(name: &str) -> Vec<P> {
    match name {
        "gap" | "grid-gap" => return vec![P::Gap, P::RowGap, P::ColumnGap],
        "row-gap" | "grid-row-gap" => return vec![P::Gap, P::RowGap],
        "column-gap" | "grid-column-gap" => return vec![P::Gap, P::ColumnGap],
        _ => {}
    }
    match name {
        "background-image" => return vec![P::DotColor, P::DotRadius],
        "background-size" => return vec![P::TileWidth, P::TileHeight],
        "box-shadow" => {
            return vec![
                P::ShadowX,
                P::ShadowY,
                P::ShadowBlur,
                P::ShadowSpread,
                P::ShadowColor,
            ];
        }
        "inset" => return vec![P::Top, P::Right, P::Bottom, P::Left],
        "border-radius" => {
            return vec![
                P::BorderRadius,
                P::BorderTopLeftRadius,
                P::BorderTopRightRadius,
                P::BorderBottomRightRadius,
                P::BorderBottomLeftRadius,
            ];
        }
        _ => {}
    }
    if name.starts_with("border-")
        && name.ends_with("-radius")
        && let Some((_, corner)) = DIRECT
            .iter()
            .find(|(css, _)| *css == name && *css != "border-radius")
    {
        return vec![P::BorderRadius, *corner];
    }
    if let Some(edges) = edge_properties(name) {
        return edges;
    }
    if let Some(props) = spacing(name) {
        return props.to_vec();
    }
    if let Some((_, prop)) = DIRECT.iter().find(|(css, _)| *css == name) {
        return vec![*prop];
    }
    match name {
        "overflow" => vec![P::OverflowX],
        "background" => vec![
            P::Background,
            P::DotRadius,
            P::DotColor,
            P::TileWidth,
            P::TileHeight,
        ],
        "border" | "border-top" | "border-bottom" | "border-left" | "border-right"
        | "border-inline" | "border-block" => vec![P::BorderWidth, P::BorderColor],
        "text-decoration" => vec![P::TextDecorationLine],
        "font" => vec![P::FontSize, P::LineHeight, P::FontFamily, P::FontWeight],
        "flex" => vec![P::Grow, P::Shrink, P::FlexBasis],
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
    document_vars: BTreeMap<&'a str, Option<&'a str>>,
    root_key: Option<String>,
    horizontal_block: bool,
    viewport_width: Option<u32>,
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
    pub(crate) fn new(
        f: &'a File,
        width: Option<u32>,
        root_attribute: Option<(&str, &str)>,
        scheme: Option<&str>,
    ) -> Result<Self, String> {
        let root = f.tree.root_node();
        if structural_error(f, root) {
            return Err(format!("Cannot parse built CSS: {}", f.path));
        }
        let mut sheet = Self {
            document_vars: BTreeMap::new(),
            root_key: root_attribute.map(|(k, _)| k.to_owned()),
            horizontal_block: true,
            viewport_width: width,
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
        let mut variable_ranks = BTreeMap::new();
        let mut scoped_variables = std::collections::BTreeSet::new();
        let mut ambiguous_variables = std::collections::BTreeSet::new();
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
                        let raw = f.text(*query).trim();
                        let preference = raw
                            .strip_prefix('(')
                            .and_then(|s| s.strip_suffix(')'))
                            .and_then(|s| s.split_once(':'))
                            .filter(|(k, v)| {
                                k.trim() == "prefers-color-scheme"
                                    && matches!(v.trim(), "light" | "dark")
                            });
                        let matches = if let Some((_, v)) = preference {
                            scheme.map(|s| s == v.trim())
                        } else {
                            width.and_then(|w| width_query(raw, w))
                        };
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
                    if matches!(prop, "writing-mode" | "-webkit-writing-mode" | "all") {
                        sheet.horizontal_block = false;
                    }
                    if prop.starts_with("--") {
                        let root = selector.split(',').map(str::trim).any(|s| s == ":root")
                            && selector
                                .split(',')
                                .map(str::trim)
                                .all(|s| matches!(s, ":root" | ":host"));
                        let root_value = root_attribute.and_then(|(key, value)| {
                            let (name, raw) = selector
                                .strip_prefix(":root[")?
                                .strip_suffix(']')?
                                .split_once('=')?;
                            let raw = raw
                                .strip_prefix('"')
                                .and_then(|v| v.strip_suffix('"'))
                                .or_else(|| {
                                    raw.strip_prefix('\'').and_then(|v| v.strip_suffix('\''))
                                })
                                .unwrap_or(raw);
                            (name == key && ident(raw)).then_some(raw == value)
                        });
                        if let Some((key, selected)) = root_attribute
                            && let Some((attr, raw)) = selector
                                .strip_prefix('[')
                                .and_then(|s| s.strip_suffix(']'))
                                .and_then(|s| s.split_once('='))
                            && attr == key
                            && ident(raw.trim_matches(['"', '\'']))
                            && raw.trim_matches(['"', '\'']) != selected
                        {
                            scoped_variables.insert(prop);
                            continue;
                        }
                        if root_value == Some(false) {
                            continue;
                        }
                        if (root_attribute.is_some() || scheme.is_some())
                            && (root || root_value == Some(true))
                            && !important
                            && !uncertain
                        {
                            let rank = (layer, root_value == Some(true));
                            if variable_ranks
                                .get(prop)
                                .is_none_or(|previous| rank >= *previous)
                            {
                                variable_ranks.insert(prop, rank);
                                sheet.vars.insert(prop, Some(value));
                            }
                        } else if root_attribute.is_some() || scheme.is_some() {
                            ambiguous_variables.insert(prop);
                        } else {
                            let definition = (root && !uncertain && !important).then_some(value);
                            sheet
                                .vars
                                .entry(prop)
                                .and_modify(|v| *v = None)
                                .or_insert(definition);
                        }
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
        for name in ambiguous_variables {
            sheet.vars.insert(name, None);
        }
        sheet.document_vars = sheet.vars.clone();
        for name in scoped_variables {
            sheet.vars.insert(name, None);
        }
        Ok(sheet)
    }

    fn value(
        &self,
        property: P,
        rule: &Rule<'_>,
        vars: &BTreeMap<&str, Option<&str>>,
    ) -> Option<V> {
        let raw = rule.value;
        let raw = if let Some(name) = raw.strip_prefix("var(").and_then(|s| s.strip_suffix(')')) {
            vars.get(name.trim()).copied().flatten()?
        } else {
            raw
        };
        if rule.name == "position" {
            return matches!(raw, "static" | "relative" | "absolute" | "fixed" | "sticky")
                .then(|| V::Keyword { value: raw.into() });
        }
        if rule.name == "z-index" {
            return if raw == "auto" {
                Some(V::Keyword { value: raw.into() })
            } else {
                raw.parse::<i32>()
                    .ok()
                    .map(|v| V::Number { value: v.into() })
            };
        }
        if matches!(property, P::Top | P::Right | P::Bottom | P::Left) {
            let side = match property {
                P::Right => 1,
                P::Bottom => 2,
                P::Left => 3,
                _ => 0,
            };
            let token = if rule.name == "inset" {
                four_sides(raw)?[side]
            } else if matches!(rule.name, "top" | "right" | "bottom" | "left") {
                raw
            } else {
                return None;
            };
            return super::spacing_value("margin", token);
        }
        if let Some(i) = [
            P::BorderTopLeftRadius,
            P::BorderTopRightRadius,
            P::BorderBottomRightRadius,
            P::BorderBottomLeftRadius,
        ]
        .iter()
        .position(|p| *p == property)
        {
            let token = if rule.name == "border-radius" {
                four_sides(raw)?[i]
            } else {
                raw
            };
            return uniform_gap(token);
        }
        if rule.name == "background-image" {
            let (first, last) = raw
                .strip_prefix("radial-gradient(")?
                .strip_suffix(')')?
                .split_once(',')?;
            let (ink, radius) = first.trim().rsplit_once(' ')?;
            if last.trim().strip_prefix("transparent ")? != radius {
                return None;
            }
            let ink = if let Some(key) = ink.strip_prefix("var(").and_then(|s| s.strip_suffix(')'))
            {
                vars.get(key.trim()).copied().flatten()?
            } else {
                ink
            };
            let color = super::color(ink)?;
            let radius = uniform_gap(radius)?;
            return Some(if property == P::DotColor {
                color
            } else {
                radius
            });
        }
        if rule.name == "background-size" {
            let mut tokens = raw.split_ascii_whitespace();
            let w = uniform_gap(tokens.next()?)?;
            let h = uniform_gap(tokens.next()?)?;
            if tokens.next().is_some() {
                return None;
            }
            return Some(if property == P::TileWidth { w } else { h });
        }
        if rule.name == "box-shadow" {
            let parts: Vec<_> = raw.split_ascii_whitespace().collect();
            let (color, lengths) = parts.split_last()?;
            let color = super::color(color)?;
            let (x, y, blur, spread) = match lengths {
                [x, y] => (*x, *y, "0", "0"),
                [x, y, b] => (*x, *y, *b, "0"),
                [x, y, b, s] => (*x, *y, *b, *s),
                _ => return None,
            };
            let offset = |v| {
                super::spacing_value("margin", v)
                    .filter(|n| matches!(n,V::Length{unit,..} if *unit!=m::LengthUnit::Percent))
            };
            let values = [
                offset(x)?,
                offset(y)?,
                offset(blur)?,
                offset(spread)?,
                color,
            ];
            if matches!(values[2],V::Length{value,..} if value<0.) {
                return None;
            }
            let index = [
                P::ShadowX,
                P::ShadowY,
                P::ShadowBlur,
                P::ShadowSpread,
                P::ShadowColor,
            ]
            .iter()
            .position(|p| *p == property)?;
            return Some(values[index].clone());
        }
        if matches!(rule.name, "padding-block" | "margin-block") && self.horizontal_block {
            let mut parts = raw.split_ascii_whitespace();
            let first = parts.next()?;
            let last = parts.next().unwrap_or(first);
            if parts.next().is_some() {
                return None;
            }
            return super::spacing_value(
                rule.name,
                if matches!(property, P::PaddingTop | P::MarginTop) {
                    first
                } else {
                    last
                },
            );
        }
        if property == P::TextDecorationLine
            && matches!(raw, "none" | "underline" | "overline" | "line-through")
        {
            return Some(V::Keyword { value: raw.into() });
        }
        if rule.name == "font-family"
            && raw.split(',').all(|family| {
                let name = family.trim().trim_matches(['"', '\'']);
                !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b' ' | b'_'))
            })
        {
            return Some(V::Keyword { value: raw.into() });
        }
        // Evaluate viewport lengths in spacing before expanding their shorthand.
        let normalized;
        let raw = if spacing(rule.name).is_some() && raw.contains("min(") {
            let width = f64::from(self.viewport_width?);
            let (before, args) = raw.split_once("min(")?;
            let (args, after) = args.split_once(')')?;
            let px = |arg: &str| {
                let arg = arg.trim();
                let (value, scale) = arg
                    .strip_suffix("px")
                    .map(|v| (v, 1.0))
                    .or_else(|| arg.strip_suffix("vw").map(|v| (v, width / 100.0)))?;
                let n = value.parse::<f64>().ok()? * scale;
                n.is_finite().then_some(n)
            };
            let (a, b) = args.split_once(',')?;
            normalized = format!("{before}{}px{after}", px(a)?.min(px(b)?));
            normalized.as_str()
        } else {
            raw
        };
        if rule.name == "flex" {
            let parts: Vec<_> = raw.split_ascii_whitespace().collect();
            let (grow, shrink, basis) = match parts.as_slice() {
                ["none"] => ("0", "0", "auto"),
                ["initial"] => ("0", "1", "auto"),
                ["auto"] => ("1", "1", "auto"),
                [g] if super::grow_factor(g).is_some() => (*g, "1", "0%"),
                [b] => ("1", "1", *b),
                [g, s] if super::grow_factor(s).is_some() => (*g, *s, "0%"),
                [g, b] => (*g, "1", *b),
                [g, s, b] => (*g, *s, *b),
                _ => return None,
            };
            let grow = super::grow_factor(grow)?;
            let shrink = super::grow_factor(shrink)?;
            let basis = flex_basis(basis)?;
            return Some(if property == P::Grow {
                grow
            } else if property == P::Shrink {
                shrink
            } else {
                basis
            });
        }
        if property == P::Shrink {
            return super::grow_factor(raw);
        }
        if property == P::FlexBasis {
            return flex_basis(raw);
        }
        if matches!(property, P::RowGap | P::ColumnGap) && !raw.starts_with("calc(") {
            let mut parts = raw.split_ascii_whitespace();
            let row = uniform_gap(parts.next()?)?;
            let col = parts
                .next()
                .map(uniform_gap)
                .unwrap_or_else(|| Some(row.clone()))?;
            if parts.next().is_some() {
                return None;
            }
            return Some(if property == P::RowGap { row } else { col });
        }
        if matches!(property, P::Display | P::OverflowX) {
            let valid = if property == P::Display {
                matches!(
                    raw,
                    "grid" | "flex" | "inline-flex" | "inline-block" | "block" | "inline" | "none"
                )
            } else {
                matches!(raw, "visible" | "auto" | "scroll" | "hidden" | "clip")
            };
            return valid.then(|| V::Keyword { value: raw.into() });
        }
        if property == P::GridColumnMinimum {
            let compact = raw.split_ascii_whitespace().collect::<String>();
            if compact == "minmax(0,1fr)" {
                return Some(V::Keyword {
                    value: "single".into(),
                });
            }
            return super::length(
                compact
                    .strip_prefix("repeat(auto-fit,minmax(")?
                    .strip_suffix(",1fr))")?,
            );
        }
        if property == P::FontSize && raw.starts_with("clamp(") {
            let mut args = raw.strip_prefix("clamp(")?.strip_suffix(')')?.split(',');
            let mut quantity = |unit: &str| {
                args.next()?
                    .trim()
                    .strip_suffix(unit)?
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite() && *n >= 0.)
            };
            let lower = quantity("px")?;
            let preferred = quantity("vw")? * f64::from(self.viewport_width?) / 100.;
            let upper = quantity("px")?;
            return args.next().is_none().then_some(V::Length {
                value: lower.max(preferred.min(upper)),
                unit: m::LengthUnit::Px,
            });
        }
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
        if property == P::BackdropBlur {
            return super::size_value(
                P::BorderWidth,
                raw.strip_prefix("blur(")?.strip_suffix(')')?,
            );
        }
        if matches!(property, P::Background | P::Color) && raw.starts_with("color-mix(") {
            return alpha_mix(raw, vars);
        }
        let keywords: &[&str] = match property {
            P::BorderCollapse => &["collapse", "separate"],
            P::VerticalAlign => &[
                "baseline",
                "top",
                "middle",
                "bottom",
                "text-top",
                "text-bottom",
                "sub",
                "super",
            ],
            P::TextTransform => &["none", "uppercase", "lowercase", "capitalize"],
            P::WhiteSpace => &[
                "normal",
                "pre",
                "nowrap",
                "pre-wrap",
                "pre-line",
                "break-spaces",
            ],
            P::TextAlign => &["start", "end", "left", "right", "center", "justify"],
            _ => &[],
        };
        if !keywords.is_empty() {
            return keywords
                .contains(&raw)
                .then(|| V::Keyword { value: raw.into() });
        }
        if edge_properties(rule.name).is_some() {
            let general = [P::BorderWidth, P::BorderColor, P::BorderStyle];
            let role = general.iter().position(|p| *p == property).or_else(|| {
                EDGES
                    .iter()
                    .find_map(|(_, props)| props.iter().position(|p| *p == property))
            })?;
            if general.contains(&property)
                && EDGES.iter().any(|(side, _)| rule.name.starts_with(side))
            {
                return None;
            }
            if rule.name.ends_with("-width") {
                return super::size_value(P::BorderWidth, raw);
            }
            if rule.name.ends_with("-color") {
                return super::color(raw).or_else(|| alpha_mix(raw, vars));
            }
            if rule.name.ends_with("-style") {
                return BORDER_STYLES
                    .contains(&raw)
                    .then(|| V::Keyword { value: raw.into() });
            }

            let mut width = None;
            let mut style = None;
            let (prefix, mix) = if let Some((prefix, mix)) = raw.split_once("color-mix(") {
                (prefix, Some(alpha_mix(&format!("color-mix({mix}"), vars)?))
            } else {
                (raw, None)
            };
            let mut color = mix;
            for word in prefix.split_ascii_whitespace() {
                let word = if let Some(name) =
                    word.strip_prefix("var(").and_then(|s| s.strip_suffix(')'))
                {
                    vars.get(name.trim()).copied().flatten()?
                } else {
                    word
                };
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
            return match role {
                0 => width,
                1 => color,
                _ => Some(V::Keyword {
                    value: style.unwrap_or("none").into(),
                }),
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
        let V::Length { value, unit } = uniform_gap(vars.get(name).copied().flatten()?)? else {
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
        let mut tag = open;
        let document_scope = loop {
            let name = tag
                .child_by_field_name("name")
                .map(|n| file.text(n))
                .unwrap_or("");
            if !name.starts_with(|c: char| c.is_ascii_lowercase())
                || tag
                    .children_by_field_name("attribute", &mut tag.walk())
                    .any(|a| {
                        a.kind() != "jsx_attribute"
                            || first(a)
                                .is_some_and(|k| Some(file.text(k)) == self.root_key.as_deref())
                    })
            {
                break false;
            }
            if name == "body" {
                break true;
            }
            let owner = if tag.kind() == "jsx_opening_element" {
                tag.parent()
            } else {
                Some(tag)
            };
            let Some(parent) = owner
                .and_then(|n| n.parent())
                .and_then(|n| n.child_by_field_name("open_tag"))
            else {
                break false;
            };
            tag = parent;
        };
        for &(_, property) in DIRECT {
            self.apply_property(property, classes, el, file, open, document_scope);
        }
    }

    fn apply_property(
        &self,
        property: P,
        classes: Option<&str>,
        el: &mut m::Element,
        file: &File,
        open: Ast<'_>,
        document_scope: bool,
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
        if matches!(property, P::RowGap | P::ColumnGap)
            && !unknown.is_some_and(|r| r.important)
            && el
                .style
                .iter()
                .any(|d| d.property == P::Gap && d.from.syntax == m::StyleSyntax::Inline)
        {
            return;
        }
        if let Some(i) = EDGES
            .iter()
            .find_map(|(_, props)| props.iter().position(|p| *p == property))
            && !unknown.is_some_and(|r| r.important)
            && el.style.iter().any(|d| {
                d.from.syntax == m::StyleSyntax::Inline
                    && d.property == [P::BorderWidth, P::BorderColor, P::BorderStyle][i]
            })
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
            .then(|| {
                self.value(
                    property,
                    rule,
                    if document_scope {
                        &self.document_vars
                    } else {
                        &self.vars
                    },
                )
            })
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

// Unknown siblings (expressions, fragments or component instances) may emit elements.
fn edge_child(file: &File, node: Ast<'_>, first_child: bool) -> Option<bool> {
    let parent = node.parent()?.child_by_field_name("open_tag")?;
    if !file
        .text(parent.child_by_field_name("name")?)
        .starts_with(|c: char| c.is_ascii_lowercase())
    {
        return None;
    }
    let sibling = if first_child {
        Ast::prev_named_sibling
    } else {
        Ast::next_named_sibling
    };
    let mut current = sibling(&node);
    while let Some(next) = current {
        match next.kind() {
            "jsx_text" | "jsx_opening_element" | "jsx_closing_element" => {}
            "jsx_element" | "jsx_self_closing_element" => {
                let tag = next.child_by_field_name("open_tag").unwrap_or(next);
                let name = file.text(tag.child_by_field_name("name")?);
                return name
                    .starts_with(|c: char| c.is_ascii_lowercase())
                    .then_some(false);
            }
            _ => return None,
        }
        current = sibling(&next);
    }
    Some(true)
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
    let (ancestor, edge) = if let Some(tag) = ancestor.strip_suffix(":last-child") {
        (tag, Some(false))
    } else if let Some(tag) = ancestor.strip_suffix(":first-child") {
        (tag, Some(true))
    } else {
        (ancestor, None)
    };
    let class = ancestor.strip_prefix('.');
    let id = ancestor.strip_prefix('#');
    if !ident(class.or(id).unwrap_or(ancestor)) {
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
        let matched = if let Some(required) = class.or(id) {
            if children(tag).iter().any(|n| n.kind() == "jsx_expression") {
                return None;
            }
            match children(tag).into_iter().find(|a| {
                a.kind() == "jsx_attribute"
                    && first(*a).is_some_and(|k| {
                        file.text(k) == if id.is_some() { "id" } else { "className" }
                    })
            }) {
                Some(a) => {
                    let v = crate::literal(file, *children(a).get(1)?)?;
                    let s = v.as_str()?;
                    if id.is_some() {
                        s == required
                    } else {
                        s.split_ascii_whitespace().any(|v| v == required)
                    }
                }
                None => false,
            }
        } else {
            name.eq_ignore_ascii_case(ancestor)
        };
        if matched
            && match edge {
                Some(first) => edge_child(file, parent, first)?,
                None => true,
            }
        {
            return Some(Some(
                target_weight
                    + if id.is_some() {
                        1048576
                    } else if class.is_some() {
                        1024
                    } else {
                        1
                    }
                    + if edge.is_some() { 1024 } else { 0 },
            ));
        }
        if direct || (name == "body" && class.is_none() && id.is_none() && ancestor != "html") {
            return Some(None);
        }
        parent = parent.parent()?;
    }
}

// Outer None means the selector may match, but its outcome is unknown.
fn specificity(sel: &str, classes: Option<&str>, el: &m::Element) -> Option<Option<u32>> {
    if sel == ":root" {
        return Some((el.tag == "html").then_some(1024));
    }
    if let Some(id) = sel.strip_prefix('#').filter(|s| ident(s)) {
        return match el.props.get("id") {
            None => Some(None),
            Some(m::Expr::Literal { value }) => Some((value.as_str()? == id).then_some(1048576)),
            _ => None,
        };
    }
    if let Some((tag, class)) = sel.split_once('.')
        && ident(tag)
        && ident(class)
    {
        if !tag.eq_ignore_ascii_case(&el.tag) {
            return Some(None);
        }
        return Some(
            classes?
                .split_ascii_whitespace()
                .any(|c| c == class)
                .then_some(1025),
        );
    }
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
