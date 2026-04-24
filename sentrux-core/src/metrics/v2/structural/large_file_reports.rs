use super::graph::StructuralGraph;
use super::path_roles::{
    annotate_structural_leverage, contextual_role_tags, structural_presentation_class,
    with_guardrail_evidence,
};
use super::report_common::{
    graph_path_count, has_role_tag, large_file_first_cut, large_file_split_axes,
};
use super::scoring::{large_file_score, signal_severity};
use super::utils::{dedupe_strings_preserve_order, join_or_none, sample_paths};
use super::{
    FileFacts, StructuralDebtMetrics, StructuralDebtReport, StructuralSignalClass,
    StructuralTrustTier,
};
use crate::analysis::lang_registry;
use crate::metrics::HealthReport;
use std::collections::{BTreeMap, BTreeSet};

fn large_file_summary(
    path: &str,
    line_count: usize,
    lang: &str,
    threshold: u32,
    role_tags: &[String],
) -> String {
    if has_role_tag(role_tags, "facade_with_extracted_owners") {
        return format!(
            "Guarded facade file '{}' is {} lines, above the {} threshold of {}",
            path, line_count, lang, threshold
        );
    }
    if path.starts_with("src/")
        && (has_role_tag(role_tags, "composition_root") || has_role_tag(role_tags, "entry_surface"))
    {
        return format!(
            "Composition root '{}' is {} lines, above the {} threshold of {}",
            path, line_count, lang, threshold
        );
    }
    format!(
        "File '{}' is {} lines, above the {} threshold of {}",
        path, line_count, lang, threshold
    )
}

fn large_file_impact(path: &str, role_tags: &[String]) -> String {
    if has_role_tag(role_tags, "facade_with_extracted_owners") {
        return "The facade is still broad after extraction, which makes it harder to see whether new owner seams are actually shrinking the coordination surface.".to_string();
    }
    if path.starts_with("src/")
        && (has_role_tag(role_tags, "composition_root") || has_role_tag(role_tags, "entry_surface"))
    {
        return "A broad entry surface can leak shell, runtime, and presentation concerns into one composition root.".to_string();
    }
    "Responsibility concentration increases review friction and makes later splits harder to isolate.".to_string()
}

fn large_file_inspection_focus(
    path: &str,
    role_tags: &[String],
    candidate_split_axes: &[String],
    related_surfaces: &[String],
) -> Vec<String> {
    let mut focus = if has_role_tag(role_tags, "facade_with_extracted_owners") {
        vec![
            "inspect whether remaining coordination belongs in extracted owner modules instead of the public facade".to_string(),
            "inspect whether guardrail-backed owner seams are staying thin or accumulating new logic".to_string(),
        ]
    } else if path.starts_with("src/")
        && (has_role_tag(role_tags, "composition_root") || has_role_tag(role_tags, "entry_surface"))
    {
        vec![
            "inspect whether shell composition and runtime wiring are staying separate".to_string(),
            "inspect whether the entry surface is acting as a coordinator rather than an implementation sink".to_string(),
        ]
    } else {
        vec![
            "inspect whether orchestration, adapters, and data shaping are accumulating in one file"
                .to_string(),
            "inspect whether the file can be split along responsibility boundaries instead of line-count slices".to_string(),
        ]
    };

    if let (Some(split_axis), Some(related_surface)) =
        (candidate_split_axes.first(), related_surfaces.first())
    {
        focus.insert(
            0,
            format!(
                "inspect whether the {split_axis} can become the owner for behavior that currently couples to {related_surface}"
            ),
        );
    } else if let Some(split_axis) = candidate_split_axes.first() {
        focus.insert(
            0,
            format!("inspect which behavior belongs behind the {split_axis}"),
        );
    } else if let Some(related_surface) = related_surfaces.first() {
        focus.insert(
            0,
            format!(
                "inspect whether behavior coupled to {related_surface} belongs in a smaller owner module"
            ),
        );
    }

    dedupe_strings_preserve_order(focus)
}

fn large_file_related_surfaces(outgoing_paths: Option<&BTreeSet<String>>) -> Vec<String> {
    dedupe_strings_preserve_order(sample_paths(outgoing_paths, 5))
}

