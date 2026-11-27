//! Map static style syntax and retain the source of each unsupported declaration.
pub(crate) mod built;
pub(crate) mod modules;
use crate::syntax::{File, children, field, first, unwrap_expr};
use dui_core::source_model::{self as m, StyleProperty as P, StyleValue as V};
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::Node as Ast;

fn color(s: &str) -> Option<V> {
    let h = s.strip_prefix('#')?;
    if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let hex = match h.len() {
        3 | 4 => h.chars().flat_map(|c| [c, c]).collect(),
        6 | 8 => h.to_owned(),
        _ => return None,
    };
    Some(V::Color {
        value: format!("#{}", hex.to_ascii_lowercase()),
    })
}
fn length(s: &str) -> Option<V> {
    for (suffix, unit) in [
        ("rem", m::LengthUnit::Rem),
        ("em", m::LengthUnit::Em),
        ("px", m::LengthUnit::Px),
        ("%", m::LengthUnit::Percent),
    ] {
        if let Some(n) = s
            .strip_suffix(suffix)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|n| n.is_finite())
        {
            return Some(V::Length { value: n, unit });
        }
    }
    None
}
fn letter_spacing(raw: &str) -> Option<V> {
    let raw = raw.trim();
    if raw.parse::<f64>().ok() == Some(0.) {
        return Some(V::Length {
            value: 0.,
            unit: m::LengthUnit::Px,
        });
    }
    length(raw)
}

fn line_height(raw: &str) -> Option<V> {
    let raw = raw.trim();
    if let Ok(value) = raw.parse::<f64>() {
        return (value.is_finite() && value >= 0.).then_some(V::Number { value });
    }
    length(raw).filter(|v| matches!(v,V::Length{value,..} if *value>=0.))
}

fn size_property(name: &str) -> Option<P> {
    Some(match name {
        "width" => P::Width,
        "height" => P::Height,
        "border-width" => P::BorderWidth,
        "min-width" => P::MinWidth,
        "max-width" => P::MaxWidth,
        "min-height" => P::MinHeight,
        "max-height" => P::MaxHeight,
        _ => return None,
    })
}
fn size_value(property: P, raw: &str) -> Option<V> {
    let raw = raw.trim();
    if property == P::BorderWidth {
        return uniform_length(raw, 4, false);
    }
    if raw.eq_ignore_ascii_case("fit-content") && matches!(property, P::Width | P::Height) {
        return Some(V::Keyword {
            value: "fit".into(),
        });
    }
    if raw.eq_ignore_ascii_case("auto") && !matches!(property, P::MaxWidth | P::MaxHeight) {
        return Some(V::Keyword {
            value: "auto".into(),
        });
    }
    if raw.parse::<f64>().ok() == Some(0.) {
        return Some(V::Length {
            value: 0.,
            unit: m::LengthUnit::Px,
        });
    }
    length(raw).filter(|v| matches!(v,V::Length{value,..} if *value >= 0.))
}
fn grow_factor(raw: &str) -> Option<V> {
    raw.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 0.)
        .map(|value| V::Number { value })
}

// A scalar model property can preserve shorthand only if all sides agree.
fn uniform_length(raw: &str, max_values: usize, allow_percent: bool) -> Option<V> {
    let parse = |word: &str| {
        if word.parse::<f64>().ok() == Some(0.) {
            return Some(V::Length {
                value: 0.,
                unit: m::LengthUnit::Px,
            });
        }
        length(word).filter(|v| {
            matches!(v, V::Length { value, unit }
            if *value >= 0. && (allow_percent || *unit != m::LengthUnit::Percent))
        })
    };
    let mut words = raw.split_ascii_whitespace();
    let first = parse(words.next()?)?;
    for (index, word) in words.enumerate() {
        if index + 1 >= max_values || parse(word)? != first {
            return None;
        }
    }
    Some(first)
}

fn uniform_gap(raw: &str) -> Option<V> {
    uniform_length(raw, 2, true)
}

