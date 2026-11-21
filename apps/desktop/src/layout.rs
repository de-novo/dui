//! Pure 2D screen map layout mirroring `apps/canvas/src/features/map/layout.ts`.

use crate::store::ScreenItem;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug)]
pub struct NodeMetrics {
    pub card_width: f32,
    pub frame_height: f32,
    pub caption_height: f32,
    pub card_height: f32,
    pub col_gap: f32,
    pub row_gap: f32,
}

#[derive(Clone, Debug)]
pub struct LayoutNode {
    pub screen_idx: usize,
    pub x: f32,
    pub y: f32,
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
    let col_gap = (w * 0.24).max(120.0).round();
    let row_gap = (card_height * 0.16).max(96.0).round();
    NodeMetrics {
        card_width: w,
        frame_height,
        caption_height,
        card_height,
        col_gap,
        row_gap,
    }
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

    let mut path_to_idx: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, sc) in screens.iter().enumerate() {
        path_to_idx.insert(sc.path.as_str(), i);
    }

    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); screens.len()];
    let mut undirected: Vec<Vec<usize>> = vec![Vec::new(); screens.len()];
    let mut edges = Vec::new();
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
                edges.push(LayoutEdge {
                    from_idx: i,
                    to_idx: target,
                    source: edge.source.clone(),
                });
            }
        }
    }

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
    let step_y = metrics.card_height + metrics.row_gap;
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
        let mut local_positions: Vec<(usize, usize, usize)> = Vec::new();
        let mut comp_cols = 0usize;
        let mut comp_rows = 1usize;
        for list in cols.into_values() {
            let sub_cols = list.len().div_ceil(max_rows_per_band).max(1);
            let rows_here = list.len().div_ceil(sub_cols).max(1);
            comp_rows = comp_rows.max(rows_here);
            for (i, idx) in list.into_iter().enumerate() {
                let sc = i % sub_cols;
                let sr = i / sub_cols;
                local_positions.push((idx, comp_cols + sc, sr));
            }
            comp_cols += sub_cols;
        }
        if shelf_col > 0 && shelf_col + comp_cols > max_shelf_cols {
            shelf_col = 0;
            shelf_y += shelf_row_height + metrics.row_gap * 0.6;
            shelf_row_height = 0.0;
        }
        for (idx, lc, lr) in local_positions {
            nodes.push(LayoutNode {
                screen_idx: idx,
                x: (shelf_col + lc) as f32 * step_x,
                y: shelf_y + lr as f32 * step_y,
            });
        }
        shelf_col += comp_cols;
        shelf_row_height = shelf_row_height.max(comp_rows as f32 * step_y);
    }

    let cursor_y = if shelf_row_height > 0.0 {
        shelf_y + shelf_row_height + metrics.row_gap * 0.6
    } else {
        shelf_y
    };

    // Arrange isolated screens in a balanced landscape grid matching the viewport aspect.
    if !isolated.is_empty() {
        let target_cols = ((isolated.len() as f32).sqrt().ceil() as usize).clamp(4, 6);
        for (i, idx) in isolated.into_iter().enumerate() {
            let col = i % target_cols;
            let row = i / target_cols;
            nodes.push(LayoutNode {
                screen_idx: idx,
                x: col as f32 * step_x,
                y: cursor_y + row as f32 * step_y,
            });
        }
    }

    nodes.sort_by_key(|n| n.screen_idx);

    let mut max_x: f32 = 0.0;
    let mut max_y: f32 = 0.0;
    for n in &nodes {
        max_x = max_x.max(n.x + metrics.card_width);
        max_y = max_y.max(n.y + metrics.card_height);
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
                let overlap_y = (a.y - b.y).abs() < layout.metrics.card_height;
                assert!(!(overlap_x && overlap_y), "nodes {i} and {j} overlap");
            }
        }
    }
}
