use serde_json::{Value, json};
use std::{collections::BTreeMap, error::Error, io};
use taffy::prelude::*;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn px(s: &str) -> f32 {
    s.trim_end_matches("px").parse().unwrap_or(0.)
}
fn dim(s: &str) -> LengthPercentageAuto {
    if let Some(n) = s.strip_suffix('%') {
        percent(px(n) / 100.)
    } else if s.ends_with("px") {
        length(px(s))
    } else {
        auto()
    }
}
fn margin(s: &str) -> LengthPercentageAuto {
    if s == "auto" {
        auto()
    } else if let Some(n) = s.strip_suffix('%') {
        percent(px(n) / 100.)
    } else {
        length(px(s))
    }
}
fn build(
    n: &Value,
    t: &mut TaffyTree<(Vec<f32>, f32)>,
    ids: &mut Vec<(String, NodeId)>,
) -> Result<NodeId> {
    let css = |k: &str| n["css"][k].as_str().unwrap_or("");
    let style = Style {
        display: if css("display") == "flex" {
            Display::Flex
        } else {
            Display::Block
        },
        flex_direction: if css("flex-direction") == "column" {
            FlexDirection::Column
        } else {
            FlexDirection::Row
        },
        align_items: (css("align-items") == "center")
            .then_some(AlignItems::CENTER)
            .unwrap_or_default(),
        justify_content: (css("justify-content") == "center")
            .then_some(JustifyContent::CENTER)
            .unwrap_or_default(),
        size: Size {
            width: dim(css("width")).into(),
            height: dim(css("height")).into(),
        },
        min_size: Size {
            width: dim(css("min-width")),
            height: dim(css("min-height")),
        },
        max_size: Size {
            width: dim(css("max-width")),
            height: dim(css("max-height")),
        },
        gap: Size {
            width: length(px(css("column-gap"))),
            height: length(px(css("row-gap"))),
        },
        margin: Rect {
            left: margin(css("margin-left")),
            right: margin(css("margin-right")),
            top: margin(css("margin-top")),
            bottom: margin(css("margin-bottom")),
        },
        padding: Rect {
            left: length(px(css("padding-left"))),
            right: length(px(css("padding-right"))),
            top: length(px(css("padding-top"))),
            bottom: length(px(css("padding-bottom"))),
        },
        border: Rect {
            left: length(px(css("border-left-width"))),
            right: length(px(css("border-right-width"))),
            top: length(px(css("border-top-width"))),
            bottom: length(px(css("border-bottom-width"))),
        },
        ..Default::default()
    };
    let children = n["children"]
        .as_array()
        .ok_or("children")?
        .iter()
        .map(|c| build(c, t, ids))
        .collect::<Result<Vec<_>>>()?;
    let id = if children.is_empty() {
        let lines = n["lines"]
            .as_array()
            .ok_or("lines")?
            .iter()
            .filter_map(Value::as_f64)
            .map(|x| x as f32)
            .collect();
        t.new_leaf_with_context(style, (lines, px(css("line-height"))))?
    } else {
        t.new_with_children(style, &children)?
    };
    ids.push((n["id"].as_str().ok_or("id")?.to_owned(), id));
    Ok(id)
}
fn main() -> Result<()> {
    let input: Value = serde_json::from_reader(io::stdin())?;
    let mut tree = TaffyTree::new();
    tree.disable_rounding();
    let mut ids = Vec::new();
    let root = build(&input["tree"], &mut tree, &mut ids)?;
    let space = Size {
        width: AvailableSpace::Definite(input["width"].as_f64().ok_or("width")? as f32),
        height: AvailableSpace::Definite(input["height"].as_f64().ok_or("height")? as f32),
    };
    tree.compute_layout_with_measure(root, space, |inputs, _, ctx, style| {
        taffy::compute_leaf_layout(
            inputs,
            style,
            |_, _| 0.,
            |known, _available| {
                let (w, h) = ctx
                    .as_ref()
                    .map(|(lines, line_height)| {
                        (
                            lines.iter().copied().fold(0., f32::max),
                            lines.len() as f32 * line_height,
                        )
                    })
                    .unwrap_or((0., 0.));
                Size {
                    width: known.width.unwrap_or(w),
                    height: known.height.unwrap_or(h),
                }
            },
        )
    })?;
    let mut boxes = BTreeMap::new();
    for (key, id) in ids {
        let layout = tree.layout(id)?;
        let (mut x, mut y) = (layout.location.x, layout.location.y);
        let mut parent = tree.parent(id);
        while let Some(p) = parent {
            let pos = tree.layout(p)?.location;
            x += pos.x;
            y += pos.y;
            parent = tree.parent(p);
        }
        boxes.insert(key, json!([x, y, layout.size.width, layout.size.height]));
    }
    println!("{}", serde_json::to_string(&boxes)?);
    Ok(())
}