pub fn theme(f: &File) -> Vec<m::Token> {
    let mut tokens = BTreeMap::new();
    for rule in children(f.tree.root_node()) {
        let parts = children(rule);
        if parts.first().is_none_or(|n| f.text(*n) != "@theme") {
            continue;
        }
        let Some(block) = parts.iter().find(|n| n.kind() == "block") else {
            continue;
        };
        for decl in children(*block)
            .into_iter()
            .filter(|n| n.kind() == "declaration")
        {
            let parts = children(decl);
            let [name, value, rest @ ..] = parts.as_slice() else {
                continue;
            };
            let name = f.text(*name);
            let val = &f.src[value.start_byte()..rest.last().unwrap_or(value).end_byte()];
            let group = if name.starts_with("--color-") {
                m::TokenGroup::Color
            } else if name == "--spacing" || name.starts_with("--spacing-") {
                m::TokenGroup::Spacing
            } else if name.starts_with("--radius-") {
                m::TokenGroup::Radius
            } else {
                continue;
            };
            let value = (if group == m::TokenGroup::Color {
                color(val)
            } else if group == m::TokenGroup::Spacing && val.parse::<f64>().ok() == Some(0.) {
                Some(V::Length {
                    value: 0.,
                    unit: m::LengthUnit::Px,
                })
            } else {
                length(val)
            })
            .unwrap_or_else(|| V::Unresolved {
                text: val.into(),
                reason: "Unsupported theme value".into(),
            });
            tokens.insert(
                name.to_owned(),
                m::Token {
                    id: format!("{}#{name}", f.path),
                    name: name.into(),
                    group,
                    value,
                    modes: Default::default(),
                    syntax: m::StyleSyntax::Theme,
                    location: Some(f.loc(decl)),
                    package: None,
                },
            );
        }
    }
    resolve_aliases(&mut tokens);
    tokens.into_values().collect()
}

// Resolve only single-argument var() references inside one parsed @theme stylesheet.
// Cache the terminal value, but keep the immediate target ID for consumers editing tokens.
fn resolve_aliases(tokens: &mut BTreeMap<String, m::Token>) {
    let mut links = BTreeMap::new();
    for (name, token) in tokens.iter() {
        let V::Unresolved { text, .. } = &token.value else {
            continue;
        };
        let Some(target) = text
            .strip_prefix("var(")
            .and_then(|s| s.strip_suffix(')'))
            .map(str::trim)
        else {
            continue;
        };
        if let Some(other) = tokens.get(target)
            && other.group == token.group
        {
            links.insert(name.clone(), target.to_owned());
        }
    }
    let mut resolved: BTreeMap<String, Option<V>> = BTreeMap::new();
    for start in links.keys() {
        let mut seen = BTreeSet::new();
        let mut current = start;
        let terminal = loop {
            if let Some(cached) = resolved.get(current) {
                break cached.clone();
            }
            let Some(next) = links.get(current) else {
                break tokens.get(current).and_then(|t| match &t.value {
                    v @ (V::Color { .. } | V::Length { .. }) => Some(v.clone()),
                    V::Number { .. }
                    | V::Keyword { .. }
                    | V::Token { .. }
                    | V::Unresolved { .. } => None,
                });
            };
            if !seen.insert(current.clone()) {
                break None;
            }
            current = next;
        };
        for name in seen {
            resolved.insert(name, terminal.clone());
        }
    }
    for (name, target) in links {
        if let Some(Some(value)) = resolved.remove(&name) {
            let id = tokens[&target].id.clone();
            if let Some(token) = tokens.get_mut(&name) {
                token.value = V::Token {
                    token: id,
                    resolved: Some(Box::new(value)),
                };
            }
        }
    }
}
const PADDING: &[P] = &[
    P::PaddingTop,
    P::PaddingEnd,
    P::PaddingBottom,
    P::PaddingStart,
];
// Both spellings use CSS property names here. Start/end stay logical, including in RTL.
fn spacing(name: &str) -> Option<&'static [P]> {
    Some(match name {
        "padding" => PADDING,
        "padding-inline" => &[P::PaddingStart, P::PaddingEnd],
        "padding-top" => &[P::PaddingTop],
        "padding-bottom" => &[P::PaddingBottom],
        "padding-inline-start" => &[P::PaddingStart],
        "padding-inline-end" => &[P::PaddingEnd],
        "margin" => &[P::MarginTop, P::MarginEnd, P::MarginBottom, P::MarginStart],
        "margin-inline" => &[P::MarginStart, P::MarginEnd],
        "margin-top" => &[P::MarginTop],
        "margin-bottom" => &[P::MarginBottom],
        "margin-inline-start" => &[P::MarginStart],
        "margin-inline-end" => &[P::MarginEnd],
        _ => return None,
    })
}

