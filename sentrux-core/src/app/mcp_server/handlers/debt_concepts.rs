use super::*;

pub(crate) fn build_concept_debt_summaries(
    findings: &[Value],
    obligations: &[crate::metrics::v2::ObligationReport],
) -> Vec<ConceptDebtSummary> {
    let mut aggregates = BTreeMap::<String, ConceptDebtAggregate>::new();

    for finding in findings {
        accumulate_concept_debt_finding(&mut aggregates, finding);
    }

    for obligation in obligations {
        let Some(concept_id) = obligation.concept_id.as_ref() else {
            continue;
        };
        let entry = aggregates.entry(concept_id.clone()).or_default();
        entry.obligation_count += 1;
        entry.missing_site_count += obligation.missing_sites.len();
        entry.context_burden += obligation.context_burden;
        entry.files.extend(obligation.files.iter().cloned());
    }

    let mut summaries = aggregates
        .into_iter()
        .map(|(concept_id, aggregate)| summarize_concept_debt(concept_id, aggregate))
        .filter(|summary| summary.finding_count > 0 || summary.missing_site_count > 0)
        .collect::<Vec<_>>();

    summaries.sort_by(|left, right| {
        right
            .score_0_10000
            .cmp(&left.score_0_10000)
            .then_with(|| right.high_severity_count.cmp(&left.high_severity_count))
            .then_with(|| left.concept_id.cmp(&right.concept_id))
    });
    summaries
}

fn accumulate_concept_debt_finding(
    aggregates: &mut BTreeMap<String, ConceptDebtAggregate>,
    finding: &Value,
) {
    let Some(concept_id) = finding_concept_id(finding) else {
        return;
    };
    let entry = aggregates.entry(concept_id.to_string()).or_default();
    let kind = finding_kind(finding).to_string();
    entry.finding_count += 1;
    if severity_of_value(finding) == FindingSeverity::High {
        entry.high_severity_count += 1;
    }
    if matches!(
        kind.as_str(),
        "multi_writer_concept"
            | "forbidden_writer"
            | "writer_outside_allowlist"
            | "forbidden_raw_read"
            | "authoritative_import_bypass"
            | "concept_boundary_pressure"
    ) {
        entry.boundary_pressure_count += 1;
    }
    entry
        .kinds
        .entry(kind)
        .and_modify(|count| *count += 1)
        .or_insert(1);
    entry.files.extend(finding_files(finding));
}

fn summarize_concept_debt(
    concept_id: String,
    aggregate: ConceptDebtAggregate,
) -> ConceptDebtSummary {
    let ConceptDebtAggregate {
        finding_count,
        high_severity_count,
        boundary_pressure_count,
        obligation_count,
        missing_site_count,
        context_burden,
        kinds,
        files,
    } = aggregate;
    let mut dominant_kinds = kinds.into_iter().collect::<Vec<_>>();
    dominant_kinds.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let dominant_kinds = dominant_kinds
        .into_iter()
        .map(|(kind, _)| kind)
        .take(3)
        .collect::<Vec<_>>();
    let score_0_10000 = concept_debt_score(
        finding_count,
        high_severity_count,
        boundary_pressure_count,
        missing_site_count,
        context_burden,
    );
    let inspection_focus = concept_debt_inspection_focus(&dominant_kinds, missing_site_count > 0);

    ConceptDebtSummary {
        summary: concept_debt_summary(
            &concept_id,
            finding_count,
            obligation_count,
            missing_site_count,
            high_severity_count,
            boundary_pressure_count,
        ),
        concept_id,
        score_0_10000,
        finding_count,
        high_severity_count,
        boundary_pressure_count,
        obligation_count,
        missing_site_count,
        context_burden,
        dominant_kinds,
        files: files.into_iter().collect(),
        inspection_focus,
    }
}

pub(crate) fn concept_debt_score(
    finding_count: usize,
    high_severity_count: usize,
    boundary_pressure_count: usize,
    missing_site_count: usize,
    context_burden: usize,
) -> u32 {
    let high_pressure = (high_severity_count as u32 * CONCEPT_DEBT_HIGH_SEVERITY_UNIT)
        .min(CONCEPT_DEBT_HIGH_SEVERITY_MAX);
    let boundary_pressure = (boundary_pressure_count as u32 * CONCEPT_DEBT_BOUNDARY_UNIT)
        .min(CONCEPT_DEBT_BOUNDARY_MAX);
    let finding_pressure =
        (finding_count as u32 * CONCEPT_DEBT_FINDING_UNIT).min(CONCEPT_DEBT_FINDING_MAX);
    let missing_pressure = (missing_site_count as u32 * CONCEPT_DEBT_MISSING_SITE_UNIT)
        .min(CONCEPT_DEBT_MISSING_SITE_MAX);
    let context_pressure =
        (context_burden as u32 * CONCEPT_DEBT_CONTEXT_UNIT).min(CONCEPT_DEBT_CONTEXT_MAX);

    (high_pressure + boundary_pressure + finding_pressure + missing_pressure + context_pressure)
        .min(10_000)
}

