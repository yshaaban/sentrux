//! Structural debt reports built from existing health metrics and snapshot facts.

use crate::analysis::lang_registry;
use crate::core::snapshot::{flatten_files_ref, Snapshot};
use crate::core::types::{FileNode, ImportEdge};
use crate::metrics::testgap::is_test_file;
use crate::metrics::{is_mod_declaration_edge, is_package_index_for_path, HealthReport};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct StructuralDebtMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fan_in: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fan_out: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instability_0_10000: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dead_symbol_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dead_line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycle_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_complexity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inbound_reference_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_surface_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reachable_from_tests: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct StructuralDebtReport {
    pub kind: String,
    pub scope: String,
    pub signal_class: String,
    pub signal_families: Vec<String>,
    pub severity: String,
    pub score_0_10000: u32,
    pub summary: String,
    pub impact: String,
    pub files: Vec<String>,
    pub evidence: Vec<String>,
    pub inspection_focus: Vec<String>,
    pub metrics: StructuralDebtMetrics,
}

#[derive(Debug, Clone, Default)]
struct FileFacts {
    lang: String,
    lines: usize,
    function_count: u32,
    max_complexity: u32,
    is_test: bool,
    is_package_index: bool,
    has_entry_tag: bool,
    public_function_count: usize,
}

#[derive(Debug, Default)]
struct StructuralGraph {
    outgoing: BTreeMap<String, BTreeSet<String>>,
    incoming: BTreeMap<String, BTreeSet<String>>,
}

pub fn build_structural_debt_reports(
    snapshot: &Snapshot,
    health: &HealthReport,
) -> Vec<StructuralDebtReport> {
    let file_facts = build_file_facts(snapshot);
    let graph = build_structural_graph(snapshot);
    let mut reports = Vec::new();

    reports.extend(build_large_file_reports(health, &file_facts, &graph));
    reports.extend(build_dependency_sprawl_reports(health, &file_facts, &graph));
    reports.extend(build_unstable_hotspot_reports(health, &file_facts, &graph));
    reports.extend(build_cycle_cluster_reports(health, &file_facts));
    reports.extend(build_dead_private_code_cluster_reports(health, &file_facts));
    reports.extend(build_dead_island_reports(
        snapshot,
        health,
        &file_facts,
        &graph,
    ));

    reports.sort_by(|left, right| {
        severity_priority(&right.severity)
            .cmp(&severity_priority(&left.severity))
            .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
            .then_with(|| left.scope.cmp(&right.scope))
    });
    reports
}

fn build_file_facts(snapshot: &Snapshot) -> BTreeMap<String, FileFacts> {
    flatten_files_ref(&snapshot.root)
        .into_iter()
        .filter(|file| !file.lang.is_empty() && file.lang != "unknown")
        .map(|file| (file.path.clone(), file_facts(file)))
        .collect()
}

