//! Recorded screen connections and the spanning forest used for overview placement.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Debug)]
pub struct MapEdge {
    pub from_idx: usize,
    pub to_idx: usize,
    pub source: String,
    overview: bool,
}

impl MapEdge {
    pub fn neighbor(&self, screen: usize) -> Option<usize> {
        if self.from_idx == screen {
            Some(self.to_idx)
        } else if self.to_idx == screen {
            Some(self.from_idx)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct ScreenGraph {
    /// Within each component, pair each screen index with its layout depth.
    pub components: Vec<Vec<(usize, usize)>>,
    edges: Vec<MapEdge>,
}

impl ScreenGraph {
    pub fn new<'a>(
        paths: impl ExactSizeIterator<Item = &'a str>,
        links: impl Iterator<Item = (&'a str, &'a str, &'a str)>,
    ) -> Self {
        let mut adjacent = vec![Vec::new(); paths.len()];
        let indices: BTreeMap<_, _> = paths.enumerate().map(|(i, path)| (path, i)).collect();
        let mut edges = Vec::new();
        let mut recorded = BTreeSet::new();
        for (from, to, source) in links {
            let (Some(&from_idx), Some(&to_idx)) = (indices.get(from), indices.get(to)) else {
                continue;
            };
            if from_idx != to_idx {
                recorded.insert((from_idx, to_idx, source));
            }
        }
        for (from_idx, to_idx, source) in recorded {
            let edge_idx = edges.len();
            adjacent[from_idx].push((to_idx, edge_idx));
            adjacent[to_idx].push((from_idx, edge_idx));
            edges.push(MapEdge {
                from_idx,
                to_idx,
                source: source.into(),
                overview: false,
            });
        }

        let mut visited = vec![false; adjacent.len()];
        let mut components = Vec::new();
        for root in 0..adjacent.len() {
            if visited[root] {
                continue;
            }
            let mut component = Vec::new();
            let mut queue = VecDeque::from([(root, 0)]);
            visited[root] = true;
            while let Some((screen, depth)) = queue.pop_front() {
                component.push((screen, depth));
                for &(neighbor, edge_idx) in &adjacent[screen] {
                    if !visited[neighbor] {
                        visited[neighbor] = true;
                        edges[edge_idx].overview = true;
                        queue.push_back((neighbor, depth + 1));
                    }
                }
            }
            component.sort_unstable();
            components.push(component);
        }
        Self { components, edges }
    }

    pub fn into_visible_edges(self, focus: Option<usize>) -> Vec<MapEdge> {
        let mut visible = self.edges;
        let incident = |e: &MapEdge| focus.and_then(|id| e.neighbor(id)).is_some();
        visible.retain(|edge| edge.overview || incident(edge));
        // Paint highlighted connections over the dimmed overview.
        visible.sort_by_key(incident);
        visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Link<'a> = (&'a str, &'a str, &'a str);

    fn draw<'a>(paths: &[&'a str], links: &[Link<'a>], focus: Option<usize>) -> Vec<MapEdge> {
        ScreenGraph::new(paths.iter().copied(), links.iter().copied()).into_visible_edges(focus)
    }

    fn assert_incident(paths: &[&str], links: &[Link<'_>]) {
        for (selected, &path) in paths.iter().enumerate() {
            let visible = draw(paths, links, Some(selected));
            let actual: BTreeSet<_> = visible
                .iter()
                .filter_map(|edge| {
                    let neighbor = edge.neighbor(selected)?;
                    Some((
                        paths[neighbor],
                        paths[edge.from_idx],
                        paths[edge.to_idx],
                        edge.source.as_str(),
                    ))
                })
                .collect();
            let expected: BTreeSet<_> = links
                .iter()
                .filter_map(|&(from, to, source)| {
                    if from == to || !paths.contains(&from) || !paths.contains(&to) {
                        return None;
                    }
                    if from == path {
                        Some((to, from, to, source))
                    } else if to == path {
                        Some((from, from, to, source))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(actual, expected, "selected {path}, archive {links:?}");
        }
    }

    #[test]
    fn shared_destination_and_both_origins_show_the_stored_arrows() {
        let paths = ["/", "/downloads", "/orders"];
        let links = [
            ("/", "/downloads", "extracted"),
            ("/orders", "/downloads", "verified"),
        ];
        assert_eq!(draw(&paths, &links, None).len(), links.len());
        assert_incident(&paths, &links);
    }

    #[test]
    fn focus_restores_non_tree_links_for_every_three_screen_topology() {
        let paths = ["a", "b", "c"];
        let possible: Vec<_> = paths
            .iter()
            .flat_map(|&from| {
                paths
                    .iter()
                    .copied()
                    .filter(move |&to| to != from)
                    .map(move |to| (from, to, "extracted"))
            })
            .collect();
        for mask in 0..(1 << possible.len()) {
            let links: Vec<_> = possible
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, &link)| link)
                .collect();
            assert_incident(&paths, &links);
            // Three screens can have at most two distinct pairs before a cycle closes.
            let pairs: BTreeSet<_> = links
                .iter()
                .map(|&(a, b, _)| (a.min(b), a.max(b)))
                .collect();
            assert_eq!(
                draw(&paths, &links, None).len(),
                pairs.len().min(paths.len() - 1)
            );
        }
    }

    #[test]
    fn distinct_evidence_survives_duplicate_rows_and_unresolved_targets() {
        let paths = ["a", "b", "isolated"];
        let links = [
            ("a", "b", "verified"),
            ("a", "b", "extracted"),
            ("a", "b", "verified"),
            ("b", "a", "inferred"),
            ("b", "missing", "extracted"),
            ("a", "a", "extracted"),
        ];
        assert_incident(&paths, &links);
        assert_eq!(draw(&paths, &links, Some(0)).len(), 3);
        assert_eq!(draw(&paths, &links, None).len(), 1);
    }
}
