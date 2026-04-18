use super::debt_concepts::{
    concept_candidate_split_axes, concept_signal_class, concept_signal_families,
    concept_signal_impact, concept_signal_metrics, signal_severity,
};
use super::*;

pub(super) fn collect_debt_signals(
    concept_summaries: &[ConceptDebtSummary],
    structural_reports: &[crate::metrics::v2::StructuralDebtReport],
    findings: &[Value],
    clone_families: &[Value],
    concentration_reports: &[crate::metrics::v2::ConcentrationReport],
) -> Vec<DebtSignal> {
    let (mut signals, mut covered_hotspot_paths) =
        collect_concept_debt_signals(concept_summaries, concentration_reports);
    let structural_signals = structural_reports
        .iter()
        .map(super::structural_signal)
        .collect::<Vec<_>>();
    covered_hotspot_paths.extend(structural_hotspot_paths(&structural_signals));
    signals.extend(structural_signals);
    signals.extend(clone_debt_signals(findings, clone_families));
    signals.extend(uncovered_hotspot_signals(
        concentration_reports,
        &covered_hotspot_paths,
    ));
    sort_debt_signals(&mut signals);
    signals
}

fn collect_concept_debt_signals(
    concept_summaries: &[ConceptDebtSummary],
    concentration_reports: &[crate::metrics::v2::ConcentrationReport],
) -> (Vec<DebtSignal>, BTreeSet<String>) {
    let mut covered_hotspot_paths = BTreeSet::new();
    let signals = concept_summaries
        .iter()
        .filter(|summary| summary.score_0_10000 >= 2500)
        .map(|summary| {
            let related_hotspots = related_debt_hotspots(summary, concentration_reports);
            covered_hotspot_paths.extend(related_hotspots.iter().map(|report| report.path.clone()));
            build_concept_debt_signal(summary, &related_hotspots)
        })
        .collect::<Vec<_>>();
    (signals, covered_hotspot_paths)
}

fn related_debt_hotspots<'a>(
    summary: &ConceptDebtSummary,
    concentration_reports: &'a [crate::metrics::v2::ConcentrationReport],
) -> Vec<&'a crate::metrics::v2::ConcentrationReport> {
    concentration_reports
        .iter()
        .filter(|report| summary.files.iter().any(|path| path == &report.path))
        .collect::<Vec<_>>()
}

fn build_concept_debt_signal(
    summary: &ConceptDebtSummary,
    related_hotspots: &[&crate::metrics::v2::ConcentrationReport],
) -> DebtSignal {
    let score_0_10000 = concept_debt_signal_score(summary, related_hotspots);
    let evidence = concept_debt_signal_evidence(summary, related_hotspots);
    annotate_debt_signal(DebtSignal {
        kind: "concept".to_string(),
        trust_tier: DebtTrustTier::Trusted,
        presentation_class: classify_debt_presentation_class(
            "concept",
            DebtTrustTier::Trusted,
            &summary.concept_id,
            &summary.files,
            &[],
            evidence.len(),
            summary.finding_count,
            summary.boundary_pressure_count,
            summary.missing_site_count,
        ),
        leverage_class: None,
        primary_lane: String::new(),
        default_surface_role: String::new(),
        scope: summary.concept_id.clone(),
        signal_class: concept_signal_class(summary),
        signal_families: concept_signal_families(summary),
        severity: signal_severity(score_0_10000),
        score_0_10000,
        summary: summary.summary.clone(),
        impact: concept_signal_impact(summary),
        files: summary.files.clone(),
        role_tags: Vec::new(),
        leverage_reasons: Vec::new(),
        evidence,
        inspection_focus: summary.inspection_focus.clone(),
        candidate_split_axes: concept_candidate_split_axes(summary),
        related_surfaces: summary.files.iter().take(5).cloned().collect(),
        metrics: concept_signal_metrics(summary),
    })
}

fn concept_debt_signal_score(
    summary: &ConceptDebtSummary,
    related_hotspots: &[&crate::metrics::v2::ConcentrationReport],
) -> u32 {
    if let Some(top_hotspot) = related_hotspots.first() {
        return (summary.score_0_10000 + top_hotspot.score_0_10000 / 3).min(10_000);
    }

    summary.score_0_10000
}