fn file_facts(file: &FileNode) -> FileFacts {
    let max_complexity = file
        .sa
        .as_ref()
        .and_then(|analysis| analysis.functions.as_ref())
        .map(|functions| {
            functions
                .iter()
                .map(|function| function.cc.unwrap_or(0).max(function.cog.unwrap_or(0)))
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);

    FileFacts {
        lang: file.lang.clone(),
        lines: file.lines as usize,
        function_count: file.funcs,
        max_complexity,
        is_test: file
            .sa
            .as_ref()
            .and_then(|analysis| analysis.tags.as_ref())
            .is_some_and(|tags| tags.iter().any(|tag| tag.contains("test")))
            || is_test_file(&file.path),
        is_package_index: is_package_index_for_path(&file.path),
        has_entry_tag: file
            .sa
            .as_ref()
            .and_then(|analysis| analysis.tags.as_ref())
            .is_some_and(|tags| tags.iter().any(|tag| tag == "entry")),
        public_function_count: file
            .sa
            .as_ref()
            .and_then(|analysis| analysis.functions.as_ref())
            .map(|functions| {
                functions
                    .iter()
                    .filter(|function| function.is_public)
                    .count()
            })
            .unwrap_or(0),
    }
}

fn build_structural_graph(snapshot: &Snapshot) -> StructuralGraph {
    let mut outgoing = BTreeMap::<String, BTreeSet<String>>::new();
    let mut incoming = BTreeMap::<String, BTreeSet<String>>::new();
    let mut seen = HashSet::<(String, String)>::new();

    for edge in filtered_import_edges(snapshot) {
        record_graph_edge(
            &mut outgoing,
            &mut incoming,
            &mut seen,
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
    }

    StructuralGraph { outgoing, incoming }
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
    let pair = (from_file.to_string(), to_file.to_string());
    if !seen.insert(pair.clone()) {
        return;
    }

    outgoing
        .entry(pair.0.clone())
        .or_default()
        .insert(pair.1.clone());
    incoming.entry(pair.1).or_default().insert(pair.0);
}

fn build_large_file_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<StructuralDebtReport> {
    health
        .long_files
        .iter()
        .filter_map(|file_metric| {
            let facts = file_facts.get(&file_metric.path)?;
            let threshold = lang_registry::profile(&facts.lang).thresholds.large_file_lines;
            let score_0_10000 =
                large_file_score(file_metric.value, threshold, facts.max_complexity);

            Some(StructuralDebtReport {
                kind: "large_file".to_string(),
                scope: file_metric.path.clone(),
                signal_class: "debt".to_string(),
                signal_families: vec!["size".to_string(), "coordination".to_string()],
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: format!(
                    "File '{}' is {} lines, above the {} threshold of {}",
                    file_metric.path, file_metric.value, facts.lang, threshold
                ),
                impact: "Responsibility concentration increases review friction and makes later splits harder to isolate.".to_string(),
                files: vec![file_metric.path.clone()],
                evidence: dedupe_strings_preserve_order(vec![
                    format!("line count: {}", file_metric.value),
                    format!("large-file threshold: {}", threshold),
                    format!("function count: {}", facts.function_count),
                    format!("peak function complexity: {}", facts.max_complexity),
                    format!(
                        "outbound dependencies: {}",
                        graph.outgoing.get(&file_metric.path).map(|paths| paths.len()).unwrap_or(0)
                    ),
                ]),
                inspection_focus: vec![
                    "inspect whether orchestration, adapters, and data shaping are accumulating in one file".to_string(),
                    "inspect whether the file can be split along responsibility boundaries instead of line-count slices".to_string(),
                ],
                metrics: StructuralDebtMetrics {
                    file_count: Some(1),
                    line_count: Some(file_metric.value),
                    function_count: Some(facts.function_count),
                    fan_out: Some(
                        graph.outgoing.get(&file_metric.path).map(|paths| paths.len()).unwrap_or(0),
                    ),
                    max_complexity: Some(facts.max_complexity),
                    ..StructuralDebtMetrics::default()
                },
            })
        })
        .collect()
}

fn build_dependency_sprawl_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<StructuralDebtReport> {
    health
        .god_files
        .iter()
        .filter_map(|file_metric| {
            let facts = file_facts.get(&file_metric.path)?;
            let fan_in = graph.incoming.get(&file_metric.path).map(|paths| paths.len()).unwrap_or(0);
            let fan_out = graph.outgoing.get(&file_metric.path).map(|paths| paths.len()).unwrap_or(0);
            let threshold = lang_registry::profile(&facts.lang).thresholds.fan_out;
            let instability = instability_0_10000(fan_in, fan_out);
            let score_0_10000 = dependency_sprawl_score(fan_out, threshold, instability);
            let dependency_examples = sample_paths(graph.outgoing.get(&file_metric.path), 3);

            Some(StructuralDebtReport {
                kind: "dependency_sprawl".to_string(),
                scope: file_metric.path.clone(),
                signal_class: "debt".to_string(),
                signal_families: vec!["coupling".to_string(), "coordination".to_string()],
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: format!(
                    "File '{}' depends on {} real surfaces, above the {} threshold of {}",
                    file_metric.path, fan_out, facts.lang, threshold
                ),
                impact: "Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.".to_string(),
                files: vec![file_metric.path.clone()],
                evidence: dedupe_strings_preserve_order(vec![
                    format!("fan-out: {}", fan_out),
                    format!("fan-out threshold: {}", threshold),
                    format!("instability: {:.2}", instability as f64 / 10_000.0),
                    if dependency_examples.is_empty() {
                        "sample dependencies: none".to_string()
                    } else {
                        format!("sample dependencies: {}", dependency_examples.join(", "))
                    },
                ]),
                inspection_focus: vec![
                    "inspect whether orchestration and policy code can move behind narrower helpers".to_string(),
                    "inspect whether unrelated adapter dependencies are accumulating in one module".to_string(),
                ],
                metrics: StructuralDebtMetrics {
                    file_count: Some(1),
                    line_count: Some(facts.lines),
                    function_count: Some(facts.function_count),
                    fan_in: Some(fan_in),
                    fan_out: Some(fan_out),
                    instability_0_10000: Some(instability),
                    max_complexity: Some(facts.max_complexity),
                    ..StructuralDebtMetrics::default()
                },
            })
        })
        .collect()
}

fn build_unstable_hotspot_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<StructuralDebtReport> {
    health
        .hotspot_files
        .iter()
        .filter_map(|file_metric| {
            let facts = file_facts.get(&file_metric.path)?;
            let fan_in = graph.incoming.get(&file_metric.path).map(|paths| paths.len()).unwrap_or(0);
            let fan_out = graph.outgoing.get(&file_metric.path).map(|paths| paths.len()).unwrap_or(0);
            let threshold = lang_registry::profile(&facts.lang).thresholds.fan_in;
            let instability = instability_0_10000(fan_in, fan_out);
            let score_0_10000 = unstable_hotspot_score(fan_in, threshold, instability);
            let dependent_examples = sample_paths(graph.incoming.get(&file_metric.path), 3);

            Some(StructuralDebtReport {
                kind: "unstable_hotspot".to_string(),
                scope: file_metric.path.clone(),
                signal_class: "debt".to_string(),
                signal_families: vec!["coupling".to_string(), "blast_radius".to_string()],
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: format!(
                    "File '{}' has {} inbound references and remains unstable",
                    file_metric.path, fan_in
                ),
                impact: "High fan-in plus instability increases blast radius and makes small edits harder to contain.".to_string(),
                files: vec![file_metric.path.clone()],
                evidence: dedupe_strings_preserve_order(vec![
                    format!("fan-in: {}", fan_in),
                    format!("hotspot threshold: {}", threshold),
                    format!("fan-out: {}", fan_out),
                    format!("instability: {:.2}", instability as f64 / 10_000.0),
                    if dependent_examples.is_empty() {
                        "sample dependents: none".to_string()
                    } else {
                        format!("sample dependents: {}", dependent_examples.join(", "))
                    },
                ]),
                inspection_focus: vec![
                    "inspect whether a stable contract can be split from the volatile implementation".to_string(),
                    "inspect whether too many callers depend directly on an orchestration-heavy file".to_string(),
                ],
                metrics: StructuralDebtMetrics {
                    file_count: Some(1),
                    line_count: Some(facts.lines),
                    function_count: Some(facts.function_count),
                    fan_in: Some(fan_in),
                    fan_out: Some(fan_out),
                    instability_0_10000: Some(instability),
                    max_complexity: Some(facts.max_complexity),
                    ..StructuralDebtMetrics::default()
                },
            })
        })
        .collect()
}

fn build_cycle_cluster_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
) -> Vec<StructuralDebtReport> {
    health
        .circular_dep_files
        .iter()
        .map(|files| {
            let scope = format!("cycle:{}", files.join("|"));
            let total_lines = files
                .iter()
                .map(|path| file_facts.get(path).map(|facts| facts.lines).unwrap_or(0))
                .sum::<usize>();
            let max_complexity = files
                .iter()
                .filter_map(|path| file_facts.get(path).map(|facts| facts.max_complexity))
                .max()
                .unwrap_or(0);
            let score_0_10000 = cycle_cluster_score(files.len(), total_lines);

            StructuralDebtReport {
                kind: "cycle_cluster".to_string(),
                scope,
                signal_class: "debt".to_string(),
                signal_families: vec!["dependency".to_string(), "layering".to_string()],
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: format!(
                    "Files {} form a dependency cycle",
                    files.join(", ")
                ),
                impact: "The cycle prevents clean layering and makes initialization order and refactors harder to isolate.".to_string(),
                files: files.clone(),
                evidence: vec![
                    format!("cycle size: {}", files.len()),
                    format!("total lines in cycle: {}", total_lines),
                    format!("peak function complexity inside cycle: {}", max_complexity),
                ],
                inspection_focus: vec![
                    "inspect whether one back-edge can be removed by splitting contracts from implementations".to_string(),
                    "inspect whether shared types can move to a lower-dependency seam".to_string(),
                ],
                metrics: StructuralDebtMetrics {
                    file_count: Some(files.len()),
                    line_count: Some(total_lines),
                    cycle_size: Some(files.len()),
                    max_complexity: Some(max_complexity),
                    ..StructuralDebtMetrics::default()
                },
            }
        })
        .collect()
}

fn build_dead_private_code_cluster_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
) -> Vec<StructuralDebtReport> {
    let mut dead_by_file = BTreeMap::<String, Vec<_>>::new();
    for function in &health.dead_functions {
        dead_by_file
            .entry(function.file.clone())
            .or_default()
            .push(function.clone());
    }

    dead_by_file
        .into_iter()
        .filter_map(|(path, functions)| {
            let dead_symbol_count = functions.len();
            let dead_line_count = functions.iter().map(|function| function.value as usize).sum::<usize>();
            if dead_symbol_count < 2 && dead_line_count < 40 {
                return None;
            }
            let facts = file_facts.get(&path)?;
            let score_0_10000 = dead_private_cluster_score(dead_symbol_count, dead_line_count);
            let function_names = functions
                .iter()
                .take(3)
                .map(|function| function.func.clone())
                .collect::<Vec<_>>();

            Some(StructuralDebtReport {
                kind: "dead_private_code_cluster".to_string(),
                scope: path.clone(),
                signal_class: if dead_line_count >= 80 { "debt" } else { "watchpoint" }.to_string(),
                signal_families: vec!["staleness".to_string(), "maintainability".to_string()],
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: format!(
                    "File '{}' contains {} uncalled private functions totaling {} lines",
                    path, dead_symbol_count, dead_line_count
                ),
                impact: "Stale private code increases maintenance noise and can mislead future edits into reviving obsolete paths.".to_string(),
                files: vec![path.clone()],
                evidence: dedupe_strings_preserve_order(vec![
                    format!("dead private functions: {}", dead_symbol_count),
                    format!("dead private lines: {}", dead_line_count),
                    format!("sample dead functions: {}", function_names.join(", ")),
                    format!("total file lines: {}", facts.lines),
                ]),
                inspection_focus: vec![
                    "inspect whether the dead helpers should be deleted or intentionally reconnected".to_string(),
                    "inspect whether the file still reflects the supported control flow".to_string(),
                ],
                metrics: StructuralDebtMetrics {
                    file_count: Some(1),
                    line_count: Some(facts.lines),
                    function_count: Some(facts.function_count),
                    dead_symbol_count: Some(dead_symbol_count),
                    dead_line_count: Some(dead_line_count),
                    max_complexity: Some(facts.max_complexity),
                    ..StructuralDebtMetrics::default()
                },
            })
        })
        .collect()
}