fn spacing_value(name: &str, raw: &str) -> Option<V> {
    if name.starts_with("margin") && raw == "auto" {
        return Some(V::Keyword {
            value: "auto".into(),
        });
    }
    let value = if raw == "0" {
        V::Length {
            value: 0.,
            unit: m::LengthUnit::Px,
        }
    } else {
        length(raw)?
    };
    if name.starts_with("padding") && matches!(value,V::Length{value,..} if value<0.) {
        return None;
    }
    Some(value)
}

// Physical left/right only have a direction-independent logical mapping when equal.
fn shorthand(name: &str, raw: &str) -> Option<Vec<(P, V)>> {
    if matches!(name, "padding-inline" | "margin-inline") {
        let mut words = raw.split_ascii_whitespace();
        let start = spacing_value(name, words.next()?)?;
        let end = match words.next() {
            Some(word) => spacing_value(name, word)?,
            None => start.clone(),
        };
        if words.next().is_some() {
            return None;
        }
        return Some(spacing(name)?.iter().copied().zip([start, end]).collect());
    }
    if !matches!(name, "margin" | "padding") {
        return None;
    }
    let values: Vec<_> = raw
        .split_ascii_whitespace()
        .map(|word| spacing_value(name, word))
        .collect::<Option<_>>()?;
    let sides = match values.as_slice() {
        [vertical, horizontal] => [vertical, horizontal, vertical, horizontal],
        [top, horizontal, bottom] => [top, horizontal, bottom, horizontal],
        [top, right, bottom, left] if right == left => [top, right, bottom, left],
        _ => return None,
    };
    Some(
        spacing(name)?
            .iter()
            .copied()
            .zip(sides.into_iter().cloned())
            .collect(),
    )
}
fn scaled_spacing(class: &str, step: u16, tokens: &[m::Token]) -> V {
    let mut bases = tokens.iter().filter(|token| token.name == "--spacing");
    let scale = f64::from(step);
    let Some(base) = bases.next() else {
        return V::Length {
            value: scale / 4.,
            unit: m::LengthUnit::Rem,
        };
    };
    let length = if let V::Token { resolved, .. } = &base.value {
        resolved.as_deref()
    } else {
        Some(&base.value)
    };
    if bases.next().is_none()
        && let Some(V::Length { value, unit }) = length
        && (value * scale).is_finite()
    {
        return V::Length {
            value: value * scale,
            unit: *unit,
        };
    }
    V::Unresolved {
        text: class.into(),
        reason: "Cannot resolve the --spacing scale".into(),
    }
}

fn classes(s: &str, tokens: &[m::Token]) -> Option<(Vec<P>, V)> {
    let (prop, keyword) = match s {
        "flex" | "flex-row" => (P::Direction, "row"),
        "flex-col" => (P::Direction, "column"),
        "items-center" => (P::AlignItems, "center"),
        "justify-between" => (P::JustifyContent, "spaceBetween"),
        _ => {
            let (prefix, key) = s.split_once('-')?;
            let (props, group): (&[P], &str) = match prefix {
                "gap" => (&[P::Gap], "spacing"),
                "p" => (PADDING, "spacing"),
                "px" => (&[P::PaddingStart, P::PaddingEnd], "spacing"),
                "py" => (&[P::PaddingTop, P::PaddingBottom], "spacing"),
                "text" => (&[P::Color], "color"),
                "bg" => (&[P::Background], "color"),
                "rounded" => (&[P::BorderRadius], "radius"),
                _ => return None,
            };
            let name = format!("--{group}-{key}");
            let mut matches = tokens.iter().filter(|t| t.name == name);
            let Some(token) = matches.next() else {
                if group != "spacing" {
                    return None;
                }
                let step = key.parse::<u16>().ok()?;
                return Some((props.into(), scaled_spacing(s, step, tokens)));
            };
            if matches.next().is_some() {
                return None;
            }
            return Some((
                props.into(),
                V::Token {
                    token: token.id.clone(),
                    resolved: match &token.value {
                        V::Token { resolved, .. } => resolved.clone(),
                        V::Unresolved { .. } => None,
                        value @ (V::Length { .. }
                        | V::Color { .. }
                        | V::Number { .. }
                        | V::Keyword { .. }) => Some(Box::new(value.clone())),
                    },
                },
            ));
        }
    };
    Some((
        vec![prop],
        V::Keyword {
            value: keyword.into(),
        },
    ))
}
fn flex_keyword(name: &str, literal: Option<&str>, source: &str) -> Option<(Vec<P>, V)> {
    let property = match name {
        "align-items" => P::AlignItems,
        "justify-content" => P::JustifyContent,
        "flex-direction" => P::Direction,
        "flex-wrap" => P::Wrap,
        _ => return None,
    };
    let value = literal.map(str::trim).map(str::to_ascii_lowercase);
    let keyword = match (name, value.as_deref()) {
        ("align-items" | "justify-content", Some("start" | "flex-start")) => Some("start"),
        ("align-items" | "justify-content", Some("end" | "flex-end")) => Some("end"),
        ("align-items" | "justify-content", Some("center")) => Some("center"),
        ("align-items", Some(v @ ("stretch" | "baseline"))) => Some(v),
        ("justify-content", Some("space-between")) => Some("spaceBetween"),
        ("justify-content", Some("space-around")) => Some("spaceAround"),
        ("justify-content", Some("space-evenly")) => Some("spaceEvenly"),
        ("flex-direction", Some(v @ ("row" | "column"))) => Some(v),
        ("flex-wrap", Some(v @ ("wrap" | "nowrap"))) => Some(v),
        _ => None,
    };
    let value = match keyword {
        Some(v) => V::Keyword { value: v.into() },
        None => V::Unresolved {
            text: source.into(),
            reason: "Unsupported flex layout value".into(),
        },
    };
    Some((vec![property], value))
}

