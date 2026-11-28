use std::fmt::Write;
use taffy::prelude::*;

fn boxes(
    width: u32,
    cap: f32,
    gap: f32,
    wrap: bool,
    justify: JustifyContent,
    reverse: bool,
    round: bool,
) -> String {
    let mut tree = TaffyTree::<()>::new();
    if !round {
        tree.disable_rounding();
    }
    let mut ids = Vec::new();
    for (w, h) in [(94.5, 26.), (76.25, 32.), (110., 24.), (58.75, 28.)] {
        ids.push(
            tree.new_leaf(Style {
                size: Size {
                    width: length(w),
                    height: length(h),
                },
                min_size: Size {
                    width: zero(),
                    height: zero(),
                },
                ..Default::default()
            })
            .unwrap(),
        );
    }
    let inner = (width as f32 - 32.).min(cap);
    let row = tree
        .new_with_children(
            Style {
                size: Size {
                    width: length(inner),
                    height: auto(),
                },
                gap: Size {
                    width: length(gap),
                    height: length(gap),
                },
                align_items: AlignItems::START,
                justify_content: justify,
                flex_direction: if reverse {
                    FlexDirection::RowReverse
                } else {
                    FlexDirection::Row
                },
                flex_wrap: if wrap {
                    FlexWrap::Wrap
                } else {
                    FlexWrap::NoWrap
                },
                ..Default::default()
            },
            &ids,
        )
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                size: Size {
                    width: length(width as f32),
                    height: auto(),
                },
                justify_content: JustifyContent::CENTER,
                ..Default::default()
            },
            &[row],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let offset = tree.layout(row).unwrap().location.x;
    let mut result = String::new();
    for id in ids {
        let b = tree.layout(id).unwrap();
        write!(
            result,
            ",{},{},{},{}",
            offset + b.location.x,
            b.location.y,
            b.size.width,
            b.size.height
        )
        .unwrap();
    }
    result
}
fn main() {
    for width in [390, 1440] {
        for cap in [168., 265., 360., 660.] {
            for gap in [0., 7.5, 18.] {
                for wrap in [false, true] {
                    for (j, justify) in [
                        JustifyContent::START,
                        JustifyContent::CENTER,
                        JustifyContent::SPACE_BETWEEN,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        for reverse in [false, true] {
                            for round in [false, true] {
                                println!(
                                    "{width},{cap},{gap},{},{j},{},{}{}",
                                    wrap as u8,
                                    reverse as u8,
                                    round as u8,
                                    boxes(width, cap, gap, wrap, justify, reverse, round)
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn centered_unshrunk_boxes() {
        let result = boxes(390, 360., 0., false, JustifyContent::START, false, false);
        assert!(result.starts_with(",16,0,94.5,26,110.5,0,76.25,32"));
    }
}