fn build_dead_island_reports(
    snapshot: &Snapshot,
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<StructuralDebtReport> {
    let app_roots = application_root_files(snapshot, file_facts, graph);
    if app_roots.is_empty() {
        return Vec::new();
    }

    let test_roots = file_facts
        .iter()
        .filter(|(_, facts)| facts.is_test)
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    let app_reachable = reachable_files(graph, &app_roots);
    let test_reachable = reachable_files(graph, &test_roots);
    let cycle_size_by_file = cycle_size_by_file(health);

    weak_components(file_facts, graph)
        .into_iter()
        .filter_map(|component| {
            let component_set = component.iter().cloned().collect::<BTreeSet<_>>();
            let is_app_reachable = component.iter().any(|path| app_reachable.contains(path));
            if is_app_reachable {
                return None;
            }

            let public_surface_count = component
                .iter()
                .map(|path| {
                    file_facts
                        .get(path)
                        .map(|facts| facts.public_function_count)
                        .unwrap_or(0)
                })
                .sum::<usize>();
            if public_surface_count > 0 {
                return None;
            }
            let has_entry_or_package_surface = component.iter().any(|path| {
                file_facts
                    .get(path)
                    .is_some_and(|facts| facts.is_package_index || facts.has_entry_tag)
            });
            if has_entry_or_package_surface {
                return None;
            }

            let inbound_reference_count =
                external_non_test_inbound_count(&component_set, file_facts, graph);
            if inbound_reference_count > 0 {
                return None;
            }

            let cycle_size = component
                .iter()
                .filter_map(|path| cycle_size_by_file.get(path).copied())
                .max()
                .unwrap_or(0);
            let total_lines = component
                .iter()
                .map(|path| file_facts.get(path).map(|facts| facts.lines).unwrap_or(0))
                .sum::<usize>();
            let reachable_from_tests = component.iter().any(|path| test_reachable.contains(path));

            if component.len() < 2 && cycle_size < 2 {
                return None;
            }

            let score_0_10000 =
                dead_island_score(component.len(), total_lines, cycle_size, reachable_from_tests);
            let scope = format!("dead_island:{}", component.join("|"));
            let sample_files = component
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            let evidence = dedupe_strings_preserve_order(vec![
                format!("component file count: {}", component.len()),
                format!("component lines: {}", total_lines),
                format!("largest internal cycle: {}", cycle_size),
                format!("external inbound references from app graph: {}", inbound_reference_count),
                format!("reachable from tests: {}", reachable_from_tests),
                format!("sample files: {}", sample_files),
            ]);

            Some(StructuralDebtReport {
                kind: "dead_island".to_string(),
                scope,
                signal_class: if reachable_from_tests {
                    "watchpoint".to_string()
                } else {
                    "debt".to_string()
                },
                signal_families: vec!["reachability".to_string(), "staleness".to_string()],
                severity: signal_severity(score_0_10000).to_string(),
                score_0_10000,
                summary: if reachable_from_tests {
                    format!(
                        "Files {} form an internally connected component that is not reachable from app roots",
                        component.join(", ")
                    )
                } else {
                    format!(
                        "Files {} form an internally connected component that is disconnected from the app-reachable graph",
                        component.join(", ")
                    )
                },
                impact: if reachable_from_tests {
                    "A test-only internal component may be stale production code or an accidentally disconnected subsystem.".to_string()
                } else {
                    "A disconnected internal component adds maintenance noise and can hide obsolete or unsupported code paths.".to_string()
                },
                files: component.clone(),
                evidence,
                inspection_focus: vec![
                    "inspect whether this component is intentionally disconnected or stale".to_string(),
                    "inspect whether it should be deleted, archived, or wired through an explicit root".to_string(),
                ],
                metrics: StructuralDebtMetrics {
                    file_count: Some(component.len()),
                    line_count: Some(total_lines),
                    cycle_size: Some(cycle_size),
                    inbound_reference_count: Some(inbound_reference_count),
                    public_surface_count: Some(public_surface_count),
                    reachable_from_tests: Some(reachable_from_tests),
                    ..StructuralDebtMetrics::default()
                },
            })
        })
        .collect()
}

fn application_root_files(
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

#[cfg(test)]
fn has_dead_island_report(reports: &[StructuralDebtReport], expected_files: &[&str]) -> bool {
    let expected_files = expected_files
        .iter()
        .map(|path| path.to_string())
        .collect::<Vec<_>>();
    reports
        .iter()
        .any(|report| report.kind == "dead_island" && report.files == expected_files)
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

fn reachable_files(graph: &StructuralGraph, roots: &BTreeSet<String>) -> BTreeSet<String> {
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

fn weak_components(
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

fn external_non_test_inbound_count(
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

fn cycle_size_by_file(health: &HealthReport) -> BTreeMap<String, usize> {
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

fn sample_paths(paths: Option<&BTreeSet<String>>, limit: usize) -> Vec<String> {
    paths
        .map(|paths| paths.iter().take(limit).cloned().collect())
        .unwrap_or_default()
}

fn large_file_score(line_count: usize, threshold: u32, max_complexity: u32) -> u32 {
    let over_threshold = scaled_ratio_pressure(line_count, threshold as usize, 3600);
    let complexity_bonus = max_complexity.saturating_sub(20).min(20) * 120;
    (2400 + over_threshold + complexity_bonus).min(10_000)
}

fn dependency_sprawl_score(fan_out: usize, threshold: usize, instability_0_10000: u32) -> u32 {
    let over_threshold = scaled_ratio_pressure(fan_out, threshold as usize, 3200);
    let instability_bonus = instability_0_10000 / 4;
    (2800 + over_threshold + instability_bonus).min(10_000)
}

fn unstable_hotspot_score(fan_in: usize, threshold: usize, instability_0_10000: u32) -> u32 {
    let over_threshold = scaled_ratio_pressure(fan_in, threshold as usize, 3000);
    let instability_bonus = instability_0_10000 / 3;
    (3200 + over_threshold + instability_bonus).min(10_000)
}

fn cycle_cluster_score(file_count: usize, total_lines: usize) -> u32 {
    let size_bonus = (file_count as u32 * 900).min(3600);
    let line_bonus = (total_lines as u32 / 12).min(2200);
    (3000 + size_bonus + line_bonus).min(10_000)
}

fn dead_private_cluster_score(dead_symbol_count: usize, dead_line_count: usize) -> u32 {
    let symbol_bonus = (dead_symbol_count as u32 * 900).min(3600);
    let line_bonus = (dead_line_count as u32 * 18).min(2800);
    (1500 + symbol_bonus + line_bonus).min(10_000)
}

fn dead_island_score(
    file_count: usize,
    total_lines: usize,
    cycle_size: usize,
    reachable_from_tests: bool,
) -> u32 {
    let file_bonus = (file_count as u32 * 900).min(3600);
    let line_bonus = (total_lines as u32 / 10).min(2600);
    let cycle_bonus = (cycle_size as u32 * 700).min(2100);
    let test_penalty = if reachable_from_tests { 1200 } else { 0 };
    (2800 + file_bonus + line_bonus + cycle_bonus).saturating_sub(test_penalty)
}

fn scaled_ratio_pressure(value: usize, threshold: usize, max_bonus: u32) -> u32 {
    if threshold == 0 || value <= threshold {
        return 0;
    }

    let pressure = ((value - threshold) as f64 / threshold as f64).min(1.0);
    (pressure * max_bonus as f64).round() as u32
}

fn instability_0_10000(fan_in: usize, fan_out: usize) -> u32 {
    let total = fan_in + fan_out;
    let instability = if total == 0 {
        0.5
    } else {
        fan_out as f64 / total as f64
    };
    (instability * 10_000.0).round() as u32
}

fn signal_severity(score_0_10000: u32) -> &'static str {
    match score_0_10000 {
        6500..=10_000 => "high",
        3000..=6499 => "medium",
        _ => "low",
    }
}

fn severity_priority(severity: &str) -> u8 {
    match severity {
        "high" => 3,
        "medium" => 2,
        _ => 1,
    }
}

fn dedupe_strings_preserve_order(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{CallEdge, EntryPoint, FileNode, StructuralAnalysis};
    use crate::metrics::root_causes::{RootCauseRaw, RootCauseScores};
    use crate::metrics::{FileMetric, FuncMetric};
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn reports_large_files_sprawl_hotspots_cycles_and_dead_private_clusters() {
        let snapshot = Snapshot {
            root: Arc::new(FileNode {
                path: ".".to_string(),
                name: ".".to_string(),
                is_dir: true,
                lines: 0,
                logic: 0,
                comments: 0,
                blanks: 0,
                funcs: 0,
                mtime: 0.0,
                gs: String::new(),
                lang: String::new(),
                sa: None,
                children: Some(vec![
                    test_file("src/app.ts", 720, 6, 28),
                    test_file("src/a.ts", 120, 3, 12),
                    test_file("src/b.ts", 120, 3, 16),
                    test_file("src/unused.ts", 110, 5, 8),
                ]),
            }),
            total_files: 4,
            total_lines: 1070,
            total_dirs: 1,
            import_graph: vec![
                ImportEdge {
                    from_file: "src/app.ts".into(),
                    to_file: "src/a.ts".into(),
                },
                ImportEdge {
                    from_file: "src/a.ts".into(),
                    to_file: "src/b.ts".into(),
                },
                ImportEdge {
                    from_file: "src/b.ts".into(),
                    to_file: "src/a.ts".into(),
                },
            ],
            call_graph: vec![
                CallEdge {
                    from_file: "src/app.ts".into(),
                    from_func: "main".into(),
                    to_file: "src/a.ts".into(),
                    to_func: "helper".into(),
                },
                CallEdge {
                    from_file: "src/app.ts".into(),
                    from_func: "main".into(),
                    to_file: "src/b.ts".into(),
                    to_func: "helper".into(),
                },
            ],
            inherit_graph: Vec::new(),
            entry_points: vec![EntryPoint {
                file: "src/app.ts".into(),
                func: "main".into(),
                lang: "typescript".into(),
                confidence: "high".into(),
            }],
            exec_depth: HashMap::new(),
        };
        let health = HealthReport {
            coupling_score: 0.0,
            circular_dep_count: 1,
            circular_dep_files: vec![vec!["src/a.ts".into(), "src/b.ts".into()]],
            total_import_edges: 3,
            cross_module_edges: 0,
            entropy: 0.0,
            entropy_bits: 0.0,
            avg_cohesion: None,
            max_depth: 2,
            god_files: vec![FileMetric {
                path: "src/app.ts".into(),
                value: 8,
            }],
            hotspot_files: vec![FileMetric {
                path: "src/a.ts".into(),
                value: 4,
            }],
            most_unstable: Vec::new(),
            complex_functions: Vec::new(),
            long_functions: Vec::new(),
            cog_complex_functions: Vec::new(),
            high_param_functions: Vec::new(),
            duplicate_groups: Vec::new(),
            dead_functions: vec![
                FuncMetric {
                    file: "src/unused.ts".into(),
                    func: "orphanAlpha".into(),
                    value: 24,
                },
                FuncMetric {
                    file: "src/unused.ts".into(),
                    func: "orphanBeta".into(),
                    value: 20,
                },
            ],
            long_files: vec![FileMetric {
                path: "src/app.ts".into(),
                value: 720,
            }],
            all_function_ccs: Vec::new(),
            all_function_lines: Vec::new(),
            all_file_lines: Vec::new(),
            god_file_ratio: 0.0,
            hotspot_ratio: 0.0,
            complex_fn_ratio: 0.0,
            long_fn_ratio: 0.0,
            comment_ratio: None,
            large_file_count: 1,
            large_file_ratio: 0.0,
            duplication_ratio: 0.0,
            dead_code_ratio: 0.0,
            high_param_ratio: 0.0,
            cog_complex_ratio: 0.0,
            quality_signal: 0.0,
            root_cause_raw: RootCauseRaw {
                modularity_q: 0.0,
                cycle_count: 1,
                max_depth: 2,
                complexity_gini: 0.0,
                redundancy_ratio: 0.0,
            },
            root_cause_scores: RootCauseScores {
                modularity: 0.0,
                acyclicity: 0.0,
                depth: 0.0,
                equality: 0.0,
                redundancy: 0.0,
            },
        };

        let reports = build_structural_debt_reports(&snapshot, &health);
        let kinds = reports
            .iter()
            .map(|report| report.kind.as_str())
            .collect::<Vec<_>>();

        assert!(kinds.contains(&"large_file"));
        assert!(kinds.contains(&"dependency_sprawl"));
        assert!(kinds.contains(&"unstable_hotspot"));
        assert!(kinds.contains(&"cycle_cluster"));
        assert!(kinds.contains(&"dead_private_code_cluster"));
    }

    #[test]
    fn reports_dead_island_for_disconnected_internal_cycle() {
        let snapshot = Snapshot {
            root: Arc::new(FileNode {
                path: ".".to_string(),
                name: ".".to_string(),
                is_dir: true,
                lines: 0,
                logic: 0,
                comments: 0,
                blanks: 0,
                funcs: 0,
                mtime: 0.0,
                gs: String::new(),
                lang: String::new(),
                sa: None,
                children: Some(vec![
                    test_file("src/app.ts", 120, 2, 10),
                    test_file("src/live.ts", 80, 2, 8),
                    test_file("src/orphan-a.ts", 90, 2, 6),
                    test_file("src/orphan-b.ts", 95, 2, 7),
                ]),
            }),
            total_files: 4,
            total_lines: 385,
            total_dirs: 1,
            import_graph: vec![
                ImportEdge {
                    from_file: "src/app.ts".into(),
                    to_file: "src/live.ts".into(),
                },
                ImportEdge {
                    from_file: "src/orphan-a.ts".into(),
                    to_file: "src/orphan-b.ts".into(),
                },
                ImportEdge {
                    from_file: "src/orphan-b.ts".into(),
                    to_file: "src/orphan-a.ts".into(),
                },
            ],
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: vec![EntryPoint {
                file: "src/app.ts".into(),
                func: "main".into(),
                lang: "typescript".into(),
                confidence: "high".into(),
            }],
            exec_depth: HashMap::new(),
        };
        let health = HealthReport {
            coupling_score: 0.0,
            circular_dep_count: 1,
            circular_dep_files: vec![vec!["src/orphan-a.ts".into(), "src/orphan-b.ts".into()]],
            total_import_edges: 3,
            cross_module_edges: 0,
            entropy: 0.0,
            entropy_bits: 0.0,
            avg_cohesion: None,
            max_depth: 1,
            god_files: Vec::new(),
            hotspot_files: Vec::new(),
            most_unstable: Vec::new(),
            complex_functions: Vec::new(),
            long_functions: Vec::new(),
            cog_complex_functions: Vec::new(),
            high_param_functions: Vec::new(),
            duplicate_groups: Vec::new(),
            dead_functions: Vec::new(),
            long_files: Vec::new(),
            all_function_ccs: Vec::new(),
            all_function_lines: Vec::new(),
            all_file_lines: Vec::new(),
            god_file_ratio: 0.0,
            hotspot_ratio: 0.0,
            complex_fn_ratio: 0.0,
            long_fn_ratio: 0.0,
            comment_ratio: None,
            large_file_count: 0,
            large_file_ratio: 0.0,
            duplication_ratio: 0.0,
            dead_code_ratio: 0.0,
            high_param_ratio: 0.0,
            cog_complex_ratio: 0.0,
            quality_signal: 0.0,
            root_cause_raw: RootCauseRaw {
                modularity_q: 0.0,
                cycle_count: 1,
                max_depth: 1,
                complexity_gini: 0.0,
                redundancy_ratio: 0.0,
            },
            root_cause_scores: RootCauseScores {
                modularity: 0.0,
                acyclicity: 0.0,
                depth: 0.0,
                equality: 0.0,
                redundancy: 0.0,
            },
        };

        let reports = build_structural_debt_reports(&snapshot, &health);
        assert!(has_dead_island_report(
            &reports,
            &["src/orphan-a.ts", "src/orphan-b.ts"]
        ));
    }

    #[test]
    fn reports_dead_island_for_disconnected_non_cycle_component_when_entry_points_exist() {
        let snapshot = Snapshot {
            root: Arc::new(FileNode {
                path: ".".to_string(),
                name: ".".to_string(),
                is_dir: true,
                lines: 0,
                logic: 0,
                comments: 0,
                blanks: 0,
                funcs: 0,
                mtime: 0.0,
                gs: String::new(),
                lang: String::new(),
                sa: None,
                children: Some(vec![
                    test_file("src/app.ts", 120, 2, 10),
                    test_file("src/live.ts", 80, 2, 8),
                    test_file("src/orphan-root.ts", 90, 2, 6),
                    test_file("src/orphan-leaf.ts", 95, 2, 7),
                ]),
            }),
            total_files: 4,
            total_lines: 385,
            total_dirs: 1,
            import_graph: vec![
                ImportEdge {
                    from_file: "src/app.ts".into(),
                    to_file: "src/live.ts".into(),
                },
                ImportEdge {
                    from_file: "src/orphan-root.ts".into(),
                    to_file: "src/orphan-leaf.ts".into(),
                },
            ],
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: vec![EntryPoint {
                file: "src/app.ts".into(),
                func: "main".into(),
                lang: "typescript".into(),
                confidence: "high".into(),
            }],
            exec_depth: HashMap::new(),
        };
        let health = empty_health_report();

        let reports = build_structural_debt_reports(&snapshot, &health);
        assert!(has_dead_island_report(
            &reports,
            &["src/orphan-leaf.ts", "src/orphan-root.ts"]
        ));
    }

    #[test]
    fn does_not_report_dead_island_for_zero_inbound_root_when_no_entry_points_exist() {
        let snapshot = Snapshot {
            root: Arc::new(FileNode {
                path: ".".to_string(),
                name: ".".to_string(),
                is_dir: true,
                lines: 0,
                logic: 0,
                comments: 0,
                blanks: 0,
                funcs: 0,
                mtime: 0.0,
                gs: String::new(),
                lang: String::new(),
                sa: None,
                children: Some(vec![
                    test_file("src/root.ts", 120, 2, 10),
                    test_file("src/helper.ts", 80, 2, 8),
                ]),
            }),
            total_files: 2,
            total_lines: 200,
            total_dirs: 1,
            import_graph: vec![ImportEdge {
                from_file: "src/root.ts".into(),
                to_file: "src/helper.ts".into(),
            }],
            call_graph: Vec::new(),
            inherit_graph: Vec::new(),
            entry_points: Vec::new(),
            exec_depth: HashMap::new(),
        };
        let health = empty_health_report();

        let reports = build_structural_debt_reports(&snapshot, &health);
        assert!(!reports.iter().any(|report| report.kind == "dead_island"));
    }

    fn test_file(path: &str, lines: u32, funcs: u32, max_complexity: u32) -> FileNode {
        FileNode {
            path: path.to_string(),
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            is_dir: false,
            lines,
            logic: lines.saturating_sub(10),
            comments: 5,
            blanks: 5,
            funcs,
            mtime: 0.0,
            gs: String::new(),
            lang: "typescript".to_string(),
            sa: Some(StructuralAnalysis {
                functions: Some(vec![crate::core::types::FuncInfo {
                    n: "main".to_string(),
                    sl: 1,
                    el: lines,
                    ln: lines,
                    cc: Some(max_complexity),
                    cog: Some(max_complexity),
                    pc: Some(0),
                    bh: Some(1),
                    d: None,
                    co: None,
                    is_public: false,
                    is_method: false,
                }]),
                cls: None,
                imp: None,
                co: None,
                tags: None,
                comment_lines: None,
            }),
            children: None,
        }
    }

    fn empty_health_report() -> HealthReport {
        HealthReport {
            coupling_score: 0.0,
            circular_dep_count: 0,
            circular_dep_files: Vec::new(),
            total_import_edges: 0,
            cross_module_edges: 0,
            entropy: 0.0,
            entropy_bits: 0.0,
            avg_cohesion: None,
            max_depth: 0,
            god_files: Vec::new(),
            hotspot_files: Vec::new(),
            most_unstable: Vec::new(),
            complex_functions: Vec::new(),
            long_functions: Vec::new(),
            cog_complex_functions: Vec::new(),
            high_param_functions: Vec::new(),
            duplicate_groups: Vec::new(),
            dead_functions: Vec::new(),
            long_files: Vec::new(),
            all_function_ccs: Vec::new(),
            all_function_lines: Vec::new(),
            all_file_lines: Vec::new(),
            god_file_ratio: 0.0,
            hotspot_ratio: 0.0,
            complex_fn_ratio: 0.0,
            long_fn_ratio: 0.0,
            comment_ratio: None,
            large_file_count: 0,
            large_file_ratio: 0.0,
            duplication_ratio: 0.0,
            dead_code_ratio: 0.0,
            high_param_ratio: 0.0,
            cog_complex_ratio: 0.0,
            quality_signal: 0.0,
            root_cause_raw: RootCauseRaw {
                modularity_q: 0.0,
                cycle_count: 0,
                max_depth: 0,
                complexity_gini: 0.0,
                redundancy_ratio: 0.0,
            },
            root_cause_scores: RootCauseScores {
                modularity: 0.0,
                acyclicity: 0.0,
                depth: 0.0,
                equality: 0.0,
                redundancy: 0.0,
            },
        }
    }
}