pub(crate) fn concept_debt_summary(
    concept_id: &str,
    finding_count: usize,
    obligation_count: usize,
    missing_site_count: usize,
    high_severity_count: usize,
    boundary_pressure_count: usize,
) -> String {
    if boundary_pressure_count > 0 && missing_site_count > 0 {
        return format!(
            "Concept '{}' shows {} boundary/ownership findings and {} missing update sites",
            concept_id, boundary_pressure_count, missing_site_count
        );
    }
    if high_severity_count > 0 && missing_site_count > 0 {
        return format!(
            "Concept '{}' shows {} high-severity findings and {} missing update sites",
            concept_id, high_severity_count, missing_site_count
        );
    }
    if missing_site_count > 0 {
        return format!(
            "Concept '{}' spans {} obligation reports with {} missing update sites",
            concept_id, obligation_count, missing_site_count
        );
    }
    if high_severity_count > 0 {
        return format!(
            "Concept '{}' has {} high-severity ownership or access findings",
            concept_id, high_severity_count
        );
    }
    format!(
        "Concept '{}' has {} repeated structural findings",
        concept_id, finding_count
    )
}

pub(crate) fn concept_debt_inspection_focus(
    dominant_kinds: &[String],
    has_missing_sites: bool,
) -> Vec<String> {
    let mut focus = Vec::new();
    for kind in dominant_kinds {
        match kind.as_str() {
            "multi_writer_concept"
            | "forbidden_writer"
            | "writer_outside_allowlist"
            | "concept_boundary_pressure" => {
                focus.push("inspect write ownership and boundary enforcement".to_string());
            }
            "forbidden_raw_read" | "authoritative_import_bypass" => {
                focus.push(
                    "inspect whether reads bypass the canonical accessor or public boundary"
                        .to_string(),
                );
            }
            _ => {}
        }
    }
    if has_missing_sites {
        focus.push(
            "inspect the explicit propagation sites and completeness tests for this concept"
                .to_string(),
        );
    }
    if focus.is_empty() {
        focus.push("inspect the concept boundary and repeated finding kinds".to_string());
    }
    focus = dedupe_strings_preserve_order(focus);
    focus.truncate(3);
    focus
}

pub(crate) fn signal_severity(score_0_10000: u32) -> FindingSeverity {
    match score_0_10000 {
        6500..=10_000 => FindingSeverity::High,
        3000..=6499 => FindingSeverity::Medium,
        _ => FindingSeverity::Low,
    }
}

pub(crate) fn concept_signal_class(summary: &ConceptDebtSummary) -> SignalClass {
    if summary.boundary_pressure_count > 0 || summary.high_severity_count > 0 {
        SignalClass::Debt
    } else if summary.missing_site_count > 0 {
        SignalClass::Hardening
    } else {
        SignalClass::Watchpoint
    }
}

pub(crate) fn concept_signal_families(summary: &ConceptDebtSummary) -> Vec<String> {
    let mut families = Vec::new();
    if summary.boundary_pressure_count > 0 {
        families.push("ownership".to_string());
        families.push("boundary".to_string());
    }
    if summary.missing_site_count > 0 || summary.obligation_count > 0 {
        families.push("propagation".to_string());
    }
    if summary.high_severity_count > 0 && families.is_empty() {
        families.push("boundary".to_string());
    }
    if families.is_empty() {
        families.push("consistency".to_string());
    }
    dedupe_strings_preserve_order(families)
}

pub(crate) fn concept_signal_metrics(summary: &ConceptDebtSummary) -> DebtSignalMetrics {
    DebtSignalMetrics {
        finding_count: Some(summary.finding_count),
        high_severity_count: Some(summary.high_severity_count),
        boundary_pressure_count: Some(summary.boundary_pressure_count),
        obligation_count: Some(summary.obligation_count),
        missing_site_count: Some(summary.missing_site_count),
        context_burden: Some(summary.context_burden),
        file_count: Some(summary.files.len()),
        ..DebtSignalMetrics::default()
    }
}

pub(crate) fn concept_signal_impact(summary: &ConceptDebtSummary) -> String {
    if summary.boundary_pressure_count > 0 && summary.missing_site_count > 0 {
        return "Split ownership and incomplete propagation make this concept easier to regress through partial edits.".to_string();
    }
    if summary.boundary_pressure_count > 0 {
        return "Unclear ownership or boundary erosion makes the concept harder to reason about and easier to update inconsistently.".to_string();
    }
    if summary.missing_site_count > 0 {
        return "Explicit propagation burden means future changes can miss required update sites unless the concept is hardened.".to_string();
    }
    "Repeated structural findings suggest this concept will remain brittle until the boundary and update path are clearer.".to_string()
}

pub(crate) fn concept_candidate_split_axes(summary: &ConceptDebtSummary) -> Vec<String> {
    let mut axes = Vec::new();
    if summary.boundary_pressure_count > 0 {
        axes.push("ownership boundary".to_string());
    }
    if summary.missing_site_count > 0 {
        axes.push("propagation surface".to_string());
    }
    if axes.is_empty() {
        axes.push("concept boundary".to_string());
    }
    axes
}