fn css_property_name(name: &str) -> String {
    let mut css_name = String::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            css_name.push('-');
            css_name.push(c.to_ascii_lowercase());
        } else {
            css_name.push(c);
        }
    }
    css_name
}
fn inline(f: &File, pair: Ast<'_>) -> Option<(Vec<P>, V)> {
    let name = f.name(pair, "key")?;
    let raw = field(pair, "value")?;
    if name == "flexGrow" {
        let source = f.text(raw);
        let value = match unwrap_expr(raw).kind() {
            "string" => crate::literal(f, raw).and_then(|v| grow_factor(v.as_str()?)),
            "number" | "unary_expression" => grow_factor(f.text(unwrap_expr(raw))),
            _ => None,
        }
        .unwrap_or_else(|| V::Unresolved {
            text: source.into(),
            reason: "Expected a nonnegative flex grow factor".into(),
        });
        return Some((vec![P::Grow], value));
    }
    // React spells standard CSS properties in camelCase and numeric lengths in logical pixels.
    let css_name = css_property_name(name);
    if let Some(props) = spacing(&css_name) {
        let value = match crate::literal(f, raw) {
            Some(serde_json::Value::String(text)) => spacing_value(&css_name, &text)?,
            _ if matches!(raw.kind(), "number" | "unary_expression") => {
                let n = f.text(raw).parse::<f64>().ok().filter(|v| v.is_finite())?;
                if css_name.starts_with("padding") && n < 0. {
                    return None;
                }
                V::Length {
                    value: n,
                    unit: m::LengthUnit::Px,
                }
            }
            _ => return None,
        };
        return Some((props.to_vec(), value));
    }
    let literal = crate::literal(f, raw);
    if let Some(property) = size_property(&css_name) {
        let value = literal
            .as_ref()
            .and_then(|v| {
                if let Some(n) = v.as_f64().filter(|n| *n >= 0.) {
                    Some(V::Length {
                        value: n,
                        unit: m::LengthUnit::Px,
                    })
                } else {
                    size_value(property, v.as_str()?)
                }
            })
            .unwrap_or_else(|| V::Unresolved {
                text: f.text(raw).into(),
                reason: "Unsupported size value".into(),
            });
        return Some((vec![property], value));
    }
    if let Some(mapped) = flex_keyword(
        &css_name,
        literal.as_ref().and_then(|v| v.as_str()),
        f.text(raw),
    ) {
        return Some(mapped);
    }
    if name == "letterSpacing" {
        let expr = unwrap_expr(raw);
        let value = match expr.kind() {
            "number" | "unary_expression" => f
                .text(expr)
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite())
                .map(|value| V::Length {
                    value,
                    unit: m::LengthUnit::Px,
                }),
            _ => literal
                .as_ref()
                .and_then(|v| v.as_str())
                .and_then(letter_spacing),
        }
        .unwrap_or_else(|| V::Unresolved {
            text: f.text(raw).into(),
            reason: "Unsupported letter spacing".into(),
        });
        return Some((vec![P::LetterSpacing], value));
    }
    if name == "lineHeight" {
        let value = line_height(
            literal
                .as_ref()
                .and_then(|v| v.as_str())
                .unwrap_or(f.text(raw)),
        )
        .unwrap_or_else(|| V::Unresolved {
            text: f.text(raw).into(),
            reason: "Unsupported line height".into(),
        });
        return Some((vec![P::LineHeight], value));
    }
    let val = literal?;
    let props = match name {
        "gap" => {
            let value = if let Some(n) = val.as_f64().filter(|n| *n >= 0.) {
                V::Length {
                    value: n,
                    unit: m::LengthUnit::Px,
                }
            } else {
                uniform_gap(val.as_str()?)?
            };
            return Some((vec![P::Gap], value));
        }
        "borderRadius" => vec![P::BorderRadius],
        "fontSize" => vec![P::FontSize],
        "color" | "backgroundColor" => {
            return Some((
                vec![if name == "color" {
                    P::Color
                } else {
                    P::Background
                }],
                color(val.as_str()?)?,
            ));
        }
        "order" => {
            let n = val
                .as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .or_else(|| val.as_str()?.trim().parse::<i32>().ok())?;
            return Some((
                vec![P::Order],
                V::Number {
                    value: f64::from(n),
                },
            ));
        }
        "fontWeight" | "opacity" => {
            return Some((
                vec![if name == "opacity" {
                    P::Opacity
                } else {
                    P::FontWeight
                }],
                V::Number {
                    value: val.as_f64()?,
                },
            ));
        }
        _ => return None,
    };
    let value = if let Some(n) = val.as_f64() {
        V::Length {
            value: n,
            unit: m::LengthUnit::Px,
        }
    } else {
        length(val.as_str()?)?
    };
    Some((props, value))
}
pub fn attribute(
    f: &File,
    attr: Ast<'_>,
    tokens: &[m::Token],
    tailwind: bool,
    element: &mut m::Element,
) {
    let parts = children(attr);
    let class = parts.first().is_some_and(|n| f.text(*n) == "className");
    let syntax = if class {
        m::StyleSyntax::Class
    } else {
        m::StyleSyntax::Inline
    };
    let mut record = |text: &str, location: m::Location, mapped: Option<(Vec<P>, V)>| {
        if let Some((props, value)) = mapped {
            for property in props {
                element.style.push(m::StyleDeclaration {
                    property,
                    value: value.clone(),
                    when: None,
                    from: m::StyleSource {
                        syntax,
                        text: text.into(),
                        location: location.clone(),
                    },
                })
            }
        } else {
            element.unresolved_style.push(m::UnresolvedStyle {
                syntax,
                text: text.into(),
                reason: "Unsupported style syntax".into(),
                location,
            })
        }
    };
    if let Some(value) = parts.get(1).copied() {
        if class && value.kind() == "string" {
            let quoted = f.text(value);
            let mut at = 1;
            for word in quoted[1..quoted.len() - 1].split_whitespace() {
                let offset = at + quoted[at..].find(word).unwrap_or(0);
                let mut loc = f.loc(value);
                let before = &quoted[..offset];
                let lines = before.matches('\n').count() as u32;
                if let Some((_, last)) = before.rsplit_once('\n') {
                    loc.line += lines;
                    loc.column = 1 + last.chars().count() as u32
                } else {
                    loc.column += before.chars().count() as u32
                }
                loc.start += offset as u32;
                loc.end = loc.start + word.len() as u32;
                record(
                    word,
                    loc,
                    if tailwind {
                        classes(word, tokens)
                    } else {
                        None
                    },
                );
                at = offset + word.len();
            }
            return;
        }
        if !class && first(value).is_some() && unwrap_expr(value).kind() == "object" {
            for pair in children(unwrap_expr(value)) {
                let expanded = f.name(pair, "key").and_then(|name| {
                    let val = crate::literal(f, field(pair, "value")?)?;
                    shorthand(&css_property_name(name), val.as_str()?)
                });
                if let Some(sides) = expanded {
                    for (property, value) in sides {
                        record(f.text(pair), f.loc(pair), Some((vec![property], value)))
                    }
                } else {
                    record(f.text(pair), f.loc(pair), inline(f, pair))
                }
            }
            return;
        }
    }
    record(f.text(attr), f.loc(attr), None)
}
