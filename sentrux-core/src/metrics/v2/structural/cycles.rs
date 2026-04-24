use super::graph::StructuralGraph;
use super::path_roles::{
    annotate_structural_leverage, contextual_role_tags, has_role, structural_presentation_class,
};
use super::scoring::{cycle_basis_adjusted_score, cycle_cluster_score, signal_severity};
use super::utils::{dedupe_strings_preserve_order, path_category};
use super::{
    FileFacts, StructuralDebtMetrics, StructuralDebtReport, StructuralSignalClass,
    StructuralTrustTier,
};
use crate::metrics::HealthReport;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum CycleEdgeBasis {
    ImportOnly,
    InteractionOnly,
    Mixed,
}

impl CycleEdgeBasis {
    fn label(self) -> &'static str {
        match self {
            CycleEdgeBasis::ImportOnly => "import-only",
            CycleEdgeBasis::InteractionOnly => "interaction/call-only",
            CycleEdgeBasis::Mixed => "mixed",
        }
    }

    fn summary_fragment(self) -> &'static str {
        match self {
            CycleEdgeBasis::ImportOnly => "an import-only dependency cycle",
            CycleEdgeBasis::InteractionOnly => "an interaction/call-only cycle",
            CycleEdgeBasis::Mixed => "a mixed import/interaction cycle",
        }
    }

    fn signal_family(self) -> &'static str {
        match self {
            CycleEdgeBasis::ImportOnly => "import_cycle",
            CycleEdgeBasis::InteractionOnly => "interaction_cycle",
            CycleEdgeBasis::Mixed => "mixed_cycle",
        }
    }
}

