use super::packets::{build_repair_packet, finding_risk_statement, RepairPacket};
use super::shared::{
    describe_list, guidance_kind_profile, looks_like_test_surface, FindingGuidanceInput,
    GuidanceFamily, AUTHORITATIVE_IMPORT_BYPASS_HINT, BEST_CUT_CANDIDATE_PREFIX,
    CANONICAL_OWNER_PREFIX, CHANGED_CLONE_MEMBER_PREFIX, CLONE_FAMILY_HINT, CLOSED_DOMAIN_HINT,
    CONCEPT_BOUNDARY_PRESSURE_HINT, CYCLE_CLUSTER_HINT, DEPENDENCY_SPRAWL_HINT,
    FORBIDDEN_RAW_READ_HINT, FORBIDDEN_WRITER_HINT, HOTSPOT_HINT, INCOMPLETE_PROPAGATION_HINT,
    INTRODUCED_DUPLICATE_PREFIX, LARGE_FILE_HINT, MISSING_TEST_COVERAGE_HINT, MULTI_WRITER_HINT,
    PREFERRED_ACCESSOR_PREFIX, PREFERRED_OWNER_PREFIX, STATE_MODEL_EXHAUSTIVE_HINT,
    UNCHANGED_CLONE_SIBLING_PREFIX, UNSTABLE_HOTSPOT_HINT, ZERO_CONFIG_BOUNDARY_VIOLATION_HINT,
};
use serde_json::Value;

pub(crate) fn repair_packet_for_finding(finding: &Value, kind: &str) -> RepairPacket {
    let input = FindingGuidanceInput::from_value(finding);
    build_repair_packet(
        kind,
        likely_fix_sites_for_finding(&input, kind),
        inspection_context_for_finding(&input),
        fix_hint_for_finding(&input, kind),
        finding_risk_statement(finding),
        None,
    )
}

pub(crate) fn fix_hint_for_value(finding: &Value, kind: &str) -> Option<String> {
    fix_hint_for_finding(&FindingGuidanceInput::from_value(finding), kind)
}

fn fix_hint_for_finding(finding: &FindingGuidanceInput, kind: &str) -> Option<String> {
    if kind == "forbidden_raw_read" {
        return forbidden_raw_read_fix_hint(finding);
    }

    match guidance_family(kind) {
        GuidanceFamily::Boundary => boundary_fix_hint_for_kind(kind),
        GuidanceFamily::Clone => clone_fix_hint_for_kind(finding, kind),
        GuidanceFamily::Cycle => Some(CYCLE_CLUSTER_HINT.to_string()),
        GuidanceFamily::Dependency => Some(DEPENDENCY_SPRAWL_HINT.to_string()),
        GuidanceFamily::Exhaustiveness => exhaustiveness_fix_hint_for_kind(kind),
        GuidanceFamily::FileScale => file_scale_fix_hint_for_kind(finding),
        GuidanceFamily::Generic => None,
        GuidanceFamily::Hotspot => hotspot_fix_hint_for_kind(kind),
        GuidanceFamily::TestCoverage => Some(MISSING_TEST_COVERAGE_HINT.to_string()),
    }
}

fn guidance_family(kind: &str) -> GuidanceFamily {
    guidance_kind_profile(kind).family
}

fn forbidden_raw_read_fix_hint(finding: &FindingGuidanceInput) -> Option<String> {
    let preferred_accessor = finding.evidence_value(PREFERRED_ACCESSOR_PREFIX);
    let canonical_owner = finding.evidence_value(CANONICAL_OWNER_PREFIX);
    if let Some(accessor) = preferred_accessor {
        if let Some(owner) = canonical_owner {
            return Some(format!(
                "Replace the raw read with {accessor} from {owner} instead of recreating the projection in the caller."
            ));
        }
        return Some(format!(
            "Replace the raw read with {accessor} instead of recreating the projection in the caller."
        ));
    }
    canonical_owner.map(|owner| {
        format!("Move the read behind {owner} instead of recreating the projection in the caller.")
    })
}

