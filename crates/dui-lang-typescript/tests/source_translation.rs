use dui_core::source_model as m;
use dui_lang_typescript::extract;
use std::fs;
use std::path::Path;

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::write(
        dir.path().join("style.css"),
        r#"section { --c:#921c36; --p:12px; }
em { --c:#16745c; --p:6px; }
.box { color: #333333; padding: 4px; }
.box.on { color: #921c36; padding: 12px; }
strong, em { color: var(--c); padding: var(--p); }
@media(min-width:700px) { .box { padding: 16px; } }
"#,
    )
    .expect("stylesheet");
    fs::write(
        dir.path().join("App.tsx"),
        r#"import "./style.css";
const flag = true;
const items = ["A", "B"];
const n = 7;
const labelProps = { label: "Spread" };
function Label({ label = "Default" }: { label?: string }) { return <strong>{label}</strong>; }
function Frame({ children }: { children?: any }) { return <article>{children}</article>; }
export default function App() {
  return <section className={"box " + (flag ? "on" : "")}>
    Count: {n} items
    {flag && <em>On</em>}
    {items.map((item) => <span key={item}>{item}</span>)}
    <Label {...labelProps} label="Override" />
    <Frame><strong>A</strong></Frame>
  </section>;
}
"#,
    )
    .expect("entry");
    dir
}

#[test]
fn source_stylesheets_and_static_bindings_are_preserved() {
    let dir = project();
    let model = extract(dir.path(), Path::new("App.tsx")).expect("extract");
    assert!(
        model
            .diagnostics
            .iter()
            .all(|d| d.code != "unsupported-stylesheet")
    );
    let app = model
        .components
        .iter()
        .find(|component| component.name == "App")
        .expect("App");
    let m::Node::Element(root) = &app.root else {
        panic!("App root must remain an element")
    };
    assert!(
        root.style.iter().any(|declaration| {
            declaration.property == m::StyleProperty::PaddingTop
                && declaration.value
                    == m::StyleValue::Length {
                        value: 16.,
                        unit: m::LengthUnit::Px,
                    }
                && declaration
                    .when
                    .as_ref()
                    .and_then(|when| when.min_width.as_ref())
                    .is_some()
        }),
        "media-rule declarations must remain viewport-conditional: {:#?}",
        root.style
    );

    let list = root
        .children
        .iter()
        .find_map(|node| match node {
            m::Node::List(list) => Some(list),
            _ => None,
        })
        .expect("static list");
    assert_eq!(
        list.source,
        m::Expr::Literal {
            value: serde_json::json!(["A", "B"])
        }
    );
    assert!(matches!(list.key, Some(m::Expr::Item { .. })));

    let instance = root
        .children
        .iter()
        .find_map(|node| match node {
            m::Node::Instance(instance) if instance.component.ends_with("#Label") => Some(instance),
            _ => None,
        })
        .expect("Label instance");
    assert_eq!(
        instance.props.get("label"),
        Some(&m::Expr::Literal {
            value: serde_json::json!("Override")
        }),
        "ordered explicit props must override proven spread values"
    );

    let label = model
        .components
        .iter()
        .find(|component| component.name == "Label")
        .expect("Label");
    assert_eq!(
        label.slots[0].initial,
        Some(m::Expr::Literal {
            value: serde_json::json!("Default")
        })
    );
    let frame = model
        .components
        .iter()
        .find(|component| component.name == "Frame")
        .expect("Frame");
    assert_eq!(frame.slots[0].ty, m::TypeRef::Node);
}

#[test]
fn scoped_custom_properties_resolve_through_the_cascade() {
    let dir = project();
    let model = extract(dir.path(), Path::new("App.tsx")).expect("extract");
    let app = model
        .components
        .iter()
        .find(|component| component.name == "App")
        .expect("App");
    fn find_tag<'a>(node: &'a m::Node, tag: &str) -> Option<&'a m::Element> {
        match node {
            m::Node::Element(element) if element.tag == tag => Some(element),
            m::Node::Element(element) => element
                .children
                .iter()
                .find_map(|child| find_tag(child, tag)),
            m::Node::Branch(branch) => branch
                .arms
                .iter()
                .flat_map(|arm| arm.children.iter())
                .find_map(|child| find_tag(child, tag)),
            _ => None,
        }
    }
    let em = find_tag(&app.root, "em").expect("em");
    assert!(
        em.style.iter().any(|declaration| {
            declaration.property == m::StyleProperty::Color
                && declaration.value
                    == m::StyleValue::Color {
                        value: "#16745c".into(),
                    }
        }),
        "inner scoped variable must override the inherited value: {:#?}",
        em.style
    );
}