#[derive(Debug, Clone)]
struct CycleEdgeBasisEvidence {
    basis: CycleEdgeBasis,
    import_edges: Vec<(String, String)>,
    interaction_edges: Vec<(String, String)>,
    import_cycle_present: bool,
    interaction_cycle_present: bool,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct CycleCutCandidate {
    pub source: String,
    pub target: String,
    pub seam_kind: String,
    pub score_0_10000: u32,
    pub summary: String,
    pub evidence: Vec<String>,
    pub reduction_file_count: usize,
    pub remaining_cycle_size: usize,
}

fn cycle_role_tags(
    files: &[String],
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
) -> Vec<String> {
    dedupe_strings_preserve_order(
        files
            .iter()
            .filter_map(|path| {
                file_facts
                    .get(path)
                    .map(|facts| contextual_role_tags(path, facts, graph, file_facts))
            })
            .flat_map(|role_tags| role_tags.into_iter())
            .collect(),
    )
}

fn role_tags_summary(role_tags: &[String]) -> String {
    if role_tags.is_empty() {
        return "role tags: none".to_string();
    }
    format!("role tags in cycle: {}", role_tags.join(", "))
}

fn cycle_cluster_impact(edge_basis: CycleEdgeBasis, role_tags: &[String]) -> String {
    if edge_basis == CycleEdgeBasis::InteractionOnly {
        return "The cycle is based on call/interaction evidence rather than an import resolver cycle, so treat it as a watchpoint unless the patch directly widened the interaction loop.".to_string();
    }
    if edge_basis == CycleEdgeBasis::Mixed {
        return "The cycle mixes hard import edges with looser call/interaction edges, so the import edges are the higher-confidence cut points while the wider cluster stays a layering watchpoint.".to_string();
    }
    if role_tags.iter().any(|tag| tag == "component_barrel") {
        return "The cycle touches a component-facing barrel, which makes it harder to keep broad component access separate from deeper app and runtime seams.".to_string();
    }
    if role_tags.iter().any(|tag| tag == "guarded_boundary") {
        return "The cycle crosses a guardrail-backed boundary, which increases refactor friction and makes it harder to keep the intended layering intact.".to_string();
    }
    if role_tags
        .iter()
        .any(|tag| tag == "facade_with_extracted_owners")
    {
        return "The cycle still touches a guarded facade, which can hide whether extracted owners are actually reducing the coordination surface.".to_string();
    }
    "The cycle prevents clean layering and makes initialization order and refactors harder to isolate.".to_string()
}

fn cycle_cluster_focus(edge_basis: CycleEdgeBasis, role_tags: &[String]) -> Vec<String> {
    if edge_basis == CycleEdgeBasis::InteractionOnly {
        return vec![
            "inspect whether the interaction loop is real runtime coupling or a loose call-graph artifact".to_string(),
            "avoid treating this as an import-cycle repair unless an import resolver also reports the cycle".to_string(),
        ];
    }
    if edge_basis == CycleEdgeBasis::Mixed {
        return vec![
            "inspect import-edge cut candidates before acting on the wider interaction cluster"
                .to_string(),
            "separate hard dependency repairs from softer runtime interaction watchpoints"
                .to_string(),
        ];
    }
    if role_tags.iter().any(|tag| tag == "component_barrel") {
        return vec![
            "inspect whether the best cut keeps component-facing barrel access while moving deeper orchestration behind a narrower seam".to_string(),
            "inspect whether app or runtime dependencies can stop flowing back through the shared barrel".to_string(),
        ];
    }
    if role_tags.iter().any(|tag| tag == "guarded_boundary") {
        return vec![
            "inspect whether the best cut preserves the guardrail-backed boundary instead of widening it".to_string(),
            "inspect whether boundary callers can move to narrower authority modules".to_string(),
        ];
    }
    if role_tags
        .iter()
        .any(|tag| tag == "facade_with_extracted_owners")
    {
        return vec![
            "inspect whether the cycle runs through a facade that should stay thin".to_string(),
            "inspect whether extracted owner modules can absorb the back-edge instead of routing it through the facade".to_string(),
        ];
    }
    vec![
        "inspect whether one back-edge can be removed by splitting contracts from implementations"
            .to_string(),
        "inspect whether shared types can move to a lower-dependency seam".to_string(),
    ]
}

pub(super) fn build_cycle_cluster_reports(
    health: &HealthReport,
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
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
            let role_tags = cycle_role_tags(files, file_facts, graph);
            let basis_evidence = cycle_edge_basis_evidence(files, graph);
            let cut_candidates =
                cycle_cut_candidates(files, file_facts, graph, basis_evidence.basis);
            let base_score =
                cycle_cluster_score(files.len(), total_lines, &role_tags, &cut_candidates);
            let score_0_10000 =
                cycle_basis_adjusted_score(base_score, basis_evidence.basis.label());
            let cut_candidate_count = cut_candidates.len();
            let largest_cycle_after_best_cut = cut_candidates
                .first()
                .map(|candidate| candidate.remaining_cycle_size)
                .unwrap_or(files.len());
            let related_surfaces = cycle_related_surfaces(files, &cut_candidates);
            let candidate_split_axes = cycle_split_axes(&cut_candidates, basis_evidence.basis);

            annotate_structural_leverage(StructuralDebtReport {
                kind: "cycle_cluster".to_string(),
                trust_tier: StructuralTrustTier::Watchpoint,
                presentation_class: structural_presentation_class(
                    "cycle_cluster",
                    files.first().map(String::as_str).unwrap_or_default(),
                    StructuralTrustTier::Watchpoint,
                    &role_tags,
                ),
                leverage_class: Default::default(),
                scope,
                signal_class: StructuralSignalClass::Watchpoint,
                signal_families: vec![
                    "dependency".to_string(),
                    "layering".to_string(),
                    basis_evidence.basis.signal_family().to_string(),
                ],
                severity: signal_severity(score_0_10000),
                score_0_10000,
                summary: format!(
                    "Files {} form {}",
                    files.join(", "),
                    basis_evidence.basis.summary_fragment()
                ),
                impact: cycle_cluster_impact(basis_evidence.basis, &role_tags),
                files: files.clone(),
                role_tags: role_tags.clone(),
                leverage_reasons: Vec::new(),
                evidence: dedupe_strings_preserve_order({
                    let mut evidence = vec![
                        format!("edge basis: {}", basis_evidence.basis.label()),
                        format!(
                            "internal edge counts: import {}, interaction/call {}",
                            basis_evidence.import_edges.len(),
                            basis_evidence.interaction_edges.len()
                        ),
                        format!(
                            "import-cycle evidence: {}",
                            present_absent(basis_evidence.import_cycle_present)
                        ),
                        format!(
                            "interaction-cycle evidence: {}",
                            present_absent(basis_evidence.interaction_cycle_present)
                        ),
                    ];
                    evidence.extend(representative_edge_evidence(
                        "representative import edges",
                        &basis_evidence.import_edges,
                    ));
                    evidence.extend(representative_edge_evidence(
                        "representative interaction/call edges",
                        &basis_evidence.interaction_edges,
                    ));
                    evidence.extend([
                        format!("cycle size: {}", files.len()),
                        format!("total lines in cycle: {}", total_lines),
                        format!("peak function complexity inside cycle: {}", max_complexity),
                        format!("candidate cuts: {}", cut_candidates.len()),
                        best_cycle_cut_evidence(&cut_candidates),
                        role_tags_summary(&role_tags),
                    ]);
                    evidence
                }),
                inspection_focus: cycle_cluster_focus(basis_evidence.basis, &role_tags),
                candidate_split_axes,
                related_surfaces,
                cut_candidates,
                metrics: StructuralDebtMetrics {
                    file_count: Some(files.len()),
                    line_count: Some(total_lines),
                    cycle_size: Some(files.len()),
                    max_complexity: Some(max_complexity),
                    cut_candidate_count: Some(cut_candidate_count),
                    largest_cycle_after_best_cut: Some(largest_cycle_after_best_cut),
                    role_count: Some(role_tags.len()),
                    ..StructuralDebtMetrics::default()
                },
            })
        })
        .collect()
}