fn concept_debt_signal_evidence(
    summary: &ConceptDebtSummary,
    related_hotspots: &[&crate::metrics::v2::ConcentrationReport],
) -> Vec<String> {
    let mut evidence = summary
        .dominant_kinds
        .iter()
        .map(|kind| format!("finding kind: {kind}"))
        .collect::<Vec<_>>();
    if summary.missing_site_count > 0 {
        evidence.push(format!(
            "missing update sites: {}",
            summary.missing_site_count
        ));
    }
    if summary.context_burden > 0 {
        evidence.push(format!("context burden: {}", summary.context_burden));
    }
    if let Some(top_hotspot) = related_hotspots.first() {
        evidence.push(format!("hotspot file: {}", top_hotspot.path));
        evidence.extend(top_hotspot.reasons.iter().cloned().take(2));
    }
    evidence
}

fn structural_hotspot_paths(signals: &[DebtSignal]) -> BTreeSet<String> {
    signals
        .iter()
        .filter(|signal| signal.kind == "unstable_hotspot")
        .flat_map(|signal| signal.files.iter().cloned())
        .collect::<BTreeSet<_>>()
}

fn clone_debt_signals(findings: &[Value], clone_families: &[Value]) -> Vec<DebtSignal> {
    if !clone_families.is_empty() {
        return clone_families
            .iter()
            .filter_map(super::clone_family_signal)
            .collect::<Vec<_>>();
    }

    findings
        .iter()
        .filter(|finding| finding_kind(finding) == "exact_clone_group")
        .filter_map(super::clone_group_signal)
        .collect::<Vec<_>>()
}

fn uncovered_hotspot_signals(
    concentration_reports: &[crate::metrics::v2::ConcentrationReport],
    covered_hotspot_paths: &BTreeSet<String>,
) -> Vec<DebtSignal> {
    concentration_reports
        .iter()
        .filter(|report| report.score_0_10000 >= 4000)
        .filter(|report| !covered_hotspot_paths.contains(&report.path))
        .filter_map(super::hotspot_signal)
        .collect::<Vec<_>>()
}

fn sort_debt_signals(signals: &mut [DebtSignal]) {
    signals.sort_by(|left, right| {
        right
            .severity
            .priority()
            .cmp(&left.severity.priority())
            .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
            .then_with(|| left.scope.cmp(&right.scope))
    });
}

pub(super) fn truncate_debt_signals(mut signals: Vec<DebtSignal>, limit: usize) -> Vec<DebtSignal> {
    signals.truncate(limit);
    signals
}

pub(super) fn build_inspection_watchpoints(
    concept_summaries: &[ConceptDebtSummary],
    clone_families: &[Value],
    concentration_reports: &[crate::metrics::v2::ConcentrationReport],
    limit: usize,
) -> Vec<InspectionWatchpoint> {
    let mut watchpoints = concept_summaries
        .iter()
        .filter_map(|summary| {
            let matching_clone_families = related_clone_families(summary, clone_families);
            let matching_hotspots = related_hotspots(summary, concentration_reports);
            if summary.score_0_10000 < 3000
                && matching_clone_families.is_empty()
                && matching_hotspots.is_empty()
                && summary.boundary_pressure_count == 0
            {
                return None;
            }

            let score_0_10000 = inspection_watchpoint_score(
                summary,
                matching_clone_families.len(),
                matching_hotspots.len(),
            );

            Some(annotate_inspection_watchpoint(InspectionWatchpoint {
                kind: "concept_watchpoint".to_string(),
                trust_tier: DebtTrustTier::Watchpoint,
                presentation_class: PresentationClass::Watchpoint,
                leverage_class: None,
                primary_lane: String::new(),
                default_surface_role: String::new(),
                scope: summary.concept_id.clone(),
                severity: signal_severity(score_0_10000),
                score_0_10000,
                summary: inspection_watchpoint_summary(
                    summary,
                    matching_clone_families.len(),
                    matching_hotspots.len(),
                ),
                impact: concept_signal_impact(summary),
                files: summary.files.clone(),
                role_tags: Vec::new(),
                leverage_reasons: Vec::new(),
                evidence: inspection_watchpoint_evidence(
                    summary,
                    matching_clone_families.as_slice(),
                    matching_hotspots.as_slice(),
                ),
                inspection_focus: inspection_watchpoint_focus(
                    summary,
                    matching_clone_families.as_slice(),
                    matching_hotspots.as_slice(),
                ),
                candidate_split_axes: concept_candidate_split_axes(summary),
                related_surfaces: summary.files.iter().take(5).cloned().collect(),
                signal_families: inspection_watchpoint_signal_families(
                    summary,
                    matching_clone_families.len(),
                    matching_hotspots.len(),
                ),
                clone_family_count: matching_clone_families.len(),
                hotspot_count: matching_hotspots.len(),
                missing_site_count: summary.missing_site_count,
                boundary_pressure_count: summary.boundary_pressure_count,
            }))
        })
        .collect::<Vec<_>>();

    watchpoints.sort_by(|left, right| {
        right
            .severity
            .priority()
            .cmp(&left.severity.priority())
            .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
            .then_with(|| left.scope.cmp(&right.scope))
    });
    watchpoints.truncate(limit);
    watchpoints
}