fn session_introduced_clone_fix_hint(finding: &FindingGuidanceInput) -> Option<String> {
    let introduced_duplicate = finding.evidence_value(INTRODUCED_DUPLICATE_PREFIX);
    let preferred_owner = finding.evidence_value(PREFERRED_OWNER_PREFIX);
    if let (Some(introduced_duplicate), Some(preferred_owner)) =
        (introduced_duplicate, preferred_owner.as_ref())
    {
        return Some(format!(
            "Collapse the new duplicate {introduced_duplicate} into {preferred_owner} instead of maintaining both paths."
        ));
    }
    preferred_owner.map(|preferred_owner| {
        format!(
            "Route the new duplicate back through {preferred_owner} before the two paths drift."
        )
    })
}

fn clone_propagation_drift_fix_hint(finding: &FindingGuidanceInput) -> Option<String> {
    let changed_member = finding.evidence_value(CHANGED_CLONE_MEMBER_PREFIX);
    let unchanged_sibling = finding.evidence_value(UNCHANGED_CLONE_SIBLING_PREFIX);
    if let (Some(changed_member), Some(unchanged_sibling)) =
        (changed_member.as_ref(), unchanged_sibling.as_ref())
    {
        return Some(format!(
            "Sync {unchanged_sibling} with the behavior change in {changed_member}, or collapse both paths behind one shared owner."
        ));
    }
    unchanged_sibling.map(|unchanged_sibling| {
        format!(
            "Update {unchanged_sibling} to match the changed clone path, or remove the duplicate split."
        )
    })
}

fn boundary_fix_hint_for_kind(kind: &str) -> Option<String> {
    match kind {
        "forbidden_raw_read" => Some(FORBIDDEN_RAW_READ_HINT.to_string()),
        "forbidden_writer" | "writer_outside_allowlist" => Some(FORBIDDEN_WRITER_HINT.to_string()),
        "multi_writer_concept" => Some(MULTI_WRITER_HINT.to_string()),
        "authoritative_import_bypass" => Some(AUTHORITATIVE_IMPORT_BYPASS_HINT.to_string()),
        "concept_boundary_pressure" => Some(CONCEPT_BOUNDARY_PRESSURE_HINT.to_string()),
        "zero_config_boundary_violation" => Some(ZERO_CONFIG_BOUNDARY_VIOLATION_HINT.to_string()),
        _ => None,
    }
}

fn clone_fix_hint_for_kind(finding: &FindingGuidanceInput, kind: &str) -> Option<String> {
    match kind {
        "session_introduced_clone" => session_introduced_clone_fix_hint(finding),
        "clone_propagation_drift" => clone_propagation_drift_fix_hint(finding),
        "touched_clone_family" => {
            if let Some(unchanged_sibling) = finding.evidence_value(UNCHANGED_CLONE_SIBLING_PREFIX)
            {
                return Some(format!(
                    "Inspect sibling clone {unchanged_sibling} before finishing the patch, or collapse the duplicate paths behind one shared helper."
                ));
            }
            Some(CLONE_FAMILY_HINT.to_string())
        }
        "exact_clone_group" | "clone_group" | "clone_family" => Some(CLONE_FAMILY_HINT.to_string()),
        _ => None,
    }
}

fn file_scale_fix_hint_for_kind(finding: &FindingGuidanceInput) -> Option<String> {
    let split_axes = finding
        .candidate_split_axes
        .iter()
        .take(2)
        .cloned()
        .collect::<Vec<_>>();
    let related_surfaces = finding
        .related_surfaces
        .iter()
        .take(2)
        .cloned()
        .filter(|surface| !looks_like_test_surface(surface))
        .collect::<Vec<_>>();
    if split_axes.len() == 1 && related_surfaces.len() == 1 {
        let split_axis = &split_axes[0];
        let related_surface = &related_surfaces[0];
        return Some(format!(
            "Split the file along the {split_axis} and move the behavior that couples to {related_surface} behind a smaller owner before adding more code here."
        ));
    }
    if !split_axes.is_empty() && !related_surfaces.is_empty() {
        return Some(format!(
            "Split the file along {} and move the behavior that couples to {} behind smaller owners before adding more code here.",
            describe_split_axes(&split_axes),
            describe_list(&related_surfaces),
        ));
    }
    if !split_axes.is_empty() {
        return Some(format!(
            "Split the file along {} and keep the public surface thin.",
            describe_split_axes(&split_axes),
        ));
    }
    if !related_surfaces.is_empty() {
        return Some(format!(
            "Extract the behavior that couples to {} behind smaller owners and keep the public surface thin.",
            describe_list(&related_surfaces),
        ));
    }
    Some(LARGE_FILE_HINT.to_string())
}

