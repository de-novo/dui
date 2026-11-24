//! Pure 2D screen map layout mirroring `apps/canvas/src/features/map/layout.ts`.

use crate::{screen_graph::ScreenGraph, store::ScreenItem};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct NodeMetrics {
    pub card_width: f32,
    pub frame_height: f32,
    pub caption_height: f32,
    pub col_gap: f32,
    pub row_gap: f32,
}

#[derive(Clone, Debug)]
pub struct LayoutNode {
    pub screen_idx: usize,
    pub x: f32,
    pub y: f32,
    pub frame_height: f32,
    pub card_height: f32,
}

#[derive(Clone, Debug)]
pub struct MapLayout {
    pub metrics: NodeMetrics,
    pub nodes: Vec<LayoutNode>,
    pub graph: ScreenGraph,
}

pub struct PaintedCardMetrics {
    pub width: f32,
    pub padding: f32,
    pub caption_height: f32,
    zoom: f32,
}

impl PaintedCardMetrics {
    pub fn new(metrics: NodeMetrics, zoom: f32) -> Self {
        let width = (metrics.card_width * zoom).max(16.0);
        Self {
            width,
            padding: (width * 0.04).clamp(4.0, 12.0),
            caption_height: (metrics.caption_height * zoom).max(18.0),
            zoom,
        }
    }

    pub fn heights(&self, node: &LayoutNode) -> (f32, f32) {
        let frame = (node.frame_height * self.zoom).max(10.0);
        (frame, frame + self.caption_height + self.padding * 2.0)
    }
}

impl MapLayout {
    fn painted_size(&self, zoom: f32) -> (f32, f32) {
        let card = PaintedCardMetrics::new(self.metrics, zoom);
        self.nodes.iter().fold((0.0, 0.0), |(w, h), node| {
            (
                w.max(node.x * zoom + card.width),
                h.max(node.y * zoom + card.heights(node).1),
            )
        })
    }