fn large_file_actionable_evidence(
    candidate_split_axes: &[String],
    related_surfaces: &[String],
    first_cut_extraction: &str,
    first_cut_admissibility: &str,
) -> Vec<String> {
    let mut evidence = Vec::new();

    if !candidate_split_axes.is_empty() {
        evidence.push(format!(
            "suggested split axes: {}",
            join_or_none(candidate_split_axes)
        ));
    }
    if !related_surfaces.is_empty() {
        evidence.push(format!(
            "related surfaces to peel out first: {}",
            join_or_none(related_surfaces)
        ));
    }
    if let (Some(split_axis), Some(related_surface)) =
        (candidate_split_axes.first(), related_surfaces.first())
    {
        evidence.push(format!(
            "recommended first cut: move the behavior that couples to {related_surface} behind the {split_axis}"
        ));
    }
    evidence.push(format!("first-cut extraction: {first_cut_extraction}"));
    evidence.push(format!(
        "first-cut admissibility: {first_cut_admissibility}"
    ));

    evidence
}

pub(super) fn build_large_file_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<StructuralDebtReport> {
    health
        .long_files
        .iter()
        .filter_map(|file_metric| {
            let facts = file_facts.get(&file_metric.path)?;
            let role_tags = contextual_role_tags(&file_metric.path, facts, graph, file_facts);
            let threshold = lang_registry::profile(&facts.lang)
                .thresholds
                .large_file_lines;
            let outgoing_paths = graph.outgoing.get(&file_metric.path);
            let fan_out = graph_path_count(outgoing_paths);
            let candidate_split_axes = large_file_split_axes(facts, outgoing_paths);
            let related_surfaces = large_file_related_surfaces(outgoing_paths);
            let first_cut =
                large_file_first_cut(facts, &role_tags, outgoing_paths, &candidate_split_axes);
            let mut evidence = vec![
                format!("line count: {}", file_metric.value),
                format!("large-file threshold: {}", threshold),
                format!("function count: {}", facts.function_count),
                format!("peak function complexity: {}", facts.max_complexity),
                format!("outbound dependencies: {}", fan_out),
            ];
            evidence.extend(large_file_actionable_evidence(
                &candidate_split_axes,
                &related_surfaces,
                &first_cut.extraction,
                &first_cut.admissibility,
            ));
            let score_0_10000 = large_file_score(
                file_metric.value,
                threshold,
                facts.max_complexity,
                first_cut.confidence_0_10000,
            );

            Some(annotate_structural_leverage(StructuralDebtReport {
                kind: "large_file".to_string(),
                trust_tier: StructuralTrustTier::Trusted,
                presentation_class: structural_presentation_class(
                    "large_file",
                    &file_metric.path,
                    StructuralTrustTier::Trusted,
                    &role_tags,
                ),
                leverage_class: Default::default(),
                scope: file_metric.path.clone(),
                signal_class: StructuralSignalClass::Debt,
                signal_families: vec!["size".to_string(), "coordination".to_string()],
                severity: signal_severity(score_0_10000),
                score_0_10000,
                summary: large_file_summary(
                    &file_metric.path,
                    file_metric.value,
                    &facts.lang,
                    threshold,
                    &role_tags,
                ),
                impact: large_file_impact(&file_metric.path, &role_tags),
                files: vec![file_metric.path.clone()],
                role_tags: role_tags.clone(),
                leverage_reasons: Vec::new(),
                evidence: dedupe_strings_preserve_order(with_guardrail_evidence(facts, evidence)),
                inspection_focus: large_file_inspection_focus(
                    &file_metric.path,
                    &role_tags,
                    &candidate_split_axes,
                    &related_surfaces,
                ),
                candidate_split_axes,
                related_surfaces,
                cut_candidates: Vec::new(),
                metrics: StructuralDebtMetrics {
                    file_count: Some(1),
                    line_count: Some(file_metric.value),
                    function_count: Some(facts.function_count),
                    fan_out: Some(fan_out),
                    max_complexity: Some(facts.max_complexity),
                    guardrail_test_count: Some(facts.guardrail_tests.len()),
                    role_count: Some(role_tags.len()),
                    ..StructuralDebtMetrics::default()
                },
            }))
        })
        .collect()
}
