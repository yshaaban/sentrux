use super::graph::{
    application_root_files, cycle_size_by_file, external_non_test_inbound_count, reachable_files,
    weak_components, StructuralGraph,
};
use super::path_roles::{annotate_structural_leverage, structural_presentation_class};
use super::scoring::{dead_island_score, signal_severity};
use super::utils::dedupe_strings_preserve_order;
use super::{
    FileFacts, StructuralDebtMetrics, StructuralDebtReport, StructuralSignalClass,
    StructuralTrustTier,
};
use crate::core::snapshot::Snapshot;
use crate::metrics::HealthReport;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn build_dead_island_reports(
    snapshot: &Snapshot,
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<StructuralDebtReport> {
    let app_roots = application_root_files(snapshot, file_facts, graph);
    if app_roots.is_empty() {
        return Vec::new();
    }

    let test_roots = dead_island_test_roots(file_facts);
    let app_reachable = reachable_files(graph, &app_roots);
    let test_reachable = reachable_files(graph, &test_roots);
    let cycle_size_by_file = cycle_size_by_file(health);

    weak_components(file_facts, graph)
        .into_iter()
        .filter_map(|component| {
            let component_set = component.iter().cloned().collect::<BTreeSet<_>>();
            if skip_dead_island_component(
                &component,
                &component_set,
                file_facts,
                &app_reachable,
                graph,
            ) {
                return None;
            }
            let component_metrics = dead_island_component_metrics(
                &component,
                &component_set,
                file_facts,
                graph,
                &cycle_size_by_file,
                &test_reachable,
            )?;
            Some(build_dead_island_report(&component, component_metrics))
        })
        .collect()
}

fn dead_island_test_roots(file_facts: &BTreeMap<String, FileFacts>) -> BTreeSet<String> {
    file_facts
        .iter()
        .filter(|(_, facts)| facts.is_test)
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>()
}

fn skip_dead_island_component(
    component: &[String],
    component_set: &BTreeSet<String>,
    file_facts: &BTreeMap<String, FileFacts>,
    app_reachable: &BTreeSet<String>,
    graph: &StructuralGraph,
) -> bool {
    component.iter().any(|path| app_reachable.contains(path))
        || component
            .iter()
            .all(|path| is_dead_island_support_path(path))
        || dead_island_public_surface_count(component, file_facts) > 0
        || has_dead_island_entry_or_package_surface(component, file_facts)
        || external_non_test_inbound_count(component_set, file_facts, graph) > 0
}

fn dead_island_public_surface_count(
    component: &[String],
    file_facts: &BTreeMap<String, FileFacts>,
) -> usize {
    component
        .iter()
        .map(|path| {
            file_facts
                .get(path)
                .map(|facts| facts.public_function_count)
                .unwrap_or(0)
        })
        .sum::<usize>()
}

fn has_dead_island_entry_or_package_surface(
    component: &[String],
    file_facts: &BTreeMap<String, FileFacts>,
) -> bool {
    component.iter().any(|path| {
        file_facts
            .get(path)
            .is_some_and(|facts| facts.is_package_index || facts.has_entry_tag)
    })
}

struct DeadIslandComponentMetrics {
    public_surface_count: usize,
    inbound_reference_count: usize,
    cycle_size: usize,
    total_lines: usize,
    reachable_from_tests: bool,
}

fn dead_island_component_metrics(
    component: &[String],
    component_set: &BTreeSet<String>,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
    cycle_size_by_file: &BTreeMap<String, usize>,
    test_reachable: &BTreeSet<String>,
) -> Option<DeadIslandComponentMetrics> {
    let inbound_reference_count = external_non_test_inbound_count(component_set, file_facts, graph);
    let cycle_size = component
        .iter()
        .filter_map(|path| cycle_size_by_file.get(path).copied())
        .max()
        .unwrap_or(0);
    if component.len() < 2 && cycle_size < 2 {
        return None;
    }

    Some(DeadIslandComponentMetrics {
        public_surface_count: dead_island_public_surface_count(component, file_facts),
        inbound_reference_count,
        cycle_size,
        total_lines: component
            .iter()
            .map(|path| file_facts.get(path).map(|facts| facts.lines).unwrap_or(0))
            .sum::<usize>(),
        reachable_from_tests: component.iter().any(|path| test_reachable.contains(path)),
    })
}

fn build_dead_island_report(
    component: &[String],
    metrics: DeadIslandComponentMetrics,
) -> StructuralDebtReport {
    let score_0_10000 = dead_island_score(
        component.len(),
        metrics.total_lines,
        metrics.cycle_size,
        metrics.reachable_from_tests,
    );
    let sample_files = component
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let evidence = dedupe_strings_preserve_order(vec![
        format!("component file count: {}", component.len()),
        format!("component lines: {}", metrics.total_lines),
        format!("largest internal cycle: {}", metrics.cycle_size),
        format!(
            "external inbound references from app graph: {}",
            metrics.inbound_reference_count
        ),
        format!("reachable from tests: {}", metrics.reachable_from_tests),
        format!("sample files: {}", sample_files),
    ]);

    annotate_structural_leverage(StructuralDebtReport {
        kind: "dead_island".to_string(),
        trust_tier: StructuralTrustTier::Watchpoint,
        presentation_class: structural_presentation_class(
            "dead_island",
            component.first().map(String::as_str).unwrap_or_default(),
            StructuralTrustTier::Watchpoint,
            &Vec::new(),
        ),
        leverage_class: Default::default(),
        scope: format!("dead_island:{}", component.join("|")),
        signal_class: if metrics.reachable_from_tests {
            StructuralSignalClass::Watchpoint
        } else {
            StructuralSignalClass::Debt
        },
        signal_families: vec!["reachability".to_string(), "staleness".to_string()],
        severity: signal_severity(score_0_10000),
        score_0_10000,
        summary: dead_island_summary(component, metrics.reachable_from_tests),
        impact: dead_island_impact(metrics.reachable_from_tests),
        files: component.to_vec(),
        role_tags: Vec::new(),
        leverage_reasons: Vec::new(),
        evidence,
        inspection_focus: vec![
            "inspect whether this component is intentionally disconnected or stale".to_string(),
            "inspect whether it should be deleted, archived, or wired through an explicit root"
                .to_string(),
        ],
        candidate_split_axes: vec![
            "reachable entry surface".to_string(),
            "public contract boundary".to_string(),
        ],
        related_surfaces: component.iter().take(5).cloned().collect(),
        cut_candidates: Vec::new(),
        metrics: StructuralDebtMetrics {
            file_count: Some(component.len()),
            line_count: Some(metrics.total_lines),
            cycle_size: Some(metrics.cycle_size),
            inbound_reference_count: Some(metrics.inbound_reference_count),
            public_surface_count: Some(metrics.public_surface_count),
            reachable_from_tests: Some(metrics.reachable_from_tests),
            cut_candidate_count: Some(0),
            largest_cycle_after_best_cut: Some(metrics.cycle_size),
            ..StructuralDebtMetrics::default()
        },
    })
}

fn dead_island_summary(component: &[String], reachable_from_tests: bool) -> String {
    if reachable_from_tests {
        return format!(
            "Files {} form an internally connected component that is not reachable from app roots",
            component.join(", ")
        );
    }

    format!(
        "Files {} form an internally connected component that is disconnected from the app-reachable graph",
        component.join(", ")
    )
}

fn dead_island_impact(reachable_from_tests: bool) -> String {
    if reachable_from_tests {
        return "A test-only internal component may be stale production code or an accidentally disconnected subsystem.".to_string();
    }

    "A disconnected internal component adds maintenance noise and can hide obsolete or unsupported code paths.".to_string()
}

fn is_dead_island_support_path(path: &str) -> bool {
    let normalized = path.to_ascii_lowercase();
    normalized.starts_with(".github/")
        || normalized.starts_with("docs/")
        || normalized.starts_with("examples/")
        || normalized.starts_with("samples/")
        || normalized.starts_with("fixtures/")
        || normalized.starts_with("test/")
        || normalized.starts_with("tests/")
        || normalized.starts_with("testdata/")
        || normalized.contains("/examples/")
        || normalized.contains("/samples/")
        || normalized.contains("/fixtures/")
        || normalized.contains("/test/")
        || normalized.contains("/tests/")
        || normalized.contains("/testdata/")
}