pub(super) fn debt_signal_watchpoints(
    signals: &[DebtSignal],
    limit: usize,
) -> Vec<InspectionWatchpoint> {
    let mut watchpoints = signals
        .iter()
        .filter(|signal| signal.trust_tier == DebtTrustTier::Watchpoint)
        .map(|signal| {
            annotate_inspection_watchpoint(InspectionWatchpoint {
                kind: signal.kind.clone(),
                trust_tier: signal.trust_tier,
                presentation_class: signal.presentation_class,
                leverage_class: signal.leverage_class.clone(),
                primary_lane: signal.primary_lane.clone(),
                default_surface_role: signal.default_surface_role.clone(),
                scope: signal.scope.clone(),
                severity: signal.severity,
                score_0_10000: signal.score_0_10000,
                summary: signal.summary.clone(),
                impact: signal.impact.clone(),
                files: signal.files.clone(),
                role_tags: signal.role_tags.clone(),
                leverage_reasons: signal.leverage_reasons.clone(),
                evidence: signal.evidence.clone(),
                inspection_focus: signal.inspection_focus.clone(),
                candidate_split_axes: signal.candidate_split_axes.clone(),
                related_surfaces: signal.related_surfaces.clone(),
                signal_families: signal.signal_families.clone(),
                clone_family_count: usize::from(signal.kind == "clone_family"),
                hotspot_count: usize::from(signal.kind == "hotspot"),
                missing_site_count: signal.metrics.missing_site_count.unwrap_or(0),
                boundary_pressure_count: signal.metrics.boundary_pressure_count.unwrap_or(0),
            })
        })
        .collect::<Vec<_>>();

    watchpoints.sort_by(|left, right| {
        right
            .severity
            .priority()
            .cmp(&left.severity.priority())
            .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
            .then_with(|| left.scope.cmp(&right.scope))
    });
    watchpoints.truncate(limit);
    watchpoints
}

pub(super) fn merge_watchpoints(
    left: Vec<InspectionWatchpoint>,
    right: Vec<InspectionWatchpoint>,
    limit: usize,
) -> Vec<InspectionWatchpoint> {
    let mut watchpoints = left;
    watchpoints.extend(right);
    watchpoints.sort_by(|left, right| {
        right
            .severity
            .priority()
            .cmp(&left.severity.priority())
            .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
            .then_with(|| left.scope.cmp(&right.scope))
    });
    watchpoints.truncate(limit);
    watchpoints
}

pub(super) fn build_debt_clusters(signals: &[DebtSignal], limit: usize) -> Vec<DebtCluster> {
    let mut visited = BTreeSet::new();
    let mut clusters = Vec::new();

    for start_index in 0..signals.len() {
        if !visited.insert(start_index) {
            continue;
        }

        let component = debt_cluster_component(start_index, signals, &mut visited);
        if component.len() < 2 {
            continue;
        }

        if let Some(cluster) = debt_cluster(&component) {
            clusters.push(cluster);
        }
    }

    clusters.sort_by(|left, right| {
        right
            .severity
            .priority()
            .cmp(&left.severity.priority())
            .then_with(|| right.score_0_10000.cmp(&left.score_0_10000))
            .then_with(|| left.scope.cmp(&right.scope))
    });
    clusters.truncate(limit);
    clusters
}

fn debt_cluster_component(
    start_index: usize,
    signals: &[DebtSignal],
    visited: &mut BTreeSet<usize>,
) -> Vec<DebtSignal> {
    let mut queue = vec![start_index];
    let mut component = Vec::new();

    while let Some(index) = queue.pop() {
        let signal = signals[index].clone();
        for next_index in 0..signals.len() {
            if visited.contains(&next_index) {
                continue;
            }
            if files_overlap(&signal.files, &signals[next_index].files) {
                visited.insert(next_index);
                queue.push(next_index);
            }
        }
        component.push(signal);
    }

    component
}

