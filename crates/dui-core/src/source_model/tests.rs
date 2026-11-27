use super::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

type Mutation = Box<dyn Fn(&mut Value)>;

const REFERENCE: &str = include_str!("../../../../fixtures/source-model/model.json");
const SOURCES: &[(&str, &str)] = &[
    (
        "src/App.tsx",
        include_str!("../../../../fixtures/source-model/src/App.tsx"),
    ),
    (
        "src/Orders.module.css",
        include_str!("../../../../fixtures/source-model/src/Orders.module.css"),
    ),
    (
        "src/Orders.tsx",
        include_str!("../../../../fixtures/source-model/src/Orders.tsx"),
    ),
    (
        "src/Settings.tsx",
        include_str!("../../../../fixtures/source-model/src/Settings.tsx"),
    ),
    (
        "src/api.ts",
        include_str!("../../../../fixtures/source-model/src/api.ts"),
    ),
    (
        "src/theme.css",
        include_str!("../../../../fixtures/source-model/src/theme.css"),
    ),
];

fn reference() -> Value {
    serde_json::from_str(REFERENCE).expect("reference model is JSON")
}

fn rust_errors(document: &Value) -> Vec<String> {
    match SourceModel::from_json(&document.to_string()) {
        Ok(_) => Vec::new(),
        Err(errors) => errors.iter().map(ToString::to_string).collect(),
    }
}

#[test]
fn reference_model_is_valid_and_canonical() {
    let model = SourceModel::from_json(REFERENCE).unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    assert_eq!(model.to_json().expect("serializes"), REFERENCE);
}

#[test]
fn reference_model_covers_styles_tokens_branches_and_lists() {
    let model = SourceModel::from_json(REFERENCE).expect("valid");
    let mut kinds = BTreeSet::new();
    let mut styled = 0;
    let mut stack: Vec<&Node> = model.components.iter().map(|c| &c.root).collect();
    while let Some(node) = stack.pop() {
        kinds.insert(match node {
            Node::Element(element) => {
                styled += element.style.len();
                "element"
            }
            Node::Instance(_) => "instance",
            Node::Text(_) => "text",
            Node::Branch(_) => "branch",
            Node::List(_) => "list",
            Node::Outlet(_) => "outlet",
            Node::Placeholder(_) => "placeholder",
        });
        stack.extend(node.children());
    }
    assert_eq!(
        kinds.into_iter().collect::<Vec<_>>(),
        ["branch", "element", "instance", "list", "text"]
    );
    assert!(styled > 30);
    assert_eq!(model.tokens.len(), 7);
    assert_eq!(model.item_schemas.len(), 1);
}

/// Every location in the reference names real source text: hashes match, byte ranges sit
/// on character boundaries, and line and column agree with the start offset.
#[test]
fn reference_locations_point_into_the_fixture_sources() {
    let model = SourceModel::from_json(REFERENCE).expect("valid");
    for file in &model.files {
        let source = SOURCES
            .iter()
            .find(|(path, _)| *path == file.path)
            .map(|(_, text)| *text)
            .unwrap_or_else(|| panic!("no fixture source for {}", file.path));
        assert_eq!(
            hex(&Sha256::digest(source.as_bytes())),
            file.sha256,
            "{}",
            file.path
        );
    }
    let mut locations = Vec::new();
    collect_locations(&reference(), &mut locations);
    assert!(locations.len() > 90);
    for location in locations {
        let location: Location = serde_json::from_value(location).expect("location");
        let source = SOURCES
            .iter()
            .find(|(path, _)| *path == location.file)
            .map(|(_, text)| *text)
            .expect("known file");
        let (start, end) = (location.start as usize, location.end as usize);
        let text = source
            .get(start..end)
            .expect("range on character boundaries");
        assert!(!text.trim().is_empty(), "{location:?}");
        let before = &source[..start];
        let line = before.matches('\n').count() + 1;
        let column = before
            .rsplit('\n')
            .next()
            .map_or(0, |tail| tail.chars().count())
            + 1;
        assert_eq!(
            (line, column),
            (location.line as usize, location.column as usize)
        );
    }
}