    pub fn fit(&self, width: f32, height: f32) -> (f32, f32, f32) {
        // Fixed pixel minima for captions and padding make the painted size
        // monotonic in zoom, but not proportional to it.
        let mut zoom = 1.0;
        let (w, h) = self.painted_size(zoom);
        if w > width || h > height {
            let mut lo = 0.0;
            let mut hi = zoom;
            for _ in 0..24 {
                let mid = (lo + hi) * 0.5;
                let (w, h) = self.painted_size(mid);
                if w <= width && h <= height {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            zoom = lo.max(f32::EPSILON);
        }
        let (w, h) = self.painted_size(zoom);
        (zoom, w, h)
    }
}

pub fn node_metrics(capture_width: u32) -> NodeMetrics {
    let w = (capture_width.max(320) as f32).min(1440.0);
    let aspect = if capture_width <= 480 { 1.55 } else { 0.64 };
    let frame_height = (w * aspect).round();
    let caption_height = (w * 0.085).clamp(44.0, 96.0).round();
    let card_height = frame_height + caption_height;
    let col_gap = (w * 0.28).max(220.0).round();
    let row_gap = (card_height * 0.28).max(280.0).round();
    NodeMetrics {
        card_width: w,
        frame_height,
        caption_height,
        col_gap,
        row_gap,
    }
}

fn screen_frame_height(screen: &ScreenItem, capture_width: u32, metrics: NodeMetrics) -> f32 {
    let cell = screen
        .cells
        .iter()
        .find(|c| {
            c.width_px == capture_width && c.image_width.is_some() && c.image_height.is_some()
        })
        .or_else(|| {
            screen
                .cells
                .iter()
                .find(|c| c.image_width.is_some() && c.image_height.is_some())
        });
    if let Some(c) = cell
        && let (Some(iw), Some(ih)) = (c.image_width, c.image_height)
        && iw > 0
        && ih > 0
    {
        return (metrics.card_width * (ih as f32 / iw as f32))
            .round()
            .max(240.0);
    }
    metrics.frame_height
}

pub fn compute_layout(screens: &[ScreenItem], capture_width: u32) -> MapLayout {
    let metrics = node_metrics(capture_width);
    let screen_frame_heights: Vec<f32> = screens
        .iter()
        .map(|sc| screen_frame_height(sc, capture_width, metrics))
        .collect();
    let screen_card_heights: Vec<f32> = screen_frame_heights
        .iter()
        .map(|fh| fh + metrics.caption_height)
        .collect();

    let graph = ScreenGraph::new(
        screens.iter().map(|screen| screen.path.as_str()),
        screens
            .iter()
            .flat_map(|screen| &screen.outgoing)
            .map(|edge| {
                (
                    edge.from_path.as_str(),
                    edge.to_path.as_str(),
                    edge.source.as_str(),
                )
            }),
    );
    let mut isolated = Vec::new();

    let step_x = metrics.card_width + metrics.col_gap;
    let mut nodes: Vec<LayoutNode> = Vec::with_capacity(screens.len());
    let max_shelf_cols = 6usize;
    let mut shelf_col = 0usize;
    let mut shelf_y: f32 = 0.0;
    let mut shelf_row_height: f32 = 0.0;

    // Place connected components side-by-side on horizontal shelves so overview maps fill landscape viewports.
    for component in &graph.components {
        if let [(idx, _)] = component.as_slice() {
            isolated.push(*idx);
            continue;
        }
        let mut cols: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for &(idx, depth) in component {
            cols.entry(depth).or_default().push(idx);
        }
        let max_rows_per_band = if capture_width <= 480 { 3 } else { 4 };
        let mut local_positions: Vec<(usize, usize, f32)> = Vec::new();
        let mut comp_cols = 0usize;
        let mut comp_height: f32 = 0.0;
        for list in cols.into_values() {
            let sub_cols = list.len().div_ceil(max_rows_per_band).max(1);
            let mut col_y = vec![0.0_f32; sub_cols];
            for (i, idx) in list.into_iter().enumerate() {
                let sc = i % sub_cols;
                let y = col_y[sc];
                local_positions.push((idx, comp_cols + sc, y));
                col_y[sc] += screen_card_heights[idx] + metrics.row_gap;
                comp_height = comp_height.max(col_y[sc]);
            }
            comp_cols += sub_cols;
        }
        if shelf_col > 0 && shelf_col + comp_cols > max_shelf_cols {
            shelf_col = 0;
            shelf_y += shelf_row_height + metrics.row_gap * 0.6;
            shelf_row_height = 0.0;
        }
        for (idx, lc, local_y) in local_positions {
            nodes.push(LayoutNode {
                screen_idx: idx,
                x: (shelf_col + lc) as f32 * step_x,
                y: shelf_y + local_y,
                frame_height: screen_frame_heights[idx],
                card_height: screen_card_heights[idx],
            });
        }
        shelf_col += comp_cols;
        shelf_row_height = shelf_row_height.max(comp_height);
    }

    let cursor_y = if shelf_row_height > 0.0 {
        shelf_y + shelf_row_height + metrics.row_gap * 0.6
    } else {
        shelf_y
    };

    // Arrange isolated screens in a balanced landscape grid respecting each card's real height.
    if !isolated.is_empty() {
        let target_cols = ((isolated.len() as f32).sqrt().ceil() as usize).clamp(4, 6);
        let mut col_y = vec![cursor_y; target_cols];
        for (i, idx) in isolated.into_iter().enumerate() {
            let col = i % target_cols;
            let y = col_y[col];
            nodes.push(LayoutNode {
                screen_idx: idx,
                x: col as f32 * step_x,
                y,
                frame_height: screen_frame_heights[idx],
                card_height: screen_card_heights[idx],
            });
            col_y[col] += screen_card_heights[idx] + metrics.row_gap;
        }
    }

    nodes.sort_by_key(|n| n.screen_idx);

    MapLayout {
        metrics,
        nodes,
        graph,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::EdgeItem;

    fn dummy_screen(path: &str, to: Option<&str>) -> ScreenItem {
        ScreenItem {
            id: path.into(),
            path: path.into(),
            native_source: None,
            cells: Vec::new(),
            outgoing: to
                .map(|t| EdgeItem {
                    from_path: path.into(),
                    to_path: t.into(),
                    source: "verified".into(),
                })
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn layout_places_connected_and_isolated_screens_without_overlap() {
        let screens = vec![
            dummy_screen("/", Some("/login")),
            dummy_screen("/login", None),
            dummy_screen("/terms", None),
            dummy_screen("/privacy", None),
        ];
        let layout = compute_layout(&screens, 1280);
        assert_eq!(layout.nodes.len(), 4);
        assert_eq!(layout.graph.into_visible_edges(None).len(), 1);
        for i in 0..layout.nodes.len() {
            for j in (i + 1)..layout.nodes.len() {
                let a = &layout.nodes[i];
                let b = &layout.nodes[j];
                let overlap_x = (a.x - b.x).abs() < layout.metrics.card_width;
                let overlap_y = a.y < b.y + b.card_height && b.y < a.y + a.card_height;
                assert!(!(overlap_x && overlap_y), "nodes {i} and {j} overlap");
            }
        }
    }

    #[test]
    fn fit_includes_padding_and_minimum_caption_height_on_the_last_row() {
        let screens: Vec<_> = (0..8)
            .map(|i| dummy_screen(&format!("/{i}"), None))
            .collect();
        let layout = compute_layout(&screens, 1280);
        for height in [180.0, 480.0, 800.0] {
            let (zoom, width, painted_height) = layout.fit(5000.0, height);
            let card = PaintedCardMetrics::new(layout.metrics, zoom);
            assert!(width <= 5000.0 && painted_height <= height);
            assert!((height - painted_height).abs() < 0.01);
            for node in &layout.nodes {
                assert!(node.y * zoom + card.heights(node).1 <= height);
            }
        }
    }

    #[test]
    fn layout_sizes_tall_full_page_screens_without_vertical_overlap() {
        use crate::store::CellItem;

        let make_tall = |path: &str, h: u32| ScreenItem {
            id: path.into(),
            path: path.into(),
            native_source: None,
            cells: vec![CellItem {
                id: format!("c-{path}"),
                state_id: "s0".into(),
                width_px: 1280,
                locale: None,
                status_kind: "verified".into(),
                reason: None,
                screen_evidence: None,
                observed_path: Some(path.into()),
                http_errors: Vec::new(),
                blob_id: None,
                blob_path: None,
                image_width: Some(1280),
                image_height: Some(h),
            }],
            outgoing: Vec::new(),
        };

        let screens = vec![
            make_tall("/a", 3200),
            make_tall("/b", 800),
            make_tall("/c", 1600),
            make_tall("/d", 800),
            make_tall("/e", 2400),
        ];
        let layout = compute_layout(&screens, 1280);
        assert_eq!(layout.nodes[0].frame_height, 3200.0);
        assert_eq!(layout.nodes[4].frame_height, 2400.0);
        for i in 0..layout.nodes.len() {
            for j in (i + 1)..layout.nodes.len() {
                let a = &layout.nodes[i];
                let b = &layout.nodes[j];
                let overlap_x = (a.x - b.x).abs() < layout.metrics.card_width;
                let overlap_y = a.y < b.y + b.card_height && b.y < a.y + a.card_height;
                assert!(!(overlap_x && overlap_y), "tall nodes {i} and {j} overlap");
            }
        }
    }
}
