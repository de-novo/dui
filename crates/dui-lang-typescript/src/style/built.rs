//! Resolve scalar gaps from an explicitly supplied, already bundled author stylesheet.
use super::{P, V, uniform_gap};
use crate::syntax::{File, children, first};
use dui_core::source_model as m;
use std::collections::BTreeMap;
use tree_sitter::Node as Ast;

struct Rule<'a> {
    selectors: &'a str,
    layer: usize,
    uncertain: bool,
    scalar: bool,
    value: &'a str,
    source: m::StyleSource,
}

pub(crate) struct Sheet<'a> {
    rules: Vec<Rule<'a>>,
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
            rules: vec![],
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
                    } else if matches!(
                        prop,
                        "gap"
                            | "grid-gap"
                            | "row-gap"
                            | "column-gap"
                            | "grid-row-gap"
                            | "grid-column-gap"
                            | "all"
                    ) {
                        sheet.rules.push(Rule {
                            selectors: selector,
                            layer,
                            uncertain: uncertain || important,
                            scalar: matches!(prop, "gap" | "grid-gap"),
                            value,
                            source: m::StyleSource {
                                syntax: m::StyleSyntax::Stylesheet,
                                text: f.text(node).into(),
                                location: f.loc(node),
                            },
                        });
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

    fn value(&self, raw: &str) -> Option<V> {
        if let Some(v) = uniform_gap(raw) {
            return Some(v);
        }
        let (variable, factor) =
            if let Some(calc) = raw.strip_prefix("calc(").and_then(|s| s.strip_suffix(')')) {
                let (var, n) = calc.split_once('*')?;
                (var.trim(), n.trim().parse::<f64>().ok()?)
            } else {
                (raw, 1.)
            };
        let name = variable.strip_prefix("var(")?.strip_suffix(')')?.trim();
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
        el.style
            .retain(|d| d.property != P::Gap || d.from.syntax == m::StyleSyntax::Inline);
        let mut best = None;
        let mut rank = None;
        let mut unknown = None;
        for rule in &self.rules {
            for sel in rule.selectors.split(',').map(str::trim) {
                match specificity(sel, classes, el) {
                    Some(None) => {}
                    Some(Some(s)) if !rule.uncertain => {
                        if rank.is_none_or(|r| (rule.layer, s) >= r) {
                            rank = Some((rule.layer, s));
                            best = Some(rule);
                        }
                    }
                    _ => unknown = Some(rule),
                }
            }
        }
        let Some(rule) = unknown.or(best) else { return };
        if unknown.is_none() && el.style.iter().any(|d| d.property == P::Gap) {
            return;
        }
        el.style.retain(|d| d.property != P::Gap);
        let opaque_inline = el
            .unresolved_style
            .iter()
            .any(|d| d.syntax == m::StyleSyntax::Inline);
        let value = (unknown.is_none() && !opaque_inline && rule.scalar)
            .then(|| self.value(rule.value))
            .flatten()
            .unwrap_or_else(|| V::Unresolved {
                text: rule.source.text.clone(),
                reason: "Cannot determine scalar gap from built CSS".into(),
            });
        el.style.push(m::StyleDeclaration {
            property: P::Gap,
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
