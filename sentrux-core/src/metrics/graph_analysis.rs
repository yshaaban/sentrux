use crate::core::types::{EntryPoint, FileNode, ImportEdge};
use crate::metrics::stability::{
    compute_avg_cohesion, compute_coupling_score, compute_shannon_entropy, compute_stable_modules,
};
use crate::metrics::types::ModuleMetrics;
use std::collections::{HashMap, HashSet};

pub(super) fn compute_module_metrics(
    files: &[&FileNode],
    import_edges: &[ImportEdge],
    call_edges: &[crate::core::types::CallEdge],
    entry_points: &[EntryPoint],
) -> ModuleMetrics {
    let stable_modules = compute_stable_modules(import_edges);
    let (coupling_score, cross_module_edges, _) =
        compute_coupling_score(import_edges, &stable_modules);
    let (entropy_raw, entropy_bits, entropy_num_pairs) =
        compute_shannon_entropy(import_edges, &stable_modules);
    let magnitude = (coupling_score / 0.35).min(1.0);
    let entropy = entropy_raw * magnitude;
    let avg_cohesion = compute_avg_cohesion(import_edges, call_edges, files);
    let max_depth = compute_max_depth(import_edges, entry_points);
    let circular_dep_files = detect_cycles(import_edges);
    let circular_dep_count = circular_dep_files.len();

    ModuleMetrics {
        coupling_score,
        cross_module_edges,
        entropy,
        entropy_bits,
        entropy_num_pairs,
        avg_cohesion,
        max_depth,
        circular_dep_files,
        circular_dep_count,
    }
}

fn build_adjacency_list(edges: &[ImportEdge]) -> (HashSet<&str>, HashMap<&str, Vec<&str>>) {
    let mut nodes: HashSet<&str> = HashSet::new();
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges {
        nodes.insert(edge.from_file.as_str());
        nodes.insert(edge.to_file.as_str());
        adjacency
            .entry(edge.from_file.as_str())
            .or_default()
            .push(edge.to_file.as_str());
    }
    (nodes, adjacency)
}

struct TarjanState<'a> {
    index_counter: u32,
    stack: Vec<&'a str>,
    on_stack: HashSet<&'a str>,
    index_map: HashMap<&'a str, u32>,
    lowlink: HashMap<&'a str, u32>,
    sccs: Vec<Vec<String>>,
}

impl<'a> TarjanState<'a> {
    fn new() -> Self {
        Self {
            index_counter: 0,
            stack: Vec::new(),
            on_stack: HashSet::new(),
            index_map: HashMap::new(),
            lowlink: HashMap::new(),
            sccs: Vec::new(),
        }
    }

    fn visit(&mut self, node: &'a str) {
        self.index_map.insert(node, self.index_counter);
        self.lowlink.insert(node, self.index_counter);
        self.index_counter += 1;
        self.stack.push(node);
        self.on_stack.insert(node);
    }

    fn update_lowlink(&mut self, node: &'a str, neighbor: &'a str) {
        if self.on_stack.contains(neighbor) {
            let neighbor_index = self.index_map[neighbor];
            let node_lowlink = self.lowlink.get_mut(node).unwrap();
            if neighbor_index < *node_lowlink {
                *node_lowlink = neighbor_index;
            }
        }
    }

    fn pop_scc(&mut self, root: &str) {
        let mut scc = Vec::new();
        loop {
            let node = self.stack.pop().unwrap();
            self.on_stack.remove(node);
            scc.push(node.to_string());
            if node == root {
                break;
            }
        }
        if scc.len() > 1 {
            scc.sort_unstable();
            self.sccs.push(scc);
        }
    }

    fn propagate_lowlink(&mut self, parent: &'a str, child_lowlink: u32) {
        let parent_lowlink = self.lowlink.get_mut(parent).unwrap();
        if child_lowlink < *parent_lowlink {
            *parent_lowlink = child_lowlink;
        }
    }
}