fn debt_cluster(signals: &[DebtSignal]) -> Option<DebtCluster> {
    let files = debt_cluster_files(signals);
    if files.is_empty() {
        return None;
    }
    let file_count = files.len();

    let signal_kinds = debt_cluster_signal_kinds(signals);
    let signal_families = debt_cluster_signal_families(signals);
    let role_tags = debt_cluster_role_tags(signals);
    let summary = debt_cluster_summary(&files, &signal_kinds, signals.len());
    let impact = debt_cluster_impact(&signal_families);
    let evidence = debt_cluster_evidence(signals, &signal_kinds, &role_tags, files.len());
    let inspection_focus = debt_cluster_inspection_focus(signals);

    let highest_score = signals
        .iter()
        .map(|signal| signal.score_0_10000)
        .max()
        .unwrap_or(0);
    let trust_tier = cluster_trust_tier(signals);
    let presentation_class = cluster_presentation_class(signals);
    let aggregate_bonus = ((signals.len().saturating_sub(1)) as u32 * 500).min(2000);
    let score_0_10000 = (highest_score + aggregate_bonus).min(10_000);
    let primary_lane =
        classify_primary_lane("debt_cluster", trust_tier, presentation_class).to_string();
    let default_surface_role =
        classify_default_surface_role("debt_cluster", &primary_lane, presentation_class)
            .to_string();

    Some(DebtCluster {
        trust_tier,
        presentation_class,
        leverage_class: cluster_leverage_class(signals),
        primary_lane,
        default_surface_role,
        scope: format!("cluster:{}", files.join("|")),
        severity: signal_severity(score_0_10000),
        score_0_10000,
        summary,
        impact,
        files,
        role_tags,
        leverage_reasons: cluster_leverage_reasons(signals),
        evidence,
        inspection_focus,
        signal_families: signal_families.clone(),
        signal_kinds: signal_kinds.clone(),
        metrics: DebtClusterMetrics {
            signal_count: signals.len(),
            file_count,
            concept_count: signals
                .iter()
                .filter(|signal| signal.kind == "concept")
                .count(),
            clone_family_count: signals
                .iter()
                .filter(|signal| signal.kind == "clone_family")
                .count(),
            hotspot_count: signals
                .iter()
                .filter(|signal| signal.kind == "hotspot" || signal.kind == "unstable_hotspot")
                .count(),
            structural_signal_count: signals
                .iter()
                .filter(|signal| is_structural_debt_signal_kind(&signal.kind))
                .count(),
        },
    })
}

fn debt_cluster_files(signals: &[DebtSignal]) -> Vec<String> {
    dedupe_strings_preserve_order(
        signals
            .iter()
            .flat_map(|signal| signal.files.iter().cloned())
            .collect::<Vec<_>>(),
    )
}

fn debt_cluster_signal_kinds(signals: &[DebtSignal]) -> Vec<String> {
    dedupe_strings_preserve_order(
        signals
            .iter()
            .map(|signal| signal.kind.clone())
            .collect::<Vec<_>>(),
    )
}

fn debt_cluster_signal_families(signals: &[DebtSignal]) -> Vec<String> {
    dedupe_strings_preserve_order(
        signals
            .iter()
            .flat_map(|signal| signal.signal_families.iter().cloned())
            .collect::<Vec<_>>(),
    )
}

fn debt_cluster_role_tags(signals: &[DebtSignal]) -> Vec<String> {
    dedupe_strings_preserve_order(
        signals
            .iter()
            .flat_map(|signal| signal.role_tags.iter().cloned())
            .collect::<Vec<_>>(),
    )
}

fn debt_cluster_summary(files: &[String], signal_kinds: &[String], signal_count: usize) -> String {
    if files.len() == 1 {
        return format!(
            "File '{}' intersects {} debt signals: {}",
            files[0],
            signal_count,
            signal_kinds.join(", ")
        );
    }

    format!(
        "Files {} intersect {} debt signals: {}",
        sample_file_labels(files, 3),
        signal_count,
        signal_kinds.join(", ")
    )
}

