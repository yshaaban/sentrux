use super::FileFacts;
use crate::core::snapshot::Snapshot;
use crate::core::types::ImportEdge;
use crate::metrics::{is_mod_declaration_edge, HealthReport};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug, Default)]
pub(super) struct StructuralGraph {
    pub(super) outgoing: BTreeMap<String, BTreeSet<String>>,
    pub(super) incoming: BTreeMap<String, BTreeSet<String>>,
    pub(super) import_outgoing: BTreeMap<String, BTreeSet<String>>,
    pub(super) import_incoming: BTreeMap<String, BTreeSet<String>>,
    pub(super) call_outgoing: BTreeMap<String, BTreeSet<String>>,
    pub(super) call_incoming: BTreeMap<String, BTreeSet<String>>,
}

pub(super) fn build_structural_graph(snapshot: &Snapshot) -> StructuralGraph {
    let mut outgoing = BTreeMap::<String, BTreeSet<String>>::new();
    let mut incoming = BTreeMap::<String, BTreeSet<String>>::new();
    let mut import_outgoing = BTreeMap::<String, BTreeSet<String>>::new();
    let mut import_incoming = BTreeMap::<String, BTreeSet<String>>::new();
    let mut call_outgoing = BTreeMap::<String, BTreeSet<String>>::new();
    let mut call_incoming = BTreeMap::<String, BTreeSet<String>>::new();
    let mut seen = HashSet::<(String, String)>::new();
    let mut import_seen = HashSet::<(String, String)>::new();
    let mut call_seen = HashSet::<(String, String)>::new();

    for edge in filtered_import_edges(snapshot) {
        record_graph_edge(
            &mut outgoing,
            &mut incoming,
            &mut seen,
            &edge.from_file,
            &edge.to_file,
        );
        record_graph_edge(
            &mut import_outgoing,
            &mut import_incoming,
            &mut import_seen,
            &edge.from_file,
            &edge.to_file,
        );
    }
    for edge in &snapshot.call_graph {
        record_graph_edge(
            &mut outgoing,
            &mut incoming,
            &mut seen,
            &edge.from_file,
            &edge.to_file,
        );
        record_graph_edge(
            &mut call_outgoing,
            &mut call_incoming,
            &mut call_seen,
            &edge.from_file,
            &edge.to_file,
        );
    }
    StructuralGraph {
        outgoing,
        incoming,
        import_outgoing,
        import_incoming,
        call_outgoing,
        call_incoming,
    }
}

fn filtered_import_edges(snapshot: &Snapshot) -> impl Iterator<Item = &ImportEdge> {
    snapshot
        .import_graph
        .iter()
        .filter(|edge| !is_mod_declaration_edge(edge))
}

fn record_graph_edge(
    outgoing: &mut BTreeMap<String, BTreeSet<String>>,
    incoming: &mut BTreeMap<String, BTreeSet<String>>,
    seen: &mut HashSet<(String, String)>,
    from_file: &str,
    to_file: &str,
) {
    let from = from_file.to_string();
    let to = to_file.to_string();
    if !seen.insert((from.clone(), to.clone())) {
        return;
    }

    outgoing.entry(from.clone()).or_default().insert(to.clone());
    incoming.entry(to).or_default().insert(from);
}

pub(super) fn application_root_files(
    snapshot: &Snapshot,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> BTreeSet<String> {
    let explicit_roots = snapshot
        .entry_points
        .iter()
        .map(|entry| entry.file.clone())
        .filter(|path| file_facts.get(path).is_some_and(|facts| !facts.is_test))
        .collect::<BTreeSet<_>>();

    let mut roots = explicit_roots;
    roots.extend(
        file_facts
            .iter()
            .filter(|(_, facts)| facts.has_entry_tag || facts.is_package_index)
            .filter(|(_, facts)| !facts.is_test)
            .map(|(path, _)| path.clone()),
    );

    if !roots.is_empty() {
        return roots;
    }

    file_facts
        .iter()
        .filter(|(_, facts)| !facts.is_test)
        .filter(|(path, facts)| is_zero_inbound_root_candidate(path, facts, file_facts, graph))
        .map(|(path, _)| path.clone())
        .collect()
}

fn is_zero_inbound_root_candidate(
    path: &str,
    facts: &FileFacts,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> bool {
    let inbound_only_from_tests = graph.incoming.get(path).is_none_or(|sources| {
        sources.iter().all(|source| {
            file_facts
                .get(source)
                .is_some_and(|source_facts| source_facts.is_test)
        })
    });
    let has_surface = graph
        .outgoing
        .get(path)
        .is_some_and(|targets| !targets.is_empty())
        || facts.public_function_count > 0;

    inbound_only_from_tests && has_surface
}

pub(super) fn reachable_files(
    graph: &StructuralGraph,
    roots: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut visited = roots.clone();
    let mut queue = roots.iter().cloned().collect::<Vec<_>>();

    while let Some(path) = queue.pop() {
        if let Some(targets) = graph.outgoing.get(&path) {
            for target in targets {
                if visited.insert(target.clone()) {
                    queue.push(target.clone());
                }
            }
        }
    }

    visited
}

pub(super) fn weak_components(
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<Vec<String>> {
    let relevant_files = file_facts
        .iter()
        .filter(|(_, facts)| !facts.is_test)
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    let mut components = Vec::new();

    for start in relevant_files {
        if !visited.insert(start.clone()) {
            continue;
        }
        let mut queue = vec![start.clone()];
        let mut component = vec![start];
        while let Some(path) = queue.pop() {
            for neighbor in weak_neighbors(graph, &path) {
                if visited.insert(neighbor.clone()) {
                    queue.push(neighbor.clone());
                    component.push(neighbor);
                }
            }
        }
        component.sort();
        components.push(component);
    }

    components
}

fn weak_neighbors(graph: &StructuralGraph, path: &str) -> Vec<String> {
    let mut neighbors = BTreeSet::new();
    if let Some(targets) = graph.outgoing.get(path) {
        neighbors.extend(targets.iter().cloned());
    }
    if let Some(sources) = graph.incoming.get(path) {
        neighbors.extend(sources.iter().cloned());
    }
    neighbors.into_iter().collect()
}

pub(super) fn external_non_test_inbound_count(
    component: &BTreeSet<String>,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> usize {
    let mut sources = BTreeSet::new();
    for path in component {
        if let Some(incoming) = graph.incoming.get(path) {
            for source in incoming {
                if component.contains(source) {
                    continue;
                }
                if file_facts.get(source).is_some_and(|facts| !facts.is_test) {
                    sources.insert(source.clone());
                }
            }
        }
    }
    sources.len()
}

pub(super) fn cycle_size_by_file(health: &HealthReport) -> BTreeMap<String, usize> {
    let mut sizes = BTreeMap::new();
    for cycle in &health.circular_dep_files {
        for path in cycle {
            sizes
                .entry(path.clone())
                .and_modify(|size: &mut usize| *size = (*size).max(cycle.len()))
                .or_insert(cycle.len());
        }
    }
    sizes
}
