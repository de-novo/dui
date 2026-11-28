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
    super::size_value(P::Width, raw).filter(|v| matches!(v, V::Length { .. } | V::Clamp { .. }))
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
fn css_words(raw: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = None;
    let mut depth = 0usize;
    for (index, c) in raw.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            c if c.is_ascii_whitespace() && depth == 0 => {
                if let Some(from) = start.take() {
                    out.push(&raw[from..index]);
                }
            }
            _ => {}
        }
        if !c.is_ascii_whitespace() && start.is_none() {
            start = Some(index);
        }
    }
    if let Some(from) = start {
        out.push(&raw[from..]);
    }
    out
}

fn four_sides(raw: &str) -> Option<[&str; 4]> {
    let tokens = css_words(raw);
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
    when: Option<m::Condition>,
    name: &'a str,
    important: bool,
    value: &'a str,
    source: m::StyleSource,
}

pub(crate) struct Sheet<'a> {
    evaluate_width: bool,
    document_vars: BTreeMap<&'a str, Option<&'a str>>,
    root_key: Option<String>,
    root_value: Option<String>,
    horizontal_block: bool,
    rules: BTreeMap<P, Vec<Rule<'a>>>,
    custom_rules: BTreeMap<&'a str, Vec<Rule<'a>>>,
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

// The CSS grammar may mark a media range, container query or an empty var() fallback as an
// error. These remain inside an intact rule; conditions stay unknown and values are parsed
// from source. A genuinely broken rule or declaration still rejects the sheet.
fn structural_error(f: &File, root: Ast<'_>) -> bool {
    fn recoverable(f: &File, n: Ast<'_>) -> bool {
        if !n.is_error() {
            return false;
        }
        let text = f.text(n);
        match n.parent().map(|p| p.kind()) {
            Some("parenthesized_query") => {
                !text.contains(['{', '}', ';'])
                    && (text.starts_with("<=")
                        || text.starts_with(">=")
                        || text.starts_with('<')
                        || text.starts_with('>'))
            }
            Some("arguments") => text == ",",
            Some("at_rule") => text.starts_with("content ") && !text.contains(['{', '}', ';']),
            _ => false,
        }
    }
    let mut pending = vec![root];
    while let Some(n) = pending.pop() {
        if !n.has_error() {
            continue;
        }
        if n.is_error() || n.is_missing() {
            if recoverable(f, n) {
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
    let scale = if length.ends_with("rem") || length.ends_with("em") {
        16.
    } else {
        1.
    };
    let threshold = length
        .strip_suffix("px")
        .or_else(|| length.strip_suffix("rem"))
        .or_else(|| length.strip_suffix("em"))
        .or_else(|| (length == "0").then_some(length))?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.)?
        * scale;
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

fn merge_width_condition(base: Option<&m::Condition>, next: m::Condition) -> m::Condition {
    let mut out = base.cloned().unwrap_or(m::Condition {
        min_width: None,
        max_width: None,
        interaction: None,
        color_scheme: None,
        expr: None,
        equals: None,
    });
    if let Some(width) = next.min_width {
        out.min_width = Some(match out.min_width {
            Some(current) if current.value >= width.value => current,
            _ => width,
        });
    }
    if let Some(width) = next.max_width {
        out.max_width = Some(match out.max_width {
            Some(current) if current.value <= width.value => current,
            _ => width,
        });
    }
    out
}

/// Parse a width-only media feature into the model's viewport condition. Unsupported logic stays
/// uncertain; units remain symbolic so the renderer evaluates them against each logical card.
fn width_condition(raw: &str) -> Option<m::Condition> {
    let raw = raw.trim();
    if raw.starts_with("not all and ") {
        return None;
    }
    let feature = raw.strip_prefix('(')?.strip_suffix(')')?.trim();
    let (name, value) = feature.split_once(':')?;
    let super::V::Length { value, unit } = super::length(value.trim())? else {
        return None;
    };
    if value < 0.
        || !matches!(
            unit,
            m::LengthUnit::Px | m::LengthUnit::Rem | m::LengthUnit::Em
        )
    {
        return None;
    }
    let width = Some(m::Length { value, unit });
    let (min_width, max_width) = match name.trim() {
        "min-width" => (width, None),
        "max-width" => (None, width),
        _ => return None,
    };
    Some(m::Condition {
        min_width,
        max_width,
        interaction: None,
        color_scheme: None,
        expr: None,
        equals: None,
    })
}

fn split_var_argument(raw: &str) -> Option<(&str, Option<&str>)> {
    let inner = raw.strip_prefix("var(")?.strip_suffix(')')?;
    let mut depth = 0usize;
    for (index, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                return Some((inner[..index].trim(), Some(inner[index + 1..].trim())));
            }
            _ => {}
        }
    }
    Some((inner.trim(), None))
}

/// Resolve one whole var() reference, preferring the custom property and using its fallback
/// only when the property is absent or uncertain. Nested fallbacks remain source text, never guesses.
fn resolved_var<'a>(
    raw: &'a str,
    vars: &BTreeMap<&'a str, Option<&'a str>>,
    depth: usize,
) -> Option<&'a str> {
    if depth >= 8 {
        return None;
    }
    let (name, fallback) = split_var_argument(raw)?;
    if let Some(value) = vars.get(name).copied().flatten() {
        // Theme variables commonly alias another variable (`--default-font-family:
        // var(--font-sans)`). Follow that chain; a cycle or unknown link stays unresolved.
        return if value.starts_with("var(") {
            resolved_var(value, vars, depth + 1)
        } else {
            Some(value)
        };
    }
    let fallback = fallback?;
    if fallback.starts_with("var(") {
        resolved_var(fallback, vars, depth + 1)
    } else {
        Some(fallback)
    }
}