fn cycle_edge_basis_evidence(files: &[String], graph: &StructuralGraph) -> CycleEdgeBasisEvidence {
    let nodes = files.iter().cloned().collect::<BTreeSet<_>>();
    let import_edges = cycle_internal_edges(&nodes, &graph.import_outgoing);
    let interaction_edges = cycle_internal_edges(&nodes, &graph.call_outgoing);
    let import_cycle_present = has_cyclic_component(&nodes, &graph.import_outgoing);
    let interaction_cycle_present = has_cyclic_component(&nodes, &graph.call_outgoing);
    let basis = match (import_edges.is_empty(), interaction_edges.is_empty()) {
        (false, true) => CycleEdgeBasis::ImportOnly,
        (true, false) => CycleEdgeBasis::InteractionOnly,
        (false, false) => CycleEdgeBasis::Mixed,
        (true, true) if import_cycle_present => CycleEdgeBasis::ImportOnly,
        (true, true) if interaction_cycle_present => CycleEdgeBasis::InteractionOnly,
        (true, true) => CycleEdgeBasis::ImportOnly,
    };

    CycleEdgeBasisEvidence {
        basis,
        import_edges,
        interaction_edges,
        import_cycle_present,
        interaction_cycle_present,
    }
}

fn has_cyclic_component(
    nodes: &BTreeSet<String>,
    outgoing: &BTreeMap<String, BTreeSet<String>>,
) -> bool {
    let adjacency = cycle_adjacency(nodes, outgoing, None);
    strongly_connected_components(nodes, &adjacency)
        .iter()
        .any(|component| is_cyclic_component(component, &adjacency))
}

fn present_absent(present: bool) -> &'static str {
    if present {
        "present"
    } else {
        "absent"
    }
}

fn representative_edge_evidence(label: &str, edges: &[(String, String)]) -> Vec<String> {
    if edges.is_empty() {
        return vec![format!("{label}: none")];
    }
    vec![format!(
        "{label}: {}",
        edges
            .iter()
            .take(3)
            .map(|(source, target)| format!("{source} -> {target}"))
            .collect::<Vec<_>>()
            .join("; ")
    )]
}

