use super::graph::StructuralGraph;
use super::path_roles::{
    annotate_structural_leverage, contextual_role_tags, related_structural_surfaces,
    structural_presentation_class, with_guardrail_evidence,
};
use super::report_common::{
    dependency_category_summaries, graph_path_count, has_role_tag, hotspot_split_axes,
};
use super::scoring::{instability_0_10000, signal_severity, unstable_hotspot_score};
use super::utils::{dedupe_strings_preserve_order, join_or_none, sample_paths};
use super::{
    FileFacts, StructuralDebtMetrics, StructuralDebtReport, StructuralSignalClass,
    StructuralTrustTier,
};
use crate::analysis::lang_registry;
use crate::metrics::HealthReport;
use std::collections::BTreeMap;

fn unstable_hotspot_summary(path: &str, fan_in: usize, role_tags: &[String]) -> String {
    if has_role_tag(role_tags, "transport_facade") {
        return format!(
            "Guarded transport facade '{}' has {} inbound references and remains unstable",
            path, fan_in
        );
    }
    if has_role_tag(role_tags, "component_barrel") {
        return format!(
            "Component-facing barrel '{}' has {} inbound references and remains unstable",
            path, fan_in
        );
    }
    if has_role_tag(role_tags, "guarded_boundary") {
        return format!(
            "Guarded boundary file '{}' has {} inbound references and remains unstable",
            path, fan_in
        );
    }
    if has_role_tag(role_tags, "facade_with_extracted_owners") {
        return format!(
            "Guarded facade '{}' still has {} inbound references and remains unstable",
            path, fan_in
        );
    }
    format!(
        "File '{}' has {} inbound references and remains unstable",
        path, fan_in
    )
}

fn unstable_hotspot_impact(role_tags: &[String]) -> String {
    if has_role_tag(role_tags, "transport_facade") {
        return "A transport facade with heavy fan-in needs clear ownership boundaries so lifecycle or domain logic does not leak into transport glue.".to_string();
    }
    if has_role_tag(role_tags, "component_barrel") {
        return "A volatile component-facing barrel makes it harder to keep presentation access broad while keeping deeper orchestration changes contained.".to_string();
    }
    if has_role_tag(role_tags, "guarded_boundary") {
        return "A volatile boundary surface increases blast radius even when callers stay inside the intended layer.".to_string();
    }
    if has_role_tag(role_tags, "facade_with_extracted_owners") {
        return "A volatile public facade can hide whether the real extracted owners are taking the intended load or whether coordination is flowing back uphill.".to_string();
    }
    "High fan-in plus instability increases blast radius and makes small edits harder to contain."
        .to_string()
}

fn unstable_hotspot_focus(role_tags: &[String]) -> Vec<String> {
    if has_role_tag(role_tags, "transport_facade") {
        return vec![
            "inspect whether lifecycle or domain policy is accumulating inside transport glue"
                .to_string(),
            "inspect whether callers or owner modules can take decisions outside the facade"
                .to_string(),
        ];
    }
    if has_role_tag(role_tags, "component_barrel") {
        return vec![
            "inspect which component-facing reads really need the shared barrel and which can move behind narrower selectors".to_string(),
            "inspect whether broad inbound traffic is hiding a smaller set of volatility-heavy owner modules".to_string(),
        ];
    }
    if has_role_tag(role_tags, "guarded_boundary") {
        return vec![
            "inspect whether a narrower public boundary can serve the common consumers".to_string(),
            "inspect whether intended callers are mixed with broader orchestration traffic"
                .to_string(),
        ];
    }
    if has_role_tag(role_tags, "facade_with_extracted_owners") {
        return vec![
            "inspect whether volatile logic belongs in extracted owners instead of the facade"
                .to_string(),
            "inspect whether too many callers still depend on coordination-heavy facade behavior"
                .to_string(),
        ];
    }
    vec![
        "inspect whether a stable contract can be split from the volatile implementation"
            .to_string(),
        "inspect whether too many callers depend directly on an orchestration-heavy file"
            .to_string(),
    ]
}

pub(super) fn build_unstable_hotspot_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<StructuralDebtReport> {
    health
        .hotspot_files
        .iter()
        .filter_map(|file_metric| {
            let facts = file_facts.get(&file_metric.path)?;
            let role_tags = contextual_role_tags(&file_metric.path, facts, graph, file_facts);
            let incoming_paths = graph.incoming.get(&file_metric.path);
            let outgoing_paths = graph.outgoing.get(&file_metric.path);
            let fan_in = graph_path_count(incoming_paths);
            let fan_out = graph_path_count(outgoing_paths);
            let threshold = lang_registry::profile(&facts.lang).thresholds.fan_in;
            let instability = instability_0_10000(fan_in, fan_out);
            let score_0_10000 = unstable_hotspot_score(fan_in, threshold, instability);
            let dependent_examples = sample_paths(incoming_paths, 3);

            Some(annotate_structural_leverage(StructuralDebtReport {
                kind: "unstable_hotspot".to_string(),
                trust_tier: StructuralTrustTier::Trusted,
                presentation_class: structural_presentation_class(
                    "unstable_hotspot",
                    &file_metric.path,
                    StructuralTrustTier::Trusted,
                    &role_tags,
                ),
                leverage_class: Default::default(),
                scope: file_metric.path.clone(),
                signal_class: StructuralSignalClass::Debt,
                signal_families: vec!["coupling".to_string(), "blast_radius".to_string()],
                severity: signal_severity(score_0_10000),
                score_0_10000,
                summary: unstable_hotspot_summary(&file_metric.path, fan_in, &role_tags),
                impact: unstable_hotspot_impact(&role_tags),
                files: vec![file_metric.path.clone()],
                role_tags: role_tags.clone(),
                leverage_reasons: Vec::new(),
                evidence: dedupe_strings_preserve_order(with_guardrail_evidence(
                    facts,
                    vec![
                        format!("fan-in: {}", fan_in),
                        format!("hotspot threshold: {}", threshold),
                        format!("fan-out: {}", fan_out),
                        format!("instability: {:.2}", instability as f64 / 10_000.0),
                        format!(
                            "dominant dependent categories: {}",
                            join_or_none(&dependency_category_summaries(incoming_paths, 3))
                        ),
                        if dependent_examples.is_empty() {
                            "sample dependents: none".to_string()
                        } else {
                            format!("sample dependents: {}", dependent_examples.join(", "))
                        },
                    ],
                )),
                inspection_focus: unstable_hotspot_focus(&role_tags),
                candidate_split_axes: hotspot_split_axes(facts, incoming_paths, outgoing_paths, 3),
                related_surfaces: related_structural_surfaces(facts, dependent_examples),
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
