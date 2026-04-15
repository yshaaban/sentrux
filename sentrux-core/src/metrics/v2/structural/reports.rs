use super::graph::{
    application_root_files, cycle_size_by_file, external_non_test_inbound_count, reachable_files,
    weak_components, StructuralGraph,
};
use super::path_roles::{
    annotate_structural_leverage, contextual_role_tags, has_role, related_structural_surfaces,
    structural_presentation_class, with_guardrail_evidence,
};
use super::scoring::{
    dead_island_score, dead_private_cluster_score, dependency_sprawl_score, instability_0_10000,
    large_file_score, signal_severity, unstable_hotspot_score,
};
use super::utils::{dedupe_strings_preserve_order, join_or_none, path_category, sample_paths};
use super::{
    FileFacts, StructuralDebtMetrics, StructuralDebtReport, StructuralSignalClass,
    StructuralTrustTier,
};
use crate::analysis::lang_registry;
use crate::core::snapshot::Snapshot;
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

fn graph_path_count(paths: Option<&BTreeSet<String>>) -> usize {
    paths.map(BTreeSet::len).unwrap_or(0)
}

fn large_file_related_surfaces(outgoing_paths: Option<&BTreeSet<String>>) -> Vec<String> {
    dedupe_strings_preserve_order(sample_paths(outgoing_paths, 5))
}

fn large_file_actionable_evidence(
    candidate_split_axes: &[String],
    related_surfaces: &[String],
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

    evidence
}

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
            let score_0_10000 =
                large_file_score(file_metric.value, threshold, facts.max_complexity);
            let outgoing_paths = graph.outgoing.get(&file_metric.path);
            let fan_out = graph_path_count(outgoing_paths);
            let candidate_split_axes = large_file_split_axes(facts, outgoing_paths);
            let related_surfaces = large_file_related_surfaces(outgoing_paths);
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
            ));

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

pub(super) fn build_dead_private_code_cluster_reports(
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
            let mut unique_functions = BTreeMap::new();
            for function in functions {
                unique_functions
                    .entry(function.func.clone())
                    .and_modify(|existing: &mut crate::metrics::FuncMetric| {
                        if function.value > existing.value {
                            *existing = function.clone();
                        }
                    })
                    .or_insert(function);
            }
            let functions = unique_functions.into_values().collect::<Vec<_>>();
            let dead_symbol_count = functions.len();
            let dead_line_count = functions
                .iter()
                .map(|function| function.value as usize)
                .sum::<usize>();
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

            Some(annotate_structural_leverage(StructuralDebtReport {
                kind: "dead_private_code_cluster".to_string(),
                trust_tier: StructuralTrustTier::Experimental,
                presentation_class: structural_presentation_class(
                    "dead_private_code_cluster",
                    &path,
                    StructuralTrustTier::Experimental,
                    &facts.role_tags,
                ),
                leverage_class: Default::default(),
                scope: path.clone(),
                signal_class: StructuralSignalClass::Watchpoint,
                signal_families: vec!["staleness".to_string(), "maintainability".to_string()],
                severity: signal_severity(score_0_10000),
                score_0_10000,
                summary: format!(
                    "File '{}' contains {} uncalled private functions totaling {} lines",
                    path, dead_symbol_count, dead_line_count
                ),
                impact: "Stale private code increases maintenance noise and can mislead future edits into reviving obsolete paths.".to_string(),
                files: vec![path.clone()],
                role_tags: facts.role_tags.clone(),
                leverage_reasons: Vec::new(),
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
                candidate_split_axes: Vec::new(),
                related_surfaces: Vec::new(),
                cut_candidates: Vec::new(),
                metrics: StructuralDebtMetrics {
                    file_count: Some(1),
                    line_count: Some(facts.lines),
                    function_count: Some(facts.function_count),
                    dead_symbol_count: Some(dead_symbol_count),
                    dead_line_count: Some(dead_line_count),
                    max_complexity: Some(facts.max_complexity),
                    role_count: Some(facts.role_tags.len()),
                    ..StructuralDebtMetrics::default()
                },
            }))
        })
        .collect()
}

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

            Some(annotate_structural_leverage(StructuralDebtReport {
                kind: "dead_island".to_string(),
                trust_tier: StructuralTrustTier::Watchpoint,
                presentation_class: structural_presentation_class(
                    "dead_island",
                    component.first().map(String::as_str).unwrap_or_default(),
                    StructuralTrustTier::Watchpoint,
                    &Vec::new(),
                ),
                leverage_class: Default::default(),
                scope,
                signal_class: if reachable_from_tests {
                    StructuralSignalClass::Watchpoint
                } else {
                    StructuralSignalClass::Debt
                },
                signal_families: vec!["reachability".to_string(), "staleness".to_string()],
                severity: signal_severity(score_0_10000),
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
                role_tags: Vec::new(),
                leverage_reasons: Vec::new(),
                evidence,
                inspection_focus: vec![
                    "inspect whether this component is intentionally disconnected or stale".to_string(),
                    "inspect whether it should be deleted, archived, or wired through an explicit root".to_string(),
                ],
                candidate_split_axes: vec![
                    "reachable entry surface".to_string(),
                    "public contract boundary".to_string(),
                ],
                related_surfaces: component.iter().take(5).cloned().collect(),
                cut_candidates: Vec::new(),
                metrics: StructuralDebtMetrics {
                    file_count: Some(component.len()),
                    line_count: Some(total_lines),
                    cycle_size: Some(cycle_size),
                    inbound_reference_count: Some(inbound_reference_count),
                    public_surface_count: Some(public_surface_count),
                    reachable_from_tests: Some(reachable_from_tests),
                    cut_candidate_count: Some(0),
                    largest_cycle_after_best_cut: Some(cycle_size),
                    ..StructuralDebtMetrics::default()
                },
            }))
        })
        .collect()
}