fn hotspot_fix_hint_for_kind(kind: &str) -> Option<String> {
    match kind {
        "unstable_hotspot" => Some(UNSTABLE_HOTSPOT_HINT.to_string()),
        "hotspot" => Some(HOTSPOT_HINT.to_string()),
        _ => None,
    }
}

fn exhaustiveness_fix_hint_for_kind(kind: &str) -> Option<String> {
    match kind {
        "closed_domain_exhaustiveness" => Some(CLOSED_DOMAIN_HINT.to_string()),
        "state_model_missing_exhaustive_switch" | "state_model_missing_assert_never" => {
            Some(STATE_MODEL_EXHAUSTIVE_HINT.to_string())
        }
        "incomplete_propagation" => Some(INCOMPLETE_PROPAGATION_HINT.to_string()),
        _ => None,
    }
}

fn likely_fix_sites_for_finding(finding: &FindingGuidanceInput, kind: &str) -> Vec<String> {
    let explicit_sites = finding
        .likely_fix_sites
        .iter()
        .take(5)
        .cloned()
        .collect::<Vec<_>>();
    if !explicit_sites.is_empty() {
        return explicit_sites;
    }

    match kind {
        "large_file" | "dependency_sprawl" | "unstable_hotspot" | "hotspot" => {
            finding.files.iter().take(1).cloned().collect::<Vec<_>>()
        }
        "cycle_cluster" => cycle_cut_fix_sites(finding),
        "session_introduced_clone" => session_introduced_clone_fix_sites(finding),
        "clone_propagation_drift" => clone_propagation_drift_fix_sites(finding),
        "touched_clone_family" => touched_clone_family_fix_sites(finding),
        "authoritative_import_bypass" | "concept_boundary_pressure" => {
            finding.files.iter().take(1).cloned().collect::<Vec<_>>()
        }
        _ => Vec::new(),
    }
}

fn inspection_context_for_finding(finding: &FindingGuidanceInput) -> Vec<String> {
    let inspection_context = finding
        .inspection_context
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>();
    if !inspection_context.is_empty() {
        return inspection_context;
    }

    finding.files.iter().take(3).cloned().collect::<Vec<_>>()
}

fn describe_split_axes(split_axes: &[String]) -> String {
    describe_list(
        &split_axes
            .iter()
            .map(|split_axis| format!("the {split_axis}"))
            .collect::<Vec<_>>(),
    )
}

fn cycle_cut_fix_sites(finding: &FindingGuidanceInput) -> Vec<String> {
    let Some(best_cut) = finding.evidence_value(BEST_CUT_CANDIDATE_PREFIX) else {
        return Vec::new();
    };
    let Some((from, to)) = best_cut.as_str().split_once(" -> ") else {
        return Vec::new();
    };
    vec![sanitize_fix_site(from), sanitize_fix_site(to)]
}

fn session_introduced_clone_fix_sites(finding: &FindingGuidanceInput) -> Vec<String> {
    evidence_fix_sites(
        finding,
        &[INTRODUCED_DUPLICATE_PREFIX, PREFERRED_OWNER_PREFIX],
    )
}

fn clone_propagation_drift_fix_sites(finding: &FindingGuidanceInput) -> Vec<String> {
    evidence_fix_sites(
        finding,
        &[CHANGED_CLONE_MEMBER_PREFIX, UNCHANGED_CLONE_SIBLING_PREFIX],
    )
}

fn touched_clone_family_fix_sites(finding: &FindingGuidanceInput) -> Vec<String> {
    let fix_sites = clone_propagation_drift_fix_sites(finding);
    if !fix_sites.is_empty() {
        return fix_sites;
    }

    finding.files.iter().take(2).cloned().collect::<Vec<_>>()
}

fn sanitize_fix_site(site: &str) -> String {
    site.trim()
        .split_once(" (")
        .map(|(prefix, _)| prefix)
        .unwrap_or(site.trim())
        .to_string()
}

fn evidence_fix_sites(finding: &FindingGuidanceInput, prefixes: &[&str]) -> Vec<String> {
    prefixes
        .iter()
        .copied()
        .filter_map(|prefix| finding.evidence_value(prefix))
        .map(|site| sanitize_fix_site(&site))
        .collect::<Vec<_>>()
}