fn debt_cluster_impact(signal_families: &[String]) -> String {
    if signal_families.iter().any(|family| family == "ownership")
        && signal_families.iter().any(|family| family == "propagation")
    {
        return "Overlapping ownership drift and propagation burden make partial edits easier to miss and harder to validate.".to_string();
    }
    if signal_families.iter().any(|family| family == "duplication")
        && signal_families
            .iter()
            .any(|family| family == "coordination")
    {
        return "Duplicated logic inside coordination-heavy seams increases the chance that fixes land in one path but not the others.".to_string();
    }

    "Multiple overlapping debt signals in the same surface increase change cost and make regressions harder to isolate.".to_string()
}

fn debt_cluster_evidence(
    signals: &[DebtSignal],
    signal_kinds: &[String],
    role_tags: &[String],
    file_count: usize,
) -> Vec<String> {
    let mut evidence = vec![
        format!("overlapping signals: {}", signals.len()),
        format!("signal kinds: {}", signal_kinds.join(", ")),
        format!("affected files: {}", file_count),
    ];
    if !role_tags.is_empty() {
        evidence.push(format!("role tags: {}", role_tags.join(", ")));
    }
    evidence.extend(
        signals
            .iter()
            .take(3)
            .map(|signal| format!("{}: {}", signal.kind, signal.summary)),
    );
    dedupe_strings_preserve_order(evidence)
}

fn debt_cluster_inspection_focus(signals: &[DebtSignal]) -> Vec<String> {
    let mut inspection_focus = signals
        .iter()
        .flat_map(|signal| signal.inspection_focus.iter().cloned())
        .collect::<Vec<_>>();
    inspection_focus = dedupe_strings_preserve_order(inspection_focus);
    inspection_focus.truncate(4);
    inspection_focus
}

fn cluster_trust_tier(signals: &[DebtSignal]) -> DebtTrustTier {
    if signals
        .iter()
        .any(|signal| signal.trust_tier == DebtTrustTier::Trusted)
    {
        DebtTrustTier::Trusted
    } else if signals
        .iter()
        .any(|signal| signal.trust_tier == DebtTrustTier::Watchpoint)
    {
        DebtTrustTier::Watchpoint
    } else {
        DebtTrustTier::Experimental
    }
}

fn cluster_presentation_class(signals: &[DebtSignal]) -> PresentationClass {
    signals
        .iter()
        .map(|signal| signal.presentation_class)
        .min_by_key(presentation_class_rank)
        .unwrap_or(PresentationClass::StructuralDebt)
}

fn presentation_class_rank(classification: &PresentationClass) -> usize {
    classification.rank()
}

fn cluster_leverage_class(signals: &[DebtSignal]) -> FindingLeverageClass {
    signals
        .iter()
        .filter_map(|signal| signal.leverage_class)
        .min_by_key(|classification| classification.rank())
        .unwrap_or(FindingLeverageClass::SecondaryCleanup)
}

fn cluster_leverage_reasons(signals: &[DebtSignal]) -> Vec<String> {
    dedupe_strings_preserve_order(
        signals
            .iter()
            .flat_map(|signal| signal.leverage_reasons.iter().cloned())
            .collect(),
    )
}

fn is_structural_debt_signal_kind(kind: &str) -> bool {
    matches!(
        kind,
        "large_file"
            | "dependency_sprawl"
            | "unstable_hotspot"
            | "cycle_cluster"
            | "dead_private_code_cluster"
            | "dead_island"
    )
}

fn sample_file_labels(files: &[String], limit: usize) -> String {
    let sample = files.iter().take(limit).cloned().collect::<Vec<_>>();
    if files.len() <= limit {
        return sample.join(", ");
    }
    format!("{}, and {} more", sample.join(", "), files.len() - limit)
}

fn related_clone_families<'a>(
    summary: &ConceptDebtSummary,
    clone_families: &'a [Value],
) -> Vec<&'a Value> {
    clone_families
        .iter()
        .filter(|family| files_overlap(&summary.files, &finding_files(family)))
        .collect()
}

fn related_hotspots<'a>(
    summary: &ConceptDebtSummary,
    concentration_reports: &'a [crate::metrics::v2::ConcentrationReport],
) -> Vec<&'a crate::metrics::v2::ConcentrationReport> {
    concentration_reports
        .iter()
        .filter(|report| summary.files.iter().any(|path| path == &report.path))
        .collect()
}

