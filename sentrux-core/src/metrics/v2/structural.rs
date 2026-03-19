//! Structural debt reports built from existing health metrics and snapshot facts.

use crate::analysis::lang_registry;
use crate::core::snapshot::{flatten_files_ref, Snapshot};
use crate::core::types::{FileNode, ImportEdge};
use crate::metrics::{is_mod_declaration_edge, HealthReport};
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
}