fn dependency_category_summaries(paths: Option<&BTreeSet<String>>, limit: usize) -> Vec<String> {
    let Some(paths) = paths else {
        return Vec::new();
    };

    let mut counts = BTreeMap::<String, usize>::new();
    for path in paths {
        let category = path_category(path);
        counts
            .entry(category)
            .and_modify(|count| *count += 1)
            .or_insert(1);
    }

    let mut categories = counts.into_iter().collect::<Vec<_>>();
    categories.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    categories
        .into_iter()
        .take(limit)
        .map(|(category, count)| format!("{category}({count})"))
        .collect()
}

fn dependency_category_axes(paths: Option<&BTreeSet<String>>, limit: usize) -> Vec<String> {
    let Some(paths) = paths else {
        return vec!["orchestration boundary".to_string()];
    };

    let categories = dominant_categories(paths, limit);
    if categories.is_empty() {
        return vec!["orchestration boundary".to_string()];
    }

    categories
        .into_iter()
        .map(|category| format!("{category} dependency boundary"))
        .collect()
}

fn hotspot_split_axes(
    facts: &FileFacts,
    incoming: Option<&BTreeSet<String>>,
    outgoing: Option<&BTreeSet<String>>,
    limit: usize,
) -> Vec<String> {
    let inbound_categories = dominant_categories_from_option(incoming, limit / 2 + 1)
        .into_iter()
        .map(|category| format!("{category} caller boundary"))
        .collect::<Vec<_>>();
    let outbound_categories = dominant_categories_from_option(outgoing, limit / 2 + 1)
        .into_iter()
        .map(|category| format!("{category} dependency boundary"))
        .collect::<Vec<_>>();

    let mut axes = inbound_categories;
    axes.extend(outbound_categories);
    if has_role(facts, "guarded_boundary") {
        axes.push("guarded boundary".to_string());
    }
    if has_role(facts, "facade_with_extracted_owners") {
        axes.push("facade owner boundary".to_string());
    }
    let mut axes = dedupe_strings_preserve_order(axes);
    axes.truncate(limit.max(1));
    if axes.is_empty() {
        axes.push("stable contract boundary".to_string());
    }
    axes
}

fn large_file_split_axes(
    facts: &FileFacts,
    outgoing_paths: Option<&BTreeSet<String>>,
) -> Vec<String> {
    let mut axes = outgoing_paths
        .map(|paths| dependency_category_axes(Some(paths), 3))
        .unwrap_or_default();
    if has_role(facts, "facade_with_extracted_owners") {
        axes.push("facade owner boundary".to_string());
    }
    if has_role(facts, "entry_surface") {
        axes.push("entry surface split".to_string());
    }
    if facts.max_complexity >= 40 {
        axes.push("high-complexity helper extraction".to_string());
    }
    if facts.function_count >= 20 {
        axes.push("private helper surface split".to_string());
    }
    let mut axes = dedupe_strings_preserve_order(axes);
    if axes.is_empty() {
        axes.push("orchestration boundary".to_string());
    }
    axes
}

fn dominant_categories(paths: &BTreeSet<String>, limit: usize) -> Vec<String> {
    dependency_category_summaries(Some(paths), limit)
        .into_iter()
        .filter_map(|summary| {
            summary
                .split_once('(')
                .map(|(category, _)| category.to_string())
        })
        .collect()
}

fn dominant_categories_from_option(paths: Option<&BTreeSet<String>>, limit: usize) -> Vec<String> {
    paths
        .map(|paths| dominant_categories(paths, limit))
        .unwrap_or_default()
}

fn has_role_tag(role_tags: &[String], role: &str) -> bool {
    role_tags.iter().any(|tag| tag == role)
}
