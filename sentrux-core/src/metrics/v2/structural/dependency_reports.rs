use super::graph::StructuralGraph;
use super::path_roles::{
    annotate_structural_leverage, contextual_role_tags, related_structural_surfaces,
    structural_presentation_class, with_guardrail_evidence,
};
use super::report_common::{
    dependency_category_axes, dependency_category_summaries, graph_path_count, has_role_tag,
};
use super::scoring::{dependency_sprawl_score, instability_0_10000, signal_severity};
use super::utils::{dedupe_strings_preserve_order, join_or_none, sample_paths};
use super::{
    FileFacts, StructuralDebtMetrics, StructuralDebtReport, StructuralSignalClass,
    StructuralTrustTier,
};
use crate::analysis::lang_registry;
use crate::metrics::HealthReport;
use std::collections::BTreeMap;

fn dependency_sprawl_summary(
    path: &str,
    fan_out: usize,
    lang: &str,
    threshold: usize,
    role_tags: &[String],
) -> String {
    if has_role_tag(role_tags, "transport_facade") {
        return format!(
            "Guarded transport facade '{}' depends on {} real surfaces, above the {} threshold of {}",
            path, fan_out, lang, threshold
        );
    }
    if has_role_tag(role_tags, "component_barrel") {
        return format!(
            "Component-facing barrel '{}' depends on {} real surfaces, above the {} threshold of {}",
            path, fan_out, lang, threshold
        );
    }
    if has_role_tag(role_tags, "guarded_boundary") {
        return format!(
            "Guarded boundary file '{}' depends on {} real surfaces, above the {} threshold of {}",
            path, fan_out, lang, threshold
        );
    }
    if has_role_tag(role_tags, "composition_root") || has_role_tag(role_tags, "entry_surface") {
        return format!(
            "Composition root '{}' depends on {} real surfaces, above the {} threshold of {}",
            path, fan_out, lang, threshold
        );
    }
    format!(
        "File '{}' depends on {} real surfaces, above the {} threshold of {}",
        path, fan_out, lang, threshold
    )
}

fn dependency_sprawl_impact(role_tags: &[String]) -> String {
    if has_role_tag(role_tags, "transport_facade") {
        return "A broad transport facade makes it harder to keep lifecycle and domain policy out of glue code.".to_string();
    }
    if has_role_tag(role_tags, "component_barrel") {
        return "A broad component-facing barrel can stay intentional, but it still needs narrow boundaries so app and runtime layers do not grow back through it.".to_string();
    }
    if has_role_tag(role_tags, "guarded_boundary") {
        return "A broad boundary surface increases change surface and makes it harder to keep consumers on narrow, intended entry paths.".to_string();
    }
    if has_role_tag(role_tags, "composition_root") || has_role_tag(role_tags, "entry_surface") {
        return "Broad dependency fan-out in a composition root makes shell wiring and runtime ownership harder to keep separate.".to_string();
    }
    "Broad dependency fan-out expands change surface and makes orchestration drift harder to localize.".to_string()
}

fn dependency_sprawl_focus(role_tags: &[String]) -> Vec<String> {
    if has_role_tag(role_tags, "transport_facade") {
        return vec![
            "inspect whether transport glue is accumulating lifecycle or domain policy".to_string(),
            "inspect whether callers can depend on narrower transport contracts instead of the broad facade".to_string(),
        ];
    }
    if has_role_tag(role_tags, "component_barrel") {
        return vec![
            "inspect whether component-facing access is staying narrow while app and runtime imports remain outside the barrel".to_string(),
            "inspect whether mixed responsibilities belong in dedicated owner modules instead of the shared barrel".to_string(),
        ];
    }
    if has_role_tag(role_tags, "guarded_boundary") {
        return vec![
            "inspect whether callers are forced through a broad boundary instead of narrower owner modules".to_string(),
            "inspect whether policy-compliant imports are still pushing too much responsibility through one surface".to_string(),
        ];
    }
    if has_role_tag(role_tags, "composition_root") || has_role_tag(role_tags, "entry_surface") {
        return vec![
            "inspect whether view composition can stay separate from runtime or session wiring"
                .to_string(),
            "inspect whether shell responsibilities are spreading across too many direct imports"
                .to_string(),
        ];
    }
    vec![
        "inspect whether orchestration and policy code can move behind narrower helpers"
            .to_string(),
        "inspect whether unrelated adapter dependencies are accumulating in one module".to_string(),
    ]
}

pub(super) fn build_dependency_sprawl_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<StructuralDebtReport> {
    health
        .god_files
        .iter()
        .filter_map(|file_metric| {
            let facts = file_facts.get(&file_metric.path)?;
            let role_tags = contextual_role_tags(&file_metric.path, facts, graph, file_facts);
            let incoming_paths = graph.incoming.get(&file_metric.path);
            let outgoing_paths = graph.outgoing.get(&file_metric.path);
            let fan_in = graph_path_count(incoming_paths);
            let fan_out = graph_path_count(outgoing_paths);
            let threshold = lang_registry::profile(&facts.lang).thresholds.fan_out;
            let instability = instability_0_10000(fan_in, fan_out);
            let score_0_10000 = dependency_sprawl_score(fan_out, threshold, instability);
            let dependency_examples = sample_paths(outgoing_paths, 3);

            Some(annotate_structural_leverage(StructuralDebtReport {
                kind: "dependency_sprawl".to_string(),
                trust_tier: StructuralTrustTier::Trusted,
                presentation_class: structural_presentation_class(
                    "dependency_sprawl",
                    &file_metric.path,
                    StructuralTrustTier::Trusted,
                    &role_tags,
                ),
                leverage_class: Default::default(),
                scope: file_metric.path.clone(),
                signal_class: StructuralSignalClass::Debt,
                signal_families: vec!["coupling".to_string(), "coordination".to_string()],
                severity: signal_severity(score_0_10000),
                score_0_10000,
                summary: dependency_sprawl_summary(
                    &file_metric.path,
                    fan_out,
                    &facts.lang,
                    threshold,
                    &role_tags,
                ),
                impact: dependency_sprawl_impact(&role_tags),
                files: vec![file_metric.path.clone()],
                role_tags: role_tags.clone(),
                leverage_reasons: Vec::new(),
                evidence: dedupe_strings_preserve_order(with_guardrail_evidence(
                    facts,
                    vec![
                        format!("fan-out: {}", fan_out),
                        format!("fan-out threshold: {}", threshold),
                        format!("instability: {:.2}", instability as f64 / 10_000.0),
                        format!(
                            "dominant dependency categories: {}",
                            join_or_none(&dependency_category_summaries(outgoing_paths, 3))
                        ),
                        if dependency_examples.is_empty() {
                            "sample dependencies: none".to_string()
                        } else {
                            format!("sample dependencies: {}", dependency_examples.join(", "))
                        },
                    ],
                )),
                inspection_focus: dependency_sprawl_focus(&role_tags),
                candidate_split_axes: dependency_category_axes(outgoing_paths, 3),
                related_surfaces: related_structural_surfaces(facts, dependency_examples),
                cut_candidates: Vec::new(),
                metrics: StructuralDebtMetrics {
                    file_count: Some(1),
                    line_count: Some(facts.lines),
                    function_count: Some(facts.function_count),
                    fan_in: Some(fan_in),
                    fan_out: Some(fan_out),
                    instability_0_10000: Some(instability),
                    max_complexity: Some(facts.max_complexity),
                    guardrail_test_count: Some(facts.guardrail_tests.len()),
                    role_count: Some(role_tags.len()),
                    ..StructuralDebtMetrics::default()
                },
            }))
        })
        .collect()
}
