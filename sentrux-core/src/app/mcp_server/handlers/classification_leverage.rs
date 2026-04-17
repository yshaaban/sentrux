use super::*;

pub(crate) fn classify_leverage_class(
    kind: &str,
    trust_tier: &str,
    presentation_class: &str,
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
) -> String {
    classify_leverage_class_internal(
        kind,
        FindingTrustTier::from_str(trust_tier),
        FindingPresentationClass::from_str(presentation_class),
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
    )
    .as_str()
    .to_string()
}

pub(super) fn classify_leverage_class_internal(
    kind: &str,
    trust_tier: FindingTrustTier,
    presentation_class: FindingPresentationClass,
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
) -> FindingLeverageClass {
    if let Some(classification) =
        classify_non_structural_leverage_class(kind, trust_tier, presentation_class)
    {
        return classification;
    }
    if kind == "cycle_cluster" {
        return classify_cycle_cluster_leverage_class(role_tags, cycle_size, cut_candidate_count);
    }
    if kind == "dead_island" {
        return FindingLeverageClass::SecondaryCleanup;
    }
    if is_architecture_signal_surface(role_tags, boundary_pressure_count, missing_site_count) {
        return FindingLeverageClass::ArchitectureSignal;
    }
    if is_regrowth_watchpoint_surface(role_tags) {
        return FindingLeverageClass::RegrowthWatchpoint;
    }
    if let Some(classification) =
        classify_extracted_owner_leverage_class(kind, role_tags, line_count, max_complexity, fan_in)
    {
        return classification;
    }
    if boundary_pressure_count > 0 || missing_site_count > 0 {
        return FindingLeverageClass::LocalRefactorTarget;
    }
    if is_secondary_clone_cleanup(kind) {
        return FindingLeverageClass::SecondaryCleanup;
    }
    if is_local_refactor_target_surface(kind, guardrail_test_count, fan_out) {
        return FindingLeverageClass::LocalRefactorTarget;
    }
    FindingLeverageClass::SecondaryCleanup
}

fn classify_non_structural_leverage_class(
    kind: &str,
    trust_tier: FindingTrustTier,
    presentation_class: FindingPresentationClass,
) -> Option<FindingLeverageClass> {
    if trust_tier == FindingTrustTier::Experimental
        || presentation_class == FindingPresentationClass::Experimental
    {
        return Some(FindingLeverageClass::Experimental);
    }
    if presentation_class == FindingPresentationClass::ToolingDebt {
        return Some(FindingLeverageClass::ToolingDebt);
    }
    if matches!(kind, "session_introduced_clone" | "clone_propagation_drift") {
        return Some(FindingLeverageClass::LocalRefactorTarget);
    }
    if presentation_class == FindingPresentationClass::HardeningNote {
        return Some(FindingLeverageClass::HardeningNote);
    }
    if presentation_class == FindingPresentationClass::GuardedFacade {
        return Some(FindingLeverageClass::BoundaryDiscipline);
    }

    None
}

fn classify_cycle_cluster_leverage_class(
    role_tags: &[String],
    cycle_size: Option<usize>,
    cut_candidate_count: Option<usize>,
) -> FindingLeverageClass {
    if role_tags_include(role_tags, "component_barrel")
        || role_tags_include(role_tags, "guarded_boundary")
        || cycle_size.unwrap_or(0) >= 10
        || cut_candidate_count.unwrap_or(0) > 0
    {
        return FindingLeverageClass::ArchitectureSignal;
    }

    FindingLeverageClass::SecondaryCleanup
}

fn is_architecture_signal_surface(
    role_tags: &[String],
    boundary_pressure_count: usize,
    missing_site_count: usize,
) -> bool {
    (boundary_pressure_count > 0 && missing_site_count > 0)
        || role_tags_include(role_tags, "component_barrel")
        || role_tags_include(role_tags, "guarded_boundary")
}

fn is_regrowth_watchpoint_surface(role_tags: &[String]) -> bool {
    role_tags_include(role_tags, "composition_root")
        || role_tags_include(role_tags, "entry_surface")
}

fn classify_extracted_owner_leverage_class(
    kind: &str,
    role_tags: &[String],
    line_count: Option<usize>,
    max_complexity: Option<usize>,
    fan_in: Option<usize>,
) -> Option<FindingLeverageClass> {
    if !role_tags_include(role_tags, "facade_with_extracted_owners") {
        return None;
    }

    if extracted_owner_facade_needs_secondary_cleanup(
        kind,
        role_tags,
        line_count,
        max_complexity,
        fan_in,
    ) {
        return Some(FindingLeverageClass::SecondaryCleanup);
    }

    Some(FindingLeverageClass::LocalRefactorTarget)
}