fn cycle_cut_candidates(
    files: &[String],
    file_facts: &BTreeMap<String, FileFacts>,
    graph: &StructuralGraph,
    edge_basis: CycleEdgeBasis,
) -> Vec<CycleCutCandidate> {
    let nodes = files.iter().cloned().collect::<BTreeSet<_>>();
    let original_cycle_size = nodes.len();
    let (candidate_outgoing, candidate_incoming, edge_label) = match edge_basis {
        CycleEdgeBasis::InteractionOnly => (
            &graph.call_outgoing,
            &graph.call_incoming,
            "interaction/call",
        ),
        CycleEdgeBasis::ImportOnly | CycleEdgeBasis::Mixed => {
            (&graph.import_outgoing, &graph.import_incoming, "import")
        }
    };
    let measurement_outgoing = match edge_basis {
        CycleEdgeBasis::ImportOnly => &graph.import_outgoing,
        CycleEdgeBasis::InteractionOnly => &graph.call_outgoing,
        CycleEdgeBasis::Mixed => &graph.outgoing,
    };
    let internal_edges = cycle_internal_edges(&nodes, candidate_outgoing);
    if internal_edges.is_empty() {
        return Vec::new();
    }

    let mut candidates = internal_edges
        .into_iter()
        .filter_map(|(source, target)| {
            let (remaining_cycle_size, cyclic_node_count) =
                cyclic_sizes_without_edge(&nodes, measurement_outgoing, (&source, &target));
            let reduction_file_count = original_cycle_size.saturating_sub(cyclic_node_count);
            if reduction_file_count == 0 {
                return None;
            }

            let seam_kind = cycle_seam_kind_with_roles(&source, &target, file_facts);
            let score_0_10000 = cycle_cut_candidate_score(
                original_cycle_size,
                reduction_file_count,
                remaining_cycle_size,
                &source,
                &target,
                candidate_outgoing,
                candidate_incoming,
                seam_kind,
            );
            let source_lines = file_facts
                .get(&source)
                .map(|facts| facts.lines)
                .unwrap_or(0);
            let target_lines = file_facts
                .get(&target)
                .map(|facts| facts.lines)
                .unwrap_or(0);
            Some(CycleCutCandidate {
                source: source.clone(),
                target: target.clone(),
                seam_kind: seam_kind.to_string(),
                score_0_10000,
                summary: format!(
                    "Inspect {edge_label} edge '{}' -> '{}' to reduce the cyclic footprint by {} file(s)",
                    source, target, reduction_file_count
                ),
                evidence: vec![
                    format!("edge basis: {edge_label}"),
                    format!("seam kind: {}", seam_kind),
                    format!(
                        "remaining largest cycle after cut: {}",
                        remaining_cycle_size
                    ),
                    format!("cyclic files removed by cut: {}", reduction_file_count),
                    format!("source lines: {}", source_lines),
                    format!("target lines: {}", target_lines),
                ],
                reduction_file_count,
                remaining_cycle_size,
            })
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| {
        right
            .score_0_10000
            .cmp(&left.score_0_10000)
            .then_with(|| right.reduction_file_count.cmp(&left.reduction_file_count))
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.target.cmp(&right.target))
    });
    candidates.truncate(3);
    candidates
}

