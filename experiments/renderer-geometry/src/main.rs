//! Measure a single, explicitly identified box in a schema-v1 model.
//! The probe supports fixed pixel dimensions only.
use serde_json::{Value, json};
use std::{error::Error, io};
use taffy::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let input: Value = serde_json::from_reader(io::stdin().lock())?;
    let model = input.get("model").unwrap_or(&input);
    let offset: [f32; 2] = match input.get("translatePx") {
        Some(value) => serde_json::from_value(value.clone())?,
        None => [0., 0.],
    };
    if model["schemaVersion"] != 1 {
        return Err("expected schemaVersion 1".into());
    }
    let root = &model["components"][0]["root"];
    if root["tag"] != "div" || root["children"].as_array().is_some_and(|v| !v.is_empty()) {
        return Err("expected a single empty div".into());
    }
    let id = root["props"]["id"]["value"]
        .as_str()
        .ok_or("missing literal ID")?;
    let mut style = Style::default();
    let mut unsupported = Vec::new();
    for decl in root["style"].as_array().ok_or("missing style")? {
        let prop = decl["property"].as_str().ok_or("missing property")?;
        let value = &decl["value"];
        if value["kind"] != "length" || value["unit"] != "px" || decl.get("when").is_some() {
            unsupported.push(prop);
            continue;
        }
        let length = value["value"].as_f64().ok_or("invalid length")? as f32;
        match prop {
            "width" => style.size.width = Dimension::from_length(length),
            "height" => style.size.height = Dimension::from_length(length),
            "maxWidth" => style.max_size.width = LengthPercentageAuto::from_length(length),
            _ => unsupported.push(prop),
        }
    }
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let node = tree.new_leaf(style)?;
    tree.compute_layout(
        node,
        Size {
            width: AvailableSpace::Definite(390.),
            height: AvailableSpace::Definite(844.),
        },
    )?;
    let layout = tree.layout(node)?;
    println!(
        "{}",
        json!({"id":id, "rect":[layout.location.x+offset[0],layout.location.y+offset[1],layout.size.width,layout.size.height],"unsupported":unsupported,"unresolvedStyle":root["unresolvedStyle"]})
    );
    Ok(())
}
