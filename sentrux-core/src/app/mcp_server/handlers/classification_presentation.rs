use super::*;

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
        "cycle_cluster"
            | "dead_island"
            | "clone_family"
            | "clone_group"
            | "exact_clone_group"
            | "touched_clone_family"
            | "missing_test_coverage"
            | "zero_config_boundary_violation"
    )
}

pub(super) fn is_contract_surface_propagation_kind(kind: &str) -> bool {
    matches!(
        kind,
        "contract_surface_completeness" | "incomplete_propagation"
    )
}

fn is_hardening_note_kind(kind: &str) -> bool {
    kind == "closed_domain_exhaustiveness" || is_contract_surface_propagation_kind(kind)
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

pub(super) fn finding_presentation_class(finding: &Value) -> FindingPresentationClass {
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
                "incomplete_propagation",
                "trusted",
                "server_state_bootstrap",
                &[
                    "src/domain/server-state-bootstrap.ts".to_string(),
                    "src/app/server-state-bootstrap-registry.ts".to_string(),
                ],
                &[],
                2,
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
}
