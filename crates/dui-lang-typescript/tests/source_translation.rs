use dui_core::source_model as m;
use dui_lang_typescript::{extract, extract_in_context};
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

#[test]
fn bundled_font_faces_are_extracted_with_bounded_local_assets() {
    let dir = tempfile::tempdir().expect("temp project");
    let assets = dir.path().join("dist/assets/fonts");
    fs::create_dir_all(&assets).expect("font dir");
    let font = [b"wOF2".as_slice(), &[7u8; 128]].concat();
    fs::write(assets.join("demo.woff2"), &font).expect("font");
    fs::write(
        dir.path().join("dist/assets/index.css"),
        r#"@font-face{font-family:'Demo Sans';src:url('./fonts/demo.woff2') format('woff2'),url('https://fonts.example.com/demo.woff2') format('woff2');font-weight:400 900;font-style:normal;font-display:swap}
:root{--demo-font:'Demo Sans',ui-sans-serif,system-ui,sans-serif}
html{font-family:var(--demo-font,'Demo Sans',sans-serif)}"#,
    )
    .expect("built css");
    fs::write(
        dir.path().join("App.tsx"),
        r#"export default function App() { return <main>Hello</main>; }
"#,
    )
    .expect("entry");
    let model = extract(dir.path(), Path::new("App.tsx")).expect("extract");
    let document = model.document_style.expect("document style");
    assert_eq!(document.font_faces.len(), 1, "{:#?}", document.font_faces);
    let face = &document.font_faces[0];
    assert_eq!(face.family, "Demo Sans");
    assert_eq!(face.weight.as_deref(), Some("400 900"));
    assert_eq!(face.display.as_deref(), Some("swap"));
    let root_family = document
        .root
        .as_ref()
        .and_then(|root| {
            root.style
                .iter()
                .find(|declaration| declaration.property == m::StyleProperty::FontFamily)
        })
        .expect("document root font family");
    assert_eq!(
        root_family.value,
        m::StyleValue::Keyword {
            value: "'Demo Sans',ui-sans-serif,system-ui,sans-serif".into(),
        },
        "@font-face descriptors must not obscure the document fallback list"
    );
    assert!(
        document.body.is_none(),
        "@font-face descriptors are not body element declarations: {:#?}",
        document.body
    );
    let app = model
        .components
        .iter()
        .find(|component| component.name == "App")
        .expect("App");
    let m::Node::Element(main) = &app.root else {
        panic!("App root must remain an element")
    };
    assert!(
        main.style
            .iter()
            .all(|declaration| declaration.property != m::StyleProperty::FontFamily),
        "the document font is inherited context, not a main element declaration: {:#?}",
        main.style
    );
    let mut kinds = face.sources.iter().map(|source| match source {
        m::FontSource::Asset {
            path,
            asset,
            sha256,
            bytes,
            format,
            ..
        } => {
            assert_eq!(path, "dist/assets/fonts/demo.woff2");
            assert_eq!(*bytes, font.len() as u64);
            assert_eq!(sha256.len(), 64);
            assert_eq!(asset, &format!("asset-{sha256}.woff2"));
            assert_eq!(format.as_deref(), Some("woff2"));
            "asset"
        }
        m::FontSource::External { url, format, .. } => {
            assert_eq!(url, "https://fonts.example.com/demo.woff2");
            assert_eq!(format.as_deref(), Some("woff2"));
            "external"
        }
    });
    assert_eq!(kinds.next(), Some("asset"));
    assert_eq!(kinds.next(), Some("external"));
    assert_eq!(kinds.next(), None);
}