fn tarjan_sccs<'a>(
    nodes: &HashSet<&'a str>,
    adjacency: &HashMap<&'a str, Vec<&'a str>>,
) -> Vec<Vec<String>> {
    let mut state = TarjanState::new();

    for &start in nodes {
        if state.index_map.contains_key(start) {
            continue;
        }

        state.visit(start);
        let mut dfs_stack: Vec<(&str, usize)> = vec![(start, 0)];

        while let Some((node, neighbor_index)) = dfs_stack.last_mut() {
            let neighbors = adjacency
                .get(*node)
                .map(|values| values.as_slice())
                .unwrap_or(&[]);
            if *neighbor_index < neighbors.len() {
                let neighbor = neighbors[*neighbor_index];
                *neighbor_index += 1;

                if !state.index_map.contains_key(neighbor) {
                    state.visit(neighbor);
                    dfs_stack.push((neighbor, 0));
                } else {
                    state.update_lowlink(node, neighbor);
                }
            } else {
                let node = *node;
                let node_lowlink = state.lowlink[node];
                let node_index = state.index_map[node];

                if node_lowlink == node_index {
                    state.pop_scc(node);
                }

                dfs_stack.pop();

                if let Some((parent, _)) = dfs_stack.last() {
                    state.propagate_lowlink(parent, node_lowlink);
                }
            }
        }
    }

    state.sccs
}

fn detect_cycles(edges: &[ImportEdge]) -> Vec<Vec<String>> {
    let (nodes, adjacency) = build_adjacency_list(edges);
    tarjan_sccs(&nodes, &adjacency)
}

fn find_depth_seeds<'a>(
    edges: &'a [ImportEdge],
    entry_points: &'a [EntryPoint],
) -> (
    Vec<&'a str>,
    HashMap<&'a str, Vec<&'a str>>,
    HashSet<&'a str>,
) {
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut has_incoming: HashSet<&str> = HashSet::new();
    let mut all_nodes: HashSet<&str> = HashSet::new();

    for edge in edges {
        adjacency
            .entry(edge.from_file.as_str())
            .or_default()
            .push(edge.to_file.as_str());
        has_incoming.insert(edge.to_file.as_str());
        all_nodes.insert(edge.from_file.as_str());
        all_nodes.insert(edge.to_file.as_str());
    }

    let mut seeds: Vec<&str> = Vec::new();
    if !entry_points.is_empty() {
        for entry_point in entry_points {
            if all_nodes.contains(entry_point.file.as_str()) {
                seeds.push(entry_point.file.as_str());
            }
        }
    }
    if seeds.is_empty() {
        for &node in &all_nodes {
            if !has_incoming.contains(node) {
                seeds.push(node);
            }
        }
    }

    (seeds, adjacency, all_nodes)
}

fn dfs_propagate_to_parent(stack: &mut [(&str, usize, u32)], result: u32, node_count: usize) {
    if let Some((_parent, _index, parent_max)) = stack.last_mut() {
        let candidate = result.saturating_add(1).min(node_count as u32);
        if candidate > *parent_max {
            *parent_max = candidate;
        }
    }
}

fn longest_path_dfs<'a>(
    seeds: &[&'a str],
    adjacency: &HashMap<&'a str, Vec<&'a str>>,
    node_count: usize,
) -> HashMap<&'a str, u32> {
    let mut memo: HashMap<&str, u32> = HashMap::new();
    let mut on_stack: HashSet<&str> = HashSet::new();

    for &start in seeds {
        if memo.contains_key(start) {
            continue;
        }

        let mut stack: Vec<(&str, usize, u32)> = vec![(start, 0, 0)];
        on_stack.insert(start);

        while !stack.is_empty() {
            let (node, neighbor_index, max_child_depth) = stack.last_mut().unwrap();
            let neighbors = adjacency
                .get(*node)
                .map(|values| values.as_slice())
                .unwrap_or(&[]);

            if *neighbor_index < neighbors.len() {
                let neighbor = neighbors[*neighbor_index];
                *neighbor_index += 1;

                if let Some(&depth) = memo.get(neighbor) {
                    let candidate = depth.saturating_add(1).min(node_count as u32);
                    if candidate > *max_child_depth {
                        *max_child_depth = candidate;
                    }
                } else if !on_stack.contains(neighbor) {
                    on_stack.insert(neighbor);
                    stack.push((neighbor, 0, 0));
                }
            } else {
                let node = *node;
                let result = *max_child_depth;
                stack.pop();
                on_stack.remove(node);
                memo.insert(node, result);
                dfs_propagate_to_parent(&mut stack, result, node_count);
            }
        }
    }

    memo
}

fn compute_max_depth(edges: &[ImportEdge], entry_points: &[EntryPoint]) -> u32 {
    if edges.is_empty() {
        return 0;
    }

    let (seeds, adjacency, all_nodes) = find_depth_seeds(edges, entry_points);
    let memo = longest_path_dfs(&seeds, &adjacency, all_nodes.len());

    seeds
        .iter()
        .filter_map(|seed| memo.get(seed))
        .copied()
        .max()
        .unwrap_or(0)
}