fn collect_locations(value: &Value, out: &mut Vec<Value>) {
    match value {
        Value::Object(map) => {
            if ["file", "line", "column", "start", "end"]
                .iter()
                .all(|key| map.contains_key(*key))
                && map.len() == 5
            {
                out.push(value.clone());
            }
            map.values().for_each(|child| collect_locations(child, out));
        }
        Value::Array(items) => items.iter().for_each(|child| collect_locations(child, out)),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn screen_edges_start_on_the_screens_that_draw_the_trigger() {
    let model = SourceModel::from_json(REFERENCE).expect("valid");
    let edges = model.screen_edges();
    let pairs: Vec<(&str, &str)> = edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.edge.as_str()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("route:/", "src/Orders.tsx#Orders@0.3:link"),
            ("route:/settings", "src/Settings.tsx#Settings@0.3:link"),
        ]
    );
}

#[test]
fn shared_component_edges_start_on_every_screen_that_uses_it() {
    let mut document = reference();
    // Move the Settings link into Toggle, which only the Settings screen draws, then
    // also draw Toggle on the Orders screen.
    let toggle_link = json!({
        "kind": "element", "id": "src/Settings.tsx#Toggle@0.2", "tag": "Link",
        "location": document["edges"][1]["location"].clone()
    });
    document["components"][2]["root"]["children"]
        .as_array_mut()
        .expect("children")
        .push(toggle_link);
    document["edges"][1]["from"] = json!("src/Settings.tsx#Toggle@0.2");
    let location = document["edges"][0]["location"].clone();
    document["components"][0]["root"]["children"]
        .as_array_mut()
        .expect("children")
        .push(json!({
            "kind": "instance", "id": "src/Orders.tsx#Orders@0.4",
            "component": "src/Settings.tsx#Toggle",
            "location": location
        }));
    let model = SourceModel::from_json(&document.to_string()).expect("valid");
    let pairs: Vec<(String, String)> = model
        .screen_edges()
        .into_iter()
        .map(|edge| (edge.from, edge.edge))
        .collect();
    let shared = "src/Settings.tsx#Settings@0.3:link".to_owned();
    assert!(pairs.contains(&("route:/".to_owned(), shared.clone())));
    assert!(pairs.contains(&("route:/settings".to_owned(), shared)));
}

/// Shapes the JSON Schema also rejects (`scripts/check-source-model.mts`).
#[test]
fn reader_rejects_bad_shapes() {
    let cases: Vec<(&str, Mutation)> = vec![
        (
            "schema version",
            Box::new(|d| d["schemaVersion"] = json!(2)),
        ),
        ("unknown field", Box::new(|d| d["generatedAt"] = json!(0))),
        (
            "missing files",
            Box::new(|d| drop(d.as_object_mut().map(|m| m.remove("files")))),
        ),
        (
            "uppercase color",
            Box::new(|d| d["tokens"][0]["value"]["value"] = json!("#1D4ED8")),
        ),
        (
            "unknown style property",
            Box::new(|d| {
                d["components"][0]["root"]["style"][0]["property"] = json!("float");
            }),
        ),
        (
            "unknown node kind",
            Box::new(|d| d["components"][0]["root"]["kind"] = json!("view")),
        ),
        (
            "absolute path",
            Box::new(|d| d["files"][0]["path"] = json!("/src/App.tsx")),
        ),
        (
            "parent path",
            Box::new(|d| d["files"][0]["path"] = json!("../App.tsx")),
        ),
        (
            "bad hash",
            Box::new(|d| d["files"][0]["sha256"] = json!("abc")),
        ),
        (
            "zero line",
            Box::new(|d| d["screens"][0]["location"]["line"] = json!(0)),
        ),
        (
            "empty enum",
            Box::new(|d| d["components"][0]["slots"][1]["type"]["values"] = json!([])),
        ),
        (
            "route without slash",
            Box::new(|d| d["screens"][1]["route"] = json!("settings")),
        ),
        (
            "token without location or package",
            Box::new(|d| drop(d["tokens"][4].as_object_mut().map(|m| m.remove("package")))),
        ),
    ];
    for (name, mutate) in cases {
        let mut document = reference();
        mutate(&mut document);
        assert!(!rust_errors(&document).is_empty(), "reader accepted {name}");
    }
}