#[test]
fn cached_public_remote_fonts_are_materialized_without_network() {
    use sha2::{Digest, Sha256};
    let dir = tempfile::tempdir().expect("temp project");
    let cache = tempfile::tempdir().expect("font cache");
    // SAFETY: this test process never restores or mutates the variable concurrently; the
    // extraction tests either have no remote font or tolerate a cache miss.
    unsafe { std::env::set_var("DUI_FONT_CACHE", cache.path()) };
    let url = "https://fonts.example.test/demo.woff2";
    let font = [b"wOF2".as_slice(), &[9u8; 256]].concat();
    let sha256 = format!("{:x}", Sha256::digest(&font));
    fs::create_dir_all(cache.path().join("blobs")).expect("blob dir");
    fs::create_dir_all(cache.path().join("index")).expect("index dir");
    fs::write(cache.path().join("blobs").join(&sha256), &font).expect("blob");
    let key = format!("{:x}", Sha256::digest(format!("font-fetch-v1\n{url}").as_bytes()));
    fs::write(
        cache.path().join("index").join(format!("{key}.json")),
        format!(r#"{{"version":1,"url":"{url}","finalUrl":"{url}","sha256":"{sha256}","bytes":{}}}"#, font.len()),
    )
    .expect("cache metadata");
    fs::create_dir_all(dir.path().join("dist/assets")).expect("assets");
    fs::write(
        dir.path().join("dist/assets/index.css"),
        format!(r#"@font-face{{font-family:'Remote Demo';src:url('{url}') format('woff2');font-display:swap}}"#),
    )
    .expect("built css");
    fs::write(
        dir.path().join("App.tsx"),
        r#"export default function App() { return <main>Hello</main>; }
"#,
    )
    .expect("entry");
    let model = extract(dir.path(), Path::new("App.tsx")).expect("extract");
    let face = &model.document_style.expect("document").font_faces[0];
    let m::FontSource::External {
        asset: Some(asset),
        sha256: Some(model_sha),
        bytes: Some(bytes),
        retrieved_url: Some(retrieved),
        ..
    } = &face.sources[0]
    else {
        panic!("cached remote font must become a complete materialized source: {:#?}", face.sources)
    };
    assert_eq!(asset, &format!("asset-{sha256}.woff2"));
    assert_eq!(model_sha, &sha256);
    assert_eq!(*bytes, font.len() as u64);
    assert_eq!(retrieved, url);
}

#[test]
fn default_document_font_provider_class_is_source_proven() {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("dist/assets")).expect("assets");
    fs::write(
        dir.path().join("dist/assets/index.css"),
        r#".font-inter{font-family:Inter,"sans-serif"}.font-manrope{font-family:Manrope,"sans-serif"}"#,
    )
    .expect("built css");
    fs::write(dir.path().join("fonts.ts"), "export const fonts = ['inter', 'manrope'] as const
").expect("fonts");
    fs::write(
        dir.path().join("font-provider.tsx"),
        r#"import { useEffect, useState } from 'react'
import { fonts } from './fonts'
export function FontProvider() {
  const [font, setFont] = useState(() => fonts[0])
  useEffect(() => {
    const root = document.documentElement
    root.classList.add(`font-${font}`)
  }, [font])
  return null
}
"#,
    )
    .expect("provider");
    fs::write(
        dir.path().join("App.tsx"),
        r#"import './font-provider'
export default function App() { return <main>Hello</main>; }
"#,
    )
    .expect("entry");
    let model = extract(dir.path(), Path::new("App.tsx")).expect("extract");
    let root = model
        .document_style
        .and_then(|document| document.root)
        .expect("document root");
    let family = root
        .style
        .iter()
        .find(|declaration| declaration.property == m::StyleProperty::FontFamily)
        .expect("font family");
    assert_eq!(
        family.value,
        m::StyleValue::Keyword {
            value: "Inter,\"sans-serif\"".into()
        },
        "{:#?}",
        root.style
    );
}

#[test]
fn invalid_local_fonts_stay_out_of_the_model_and_report_a_diagnostic() {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("dist/assets")).expect("assets");
    fs::write(
        dir.path().join("dist/assets/not-a-font.woff2"),
        b"not a font",
    )
    .expect("font");
    fs::write(
        dir.path().join("dist/assets/index.css"),
        r#"@font-face{font-family:'Broken';src:url('./not-a-font.woff2') format('woff2')}"#,
    )
    .expect("built css");
    fs::write(
        dir.path().join("App.tsx"),
        r#"export default function App() { return <main>Hello</main>; }
"#,
    )
    .expect("entry");
    let model = extract(dir.path(), Path::new("App.tsx")).expect("extract");
    assert!(
        model
            .document_style
            .as_ref()
            .is_none_or(|document| document.font_faces.is_empty())
    );
    assert!(
        model
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported-font-resource"),
        "{:#?}",
        model.diagnostics
    );
}

#[test]
fn theme_states_and_logical_viewport_lengths_are_source_proven() {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("dist/assets")).expect("assets");
    fs::write(
        dir.path().join("dist/assets/index.css"),
        r#":root{--surface:#ffffff;--ink:#111111}
.dark{--surface:#020618;--ink:#f8fafc;--input:oklch(100% 0 0/.15)}
:root[data-theme=dark]{--muted:#90a1b9}
body{min-height:100svh;background-color:var(--surface);color:var(--ink)}
.h-svh{height:100svh}.h-full{height:100%}
.dark\:bg-input:is(.dark *){background-color:color-mix(in oklab,var(--input) 30%,transparent)}
.dark\:hover\:bg-input:is(.dark *):hover{background-color:#999999}
.themed{color:var(--muted)}
@media (prefers-color-scheme: dark){.themed{border-color:#445566}}"#,
    )
    .expect("built css");
    fs::write(
        dir.path().join("App.tsx"),
        r#"export default function App() { return <div className="h-svh dark:bg-input themed"><div className="h-full">Hello</div></div>; }
"#,
    )
    .expect("entry");
    let model = extract_in_context(
        dir.path(),
        Path::new("App.tsx"),
        Some(Path::new("dist/assets/index.css")),
        Some(1440),
        None,
    )
    .expect("extract");
    let document = model.document_style.expect("document style");
    let body = document.body.expect("body");
    let dark_background = body
        .style
        .iter()
        .find(|declaration| {
            declaration.property == m::StyleProperty::Background
                && declaration.when.as_ref().and_then(|when| when.color_scheme)
                    == Some(m::ColorScheme::Dark)
        })
        .expect("dark document background");
    let effective_document = match &dark_background.value {
        m::StyleValue::Unresolved {
            refinement: Some(refinement),
            ..
        } => refinement.as_ref(),
        value => value,
    };
    assert_eq!(
        effective_document,
        &m::StyleValue::Color {
            value: "#020618".into()
        },
        "scheme-scoped root variables must resolve the selectable dark document state"
    );
    let app = model
        .components
        .iter()
        .find(|component| component.name == "App")
        .expect("App");
    let m::Node::Element(root) = &app.root else {
        panic!("App root must be an element")
    };
    let declarations = |property| {
        root.style
            .iter()
            .filter(move |declaration| declaration.property == property)
            .collect::<Vec<_>>()
    };
    let height = declarations(m::StyleProperty::Height)
        .into_iter()
        .next()
        .expect("height");
    let m::StyleValue::Unresolved {
        refinement: Some(refinement),
        ..
    } = &height.value
    else {
        panic!(
            "100svh must remain unresolved with a typed refinement: {:#?}",
            height
        )
    };
    assert_eq!(
        refinement.as_ref(),
        &m::StyleValue::Length {
            value: 100.,
            unit: m::LengthUnit::Svh,
        }
    );
    for (property, expected) in [
        (m::StyleProperty::Background, "#ffffff0b"),
        (m::StyleProperty::Color, "#90a1b9"),
        (m::StyleProperty::BorderColor, "#445566"),
    ] {
        assert!(
            declarations(property).iter().any(|declaration| {
                declaration.when.as_ref().and_then(|when| when.color_scheme)
                    == Some(m::ColorScheme::Dark)
                    && declaration.value
                        == m::StyleValue::Color {
                            value: expected.into(),
                        }
            }),
            "missing selectable dark {property:?} state {expected}: {:#?}",
            declarations(property)
        );
    }
}

#[test]
fn built_css_wide_inherit_is_preserved_for_form_controls() {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("dist/assets")).expect("assets");
    fs::write(
        dir.path().join("dist/assets/index.css"),
        r#"button,input{font:inherit;font-feature-settings:inherit;font-variation-settings:inherit;letter-spacing:inherit;color:inherit;background-color:#0000}"#,
    )
    .expect("built css");
    fs::write(
        dir.path().join("App.tsx"),
        r#"export default function App() { return <main><button>Go Back</button></main>; }
"#,
    )
    .expect("entry");
    let model = extract(dir.path(), Path::new("App.tsx")).expect("extract");
    let app = model
        .components
        .iter()
        .find(|component| component.name == "App")
        .expect("App");
    fn find_button(node: &m::Node) -> Option<&m::Element> {
        match node {
            m::Node::Element(element) if element.tag == "button" => Some(element),
            m::Node::Element(element) => element.children.iter().find_map(find_button),
            _ => None,
        }
    }
    let button = find_button(&app.root).expect("button");
    for property in [
        m::StyleProperty::FontFamily,
        m::StyleProperty::FontSize,
        m::StyleProperty::LineHeight,
        m::StyleProperty::FontWeight,
        m::StyleProperty::LetterSpacing,
        m::StyleProperty::Color,
    ] {
        assert!(
            button.style.iter().any(|declaration| {
                declaration.property == property
                    && declaration.value
                        == m::StyleValue::Keyword {
                            value: "inherit".into(),
                        }
            }),
            "missing inherited {property:?}: {:#?}",
            button.style
        );
    }
}