fn resolved_value_text<'a>(raw: &'a str, vars: &BTreeMap<&'a str, Option<&'a str>>) -> &'a str {
    if raw.starts_with("var(") {
        resolved_var(raw, vars, 0).unwrap_or(raw)
    } else {
        raw
    }
}

fn length_operand<'a>(
    raw: &'a str,
    vars: &BTreeMap<&'a str, Option<&'a str>>,
) -> Option<m::Length> {
    let raw = resolved_value_text(raw.trim(), vars);
    if raw.starts_with("calc(") {
        return calc_length(raw, vars);
    }
    super::numeric_length(raw)
}

fn calc_length<'a>(raw: &'a str, vars: &BTreeMap<&'a str, Option<&'a str>>) -> Option<m::Length> {
    let inner = raw.trim().strip_prefix("calc(")?.strip_suffix(')')?.trim();
    let mut depth = 0usize;
    let mut operator = None;
    for (index, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '+' | '-' | '*' if depth == 0 && index > 0 => operator = Some((index, c)),
            _ => {}
        }
    }
    let (index, op) = operator?;
    let left = inner[..index].trim();
    let right = inner[index + 1..].trim();
    if op == '*' {
        let number = left
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(|n| (n, right))
            .or_else(|| {
                right
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .map(|n| (n, left))
            })?;
        let mut length = length_operand(number.1, vars)?;
        length.value *= number.0;
        return (length.value.is_finite()).then_some(length);
    }
    let mut left = length_operand(left, vars)?;
    let mut right = length_operand(right, vars)?;
    if left.unit != right.unit {
        use m::LengthUnit as U;
        match (left.unit, right.unit) {
            (U::Rem, U::Px) => right.value /= 16.,
            (U::Px, U::Rem) => right.value *= 16.,
            _ => return None,
        }
        right.unit = left.unit;
    }
    match op {
        '+' => left.value += right.value,
        '-' => left.value -= right.value,
        _ => return None,
    }
    (left.value.is_finite()).then_some(left)
}

fn numeric_calc(raw: &str) -> Option<f64> {
    let inner = raw.trim().strip_prefix("calc(")?.strip_suffix(')')?.trim();
    let (left, right) = inner.split_once('/')?;
    let left = left.trim().parse::<f64>().ok()?;
    let right = right.trim().parse::<f64>().ok()?;
    (right != 0.)
        .then_some(left / right)
        .filter(|v| v.is_finite())
}

fn model_length<'a>(
    property: P,
    raw: &'a str,
    vars: &BTreeMap<&'a str, Option<&'a str>>,
) -> Option<V> {
    let raw = resolved_value_text(raw.trim(), vars);
    if raw.starts_with("calc(") {
        if property == P::LineHeight
            && let Some(value) = numeric_calc(raw).filter(|v| *v >= 0.)
        {
            return Some(V::Number { value });
        }
        let length = calc_length(raw, vars)?;
        return Some(V::Length {
            value: length.value,
            unit: length.unit,
        });
    }
    if property == P::LineHeight
        && let Ok(value) = raw.parse::<f64>()
        && value.is_finite()
        && value >= 0.
    {
        return Some(V::Number { value });
    }
    super::length(raw)
}

impl<'a> Sheet<'a> {
    pub(crate) fn new(
        f: &'a File,
        width: Option<u32>,
        root_attribute: Option<(&str, &str)>,
    ) -> Result<Self, String> {
        Self::parse(f, width, root_attribute, true)
    }

    /// Parse a source-owned stylesheet. Unlike a built bundle, its `@import` statements are
    /// already represented in the module graph, so the import node itself is not an error.
    pub(crate) fn source(f: &'a File) -> Result<Self, String> {
        Self::parse(f, None, None, false)
    }