#[test]
fn cva_variant_props_become_source_proven_conditional_styles() {
    let dir = tempfile::tempdir().expect("temp project");
    fs::write(
        dir.path().join("style.css"),
        r#".base { display: inline-flex; padding: 4px; }
.variant-default { background-color: #000000; color: #ffffff; }
.variant-outline { background-color: #ffffff; color: #000000; border-width: 1px; }
.size-default { height: 36px; }
"#,
    )
    .expect("stylesheet");
    fs::write(
        dir.path().join("App.tsx"),
        r#"import "./style.css";
const buttonVariants = cva("base", {
  variants: {
    variant: { default: "variant-default", outline: "variant-outline" },
    size: { default: "size-default" },
  },
  defaultVariants: { variant: "default", size: "default" },
});
function Button({ variant, size, className, ...props }: any) {
  return <button {...props} className={cn(buttonVariants({ variant, size, className }))} />;
}
export default function App() {
  return <><Button variant="outline">Go Back</Button><Button>Back Home</Button></>;
}
"#,
    )
    .expect("entry");
    let model = extract(dir.path(), Path::new("App.tsx")).expect("extract");
    let button = model
        .components
        .iter()
        .find(|component| component.name == "Button")
        .expect("Button");
    for (name, choice) in [("variant", "default"), ("size", "default")] {
        let slot = button
            .slots
            .iter()
            .find(|slot| slot.name == name)
            .expect("variant slot");
        assert_eq!(
            slot.initial,
            Some(m::Expr::Literal {
                value: serde_json::json!(choice)
            }),
            "cva defaultVariants become prop defaults"
        );
    }
    let m::Node::Element(root) = &button.root else {
        panic!("Button root must remain an intrinsic button")
    };
    let conditional: Vec<_> = root
        .style
        .iter()
        .filter(|declaration| {
            declaration
                .when
                .as_ref()
                .and_then(|when| when.expr.as_ref())
                .is_some_and(
                    |expr| matches!(expr, m::Expr::Slot { slot, .. } if slot.ends_with(".variant")),
                )
        })
        .collect();
    assert!(
        conditional.iter().any(|declaration| {
            declaration.property == m::StyleProperty::Background
                && declaration.value
                    == m::StyleValue::Color {
                        value: "#ffffff".into(),
                    }
                && declaration
                    .when
                    .as_ref()
                    .and_then(|when| when.equals.as_ref())
                    == Some(&serde_json::json!("outline"))
        }),
        "outline variant must carry its source-proven surface conditionally: {:#?}",
        root.style
    );
    assert!(
        conditional.iter().any(|declaration| {
            declaration.property == m::StyleProperty::Background
                && declaration.value
                    == m::StyleValue::Color {
                        value: "#000000".into(),
                    }
                && declaration
                    .when
                    .as_ref()
                    .and_then(|when| when.equals.as_ref())
                    == Some(&serde_json::json!("default"))
        }),
        "default variant must carry its source-proven surface conditionally: {:#?}",
        root.style
    );
    assert!(
        root.style.iter().all(|declaration| {
            declaration.property != m::StyleProperty::Background || declaration.when.is_some()
        }),
        "variant surfaces must not be guessed unconditionally: {:#?}",
        root.style
    );
}
