//! Narrow class-render patterns. Runtime values remain opaque.
use crate::{Ast, children, field, first, syntax::File, unwrap_expr};

fn contains_exit(node: Ast<'_>) -> bool {
    match node.kind() {
        "return_statement" | "throw_statement" => true,
        "arrow_function" | "function_expression" | "function_declaration" => false,
        _ => children(node).into_iter().any(contains_exit),
    }
}

pub fn tail(method: Ast<'_>) -> Option<Ast<'_>> {
    let statements = children(field(method, "body")?);
    let (last, before) = statements.split_last()?;
    (last.kind() == "return_statement" && !before.iter().copied().any(contains_exit))
        .then(|| first(*last).map(unwrap_expr))
        .flatten()
}

/// Resolve `let part; if (test) { part = <Child/>; } return <main>{part}</main>`.
/// Reject extra references, redeclarations and writes instead of simulating execution.
pub fn conditional_local<'a>(file: &File, reference: Ast<'a>) -> Option<(Ast<'a>, Ast<'a>)> {
    let method = std::iter::successors(reference.parent(), |n| n.parent()).find(|n| {
        matches!(
            n.kind(),
            "method_definition" | "arrow_function" | "function_declaration" | "function_expression"
        )
    })?;
    if method.kind() != "method_definition" || file.name(method, "name") != Some("render") {
        return None;
    }
    let body = field(method, "body")?;
    let name = file.text(reference);
    let statements = children(body);
    let declaration = statements
        .iter()
        .filter(|n| matches!(n.kind(), "variable_declaration" | "lexical_declaration"))
        .flat_map(|n| children(*n))
        .find(|d| file.name(*d, "name") == Some(name) && field(*d, "value").is_none())?;
    let mut pending = vec![body];
    let mut references = 0;
    while let Some(n) = pending.pop() {
        if n.kind() == "identifier" && file.text(n) == name {
            references += 1;
        }
        pending.extend(children(n));
    }
    if references != 3 {
        return None;
    }
    statements
        .into_iter()
        .filter(|n| {
            n.kind() == "if_statement"
                && field(*n, "alternative").is_none()
                && declaration.end_byte() <= n.start_byte()
                && n.end_byte() <= reference.start_byte()
        })
        .find_map(|n| {
            let block = field(n, "consequence")?;
            let statements = children(block);
            let [stmt] = statements.as_slice() else {
                return None;
            };
            let assign = first(*stmt)?;
            let lhs = field(assign, "left")?;
            let rhs = unwrap_expr(field(assign, "right")?);
            (assign.kind() == "assignment_expression"
                && lhs.kind() == "identifier"
                && file.text(lhs) == name
                && matches!(rhs.kind(), "jsx_element" | "jsx_self_closing_element"))
            .then_some((unwrap_expr(field(n, "condition")?), rhs))
        })
}

pub fn props(file: &File, method: Ast<'_>, id: &str) -> Vec<crate::m::Slot> {
    use crate::m;
    let mut props = std::collections::BTreeMap::new();
    let mut pending = vec![method];
    while let Some(n) = pending.pop() {
        if n.kind() == "member_expression"
            && let Some(object) = field(n, "object")
            && object.kind() == "member_expression"
            && file.name(object, "property") == Some("props")
            && field(object, "object").is_some_and(|v| v.kind() == "this")
            && let Some(key) = field(n, "property")
        {
            let name = file.text(key);
            props.entry(name).or_insert_with(|| m::Slot {
                id: format!("{id}.{name}"),
                name: name.into(),
                kind: m::SlotKind::Prop,
                ty: crate::unknown(),
                optional: true,
                initial: None,
                source: None,
                origin: m::Origin::Syntax,
                location: file.loc(key),
            });
        }
        if n == method
            || !matches!(
                n.kind(),
                "method_definition"
                    | "class_declaration"
                    | "function_expression"
                    | "function_declaration"
            )
        {
            pending.extend(children(n).into_iter().rev());
        }
    }
    props.into_values().collect()
}
