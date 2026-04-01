use super::*;
#[cfg(test)]
use crate::metrics::v2::FindingSeverity;

#[path = "classification_details.rs"]
mod classification_details;
#[path = "classification_readers.rs"]
mod classification_readers;

#[cfg(test)]
use classification_details::{annotate_finding_detail, FindingDetail, FindingDetailMetrics};
pub(crate) use classification_details::{
    build_finding_details, decorate_finding_with_classification, is_experimental_finding,
    merge_findings, partition_experimental_findings, severity_of_value,
};
use classification_readers::finding_string_values;
pub(crate) use classification_readers::{
    combined_other_finding_values, dedupe_strings_preserve_order, finding_concept_id,
    finding_files, finding_kind, finding_payload_map, finding_scope, finding_values,
    serialized_values,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FindingTrustTier {
    Trusted,
    Watchpoint,
    Experimental,
}

impl FindingTrustTier {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Trusted => "trusted",
            Self::Watchpoint => "watchpoint",
            Self::Experimental => "experimental",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "watchpoint" => Self::Watchpoint,
            "experimental" => Self::Experimental,
            _ => Self::Trusted,
        }
    }
}

impl Default for FindingTrustTier {
    fn default() -> Self {
        Self::Trusted
    }
}

impl PartialEq<&str> for FindingTrustTier {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FindingPresentationClass {
    StructuralDebt,
    GuardedFacade,
    ToolingDebt,
    HardeningNote,
    Watchpoint,
    Experimental,
}

impl FindingPresentationClass {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::StructuralDebt => "structural_debt",
            Self::GuardedFacade => "guarded_facade",
            Self::ToolingDebt => "tooling_debt",
            Self::HardeningNote => "hardening_note",
            Self::Watchpoint => "watchpoint",
            Self::Experimental => "experimental",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "guarded_facade" => Self::GuardedFacade,
            "tooling_debt" => Self::ToolingDebt,
            "hardening_note" => Self::HardeningNote,
            "watchpoint" => Self::Watchpoint,
            "experimental" => Self::Experimental,
            _ => Self::StructuralDebt,
        }
    }
}

impl Default for FindingPresentationClass {
    fn default() -> Self {
        Self::StructuralDebt
    }
}

impl PartialEq<&str> for FindingPresentationClass {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FindingLeverageClass {
    SecondaryCleanup,
    LocalRefactorTarget,
    ArchitectureSignal,
    RegrowthWatchpoint,
    ToolingDebt,
    BoundaryDiscipline,
    HardeningNote,
    Experimental,
}

impl Default for FindingLeverageClass {
    fn default() -> Self {
        Self::SecondaryCleanup
    }
}

impl FindingLeverageClass {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::SecondaryCleanup => "secondary_cleanup",
            Self::LocalRefactorTarget => "local_refactor_target",
            Self::ArchitectureSignal => "architecture_signal",
            Self::RegrowthWatchpoint => "regrowth_watchpoint",
            Self::ToolingDebt => "tooling_debt",
            Self::BoundaryDiscipline => "boundary_discipline",
            Self::HardeningNote => "hardening_note",
            Self::Experimental => "experimental",
        }
    }

    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "local_refactor_target" => Self::LocalRefactorTarget,
            "architecture_signal" => Self::ArchitectureSignal,
            "regrowth_watchpoint" => Self::RegrowthWatchpoint,
            "tooling_debt" => Self::ToolingDebt,
            "boundary_discipline" => Self::BoundaryDiscipline,
            "hardening_note" => Self::HardeningNote,
            "experimental" => Self::Experimental,
            _ => Self::SecondaryCleanup,
        }
    }

    pub(crate) const fn rank(self) -> usize {
        match self {
            Self::ArchitectureSignal => 0,
            Self::BoundaryDiscipline => 1,
            Self::LocalRefactorTarget => 2,
            Self::RegrowthWatchpoint => 3,
            Self::SecondaryCleanup => 4,
            Self::HardeningNote => 5,
            Self::ToolingDebt => 6,
            Self::Experimental => 7,
        }
    }
}