fn files_overlap(left: &[String], right: &[String]) -> bool {
    let right_files = right.iter().collect::<BTreeSet<_>>();
    left.iter().any(|path| right_files.contains(path))
}

fn inspection_watchpoint_score(
    summary: &ConceptDebtSummary,
    clone_family_count: usize,
    hotspot_count: usize,
) -> u32 {
    let clone_pressure = (clone_family_count as u32 * INSPECTION_CLONE_PRESSURE_UNIT)
        .min(INSPECTION_CLONE_PRESSURE_MAX);
    let hotspot_pressure = (hotspot_count as u32 * INSPECTION_HOTSPOT_PRESSURE_UNIT)
        .min(INSPECTION_HOTSPOT_PRESSURE_MAX);
    let compound_bonus = if summary.boundary_pressure_count > 0 && summary.missing_site_count > 0 {
        INSPECTION_COMPOUND_BONUS
    } else {
        0
    };

    (summary.score_0_10000 + clone_pressure + hotspot_pressure + compound_bonus).min(10_000)
}

fn inspection_watchpoint_summary(
    summary: &ConceptDebtSummary,
    clone_family_count: usize,
    hotspot_count: usize,
) -> String {
    let mut overlaps = Vec::new();
    if summary.boundary_pressure_count > 0 {
        overlaps.push("boundary pressure");
    }
    if summary.missing_site_count > 0 {
        overlaps.push("propagation burden");
    }
    if clone_family_count > 0 {
        overlaps.push("clone overlap");
    }
    if hotspot_count > 0 {
        overlaps.push("coordination hotspot overlap");
    }

    if overlaps.is_empty() {
        return summary.summary.clone();
    }

    format!(
        "Concept '{}' intersects {}",
        summary.concept_id,
        overlaps.join(", ")
    )
}

fn inspection_watchpoint_evidence(
    summary: &ConceptDebtSummary,
    clone_families: &[&Value],
    hotspots: &[&crate::metrics::v2::ConcentrationReport],
) -> Vec<String> {
    let mut evidence = Vec::new();
    if summary.boundary_pressure_count > 0 {
        evidence.push(format!(
            "boundary and ownership findings: {}",
            summary.boundary_pressure_count
        ));
    }
    if summary.missing_site_count > 0 {
        evidence.push(format!(
            "missing update sites: {}",
            summary.missing_site_count
        ));
    }
    if summary.context_burden > 0 {
        evidence.push(format!("context burden: {}", summary.context_burden));
    }
    if !clone_families.is_empty() {
        evidence.push(format!("related clone families: {}", clone_families.len()));
        evidence.extend(
            clone_families
                .iter()
                .take(2)
                .filter_map(|family| family.get("summary").and_then(|value| value.as_str()))
                .map(str::to_string),
        );
    }
    if !hotspots.is_empty() {
        evidence.push(format!("related hotspots: {}", hotspots.len()));
        evidence.extend(
            hotspots
                .iter()
                .take(2)
                .map(|report| format!("hotspot file: {}", report.path)),
        );
    }

    evidence
}

fn inspection_watchpoint_focus(
    summary: &ConceptDebtSummary,
    clone_families: &[&Value],
    hotspots: &[&crate::metrics::v2::ConcentrationReport],
) -> Vec<String> {
    let mut focus = summary.inspection_focus.clone();
    if !clone_families.is_empty() {
        focus.push(
            "inspect whether the repeated clone surfaces represent shared debt or intentional divergence"
                .to_string(),
        );
    }
    if !hotspots.is_empty() {
        focus.push(
            "inspect whether orchestration, storage, and adapter responsibilities are accumulating in one seam"
                .to_string(),
        );
    }
    if summary.boundary_pressure_count > 0 && summary.missing_site_count > 0 {
        focus.push(
            "inspect whether boundary erosion is making the propagation chain easier to miss"
                .to_string(),
        );
    }
    focus = dedupe_strings_preserve_order(focus);
    focus.truncate(4);
    focus
}

fn inspection_watchpoint_signal_families(
    summary: &ConceptDebtSummary,
    clone_family_count: usize,
    hotspot_count: usize,
) -> Vec<String> {
    let mut families = concept_signal_families(summary);
    if clone_family_count > 0 {
        families.push("duplication".to_string());
    }
    if hotspot_count > 0 {
        families.push("coordination".to_string());
    }
    dedupe_strings_preserve_order(families)
}
