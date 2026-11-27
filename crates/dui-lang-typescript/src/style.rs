//! Map static style syntax and retain the source of each unsupported declaration.
use crate::syntax::{File, children, field, first, unwrap_expr};
use dui_core::source_model::{self as m, StyleProperty as P, StyleValue as V};
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
pub fn theme(f: &File) -> Vec<m::Token> {
    let mut tokens = std::collections::BTreeMap::new();
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
            let [name, value] = parts.as_slice() else {
                continue;
            };
            let name = f.text(*name);
            let val = f.text(*value);
            let group = if name.starts_with("--color-") {
                m::TokenGroup::Color
            } else if name.starts_with("--radius-") {
                m::TokenGroup::Radius
            } else {
                continue;
            };
            let value = (if group == m::TokenGroup::Color {
                color(val)
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
    tokens.into_values().collect()
}
const PADDING: &[P] = &[
    P::PaddingTop,
    P::PaddingEnd,
    P::PaddingBottom,
    P::PaddingStart,
];
fn classes(s: &str, tokens: &[m::Token]) -> Option<(Vec<P>, V)> {
    let (prop, keyword) = match s {
        "flex" | "flex-row" => (P::Direction, "row"),
        "flex-col" => (P::Direction, "column"),
        "items-center" => (P::AlignItems, "center"),
        "justify-between" => (P::JustifyContent, "spaceBetween"),
        _ => {
            let (prefix, key) = s.split_once('-')?;
            let props: &[P] = match prefix {
                "gap" => &[P::Gap],
                "p" => PADDING,
                "px" => &[P::PaddingStart, P::PaddingEnd],
                "py" => &[P::PaddingTop, P::PaddingBottom],
                _ => &[],
            };
            if !props.is_empty() {
                let step = key.parse::<u16>().ok()?;
                return Some((
                    props.into(),
                    V::Length {
                        value: f64::from(step) / 4.,
                        unit: m::LengthUnit::Rem,
                    },
                ));
            }
            let (prop, group) = match prefix {
                "text" => (P::Color, "color"),
                "bg" => (P::Background, "color"),
                "rounded" => (P::BorderRadius, "radius"),
                _ => return None,
            };
            let name = format!("--{group}-{key}");
            let mut matches = tokens.iter().filter(|t| t.name == name);
            let token = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            return Some((
                vec![prop],
                V::Token {
                    token: token.id.clone(),
                    resolved: Some(Box::new(token.value.clone())),
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
fn inline(f: &File, pair: Ast<'_>) -> Option<(Vec<P>, V)> {
    let name = f.name(pair, "key")?;
    let raw = field(pair, "value")?;
    let val = crate::literal(f, raw)?;
    let props = match name {
        "padding" => PADDING.to_vec(),
        "gap" => vec![P::Gap],
        "width" => vec![P::Width],
        "height" => vec![P::Height],
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
                record(f.text(pair), f.loc(pair), inline(f, pair))
            }
            return;
        }
    }
    record(f.text(attr), f.loc(attr), None)
}