    fn parse(
        f: &'a File,
        width: Option<u32>,
        root_attribute: Option<(&str, &str)>,
        require_bundle: bool,
    ) -> Result<Self, String> {
        let root = f.tree.root_node();
        if structural_error(f, root) {
            return Err(format!("Cannot parse built CSS: {}", f.path));
        }
        let mut sheet = Self {
            evaluate_width: width.is_some(),
            document_vars: BTreeMap::new(),
            root_key: root_attribute.map(|(k, _)| k.to_owned()),
            root_value: root_attribute.map(|(_, v)| v.to_owned()),
            horizontal_block: true,
            rules: BTreeMap::new(),
            custom_rules: BTreeMap::new(),
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
        let mut stack = vec![(root, "", usize::MAX, false, None)];
        while let Some((node, selector, layer, uncertain, when)) = stack.pop() {
            let parts = children(node);
            match node.kind() {
                "import_statement" => {
                    if require_bundle {
                        return Err("Built CSS must include its imports in one bundle".into());
                    }
                }
                "media_statement" => {
                    if let [query, body] = parts.as_slice() {
                        let matches = width.and_then(|w| width_query(f.text(*query), w));
                        if width.is_some() {
                            if matches != Some(false) {
                                stack.push((
                                    *body,
                                    selector,
                                    layer,
                                    uncertain || matches.is_none(),
                                    when.clone(),
                                ));
                            }
                        } else if let Some(condition) = width_condition(f.text(*query)) {
                            let condition = merge_width_condition(when.as_ref(), condition);
                            stack.push((*body, selector, layer, uncertain, Some(condition)));
                        } else {
                            stack.push((*body, selector, layer, true, when.clone()));
                        }
                    } else {
                        stack.extend(
                            parts
                                .into_iter()
                                .rev()
                                .map(|p| (p, selector, layer, true, when.clone())),
                        );
                    }
                }
                "declaration" => {
                    let Some(prop) = parts.first().map(|p| f.text(*p)) else {
                        continue;
                    };
                    let Some((_, raw)) = f.text(node).split_once(':') else {
                        continue;
                    };
                    let important = parts.iter().any(|p| p.kind() == "important");
                    let value = raw.trim_end_matches(';').trim();
                    let value = value
                        .strip_suffix("!important")
                        .map(str::trim_end)
                        .unwrap_or(value);
                    if matches!(prop, "writing-mode" | "-webkit-writing-mode" | "all") {
                        sheet.horizontal_block = false;
                    }
                    if prop.starts_with("--") {
                        sheet.custom_rules.entry(prop).or_default().push(Rule {
                            selectors: selector,
                            layer,
                            uncertain,
                            when: when.clone(),
                            important,
                            name: prop,
                            value,
                            source: m::StyleSource {
                                syntax: m::StyleSyntax::Stylesheet,
                                text: f.text(node).into(),
                                location: f.loc(node),
                            },
                        });
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
                        if root_attribute.is_some()
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
                        } else if root_attribute.is_some() {
                            ambiguous_variables.insert(prop);
                        } else if root && !uncertain && !important {
                            sheet.vars.insert(prop, Some(value));
                        } else if selector
                            .split(',')
                            .map(str::trim)
                            .any(|s| s.starts_with(":root["))
                        {
                            ambiguous_variables.insert(prop);
                        } else {
                            sheet.vars.entry(prop).or_insert(None);
                        }
                    } else {
                        for property in affected(prop) {
                            sheet.rules.entry(property).or_default().push(Rule {
                                selectors: selector,
                                layer,
                                uncertain,
                                important,
                                when: when.clone(),
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
                        stack.push((*body, f.text(*selectors), layer, uncertain, when.clone()));
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
                        stack.push((*body, selector, rank, uncertain, when.clone()));
                    }
                }
                _ => {
                    let unknown = uncertain || !matches!(node.kind(), "stylesheet" | "block");
                    stack.extend(
                        parts
                            .into_iter()
                            .rev()
                            .map(|p| (p, selector, layer, unknown, when.clone())),
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

    /// Effective typography on the document root and body, restricted to rules the
    /// cascade can prove for the static document tree. Uncertain winners stay explicit.
    pub(crate) fn document_style(&self) -> Option<m::DocumentStyle> {
        let root = self.document_element_style(DocumentTarget::Root);
        let body = self.document_element_style(DocumentTarget::Body);
        if root.is_none() && body.is_none() {
            return None;
        }
        Some(m::DocumentStyle { root, body })
    }

    fn document_element_style(&self, target: DocumentTarget) -> Option<m::DocumentElementStyle> {
        let mut style = Vec::new();
        for property in [
            P::FontFamily,
            P::FontSize,
            P::FontWeight,
            P::LineHeight,
            P::LetterSpacing,
        ] {
            if let Some(declaration) = self.document_declaration(target, property) {
                style.push(declaration);
            }
        }
        (!style.is_empty()).then_some(m::DocumentElementStyle { style })
    }

    fn document_declaration(
        &self,
        target: DocumentTarget,
        property: P,
    ) -> Option<m::StyleDeclaration> {
        let rules = self.rules.get(&property)?;
        type Rank = (bool, usize, u32, usize);
        let mut best: Option<(Rank, &Rule<'_>)> = None;
        let mut unknown: Option<(Rank, &Rule<'_>)> = None;
        for (order, rule) in rules.iter().enumerate() {
            let mut exact = None;
            let mut maybe = None;
            for selector in &self.selectors[rule.selectors] {
                match document_selector(
                    selector.text,
                    target,
                    self.root_key.as_deref().zip(self.root_value.as_deref()),
                ) {
                    DocumentMatch::Yes(specificity) => {
                        exact = Some(exact.map_or(specificity, |s: u32| s.max(specificity)));
                    }
                    DocumentMatch::Maybe(specificity) => {
                        maybe = Some(maybe.map_or(specificity, |s: u32| s.max(specificity)));
                    }
                    DocumentMatch::No => {}
                }
            }
            if let Some(specificity) = exact {
                let rank = (false, rule.layer, specificity, order);
                if rule.uncertain || rule.important {
                    let rank = (rule.important, rule.layer, specificity, order);
                    if unknown.is_none_or(|(current, _)| rank >= current) {
                        unknown = Some((rank, rule));
                    }
                } else if best.is_none_or(|(current, _)| rank >= current) {
                    best = Some((rank, rule));
                }
            } else if let Some(specificity) = maybe {
                let rank = (rule.important, rule.layer, specificity, order);
                if unknown.is_none_or(|(current, _)| rank >= current) {
                    unknown = Some((rank, rule));
                }
            }
        }
        let uncertain = match (best, unknown) {
            (Some((exact, _)), Some((maybe, _))) => maybe >= exact,
            (None, Some(_)) => true,
            _ => false,
        };
        let rule = if uncertain { unknown?.1 } else { best?.1 };
        let value = if uncertain {
            V::Unresolved {
                text: rule.source.text.clone(),
                reason: "Cannot prove the effective document style from built CSS".into(),
                refinement: None,
            }
        } else {
            self.value(property, rule, &self.document_vars)
                .map(|value| super::export_value(value, &rule.source.text))
                .unwrap_or_else(|| V::Unresolved {
                    text: rule.source.text.clone(),
                    reason: "Cannot determine document style value from built CSS".into(),
                    refinement: None,
                })
        };
        Some(m::StyleDeclaration {
            property,
            value,
            when: None,
            from: rule.source.clone(),
        })
    }

    fn value(
        &self,
        property: P,
        rule: &Rule<'_>,
        vars: &BTreeMap<&str, Option<&str>>,
    ) -> Option<V> {
        let raw = rule.value;
        let raw = if raw.starts_with("var(") {
            resolved_var(raw, vars, 0)?
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
            return model_length(property, token, vars)
                .filter(|value| matches!(value, V::Length { .. } | V::Clamp { .. }));
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
            let trimmed = raw.trim();
            if trimmed.starts_with("calc(") && trimmed.ends_with(')') {
                let value = model_length(property, trimmed, vars)?;
                return (rule.name.starts_with("margin") || super::nonnegative_length(&value))
                    .then_some(value);
            }
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
        if property == P::BorderRadius && rule.name != "border-radius" {
            return None;
        }
        if let Some(sides) = shorthand(name, raw) {
            return sides
                .into_iter()
                .find(|(p, _)| *p == property)
                .map(|(_, v)| v);
        }
        if property == P::FontFamily {
            return (!raw.contains(',')).then(|| V::Keyword { value: raw.into() });
        }
        if matches!(
            property,
            P::Gap
                | P::RowGap
                | P::ColumnGap
                | P::PaddingTop
                | P::PaddingBottom
                | P::PaddingStart
                | P::PaddingEnd
                | P::MarginTop
                | P::MarginBottom
                | P::MarginStart
                | P::MarginEnd
                | P::Width
                | P::Height
                | P::MinWidth
                | P::MaxWidth
                | P::MinHeight
                | P::MaxHeight
                | P::BorderWidth
                | P::BorderRadius
                | P::FontSize
                | P::LineHeight
                | P::LetterSpacing
        ) && let Some(value) = model_length(property, raw, vars)
        {
            return Some(value);
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

    /// Apply with the class string already proven by the TSX expression evaluator. `None` means
    /// a dynamic class whose outcome cannot be proven, so class requirements stay uncertain.
    pub(crate) fn apply_classes(
        &self,
        file: &File,
        open: Ast<'_>,
        el: &mut m::Element,
        classes: Option<&str>,
    ) {
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
        let scoped_vars = self.scoped_vars(file, open, el, classes);
        for &(_, property) in DIRECT {
            self.apply_property(
                property,
                classes,
                el,
                file,
                open,
                document_scope,
                &scoped_vars,
            );
        }
    }

    fn scoped_vars(
        &self,
        file: &File,
        open: Ast<'_>,
        el: &m::Element,
        classes: Option<&str>,
    ) -> BTreeMap<&'a str, Option<&'a str>> {
        let mut vars = self.vars.clone();
        let mut chain = vec![(open, el.clone(), classes.map(str::to_owned))];
        let mut tag = open;
        loop {
            let owner = if tag.kind() == "jsx_opening_element" {
                tag.parent()
            } else {
                Some(tag)
            };
            let Some(parent) = owner.and_then(|n| n.parent()).and_then(|mut n| {
                while !matches!(n.kind(), "jsx_element" | "jsx_self_closing_element") {
                    n = n.parent()?;
                }
                n.child_by_field_name("open_tag")
            }) else {
                break;
            };
            let name = parent
                .child_by_field_name("name")
                .map(|n| file.text(n))
                .unwrap_or("")
                .to_owned();
            let class = parent
                .children_by_field_name("attribute", &mut parent.walk())
                .find(|a| first(*a).is_some_and(|k| file.text(k) == "className"))
                .and_then(|a| children(a).get(1).copied())
                .and_then(|v| crate::literal(file, v))
                .and_then(|v| v.as_str().map(str::to_owned));
            let ancestor = m::Element {
                id: String::new(),
                tag: name,
                module: None,
                props: BTreeMap::new(),
                style: vec![],
                unresolved_style: vec![],
                children: vec![],
                location: file.loc(parent),
            };
            chain.push((parent, ancestor, class));
            tag = parent;
        }
        chain.reverse();
        for (open, element, classes) in chain {
            let classes = classes.as_deref();
            for (name, rules) in &self.custom_rules {
                let mut best: Option<((usize, u32, usize), &str)> = None;
                for (order, rule) in rules.iter().enumerate() {
                    if rule.uncertain || rule.important || rule.when.is_some() {
                        continue;
                    }
                    for selector in &self.selectors[rule.selectors] {
                        if selector.subject.excludes(classes, &element, false) {
                            continue;
                        }
                        if let Some(Some(specificity)) =
                            contextual_specificity(selector.text, classes, &element, file, open)
                        {
                            let rank = (rule.layer, specificity, order);
                            if best.as_ref().is_none_or(|(old, _)| rank >= *old) {
                                best = Some((rank, rule.value));
                            }
                        }
                    }
                }
                if let Some((_, value)) = best {
                    vars.insert(name, Some(value));
                }
            }
        }
        vars
    }

    fn apply_property(
        &self,
        property: P,
        classes: Option<&str>,
        el: &mut m::Element,
        file: &File,
        open: Ast<'_>,
        document_scope: bool,
        scoped_vars: &BTreeMap<&'a str, Option<&'a str>>,
    ) {
        let Some(rules) = self.rules.get(&property) else {
            return;
        };
        el.style
            .retain(|d| d.property != property || d.from.syntax == m::StyleSyntax::Inline);
        let mut exact: Vec<((usize, u32, usize), &Rule<'_>)> = Vec::new();
        let mut unknown_rules = Vec::new();
        let spread = children(open).iter().any(|n| n.kind() == "jsx_expression");
        for (order, rule) in rules.iter().enumerate() {
            let mut rule_exact: Option<(usize, u32, usize)> = None;
            let mut rule_unknown = None;
            for selector in &self.selectors[rule.selectors] {
                if selector.subject.excludes(classes, el, spread) {
                    continue;
                }
                match contextual_specificity(selector.text, classes, el, file, open) {
                    Some(None) => {}
                    Some(Some(s)) if !rule.uncertain => {
                        let candidate = (rule.layer, s, order);
                        rule_exact = Some(rule_exact.map_or(candidate, |old| old.max(candidate)));
                    }
                    outcome => {
                        let weight = outcome
                            .flatten()
                            .or_else(|| simple_selector_weight(selector.text))
                            .unwrap_or(u32::MAX);
                        let candidate = (rule.important, (rule.layer, weight, order));
                        if rule_unknown.is_none_or(|r| candidate >= r) {
                            rule_unknown = Some(candidate);
                        }
                    }
                }
            }
            if let Some(candidate) = rule_exact {
                exact.push((candidate, rule));
            } else if let Some(candidate) = rule_unknown {
                unknown_rules.push((candidate, rule));
            }
        }
        let rank = exact.iter().map(|(rank, _)| *rank).max();
        // Unknown selectors can only displace a normal winner in the same or a higher layer.
        // With a proven class list, selectors whose outcome cannot be proven are almost always
        // pseudo, responsive or content-dependent variants that do not apply to the resting card.
        // Keep uncertainty only for dynamic class strings, where the base rule itself may exist.
        let mut unknown = None;
        let mut unknown_rank = None;
        for (candidate, rule) in unknown_rules {
            let contextual = self.selectors[rule.selectors]
                .iter()
                .any(|selector| contextual_selector(selector.text));
            let keep = rule.uncertain
                || (contextual && (candidate.0 || Some(candidate.1) >= rank))
                || (classes.is_none() && (candidate.0 || rank.is_none()));
            if keep && unknown_rank.is_none_or(|current| candidate >= current) {
                unknown_rank = Some(candidate);
                unknown = Some(rule);
            }
        }
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
        if unknown.is_none()
            && !self.evaluate_width
            && exact.iter().any(|(_, rule)| rule.when.is_some())
        {
            exact.sort_by_key(|(rank, _)| *rank);
            el.style.retain(|d| d.property != property);
            let opaque_inline = el
                .unresolved_style
                .iter()
                .any(|d| d.syntax == m::StyleSyntax::Inline);
            for (_, rule) in exact {
                let value = (!opaque_inline)
                    .then(|| {
                        self.value(
                            property,
                            rule,
                            if document_scope {
                                &self.document_vars
                            } else {
                                scoped_vars
                            },
                        )
                    })
                    .flatten()
                    .unwrap_or_else(|| V::Unresolved {
                        text: rule.source.text.clone(),
                        reason: "Cannot determine style value from built CSS".into(),
                        refinement: None,
                    });
                el.style.push(m::StyleDeclaration {
                    property,
                    value: super::export_value(value, &rule.source.text),
                    when: rule.when.clone(),
                    from: rule.source.clone(),
                });
            }
            return;
        }
        let Some(rule) = unknown.or_else(|| {
            exact
                .iter()
                .max_by_key(|(rank, _)| *rank)
                .map(|(_, rule)| *rule)
        }) else {
            return;
        };
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
                        scoped_vars
                    },
                )
            })
            .flatten()
            .unwrap_or_else(|| V::Unresolved {
                text: rule.source.text.clone(),
                reason: "Cannot determine style value from built CSS".into(),
                refinement: None,
            });
        el.style.push(m::StyleDeclaration {
            property,
            value: super::export_value(value, &rule.source.text),
            when: rule.when.clone(),
            from: rule.source.clone(),
        });
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DocumentTarget {
    Root,
    Body,
}

impl DocumentTarget {
    fn tag(self) -> &'static str {
        match self {
            Self::Root => "html",
            Self::Body => "body",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DocumentMatch {
    Yes(u32),
    No,
    Maybe(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Combinator {
    Descendant,
    Child,
    Adjacent,
    Sibling,
}

fn ident_end(text: &str) -> usize {
    text.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_')))
        .unwrap_or(text.len())
}

fn split_selector_list(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    for (index, c) in text.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                out.push(text[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    out.push(text[start..].trim());
    out
}

// Split a complex selector into compounds and the combinators before them. Unsupported
// nesting stays inside a compound and later becomes uncertainty rather than a wrong match.
fn selector_parts(sel: &str) -> Option<Vec<(String, Option<Combinator>)>> {
    #[derive(Debug)]
    enum Token {
        Compound(String),
        Combinator(Combinator),
    }
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut chars = sel.trim().char_indices().peekable();
    let flush = |current: &mut String, tokens: &mut Vec<Token>| {
        if !current.trim().is_empty() {
            tokens.push(Token::Compound(current.trim().to_owned()));
        }
        current.clear();
    };
    while let Some((_, c)) = chars.next() {
        match c {
            '(' | '[' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            c if depth == 0 && c.is_ascii_whitespace() => {
                flush(&mut current, &mut tokens);
                while chars.peek().is_some_and(|(_, c)| c.is_ascii_whitespace()) {
                    chars.next();
                }
                if !matches!(chars.peek(), Some((_, '>' | '+' | '~')) | None) {
                    tokens.push(Token::Combinator(Combinator::Descendant));
                }
            }
            '>' | '+' | '~' if depth == 0 => {
                flush(&mut current, &mut tokens);
                tokens.push(Token::Combinator(match c {
                    '>' => Combinator::Child,
                    '+' => Combinator::Adjacent,
                    _ => Combinator::Sibling,
                }));
                while chars.peek().is_some_and(|(_, c)| c.is_ascii_whitespace()) {
                    chars.next();
                }
            }
            _ => current.push(c),
        }
    }
    flush(&mut current, &mut tokens);
    if tokens.len() % 2 == 0 || !matches!(tokens.first(), Some(Token::Compound(_))) {
        return None;
    }
    let mut parts = Vec::new();
    let mut combinator = None;
    for token in tokens {
        match token {
            Token::Compound(text) => {
                parts.push((text, combinator.take()));
            }
            Token::Combinator(next) => {
                if combinator.is_some() {
                    return None;
                }
                combinator = Some(next);
            }
        }
    }
    (!parts.is_empty() && parts[0].1.is_none()).then_some(parts)
}

fn combine_match(left: DocumentMatch, right: DocumentMatch) -> DocumentMatch {
    use DocumentMatch as D;
    match (left, right) {
        (D::No, _) | (_, D::No) => D::No,
        (D::Yes(a), D::Yes(b)) => D::Yes(a.saturating_add(b)),
        (D::Yes(a), D::Maybe(b)) | (D::Maybe(a), D::Yes(b)) | (D::Maybe(a), D::Maybe(b)) => {
            D::Maybe(a.saturating_add(b))
        }
    }
}

fn document_attribute(
    inner: &str,
    target: DocumentTarget,
    root_attribute: Option<(&str, &str)>,
) -> DocumentMatch {
    let (name, expected) = inner
        .split_once('=')
        .map_or((inner.trim(), None), |(name, value)| {
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(value);
            (name.trim(), Some(value))
        });
    let Some((root_name, root_value)) = root_attribute else {
        return DocumentMatch::Maybe(1024);
    };
    if name != root_name {
        return DocumentMatch::Maybe(1024);
    }
    if target == DocumentTarget::Body {
        return DocumentMatch::No;
    }
    if expected.is_none_or(|value| value == root_value) {
        DocumentMatch::Yes(1024)
    } else {
        DocumentMatch::No
    }
}

fn pseudo_contents(rest: &str) -> Option<(String, Option<&str>, usize)> {
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    let name = rest[..end].to_owned();
    let tail = &rest[end..];
    if !tail.starts_with('(') {
        return Some((name, None, end));
    }
    let mut depth = 0usize;
    for (index, c) in tail.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some((name, Some(&tail[1..index]), end + index + 1));
                }
            }
            _ => {}
        }
    }
    None
}

fn document_compound(
    raw: &str,
    target: DocumentTarget,
    root_attribute: Option<(&str, &str)>,
) -> DocumentMatch {
    let tag = target.tag();
    let mut rest = raw.trim();
    let mut specificity = 0u32;
    let mut certain = true;
    let mut anchored = false;
    if rest.starts_with('*') {
        anchored = true;
        rest = &rest[1..];
    } else {
        let end = ident_end(rest);
        if end > 0 && rest.as_bytes()[0].is_ascii_alphabetic() {
            let name = &rest[..end];
            if !name.eq_ignore_ascii_case(tag) {
                return DocumentMatch::No;
            }
            anchored = true;
            specificity = specificity.saturating_add(1);
            rest = &rest[end..];
        }
    }
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('.') {
            let end = ident_end(after);
            if end == 0 {
                return DocumentMatch::Maybe(u32::MAX);
            }
            specificity = specificity.saturating_add(1024);
            certain = false;
            rest = &after[end..];
        } else if let Some(after) = rest.strip_prefix('#') {
            let end = ident_end(after);
            if end == 0 {
                return DocumentMatch::Maybe(u32::MAX);
            }
            specificity = specificity.saturating_add(1_048_576);
            certain = false;
            rest = &after[end..];
        } else if rest.starts_with('[') {
            let Some(end) = rest.find(']') else {
                return DocumentMatch::Maybe(u32::MAX);
            };
            match document_attribute(&rest[1..end], target, root_attribute) {
                DocumentMatch::Yes(weight) => specificity = specificity.saturating_add(weight),
                DocumentMatch::No => return DocumentMatch::No,
                DocumentMatch::Maybe(weight) => {
                    specificity = specificity.saturating_add(weight);
                    certain = false;
                }
            }
            rest = &rest[end + 1..];
        } else if rest.starts_with("::") {
            return DocumentMatch::No;
        } else if let Some(after) = rest.strip_prefix(':') {
            let Some((name, contents, consumed)) = pseudo_contents(after) else {
                return DocumentMatch::Maybe(u32::MAX);
            };
            match (name.as_str(), contents) {
                ("root", None) => {
                    specificity = specificity.saturating_add(1024);
                    if target == DocumentTarget::Body {
                        return DocumentMatch::No;
                    }
                    anchored = true;
                }
                ("is" | "where", Some(list)) => {
                    let mut matched = None;
                    for branch in split_selector_list(list) {
                        let branch = if selector_parts(branch).is_some_and(|parts| parts.len() == 1)
                        {
                            document_compound(branch, target, root_attribute)
                        } else {
                            DocumentMatch::Maybe(1024)
                        };
                        let branch = if name == "where" {
                            match branch {
                                DocumentMatch::Yes(_) => DocumentMatch::Yes(0),
                                DocumentMatch::Maybe(_) => DocumentMatch::Maybe(0),
                                DocumentMatch::No => DocumentMatch::No,
                            }
                        } else {
                            branch
                        };
                        matched = Some(match (matched, branch) {
                            (None, branch) => branch,
                            (Some(DocumentMatch::Yes(a)), DocumentMatch::Yes(b)) => {
                                DocumentMatch::Yes(a.max(b))
                            }
                            (Some(DocumentMatch::Yes(a)), _) => DocumentMatch::Yes(a),
                            (_, DocumentMatch::Yes(b)) => DocumentMatch::Yes(b),
                            (Some(DocumentMatch::Maybe(a)), DocumentMatch::Maybe(b)) => {
                                DocumentMatch::Maybe(a.max(b))
                            }
                            (Some(DocumentMatch::Maybe(a)), DocumentMatch::No)
                            | (Some(DocumentMatch::No), DocumentMatch::Maybe(a)) => {
                                DocumentMatch::Maybe(a)
                            }
                            (Some(DocumentMatch::No), DocumentMatch::No) => DocumentMatch::No,
                        });
                    }
                    match matched.unwrap_or(DocumentMatch::No) {
                        DocumentMatch::Yes(weight) => {
                            specificity = specificity.saturating_add(weight)
                        }
                        DocumentMatch::No => return DocumentMatch::No,
                        DocumentMatch::Maybe(weight) => {
                            specificity = specificity.saturating_add(weight);
                            certain = false;
                        }
                    }
                }
                ("not", Some(_)) => {
                    specificity = specificity.saturating_add(1024);
                    certain = false;
                }
                _ => {
                    specificity = specificity.saturating_add(1024);
                    certain = false;
                }
            }
            rest = &after[consumed..];
        } else {
            return DocumentMatch::Maybe(u32::MAX);
        }
    }
    if !anchored {
        return DocumentMatch::No;
    }
    if certain {
        DocumentMatch::Yes(specificity)
    } else {
        DocumentMatch::Maybe(specificity)
    }
}

fn document_selector(
    sel: &str,
    target: DocumentTarget,
    root_attribute: Option<(&str, &str)>,
) -> DocumentMatch {
    let Some(parts) = selector_parts(sel) else {
        return DocumentMatch::Maybe(u32::MAX);
    };
    let Some((subject_text, _)) = parts.last() else {
        return DocumentMatch::Maybe(u32::MAX);
    };
    let mut result = document_compound(subject_text, target, root_attribute);
    if result == DocumentMatch::No || parts.len() == 1 {
        return result;
    }
    if target == DocumentTarget::Root {
        // The document root has no parent or sibling, so any left-hand requirement is impossible.
        return DocumentMatch::No;
    }
    let ancestors = &parts[..parts.len() - 1];
    let relationship = parts.last().and_then(|(_, combinator)| *combinator);
    if ancestors.len() != 1
        || matches!(
            relationship,
            Some(Combinator::Adjacent | Combinator::Sibling)
        )
    {
        return DocumentMatch::Maybe(u32::MAX);
    }
    let ancestor = document_compound(&ancestors[0].0, DocumentTarget::Root, root_attribute);
    result = combine_match(result, ancestor);
    if matches!(
        relationship,
        Some(Combinator::Child | Combinator::Descendant)
    ) {
        result
    } else {
        DocumentMatch::Maybe(u32::MAX)
    }
}

fn contextual_selector(sel: &str) -> bool {
    let mut depth = 0usize;
    let mut escaped = false;
    for c in sel.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            '>' | '+' | '~' if depth == 0 => return true,
            c if c.is_ascii_whitespace() && depth == 0 => return true,
            _ => {}
        }
    }
    false
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
        if matched
            && match edge {
                Some(first) => edge_child(file, parent, first)?,
                None => true,
            }
        {
            return Some(Some(
                target_weight
                    + if class.is_some() { 1024 } else { 1 }
                    + if edge.is_some() { 1024 } else { 0 },
            ));
        }
        if direct || (name == "body" && class.is_none() && ancestor != "html") {
            return Some(None);
        }
        parent = parent.parent()?;
    }
}

fn css_ident(raw: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let escaped = chars.next()?;
            if !escaped.is_ascii_punctuation() {
                return None;
            }
            out.push(escaped);
        } else {
            out.push(c);
        }
    }
    (!out.is_empty()).then_some(out)
}

/// Match one compound selector (no combinators). This covers the common stylesheet form
/// `.a.b`, `tag.a`, `#id.a` and data attributes without treating their conjunction as unknown.
fn unescaped_end(text: &str) -> usize {
    let mut escaped = false;
    for (index, c) in text.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if matches!(c, '.' | '#' | '[') {
            return index;
        }
    }
    text.len()
}

fn compound_specificity(sel: &str, classes: Option<&str>, el: &m::Element) -> Option<Option<u32>> {
    if sel.is_empty() || sel.contains([' ', '>', '+', '~', ':']) {
        return None;
    }
    let mut rest = sel;
    let mut weight = 0u32;
    let mut required_class = Vec::new();
    if let Some(end) = rest.find(['.', '#', '[']) {
        let tag = &rest[..end];
        if !tag.is_empty() {
            if tag == "*" {
                // Universal selector.
            } else if ident(tag) && tag.eq_ignore_ascii_case(&el.tag) {
                weight += 1;
            } else if ident(tag) {
                return Some(None);
            } else {
                return None;
            }
        }
        rest = &rest[end..];
    } else if !rest.is_empty() {
        if rest == "*" {
            return Some(Some(0));
        }
        if !ident(rest) {
            return None;
        }
        return Some(rest.eq_ignore_ascii_case(&el.tag).then_some(1));
    }
    while !rest.is_empty() {
        if let Some(class) = rest.strip_prefix('.') {
            let end = unescaped_end(class);
            let raw = &class[..end];
            if raw.is_empty() {
                return None;
            }
            required_class.push(css_ident(raw)?);
            weight += 1024;
            rest = &class[end..];
        } else if let Some(id) = rest.strip_prefix('#') {
            let end = unescaped_end(id);
            let raw = &id[..end];
            if !ident(raw) {
                return None;
            }
            let Some(m::Expr::Literal { value }) = el.props.get("id") else {
                return Some(None);
            };
            if value.as_str() != Some(raw) {
                return Some(None);
            }
            weight += 1024 * 1024;
            rest = &id[end..];
        } else if let Some(attribute) = rest.strip_prefix('[') {
            let end = attribute.find(']')?;
            let name = &attribute[..end];
            if !name.starts_with("data-") || !ident(name) {
                return None;
            }
            let present = match el.props.get(name) {
                None => false,
                Some(m::Expr::Literal { value }) => !value.is_null(),
                Some(_) => return None,
            };
            if !present {
                return Some(None);
            }
            weight += 1024;
            rest = &attribute[end + 1..];
        } else {
            return None;
        }
    }
    if required_class.is_empty() {
        return Some(Some(weight));
    }
    let classes = classes?;
    let matched = required_class.iter().all(|class| {
        classes
            .split_ascii_whitespace()
            .any(|actual| actual == class)
    });
    Some(matched.then_some(weight))
}

// Outer None means the selector may match, but its outcome is unknown.
fn specificity(sel: &str, classes: Option<&str>, el: &m::Element) -> Option<Option<u32>> {
    if let Some(result) = compound_specificity(sel, classes, el) {
        return Some(result);
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