fn is_secondary_clone_cleanup(kind: &str) -> bool {
    matches!(
        kind,
        "clone_family" | "clone_group" | "exact_clone_group" | "touched_clone_family"
    )
}

fn is_local_refactor_target_surface(
    kind: &str,
    guardrail_test_count: Option<usize>,
    fan_out: Option<usize>,
) -> bool {
    matches!(kind, "dependency_sprawl" | "unstable_hotspot" | "hotspot")
        || guardrail_test_count.unwrap_or(0) > 0
        || fan_out.unwrap_or(0) > 0
}

fn extracted_owner_facade_needs_secondary_cleanup(
    kind: &str,
    role_tags: &[String],
    line_count: Option<usize>,
    max_complexity: Option<usize>,
    fan_in: Option<usize>,
) -> bool {
    if role_tags_include(role_tags, "entry_surface") {
        return true;
    }
    if kind == "large_file" {
        return true;
    }
    if line_count.unwrap_or(0) >= 500 {
        return true;
    }
    if max_complexity.unwrap_or(0) >= 20 {
        return true;
    }
    fan_in.unwrap_or(0) >= 20
}

fn is_contained_refactor_surface(
    role_tags: &[String],
    fan_in: Option<usize>,
    fan_out: Option<usize>,
    cycle_size: Option<usize>,
    guardrail_test_count: Option<usize>,
) -> bool {
    let has_extracted_owner_surface = role_tags_include(role_tags, "facade_with_extracted_owners");
    let guardrail_count = guardrail_test_count.unwrap_or(0);
    let inbound_pressure = fan_in.unwrap_or(0);
    let dependency_breadth = fan_out.unwrap_or(0);
    let cycle_span = cycle_size.unwrap_or(0);

    (has_extracted_owner_surface || guardrail_count > 0)
        && dependency_breadth >= 3
        && (inbound_pressure == 0 || inbound_pressure <= 12)
        && (cycle_span == 0 || cycle_span <= 6)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn classify_leverage_reasons(
    kind: &str,
    trust_tier: &str,
    presentation_class: &str,
    leverage_class: &str,
    role_tags: &[String],
    fan_in: Option<usize>,
    fan_out: Option<usize>,
    cycle_size: Option<usize>,
    cut_candidate_count: Option<usize>,
    guardrail_test_count: Option<usize>,
    boundary_pressure_count: usize,
    missing_site_count: usize,
) -> Vec<String> {
    classify_leverage_reasons_internal(
        kind,
        FindingTrustTier::from_str(trust_tier),
        FindingPresentationClass::from_str(presentation_class),
        FindingLeverageClass::from_str(leverage_class),
        role_tags,
        fan_in,
        fan_out,
        cycle_size,
        cut_candidate_count,
        guardrail_test_count,
        boundary_pressure_count,
        missing_site_count,
    )
}

pub(super) fn classify_leverage_reasons_internal(
    kind: &str,
    trust_tier: FindingTrustTier,
    presentation_class: FindingPresentationClass,
    leverage_class: FindingLeverageClass,
    role_tags: &[String],
    fan_in: Option<usize>,
    fan_out: Option<usize>,
    cycle_size: Option<usize>,
    cut_candidate_count: Option<usize>,
    guardrail_test_count: Option<usize>,
    boundary_pressure_count: usize,
    missing_site_count: usize,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if trust_tier == FindingTrustTier::Experimental
        || presentation_class == FindingPresentationClass::Experimental
    {
        reasons.push("detector_under_evaluation".to_string());
    }
    reasons.extend(leverage_class_reasons(
        leverage_class,
        kind,
        role_tags,
        fan_in,
        fan_out,
        cycle_size,
        cut_candidate_count,
        guardrail_test_count,
        boundary_pressure_count,
        missing_site_count,
    ));
    dedupe_strings_preserve_order(reasons)
}

#[allow(clippy::too_many_arguments)]
fn leverage_class_reasons(
    leverage_class: FindingLeverageClass,
    kind: &str,
    role_tags: &[String],
    fan_in: Option<usize>,
    fan_out: Option<usize>,
    cycle_size: Option<usize>,
    cut_candidate_count: Option<usize>,
    guardrail_test_count: Option<usize>,
    boundary_pressure_count: usize,
    missing_site_count: usize,
) -> Vec<String> {
    match leverage_class {
        FindingLeverageClass::ToolingDebt => vec!["tooling_surface_maintenance_burden".to_string()],
        FindingLeverageClass::HardeningNote => vec!["narrow_completeness_gap".to_string()],
        FindingLeverageClass::BoundaryDiscipline => boundary_discipline_reasons(fan_in),
        FindingLeverageClass::ArchitectureSignal => architecture_signal_reasons(
            kind,
            role_tags,
            cut_candidate_count,
            boundary_pressure_count,
            missing_site_count,
        ),
        FindingLeverageClass::LocalRefactorTarget => local_refactor_target_reasons(
            role_tags,
            fan_in,
            fan_out,
            cycle_size,
            guardrail_test_count,
            boundary_pressure_count,
            missing_site_count,
        ),
        FindingLeverageClass::RegrowthWatchpoint => regrowth_watchpoint_reasons(fan_out),
        FindingLeverageClass::SecondaryCleanup => {
            secondary_cleanup_reasons(kind, role_tags, cycle_size)
        }
        FindingLeverageClass::Experimental => Vec::new(),
    }
}

fn boundary_discipline_reasons(fan_in: Option<usize>) -> Vec<String> {
    let mut reasons = vec!["boundary_or_facade_seam_pressure".to_string()];
    if fan_in.unwrap_or(0) > 0 {
        reasons.push("heavy_inbound_seam_pressure".to_string());
    }
    reasons
}

fn architecture_signal_reasons(
    kind: &str,
    role_tags: &[String],
    cut_candidate_count: Option<usize>,
    boundary_pressure_count: usize,
    missing_site_count: usize,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if role_tags_include(role_tags, "component_barrel") {
        reasons.push("shared_barrel_boundary_hub".to_string());
    }
    if role_tags_include(role_tags, "guarded_boundary") {
        reasons.push("guardrail_backed_boundary_pressure".to_string());
    }
    if kind == "cycle_cluster" {
        reasons.push("mixed_cycle_pressure".to_string());
    }
    if cut_candidate_count.unwrap_or(0) > 0 {
        reasons.push("high_leverage_cut_candidate".to_string());
    }
    if boundary_pressure_count > 0 {
        reasons.push("ownership_boundary_erosion".to_string());
    }
    if missing_site_count > 0 {
        reasons.push("propagation_burden".to_string());
    }
    reasons
}

fn local_refactor_target_reasons(
    role_tags: &[String],
    fan_in: Option<usize>,
    fan_out: Option<usize>,
    cycle_size: Option<usize>,
    guardrail_test_count: Option<usize>,
    boundary_pressure_count: usize,
    missing_site_count: usize,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if role_tags_include(role_tags, "facade_with_extracted_owners") {
        reasons.push("extracted_owner_shell_pressure".to_string());
    }
    if guardrail_test_count.unwrap_or(0) > 0 {
        reasons.push("guardrail_backed_refactor_surface".to_string());
    }
    if is_contained_refactor_surface(role_tags, fan_in, fan_out, cycle_size, guardrail_test_count) {
        reasons.push("contained_refactor_surface".to_string());
    }
    if fan_out.unwrap_or(0) > 0 {
        reasons.push("contained_dependency_pressure".to_string());
    }
    if boundary_pressure_count > 0 {
        reasons.push("narrower_ownership_split_available".to_string());
    }
    if missing_site_count > 0 {
        reasons.push("explicit_update_surface".to_string());
    }
    reasons
}

fn regrowth_watchpoint_reasons(fan_out: Option<usize>) -> Vec<String> {
    let mut reasons = vec!["intentionally_central_surface".to_string()];
    if fan_out.unwrap_or(0) > 0 {
        reasons.push("fan_out_regrowth_pressure".to_string());
    }
    reasons
}

fn secondary_cleanup_reasons(
    kind: &str,
    role_tags: &[String],
    cycle_size: Option<usize>,
) -> Vec<String> {
    if kind == "dead_island" {
        return vec!["disconnected_internal_component".to_string()];
    }
    if matches!(kind, "clone_family" | "clone_group" | "exact_clone_group") {
        return vec!["duplicate_maintenance_pressure".to_string()];
    }
    if role_tags_include(role_tags, "facade_with_extracted_owners") {
        return vec!["secondary_facade_cleanup".to_string()];
    }
    if cycle_size.unwrap_or(0) > 0 {
        return vec!["smaller_cycle_watchpoint".to_string()];
    }

    vec!["real_but_lower_leverage_cleanup".to_string()]
}

fn finding_numeric_metric(finding: &Value, key: &str) -> Option<usize> {
    finding
        .get("metrics")
        .and_then(|value| value.get(key))
        .or_else(|| finding.get(key))
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
}

pub(super) fn finding_leverage_class(finding: &Value) -> FindingLeverageClass {
    if let Some(classification) = finding
        .get("leverage_class")
        .and_then(|value| value.as_str())
    {
        return FindingLeverageClass::from_str(classification);
    }

    let role_tags = finding_string_values(finding, "role_tags");
    classify_leverage_class_internal(
        finding_kind(finding),
        finding_trust_tier(finding),
        finding_presentation_class(finding),
        &role_tags,
        finding_numeric_metric(finding, "fan_in")
            .or_else(|| finding_numeric_metric(finding, "inbound_reference_count")),
        finding_numeric_metric(finding, "fan_out"),
        finding_numeric_metric(finding, "line_count"),
        finding_numeric_metric(finding, "max_complexity"),
        finding_numeric_metric(finding, "cycle_size"),
        finding_numeric_metric(finding, "cut_candidate_count"),
        finding_numeric_metric(finding, "guardrail_test_count"),
        finding_numeric_metric(finding, "boundary_pressure_count").unwrap_or(0),
        finding_numeric_metric(finding, "missing_site_count").unwrap_or(0),
    )
}

pub(super) fn finding_leverage_reasons(finding: &Value) -> Vec<String> {
    let reasons = finding_string_values(finding, "leverage_reasons");
    if !reasons.is_empty() {
        return reasons;
    }

    let role_tags = finding_string_values(finding, "role_tags");
    let trust_tier = finding_trust_tier(finding);
    let presentation_class = finding_presentation_class(finding);
    let leverage_class = finding_leverage_class(finding);
    classify_leverage_reasons_internal(
        finding_kind(finding),
        trust_tier,
        presentation_class,
        leverage_class,
        &role_tags,
        finding_numeric_metric(finding, "fan_in")
            .or_else(|| finding_numeric_metric(finding, "inbound_reference_count")),
        finding_numeric_metric(finding, "fan_out"),
        finding_numeric_metric(finding, "cycle_size"),
        finding_numeric_metric(finding, "cut_candidate_count"),
        finding_numeric_metric(finding, "guardrail_test_count"),
        finding_numeric_metric(finding, "boundary_pressure_count").unwrap_or(0),
        finding_numeric_metric(finding, "missing_site_count").unwrap_or(0),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn backfill_leverage_fields(
    leverage_class: &mut String,
    leverage_reasons: &mut Vec<String>,
    kind: &str,
    trust_tier: &str,
    presentation_class: &str,
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
) {
    if leverage_class.is_empty() {
        *leverage_class = classify_leverage_class(
            kind,
            trust_tier,
            presentation_class,
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
    }

    if leverage_reasons.is_empty() {
        *leverage_reasons = classify_leverage_reasons(
            kind,
            trust_tier,
            presentation_class,
            leverage_class,
            role_tags,
            fan_in,
            fan_out,
            cycle_size,
            cut_candidate_count,
            guardrail_test_count,
            boundary_pressure_count,
            missing_site_count,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leverage_class_uses_size_and_complexity_for_extracted_owner_facades() {
        assert_eq!(
            classify_leverage_class(
                "dependency_sprawl",
                "trusted",
                "structural_debt",
                &[
                    "facade_with_extracted_owners".to_string(),
                    "guarded_seam".to_string(),
                ],
                Some(2),
                Some(28),
                Some(423),
                Some(4),
                None,
                None,
                Some(1),
                0,
                0,
            ),
            "local_refactor_target"
        );
        assert_eq!(
            classify_leverage_class(
                "dependency_sprawl",
                "trusted",
                "structural_debt",
                &[
                    "facade_with_extracted_owners".to_string(),
                    "guarded_seam".to_string(),
                ],
                Some(1),
                Some(22),
                Some(629),
                Some(82),
                None,
                None,
                Some(1),
                0,
                0,
            ),
            "secondary_cleanup"
        );
    }

    #[test]
    fn clone_followthrough_signals_keep_expected_leverage_defaults() {
        assert_eq!(
            classify_leverage_class(
                "session_introduced_clone",
                "trusted",
                "structural_debt",
                &[],
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                0,
                0,
            ),
            "local_refactor_target"
        );
        assert_eq!(
            classify_leverage_class(
                "clone_propagation_drift",
                "watchpoint",
                "hardening_note",
                &[],
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                0,
                0,
            ),
            "local_refactor_target"
        );
        assert_eq!(
            classify_leverage_class(
                "touched_clone_family",
                "watchpoint",
                "watchpoint",
                &[],
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                0,
                0,
            ),
            "secondary_cleanup"
        );
    }

    #[test]
    fn leverage_reasons_mark_contained_refactor_surfaces() {
        let reasons = classify_leverage_reasons(
            "dependency_sprawl",
            "trusted",
            "structural_debt",
            "local_refactor_target",
            &[
                "facade_with_extracted_owners".to_string(),
                "guarded_seam".to_string(),
            ],
            Some(4),
            Some(14),
            Some(0),
            None,
            Some(1),
            0,
            0,
        );

        assert!(reasons
            .iter()
            .any(|reason| reason == "contained_refactor_surface"));
    }
}
