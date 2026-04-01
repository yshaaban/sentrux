use super::*;

pub(crate) fn annotate_debt_signal(mut signal: DebtSignal) -> DebtSignal {
    let fan_in = signal
        .metrics
        .fan_in
        .or(signal.metrics.inbound_reference_count);
    signal.leverage_class = annotate_leverage_class(
        signal.leverage_class,
        &mut signal.leverage_reasons,
        &signal.kind,
        signal.trust_tier,
        signal.presentation_class,
        &signal.role_tags,
        fan_in,
        signal.metrics.fan_out,
        signal.metrics.line_count,
        signal.metrics.max_complexity.map(|value| value as usize),
        signal.metrics.cycle_size,
        signal.metrics.cut_candidate_count,
        signal.metrics.guardrail_test_count,
        signal.metrics.boundary_pressure_count.unwrap_or(0),
        signal.metrics.missing_site_count.unwrap_or(0),
    );
    signal
}

pub(crate) fn annotate_inspection_watchpoint(
    mut watchpoint: InspectionWatchpoint,
) -> InspectionWatchpoint {
    watchpoint.leverage_class = annotate_leverage_class(
        watchpoint.leverage_class,
        &mut watchpoint.leverage_reasons,
        &watchpoint.kind,
        watchpoint.trust_tier,
        watchpoint.presentation_class,
        &watchpoint.role_tags,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        watchpoint.boundary_pressure_count,
        watchpoint.missing_site_count,
    );
    watchpoint
}

pub(crate) fn annotate_leverage_class(
    leverage_class: Option<FindingLeverageClass>,
    leverage_reasons: &mut Vec<String>,
    kind: &str,
    trust_tier: DebtTrustTier,
    presentation_class: PresentationClass,
    role_tags: &[String],
    fan_in: Option<usize>,
    fan_out: Option<usize>,
    line_count: Option<usize>,
    max_complexity: Option<usize>,
    cycle_size: Option<usize>,
    cut_candidate_count: Option<usize>,
    guardrail_test_count: Option<usize>,
    boundary_pressure_count: usize,
    missing_site_count: usize,
) -> Option<FindingLeverageClass> {
    let mut leverage_class = leverage_class
        .map(FindingLeverageClass::as_str)
        .unwrap_or_default()
        .to_string();
    backfill_leverage_fields(
        &mut leverage_class,
        leverage_reasons,
        kind,
        trust_tier.as_str(),
        presentation_class.as_str(),
        role_tags,
        fan_in,
        fan_out,
        line_count,
        max_complexity,
        cycle_size,
        cut_candidate_count,
        guardrail_test_count,
        boundary_pressure_count,
        missing_site_count,
    );
    Some(FindingLeverageClass::from_str(&leverage_class))
}

pub(crate) fn classify_debt_presentation_class(
    kind: &str,
    trust_tier: DebtTrustTier,
    scope: &str,
    files: &[String],
    role_tags: &[String],
    evidence_count: usize,
    finding_count: usize,
    boundary_pressure_count: usize,
    missing_site_count: usize,
) -> PresentationClass {
    PresentationClass::from_str(&classify_presentation_class(
        kind,
        trust_tier.as_str(),
        scope,
        files,
        role_tags,
        evidence_count,
        finding_count,
        boundary_pressure_count,
        missing_site_count,
    ))
}