fn cycle_internal_edges(
    nodes: &BTreeSet<String>,
    outgoing: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<(String, String)> {
    let mut edges = Vec::new();
    for source in nodes {
        let Some(targets) = outgoing.get(source) else {
            continue;
        };
        for target in targets {
            if nodes.contains(target) {
                edges.push((source.clone(), target.clone()));
            }
        }
    }
    edges
}

fn cyclic_sizes_without_edge(
    nodes: &BTreeSet<String>,
    outgoing: &BTreeMap<String, BTreeSet<String>>,
    removed_edge: (&str, &str),
) -> (usize, usize) {
    let adjacency = cycle_adjacency(nodes, outgoing, Some(removed_edge));
    let components = strongly_connected_components(nodes, &adjacency);
    let cyclic_components = components
        .iter()
        .filter(|component| is_cyclic_component(component, &adjacency))
        .collect::<Vec<_>>();
    let largest_cycle_size = cyclic_components
        .iter()
        .map(|component| component.len())
        .max()
        .unwrap_or(0);
    let cyclic_node_count = cyclic_components
        .iter()
        .map(|component| component.len())
        .sum::<usize>();
    (largest_cycle_size, cyclic_node_count)
}

fn cycle_adjacency(
    nodes: &BTreeSet<String>,
    outgoing: &BTreeMap<String, BTreeSet<String>>,
    removed_edge: Option<(&str, &str)>,
) -> BTreeMap<String, Vec<String>> {
    let mut adjacency = BTreeMap::<String, Vec<String>>::new();
    for node in nodes {
        let neighbors = outgoing
            .get(node)
            .map(|targets| {
                targets
                    .iter()
                    .filter(|target| nodes.contains(*target))
                    .filter(|target| {
                        removed_edge.is_none_or(|(source, removed_target)| {
                            !(node == source && target.as_str() == removed_target)
                        })
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        adjacency.insert(node.clone(), neighbors);
    }
    adjacency
}

fn strongly_connected_components(
    nodes: &BTreeSet<String>,
    adjacency: &BTreeMap<String, Vec<String>>,
) -> Vec<Vec<String>> {
    let mut visited = BTreeSet::new();
    let mut order = Vec::new();
    for node in nodes {
        dfs_order(node, adjacency, &mut visited, &mut order);
    }

    let reverse = reverse_adjacency(nodes, adjacency);
    let mut assigned = BTreeSet::new();
    let mut components = Vec::new();
    for node in order.into_iter().rev() {
        if assigned.contains(&node) {
            continue;
        }
        let mut component = Vec::new();
        dfs_collect(&node, &reverse, &mut assigned, &mut component);
        component.sort();
        components.push(component);
    }
    components
}

fn dfs_order(
    node: &str,
    adjacency: &BTreeMap<String, Vec<String>>,
    visited: &mut BTreeSet<String>,
    order: &mut Vec<String>,
) {
    if !visited.insert(node.to_string()) {
        return;
    }
    if let Some(neighbors) = adjacency.get(node) {
        for neighbor in neighbors {
            dfs_order(neighbor, adjacency, visited, order);
        }
    }
    order.push(node.to_string());
}

fn reverse_adjacency(
    nodes: &BTreeSet<String>,
    adjacency: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
    let mut reverse = nodes
        .iter()
        .map(|node| (node.clone(), Vec::<String>::new()))
        .collect::<BTreeMap<_, _>>();
    for (source, targets) in adjacency {
        for target in targets {
            reverse
                .entry(target.clone())
                .or_default()
                .push(source.clone());
        }
    }
    reverse
}

fn dfs_collect(
    node: &str,
    adjacency: &BTreeMap<String, Vec<String>>,
    visited: &mut BTreeSet<String>,
    component: &mut Vec<String>,
) {
    if !visited.insert(node.to_string()) {
        return;
    }
    component.push(node.to_string());
    if let Some(neighbors) = adjacency.get(node) {
        for neighbor in neighbors {
            dfs_collect(neighbor, adjacency, visited, component);
        }
    }
}

fn is_cyclic_component(component: &[String], adjacency: &BTreeMap<String, Vec<String>>) -> bool {
    if component.len() > 1 {
        return true;
    }
    component.first().is_some_and(|node| {
        adjacency
            .get(node)
            .is_some_and(|neighbors| neighbors.iter().any(|neighbor| neighbor == node))
    })
}

fn cycle_seam_kind(source: &str, target: &str) -> &'static str {
    let source_category = path_category(source);
    let target_category = path_category(target);
    if is_app_store_boundary(&source_category, &target_category) {
        return "app_store_boundary";
    }
    if path_has_contract_hint(source) || path_has_contract_hint(target) {
        return "contract_or_type_extraction";
    }
    if source_category != target_category {
        return "cross_layer_boundary";
    }
    "local_module_split"
}

fn cycle_seam_kind_with_roles(
    source: &str,
    target: &str,
    file_facts: &BTreeMap<String, FileFacts>,
) -> &'static str {
    let source_facts = file_facts.get(source);
    let target_facts = file_facts.get(target);
    if source_facts.is_some_and(|facts| has_role(facts, "guarded_boundary"))
        || target_facts.is_some_and(|facts| has_role(facts, "guarded_boundary"))
    {
        let source_category = path_category(source);
        let target_category = path_category(target);
        if is_app_store_boundary(&source_category, &target_category) {
            return "guarded_app_store_boundary";
        }
        return "guarded_boundary_cut";
    }
    if source_facts.is_some_and(|facts| has_role(facts, "facade_with_extracted_owners"))
        || target_facts.is_some_and(|facts| has_role(facts, "facade_with_extracted_owners"))
    {
        return "facade_owner_boundary";
    }
    cycle_seam_kind(source, target)
}

fn cycle_cut_candidate_score(
    original_cycle_size: usize,
    reduction_file_count: usize,
    remaining_cycle_size: usize,
    source: &str,
    target: &str,
    outgoing: &BTreeMap<String, BTreeSet<String>>,
    incoming: &BTreeMap<String, BTreeSet<String>>,
    seam_kind: &str,
) -> u32 {
    let reduction_bonus = if original_cycle_size == 0 {
        0
    } else {
        ((reduction_file_count as f64 / original_cycle_size as f64) * 4500.0).round() as u32
    };
    let seam_bonus = match seam_kind {
        "guarded_app_store_boundary" => 2200,
        "guarded_boundary_cut" => 2000,
        "facade_owner_boundary" => 1900,
        "app_store_boundary" => 1800,
        "contract_or_type_extraction" => 1500,
        "cross_layer_boundary" => 1200,
        _ => 700,
    };
    let source_internal_out = outgoing
        .get(source)
        .map(|targets| targets.len())
        .unwrap_or(0) as u32;
    let target_internal_in = incoming
        .get(target)
        .map(|sources| sources.len())
        .unwrap_or(0) as u32;
    let pressure_bonus = ((source_internal_out + target_internal_in) * 180).min(1800);
    let cleanup_bonus =
        (original_cycle_size.saturating_sub(remaining_cycle_size) as u32 * 120).min(1200);

    (2000 + reduction_bonus + seam_bonus + pressure_bonus + cleanup_bonus).min(10_000)
}

fn best_cycle_cut_evidence(cut_candidates: &[CycleCutCandidate]) -> String {
    match cut_candidates.first() {
        Some(candidate) => format!(
            "best cut candidate: {} -> {} (removes {} cyclic files)",
            candidate.source, candidate.target, candidate.reduction_file_count
        ),
        None => "best cut candidate: none".to_string(),
    }
}

fn cycle_related_surfaces(files: &[String], cut_candidates: &[CycleCutCandidate]) -> Vec<String> {
    let mut related = cut_candidates
        .iter()
        .flat_map(|candidate| [candidate.source.clone(), candidate.target.clone()])
        .collect::<Vec<_>>();
    related.extend(files.iter().take(3).cloned());
    dedupe_strings_preserve_order(related)
}

fn cycle_split_axes(
    cut_candidates: &[CycleCutCandidate],
    edge_basis: CycleEdgeBasis,
) -> Vec<String> {
    let mut axes = cut_candidates
        .iter()
        .map(|candidate| candidate.seam_kind.replace('_', " "))
        .collect::<Vec<_>>();
    if axes.is_empty() {
        axes.push(match edge_basis {
            CycleEdgeBasis::ImportOnly => "contract boundary".to_string(),
            CycleEdgeBasis::InteractionOnly => "interaction boundary".to_string(),
            CycleEdgeBasis::Mixed => "import boundary".to_string(),
        });
    }
    dedupe_strings_preserve_order(axes)
}

fn path_has_contract_hint(path: &str) -> bool {
    let normalized = path.to_ascii_lowercase();
    ["contract", "schema", "types", "state", "model"]
        .iter()
        .any(|segment| normalized.contains(segment))
}

fn is_app_store_boundary(source_category: &str, target_category: &str) -> bool {
    (source_category == "app" && target_category == "store")
        || (source_category == "store" && target_category == "app")
}
