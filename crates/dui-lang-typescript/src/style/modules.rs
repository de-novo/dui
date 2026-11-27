//! A bounded cascade for statically referenced CSS Modules. Parse each module once.
//! If its selector or at-rule structure exceeds the supported subset, keep the JSX
//! reference unresolved instead of claiming an incomplete cascade.
use super::{P, V, color, length, spacing, spacing_value};
use crate::{
    Files,
    syntax::{File, children, field, unwrap_expr},
};
use dui_core::source_model as m;
use std::collections::BTreeMap;
use tree_sitter::Node as Ast;

struct Rule {
    tag: Option<String>,
    declarations: Vec<m::StyleDeclaration>,
    unresolved: Vec<m::UnresolvedStyle>,
}
type Classes = BTreeMap<String, Vec<Rule>>;
pub(crate) struct Modules(BTreeMap<String, Classes>);

pub(crate) struct Bindings<'a>(BTreeMap<&'a str, &'a Classes>);

impl Modules {
    pub(crate) fn new(files: &Files) -> Self {
        let mut modules = BTreeMap::new();
        for f in files.values().filter(|f| f.path.ends_with(".module.css")) {
            let mut classes = Classes::new();
            if !f.tree.root_node().has_error()
                && rules(f, f.tree.root_node(), None, &mut classes).is_some()
            {
                // Stable sort preserves stylesheet order within a specificity level.
                for entries in classes.values_mut() {
                    entries.sort_by_key(|r| r.tag.is_some());
                }
                modules.insert(f.path.clone(), classes);
            }
        }
        Self(modules)
    }

    pub(crate) fn bindings<'a>(&'a self, file: &'a File, function: Ast<'_>) -> Bindings<'a> {
        let mut bindings: BTreeMap<_, _> = file
            .bindings
            .iter()
            .filter(|(_, (_, export))| export == "default")
            .filter_map(|(name, (path, _))| Some((name.as_str(), self.0.get(path)?)))
            .collect();
        // Conservatively drop an import if a nested binding could shadow it.
        let mut pending = vec![function];
        while !bindings.is_empty()
            && let Some(n) = pending.pop()
        {
            let pattern = match n.kind() {
                "variable_declarator" | "function_declaration" | "class_declaration" => {
                    field(n, "name")
                }
                "required_parameter" | "optional_parameter" => {
                    field(n, "pattern").or_else(|| field(n, "name"))
                }
                "arrow_function" | "catch_clause" => field(n, "parameter"),
                _ => None,
            };
            if let Some(pattern) = pattern {
                let mut names = vec![pattern];
                while let Some(p) = names.pop() {
                    if matches!(
                        p.kind(),
                        "identifier" | "shorthand_property_identifier_pattern"
                    ) {
                        bindings.remove(file.text(p));
                    } else {
                        names.extend(children(p));
                    }
                }
            }
            pending.extend(children(n));
        }
        Bindings(bindings)
    }
}

impl Bindings<'_> {
    pub(crate) fn apply(&self, file: &File, expr: Ast<'_>, el: &mut m::Element) -> bool {
        let n = unwrap_expr(expr);
        if n.kind() != "member_expression" {
            return false;
        }
        let Some(obj) = field(n, "object").filter(|n| n.kind() == "identifier") else {
            return false;
        };
        let Some(entries) = self
            .0
            .get(file.text(obj))
            .and_then(|classes| classes.get(file.name(n, "property")?))
        else {
            return false;
        };
        for rule in entries
            .iter()
            .filter(|r| r.tag.as_ref().is_none_or(|tag| tag == &el.tag))
        {
            el.style.extend_from_slice(&rule.declarations);
            el.unresolved_style.extend_from_slice(&rule.unresolved);
        }
        true
    }
}