/// Rules only the reader checks; the JSON Schema accepts these shapes.
#[test]
fn reader_rejects_broken_references_and_order() {
    let cases: Vec<(&str, &str, Mutation)> = vec![
        (
            "unsorted components",
            "must sort after",
            Box::new(|d| {
                let components = d["components"].as_array_mut().expect("array");
                components.swap(0, 1);
            }),
        ),
        (
            "duplicate node id",
            "duplicate identifier",
            Box::new(|d| {
                d["components"][0]["root"]["children"][1]["id"] =
                    json!("src/Orders.tsx#Orders@0.0");
            }),
        ),
        (
            "slot of another component",
            "is not a slot of",
            Box::new(|d| {
                d["components"][0]["root"]["children"][2]["on"]["slot"] =
                    json!("src/Settings.tsx#Settings.notifications");
            }),
        ),
        (
            "item read outside its list",
            "is not a list around this value",
            Box::new(|d| {
                d["components"][0]["root"]["children"][0]["children"][0]["parts"][0] = json!({
                    "kind": "item", "list": "src/Orders.tsx#Orders@0.2.1.0.0", "path": ["title"]
                });
            }),
        ),
        (
            "unknown component",
            "unknown component",
            Box::new(|d| d["screens"][0]["component"] = json!("src/Orders.tsx#Missing")),
        ),
        (
            "unknown instance prop",
            "has no prop",
            Box::new(|d| {
                d["components"][1]["root"]["children"][1]["props"]["size"] = json!({
                    "kind": "literal", "value": "large"
                });
            }),
        ),
        (
            "keyword for the wrong property",
            "not a value for",
            Box::new(|d| {
                d["components"][0]["root"]["style"][0]["value"] =
                    json!({ "kind": "keyword", "value": "diagonal" });
            }),
        ),
        (
            "length for a color",
            "not a value for",
            Box::new(|d| {
                d["components"][0]["root"]["children"][0]["style"][3]["value"] =
                    json!({ "kind": "length", "value": 1, "unit": "px" });
            }),
        ),
        (
            "unknown token",
            "unknown token",
            Box::new(|d| {
                d["components"][0]["root"]["children"][0]["style"][3]["value"]["token"] =
                    json!("src/theme.css#--color-missing");
            }),
        ),
        (
            "token alias cycle",
            "cycle",
            Box::new(|d| {
                d["tokens"][0]["value"] =
                    json!({ "kind": "token", "token": "src/theme.css#--color-muted" });
                d["tokens"][1]["value"] =
                    json!({ "kind": "token", "token": "src/theme.css#--color-brand" });
            }),
        ),
        (
            "otherwise before another arm",
            "otherwise must be last",
            Box::new(|d| {
                let arms = d["components"][0]["root"]["children"][2]["arms"]
                    .as_array_mut()
                    .expect("arms");
                arms.swap(0, 1);
            }),
        ),
        (
            "location in an unlisted file",
            "is not a file of the model",
            Box::new(|d| d["screens"][0]["location"]["file"] = json!("src/Other.tsx")),
        ),
        (
            "import of an unlisted file",
            "is not another file of the model",
            Box::new(|d| d["files"][0]["imports"][0] = json!("src/Missing.tsx")),
        ),
        (
            "edge from an unknown node",
            "unknown node",
            Box::new(|d| d["edges"][0]["from"] = json!("src/Orders.tsx#Orders@9")),
        ),
        (
            "edge to an unknown screen",
            "unknown screen",
            Box::new(|d| d["edges"][0]["to"]["screen"] = json!("route:/missing")),
        ),
    ];
    for (name, expected, mutate) in cases {
        let mut document = reference();
        mutate(&mut document);
        let errors = rust_errors(&document);
        assert!(
            errors.iter().any(|error| error.contains(expected)),
            "{name}: expected {expected:?} in {errors:?}"
        );
    }
}

#[test]
fn whole_numbers_serialize_without_a_fraction() {
    let value = StyleValue::Number { value: 600.0 };
    assert_eq!(
        serde_json::to_string(&value).expect("json"),
        r#"{"kind":"number","value":600}"#
    );
    let value = StyleValue::Number { value: 1.5 };
    assert_eq!(
        serde_json::to_string(&value).expect("json"),
        r#"{"kind":"number","value":1.5}"#
    );
}