impl PartialEq<&str> for FindingLeverageClass {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

fn finding_trust_tier(finding: &Value) -> FindingTrustTier {
    finding
        .get("trust_tier")
        .and_then(|value| value.as_str())
        .map(FindingTrustTier::from_str)
        .unwrap_or_else(|| trust_tier_for_kind(finding_kind(finding), FindingTrustTier::Trusted))
}

fn looks_like_tooling_scope(scope: &str) -> bool {
    scope.starts_with("scripts/")
}

fn looks_like_transport_facade_scope(scope: &str) -> bool {
    scope.contains("/ipc.")
        || scope.contains("-ipc.")
        || scope.ends_with("/ipc.ts")
        || scope.ends_with("/ipc.tsx")
        || scope.contains("/browser-http-ipc.")
}

fn is_watchpoint_presentation_kind(kind: &str) -> bool {
    matches!(
        kind,
        "cycle_cluster" | "dead_island" | "clone_family" | "clone_group" | "exact_clone_group"
    )
}

fn is_hardening_note_kind(kind: &str) -> bool {
    matches!(
        kind,
        "closed_domain_exhaustiveness" | "contract_surface_completeness"
    )
}

fn role_tags_include(role_tags: &[String], tag: &str) -> bool {
    role_tags.iter().any(|role_tag| role_tag == tag)
}

pub(crate) fn classify_presentation_class(
    kind: &str,
    trust_tier: &str,
    scope: &str,
    files: &[String],
    role_tags: &[String],
    evidence_count: usize,
    finding_count: usize,
    boundary_pressure_count: usize,
    missing_site_count: usize,
) -> String {
    classify_presentation_class_internal(
        kind,
        FindingTrustTier::from_str(trust_tier),
        scope,
        files,
        role_tags,
        evidence_count,
        finding_count,
        boundary_pressure_count,
        missing_site_count,
    )
    .as_str()
    .to_string()
}

fn classify_presentation_class_internal(
    kind: &str,
    trust_tier: FindingTrustTier,
    scope: &str,
    files: &[String],
    role_tags: &[String],
    evidence_count: usize,
    finding_count: usize,
    boundary_pressure_count: usize,
    missing_site_count: usize,
) -> FindingPresentationClass {
    if trust_tier == FindingTrustTier::Experimental {
        return FindingPresentationClass::Experimental;
    }
    if trust_tier == FindingTrustTier::Watchpoint || is_watchpoint_presentation_kind(kind) {
        return FindingPresentationClass::Watchpoint;
    }
    if looks_like_tooling_scope(scope)
        || (!files.is_empty() && files.iter().all(|path| looks_like_tooling_scope(path)))
    {
        return FindingPresentationClass::ToolingDebt;
    }
    if role_tags.iter().any(|tag| tag == "transport_facade")
        || looks_like_transport_facade_scope(scope)
    {
        return FindingPresentationClass::GuardedFacade;
    }
    if is_hardening_note_kind(kind)
        && files.len() <= 2
        && evidence_count <= 2
        && finding_count <= 1
        && boundary_pressure_count == 0
        && missing_site_count <= 1
    {
        return FindingPresentationClass::HardeningNote;
    }

    FindingPresentationClass::StructuralDebt
}

fn finding_presentation_class(finding: &Value) -> FindingPresentationClass {
    if let Some(classification) = finding
        .get("presentation_class")
        .and_then(|value| value.as_str())
    {
        return FindingPresentationClass::from_str(classification);
    }

    let files = dedupe_strings_preserve_order(finding_files(finding));
    let role_tags = finding_string_values(finding, "role_tags");
    let evidence_count = finding_string_values(finding, "evidence").len();
    classify_presentation_class_internal(
        finding_kind(finding),
        finding_trust_tier(finding),
        &finding_scope(finding),
        &files,
        &role_tags,
        evidence_count,
        1,
        0,
        0,
    )
}

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

fn classify_leverage_class_internal(
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
    if trust_tier == FindingTrustTier::Experimental
        || presentation_class == FindingPresentationClass::Experimental
    {
        return FindingLeverageClass::Experimental;
    }
    if presentation_class == FindingPresentationClass::ToolingDebt {
        return FindingLeverageClass::ToolingDebt;
    }
    if presentation_class == FindingPresentationClass::HardeningNote {
        return FindingLeverageClass::HardeningNote;
    }
    if presentation_class == FindingPresentationClass::GuardedFacade {
        return FindingLeverageClass::BoundaryDiscipline;
    }
    if kind == "cycle_cluster" {
        if role_tags_include(role_tags, "component_barrel")
            || role_tags_include(role_tags, "guarded_boundary")
            || cycle_size.unwrap_or(0) >= 10
            || cut_candidate_count.unwrap_or(0) > 0
        {
            return FindingLeverageClass::ArchitectureSignal;
        }
        return FindingLeverageClass::SecondaryCleanup;
    }
    if kind == "dead_island" {
        return FindingLeverageClass::SecondaryCleanup;
    }
    if boundary_pressure_count > 0 && missing_site_count > 0 {
        return FindingLeverageClass::ArchitectureSignal;
    }
    if role_tags_include(role_tags, "component_barrel")
        || role_tags_include(role_tags, "guarded_boundary")
    {
        return FindingLeverageClass::ArchitectureSignal;
    }
    if role_tags_include(role_tags, "composition_root")
        || role_tags_include(role_tags, "entry_surface")
    {
        return FindingLeverageClass::RegrowthWatchpoint;
    }
    if role_tags_include(role_tags, "facade_with_extracted_owners") {
        if extracted_owner_facade_needs_secondary_cleanup(
            kind,
            role_tags,
            line_count,
            max_complexity,
            fan_in,
        ) {
            return FindingLeverageClass::SecondaryCleanup;
        }
        return FindingLeverageClass::LocalRefactorTarget;
    }
    if boundary_pressure_count > 0 || missing_site_count > 0 {
        return FindingLeverageClass::LocalRefactorTarget;
    }
    if matches!(kind, "clone_family" | "clone_group" | "exact_clone_group") {
        return FindingLeverageClass::SecondaryCleanup;
    }
    if matches!(kind, "dependency_sprawl" | "unstable_hotspot" | "hotspot")
        || guardrail_test_count.unwrap_or(0) > 0
        || fan_out.unwrap_or(0) > 0
    {
        return FindingLeverageClass::LocalRefactorTarget;
    }
    FindingLeverageClass::SecondaryCleanup
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

fn classify_leverage_reasons_internal(
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
    match leverage_class {
        FindingLeverageClass::ToolingDebt => {
            reasons.push("tooling_surface_maintenance_burden".to_string())
        }
        FindingLeverageClass::HardeningNote => reasons.push("narrow_completeness_gap".to_string()),
        FindingLeverageClass::BoundaryDiscipline => {
            reasons.push("boundary_or_facade_seam_pressure".to_string());
            if fan_in.unwrap_or(0) > 0 {
                reasons.push("heavy_inbound_seam_pressure".to_string());
            }
        }
        FindingLeverageClass::ArchitectureSignal => {
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
        }
        FindingLeverageClass::LocalRefactorTarget => {
            if role_tags_include(role_tags, "facade_with_extracted_owners") {
                reasons.push("extracted_owner_shell_pressure".to_string());
            }
            if guardrail_test_count.unwrap_or(0) > 0 {
                reasons.push("guardrail_backed_refactor_surface".to_string());
            }
            if is_contained_refactor_surface(
                role_tags,
                fan_in,
                fan_out,
                cycle_size,
                guardrail_test_count,
            ) {
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
        }
        FindingLeverageClass::RegrowthWatchpoint => {
            reasons.push("intentionally_central_surface".to_string());
            if fan_out.unwrap_or(0) > 0 {
                reasons.push("fan_out_regrowth_pressure".to_string());
            }
        }
        FindingLeverageClass::SecondaryCleanup => {
            if kind == "dead_island" {
                reasons.push("disconnected_internal_component".to_string());
            } else if matches!(kind, "clone_family" | "clone_group" | "exact_clone_group") {
                reasons.push("duplicate_maintenance_pressure".to_string());
            } else if role_tags_include(role_tags, "facade_with_extracted_owners") {
                reasons.push("secondary_facade_cleanup".to_string());
            } else if cycle_size.unwrap_or(0) > 0 {
                reasons.push("smaller_cycle_watchpoint".to_string());
            } else {
                reasons.push("real_but_lower_leverage_cleanup".to_string());
            }
        }
        FindingLeverageClass::Experimental => {}
    }
    dedupe_strings_preserve_order(reasons)
}

fn finding_numeric_metric(finding: &Value, key: &str) -> Option<usize> {
    finding
        .get("metrics")
        .and_then(|value| value.get(key))
        .or_else(|| finding.get(key))
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
}

fn finding_leverage_class(finding: &Value) -> FindingLeverageClass {
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

fn finding_leverage_reasons(finding: &Value) -> Vec<String> {
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

fn trust_tier_for_kind(kind: &str, default: FindingTrustTier) -> FindingTrustTier {
    match kind {
        "cycle_cluster" | "dead_island" => FindingTrustTier::Watchpoint,
        "dead_private_code_cluster" => FindingTrustTier::Experimental,
        _ => default,
    }
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
    fn presentation_class_demotes_narrow_hardening_notes_and_tooling_surfaces() {
        assert_eq!(
            classify_presentation_class(
                "closed_domain_exhaustiveness",
                "trusted",
                "ConnectionBannerState",
                &[
                    "src/components/app-shell/AppConnectionBanner.tsx".to_string(),
                    "src/runtime/browser-session.ts".to_string(),
                ],
                &[],
                1,
                1,
                0,
                1,
            ),
            "hardening_note"
        );
        assert_eq!(
            classify_presentation_class(
                "large_file",
                "trusted",
                "scripts/session-stress.mjs",
                &["scripts/session-stress.mjs".to_string()],
                &[],
                5,
                1,
                0,
                0,
            ),
            "tooling_debt"
        );
        assert_eq!(
            classify_presentation_class(
                "unstable_hotspot",
                "trusted",
                "src/lib/ipc.ts",
                &["src/lib/ipc.ts".to_string()],
                &["transport_facade".to_string()],
                6,
                1,
                0,
                0,
            ),
            "guarded_facade"
        );
    }

    #[test]
    fn presentation_class_marks_exact_clone_groups_as_watchpoints() {
        assert_eq!(
            classify_presentation_class(
                "exact_clone_group",
                "trusted",
                "src/a.ts|src/b.ts",
                &["src/a.ts".to_string(), "src/b.ts".to_string()],
                &[],
                0,
                1,
                0,
                0,
            ),
            "watchpoint"
        );
    }

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

    #[test]
    fn merge_findings_orders_by_typed_severity_priority() {
        let merged = merge_findings(
            vec![json!({"kind": "clone_family", "severity": "low"})],
            vec![
                json!({"kind": "multi_writer_concept", "severity": "high"}),
                json!({"kind": "contract_surface_completeness", "severity": "medium"}),
            ],
            3,
        );

        assert_eq!(merged[0]["severity"], "high");
        assert_eq!(merged[1]["severity"], "medium");
        assert_eq!(merged[2]["severity"], "low");
    }

    #[test]
    fn annotate_finding_detail_preserves_explicit_leverage_metadata() {
        let detail = annotate_finding_detail(FindingDetail {
            kind: "unstable_hotspot".to_string(),
            trust_tier: FindingTrustTier::Trusted,
            presentation_class: FindingPresentationClass::GuardedFacade,
            leverage_class: FindingLeverageClass::BoundaryDiscipline,
            leverage_class_explicit: true,
            scope: "src/lib/ipc.ts".to_string(),
            severity: FindingSeverity::Medium,
            summary: "Transport facade is under pressure".to_string(),
            impact: "Glue can absorb domain logic.".to_string(),
            files: vec!["src/lib/ipc.ts".to_string()],
            role_tags: vec!["transport_facade".to_string()],
            leverage_reasons: vec!["boundary_or_facade_seam_pressure".to_string()],
            evidence: vec!["fan-in: 42".to_string()],
            inspection_focus: vec!["inspect policy leakage".to_string()],
            candidate_split_axes: vec!["transport boundary".to_string()],
            related_surfaces: vec!["src/lib/ipc.ts".to_string()],
            metrics: FindingDetailMetrics::default(),
        });

        assert_eq!(detail.trust_tier, FindingTrustTier::Trusted);
        assert_eq!(
            detail.presentation_class,
            FindingPresentationClass::GuardedFacade
        );
        assert_eq!(
            detail.leverage_class,
            FindingLeverageClass::BoundaryDiscipline
        );
        assert_eq!(detail.severity, FindingSeverity::Medium);
        assert_eq!(
            detail.leverage_reasons,
            vec!["boundary_or_facade_seam_pressure".to_string()]
        );

        let serialized = serde_json::to_value(&detail).expect("serialize detail");
        assert_eq!(serialized["trust_tier"], "trusted");
        assert_eq!(serialized["presentation_class"], "guarded_facade");
        assert_eq!(serialized["leverage_class"], "boundary_discipline");
        assert_eq!(serialized["severity"], "medium");
    }
}
