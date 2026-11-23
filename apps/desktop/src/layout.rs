//! Pure 2D screen map layout mirroring `apps/canvas/src/features/map/layout.ts`.

use crate::store::ScreenItem;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

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
pub struct LayoutEdge {
    pub from_idx: usize,
    pub to_idx: usize,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct MapLayout {
    pub metrics: NodeMetrics,
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub content_width: f32,
    pub content_height: f32,
}

pub fn node_metrics(capture_width: u32) -> NodeMetrics {
    let w = (capture_width.max(320) as f32).min(1440.0);
    let aspect = if capture_width <= 480 { 1.55 } else { 0.64 };
    let frame_height = (w * aspect).round();
    let caption_height = (w * 0.085).clamp(44.0, 96.0).round();
    let card_height = frame_height + caption_height;
    let col_gap = (w * 0.24).max(160.0).round();
    let row_gap = (card_height * 0.26).max(260.0).round();
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
        .find(|c| c.width_px == capture_width && c.image_width.is_some() && c.image_height.is_some())
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
    if screens.is_empty() {
        return MapLayout {
            metrics,
            nodes: Vec::new(),
            edges: Vec::new(),
            content_width: 0.0,
            content_height: 0.0,
        };
    }

    let screen_frame_heights: Vec<f32> = screens
        .iter()
        .map(|sc| screen_frame_height(sc, capture_width, metrics))
        .collect();
    let screen_card_heights: Vec<f32> = screen_frame_heights
        .iter()
        .map(|fh| fh + metrics.caption_height)
        .collect();

    let mut path_to_idx: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, sc) in screens.iter().enumerate() {
        path_to_idx.insert(sc.path.as_str(), i);
    }

    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); screens.len()];
    let mut undirected: Vec<Vec<usize>> = vec![Vec::new(); screens.len()];
    let mut edge_source: BTreeMap<(usize, usize), String> = BTreeMap::new();
    let mut seen_edges: BTreeSet<(usize, usize)> = BTreeSet::new();
    for (i, sc) in screens.iter().enumerate() {
        for edge in &sc.outgoing {
            if let Some(&target) = path_to_idx.get(edge.to_path.as_str())
                && target != i
                && seen_edges.insert((i, target))
            {
                adj[i].push(target);
                undirected[i].push(target);
                undirected[target].push(i);
                edge_source.insert((i, target), edge.source.clone());
            }
        }
    }
    let mut edges = Vec::new();

    // Separate connected groups (size >= 2) from isolated screens for clean grid packing.
    let mut visited = vec![false; screens.len()];
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut isolated: Vec<usize> = Vec::new();

    for start in 0..screens.len() {
        if visited[start] {
            continue;
        }
        let mut comp = Vec::new();
        let mut queue = VecDeque::new();
        visited[start] = true;
        queue.push_back(start);
        while let Some(cur) = queue.pop_front() {
            comp.push(cur);
            for &next in &undirected[cur] {
                if !visited[next] {
                    visited[next] = true;
                    queue.push_back(next);
                }
            }
        }
        comp.sort_unstable();
        if comp.len() == 1 {
            isolated.push(comp[0]);
        } else {
            groups.push(comp);
        }
    }

    let step_x = metrics.card_width + metrics.col_gap;
    let mut nodes: Vec<LayoutNode> = Vec::with_capacity(screens.len());
    let max_shelf_cols = 6usize;
    let mut shelf_col = 0usize;
    let mut shelf_y: f32 = 0.0;
    let mut shelf_row_height: f32 = 0.0;

    // Place connected components side-by-side on horizontal shelves so overview maps fill landscape viewports.
    for comp in groups {
        let root = comp[0];
        let mut depth_map: BTreeMap<usize, usize> = BTreeMap::new();
        let mut q = VecDeque::new();
        depth_map.insert(root, 0);
        q.push_back(root);
        while let Some(u) = q.pop_front() {
            let d = depth_map[&u];
            for &v in &adj[u] {
                if let std::collections::btree_map::Entry::Vacant(e) = depth_map.entry(v) {
                    e.insert(d + 1);
                    q.push_back(v);
                    // Emit clean spanning-tree forest edges matching `apps/canvas/src/features/map/layout.ts`.
                    edges.push(LayoutEdge {
                        from_idx: u,
                        to_idx: v,
                        source: edge_source
                            .get(&(u, v))
                            .cloned()
                            .unwrap_or_else(|| "extracted".into()),
                    });
                }
            }
        }
        for &idx in &comp {
            depth_map.entry(idx).or_insert(0);
        }
        let mut cols: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (idx, col) in depth_map {
            cols.entry(col).or_default().push(idx);
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

    let mut max_x: f32 = 0.0;
    let mut max_y: f32 = 0.0;
    for n in &nodes {
        max_x = max_x.max(n.x + metrics.card_width);
        max_y = max_y.max(n.y + n.card_height);
    }

    MapLayout {
        metrics,
        nodes,
        edges,
        content_width: max_x,
        content_height: max_y,
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
        assert_eq!(layout.edges.len(), 1);
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
    fn layout_sizes_tall_full_page_screens_without_vertical_overlap() {
        use crate::store::CellItem;

        let make_tall = |path: &str, h: u32| ScreenItem {
            id: path.into(),
            path: path.into(),
            cells: vec![CellItem {
                id: format!("c-{path}"),
                state_id: "s0".into(),
                width_px: 1280,
                locale: None,
                status_kind: "verified".into(),
                reason: None,
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