fn rules(
    f: &File,
    block: Ast<'_>,
    when: Option<&m::Condition>,
    classes: &mut Classes,
) -> Option<()> {
    for rule in children(block) {
        let parts = children(rule);
        let [head, body] = parts.as_slice() else {
            return None;
        };
        if body.kind() != "block" {
            return None;
        }
        if rule.kind() == "media_statement" && when.is_none() {
            let condition = media(f, *head)?;
            rules(f, *body, Some(&condition), classes)?;
            continue;
        }
        if rule.kind() != "rule_set" || head.kind() != "selectors" {
            return None;
        }
        for selector in children(*head) {
            if selector.kind() != "class_selector" {
                return None;
            }
            let parts = children(selector);
            let (tag, name) = match parts.as_slice() {
                [name] => (None, *name),
                [tag, name] if tag.kind() == "tag_name" => (Some(f.text(*tag).into()), *name),
                _ => return None,
            };
            let mut rule = Rule {
                tag,
                declarations: vec![],
                unresolved: vec![],
            };
            for decl in children(*body) {
                let parts = children(decl);
                if decl.kind() != "declaration"
                    || parts.first().is_some_and(|n| f.text(*n) == "composes")
                    || parts.iter().any(|n| n.kind() == "important")
                {
                    return None;
                }
                let from = m::StyleSource {
                    syntax: m::StyleSyntax::Stylesheet,
                    text: f.text(decl).into(),
                    location: f.loc(decl),
                };
                if let Some(sides) = parts.first().and_then(|name| {
                    let value = f.text(decl).split_once(':')?.1.trim_end_matches(';').trim();
                    if let Some(sides) = super::shorthand(f.text(*name), value) {
                        return Some(sides);
                    }
                    declaration(f.text(*name), value).map(|(props, value)| {
                        props
                            .into_iter()
                            .map(|prop| (prop, value.clone()))
                            .collect()
                    })
                }) {
                    for (property, value) in sides {
                        rule.declarations.push(m::StyleDeclaration {
                            property,
                            value,
                            when: when.cloned(),
                            from: from.clone(),
                        });
                    }
                } else {
                    rule.unresolved.push(m::UnresolvedStyle {
                        syntax: from.syntax,
                        text: from.text,
                        location: from.location,
                        reason: "Unsupported CSS declaration".into(),
                    });
                }
            }
            classes.entry(f.text(name).into()).or_default().push(rule);
        }
    }
    Some(())
}
fn media(f: &File, query: Ast<'_>) -> Option<m::Condition> {
    if query.kind() != "feature_query" {
        return None;
    }
    let parts = children(query);
    let [name, value] = parts.as_slice() else {
        return None;
    };
    let V::Length { value, unit } = length(f.text(*value))? else {
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
    let (min_width, max_width) = match f.text(*name) {
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
    })
}
fn declaration(name: &str, raw: &str) -> Option<(Vec<P>, V)> {
    if let Some(mapped) = super::flex_keyword(name, Some(raw), raw) {
        return Some(mapped);
    }
    let (props, value) = if let Some(prop) = super::size_property(name) {
        (vec![prop], super::size_value(prop, raw))
    } else if let Some(props) = spacing(name) {
        (props.to_vec(), spacing_value(name, raw))
    } else {
        match name {
            "line-height" => (vec![P::LineHeight], super::line_height(raw)),
            "gap" => (vec![P::Gap], super::uniform_gap(raw)),
            "color" => (vec![P::Color], color(raw)),
            "background-color" => (vec![P::Background], color(raw)),
            "border-color" => (vec![P::BorderColor], color(raw)),
            "flex-grow" => (vec![P::Grow], super::grow_factor(raw)),
            "opacity" => (
                vec![P::Opacity],
                raw.parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .map(|value| V::Number { value }),
            ),
            _ => {
                let props = match name {
                    "font-size" => vec![P::FontSize],
                    "border-radius" => vec![P::BorderRadius],
                    _ => return None,
                };
                let value = if raw == "0" {
                    Some(V::Length {
                        value: 0.,
                        unit: m::LengthUnit::Px,
                    })
                } else {
                    length(raw)
                };
                (props, value)
            }
        }
    };
    Some((
        props,
        value.unwrap_or_else(|| V::Unresolved {
            text: raw.into(),
            reason: "Unsupported CSS value".into(),
        }),
    ))
}
