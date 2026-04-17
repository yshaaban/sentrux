use super::agent_format::{IssueConfidence, IssueOrigin};
use super::*;
use crate::metrics::v2::FindingSeverity;
use serde::Serialize;

const PREFERRED_ACCESSOR_PREFIX: &str = "preferred accessor: ";
const CANONICAL_OWNER_PREFIX: &str = "canonical owner: ";
const INTRODUCED_DUPLICATE_PREFIX: &str = "introduced duplicate: ";
const PREFERRED_OWNER_PREFIX: &str = "preferred owner: ";
const CHANGED_CLONE_MEMBER_PREFIX: &str = "changed clone member: ";
const UNCHANGED_CLONE_SIBLING_PREFIX: &str = "unchanged clone sibling: ";
const BEST_CUT_CANDIDATE_PREFIX: &str = "best cut candidate: ";
const LARGE_FILE_DO_NOT_TOUCH: &[&str] =
    &["Do not slice the file by line count alone; split along the cited boundary instead."];
const DEPENDENCY_SPRAWL_DO_NOT_TOUCH: &[&str] = &[
    "Do not add another direct entry-surface import while fixing this; move behavior behind a narrower owner.",
];
const CYCLE_CLUSTER_DO_NOT_TOUCH: &[&str] =
    &["Do not try to untangle the whole cluster at once; cut the highest-leverage seam first."];
const CLONE_FAMILY_DO_NOT_TOUCH: &[&str] =
    &["Do not create a third sibling path while fixing the duplicate behavior."];
const CLOSED_DOMAIN_DO_NOT_TOUCH: &[&str] =
    &["Do not rely on a production fallback or default branch to hide missing variants."];
const INCOMPLETE_PROPAGATION_DO_NOT_TOUCH: &[&str] = &[
    "Do not treat the source-side change as complete until every required sibling surface is updated.",
];
const VERIFY_AFTER_GATE_MESSAGE: &str =
    "Re-run `sentrux gate` and confirm no missing follow-through or exhaustiveness blocker remains.";
const DEFAULT_FINDING_RISK_STATEMENT: &str =
    "If ignored, this finding will keep adding change friction and make future regressions harder to isolate.";
const DEFAULT_OBLIGATION_RISK_STATEMENT: &str =
    "Changed concept follow-through is still incomplete, so the patch can look finished while one required surface still drifts.";
const INCOMPLETE_PROPAGATION_RISK_STATEMENT: &str =
    "Related contract surfaces are no longer aligned, so runtime paths can diverge or partially break.";
const CLOSED_DOMAIN_EXHAUSTIVENESS_RISK_STATEMENT: &str =
    "Finite-domain changes can silently miss one surface unless every required branch stays in sync.";
const FORBIDDEN_RAW_READ_HINT: &str =
    "Route the read through the concept's canonical accessor instead of reading raw state.";
const FORBIDDEN_WRITER_HINT: &str =
    "Move the write behind an allowed writer or update the rule if the new writer is intentional.";
const MULTI_WRITER_HINT: &str =
    "Reduce the concept to one authoritative writer or document the additional writer explicitly.";
const CLOSED_DOMAIN_HINT: &str =
    "Handle the missing variants with an explicit exhaustive switch or mapping, and keep the fallback/default branch out of the production path.";
const STATE_MODEL_EXHAUSTIVE_HINT: &str =
    "Restore the exhaustive switch and assert-never guard for the state model.";
const LARGE_FILE_HINT: &str =
    "Split the file along the boundary suggested by the evidence and keep the public surface thin.";
const DEPENDENCY_SPRAWL_HINT: &str =
    "Extract a narrower facade or move behavior behind an existing module boundary.";
const AUTHORITATIVE_IMPORT_BYPASS_HINT: &str =
    "Route the caller back through the concept's canonical import surface instead of deep-importing the owner.";
const CONCEPT_BOUNDARY_PRESSURE_HINT: &str =
    "Move the shared concept access behind one owner before another sibling bypasses the same boundary.";
const UNSTABLE_HOTSPOT_HINT: &str = "Stabilize the hotspot before adding more change pressure.";
const HOTSPOT_HINT: &str =
    "Pull orchestration or side effects behind a narrower owner before adding more behavior here.";
const CYCLE_CLUSTER_HINT: &str = "Cut the highest-leverage cycle seam first and re-run check.";
const CLONE_FAMILY_HINT: &str = "Extract shared behavior or collapse the duplicated flow.";
const INCOMPLETE_PROPAGATION_HINT: &str =
    "Update the remaining sibling surfaces listed in the evidence before considering the change complete.";
const MISSING_TEST_COVERAGE_HINT: &str = "Add a sibling test covering the new production surface.";
const ZERO_CONFIG_BOUNDARY_VIOLATION_HINT: &str =
    "Replace the deep import with the module's public API.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuidanceFamily {
    Boundary,
    Clone,
    Cycle,
    Dependency,
    Exhaustiveness,
    FileScale,
    Generic,
    Hotspot,
    TestCoverage,
}

#[derive(Debug, Clone, Copy)]
struct GuidanceKindProfile {
    family: GuidanceFamily,
    do_not_touch_yet: &'static [&'static str],
    verify_after_gate: bool,
}

fn guidance_kind_profile(kind: &str) -> GuidanceKindProfile {
    match kind {
        "large_file" => GuidanceKindProfile {
            family: GuidanceFamily::FileScale,
            do_not_touch_yet: LARGE_FILE_DO_NOT_TOUCH,
            verify_after_gate: false,
        },
        "dependency_sprawl" => GuidanceKindProfile {
            family: GuidanceFamily::Dependency,
            do_not_touch_yet: DEPENDENCY_SPRAWL_DO_NOT_TOUCH,
            verify_after_gate: false,
        },
        "cycle_cluster" => GuidanceKindProfile {
            family: GuidanceFamily::Cycle,
            do_not_touch_yet: CYCLE_CLUSTER_DO_NOT_TOUCH,
            verify_after_gate: false,
        },
        "session_introduced_clone" | "clone_propagation_drift" | "touched_clone_family" => {
            GuidanceKindProfile {
                family: GuidanceFamily::Clone,
                do_not_touch_yet: CLONE_FAMILY_DO_NOT_TOUCH,
                verify_after_gate: false,
            }
        }
        "closed_domain_exhaustiveness" => GuidanceKindProfile {
            family: GuidanceFamily::Exhaustiveness,
            do_not_touch_yet: CLOSED_DOMAIN_DO_NOT_TOUCH,
            verify_after_gate: true,
        },
        "incomplete_propagation" => GuidanceKindProfile {
            family: GuidanceFamily::Exhaustiveness,
            do_not_touch_yet: INCOMPLETE_PROPAGATION_DO_NOT_TOUCH,
            verify_after_gate: true,
        },
        "state_model_missing_exhaustive_switch" | "state_model_missing_assert_never" => {
            GuidanceKindProfile {
                family: GuidanceFamily::Exhaustiveness,
                do_not_touch_yet: &[],
                verify_after_gate: true,
            }
        }
        "forbidden_raw_read"
        | "forbidden_writer"
        | "writer_outside_allowlist"
        | "authoritative_import_bypass"
        | "concept_boundary_pressure"
        | "zero_config_boundary_violation" => GuidanceKindProfile {
            family: GuidanceFamily::Boundary,
            do_not_touch_yet: &[],
            verify_after_gate: false,
        },
        "unstable_hotspot" | "hotspot" => GuidanceKindProfile {
            family: GuidanceFamily::Hotspot,
            do_not_touch_yet: &[],
            verify_after_gate: false,
        },
        "missing_test_coverage" => GuidanceKindProfile {
            family: GuidanceFamily::TestCoverage,
            do_not_touch_yet: &[],
            verify_after_gate: false,
        },
        _ => GuidanceKindProfile {
            family: GuidanceFamily::Generic,
            do_not_touch_yet: &[],
            verify_after_gate: false,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObligationFamily {
    Propagation,
    Exhaustiveness,
    Generic,
}

fn obligation_family(kind: &str) -> ObligationFamily {
    match kind {
        "incomplete_propagation" => ObligationFamily::Propagation,
        "closed_domain_exhaustiveness" => ObligationFamily::Exhaustiveness,
        _ => ObligationFamily::Generic,
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RepairPacketRequiredFields {
    pub(crate) risk_statement: bool,
    pub(crate) repair_surface: bool,
    pub(crate) first_cut: bool,
    pub(crate) verification: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RepairPacket {
    pub(crate) risk_statement: String,
    pub(crate) likely_fix_sites: Vec<String>,
    pub(crate) inspection_context: Vec<String>,
    pub(crate) smallest_safe_first_cut: Option<String>,
    pub(crate) verify_after: Vec<String>,
    pub(crate) do_not_touch_yet: Vec<String>,
    pub(crate) completeness_0_10000: u32,
    pub(crate) complete: bool,
    pub(crate) required_fields: RepairPacketRequiredFields,
    pub(crate) missing_fields: Vec<String>,
}

pub(crate) fn repair_packet_for_finding(finding: &Value, kind: &str) -> RepairPacket {
    build_repair_packet(
        kind,
        likely_fix_sites_for_value(finding, kind),
        inspection_context_for_finding(finding),
        fix_hint_for_value(finding, kind),
        finding_risk_statement(finding),
    )
}

pub(crate) fn repair_packet_for_obligation(obligation: &Value, kind: &str) -> RepairPacket {
    build_repair_packet(
        kind,
        obligation_likely_fix_sites(obligation),
        inspection_context_for_obligation(obligation),
        obligation_fix_hint(obligation, kind),
        obligation_risk_statement(obligation, kind),
    )
}

fn build_repair_packet(
    kind: &str,
    likely_fix_sites: Vec<String>,
    inspection_context: Vec<String>,
    smallest_safe_first_cut: Option<String>,
    risk_statement: String,
) -> RepairPacket {
    let verify_after = verify_after_for_kind(kind, &likely_fix_sites, &inspection_context);
    let do_not_touch_yet = do_not_touch_yet_for_kind(kind);
    let required_fields = repair_packet_required_fields(
        !risk_statement.trim().is_empty(),
        !likely_fix_sites.is_empty(),
        smallest_safe_first_cut.is_some(),
        !verify_after.is_empty(),
    );
    let missing_fields = repair_packet_missing_fields(&required_fields);
    let completeness_0_10000 =
        repair_packet_completeness_0_10000(&required_fields, !do_not_touch_yet.is_empty());
    RepairPacket {
        risk_statement,
        likely_fix_sites,
        inspection_context,
        smallest_safe_first_cut,
        verify_after,
        do_not_touch_yet,
        completeness_0_10000,
        complete: missing_fields.is_empty(),
        required_fields,
        missing_fields,
    }
}

fn repair_packet_required_fields(
    has_risk_statement: bool,
    has_fix_sites: bool,
    has_first_cut: bool,
    has_verify_after: bool,
) -> RepairPacketRequiredFields {
    RepairPacketRequiredFields {
        risk_statement: has_risk_statement,
        repair_surface: has_fix_sites,
        first_cut: has_first_cut,
        verification: has_verify_after,
    }
}

fn repair_packet_missing_fields(required_fields: &RepairPacketRequiredFields) -> Vec<String> {
    let mut missing_fields = Vec::new();
    if !required_fields.risk_statement {
        missing_fields.push("risk_statement".to_string());
    }
    if !required_fields.repair_surface {
        missing_fields.push("repair_surface".to_string());
    }
    if !required_fields.first_cut {
        missing_fields.push("first_cut".to_string());
    }
    if !required_fields.verification {
        missing_fields.push("verification".to_string());
    }
    missing_fields
}

fn repair_packet_completeness_0_10000(
    required_fields: &RepairPacketRequiredFields,
    has_do_not_touch_yet: bool,
) -> u32 {
    let mut score = 0;
    if required_fields.risk_statement {
        score += 2500;
    }
    if required_fields.repair_surface {
        score += 2500;
    }
    if required_fields.first_cut {
        score += 2500;
    }
    if required_fields.verification {
        score += 2000;
    }
    if has_do_not_touch_yet {
        score += 500;
    }
    score.min(10_000)
}

fn finding_risk_statement(finding: &Value) -> String {
    build_finding_details(std::slice::from_ref(finding), 1)
        .into_iter()
        .next()
        .map(|detail| detail.impact)
        .unwrap_or_else(|| DEFAULT_FINDING_RISK_STATEMENT.to_string())
}

fn obligation_risk_statement(_obligation: &Value, kind: &str) -> String {
    match obligation_family(kind) {
        ObligationFamily::Propagation => INCOMPLETE_PROPAGATION_RISK_STATEMENT.to_string(),
        ObligationFamily::Exhaustiveness => CLOSED_DOMAIN_EXHAUSTIVENESS_RISK_STATEMENT.to_string(),
        ObligationFamily::Generic => DEFAULT_OBLIGATION_RISK_STATEMENT.to_string(),
    }
}

fn verify_after_for_kind(
    kind: &str,
    likely_fix_sites: &[String],
    inspection_context: &[String],
) -> Vec<String> {
    let mut steps = Vec::new();
    if !likely_fix_sites.is_empty() {
        steps.push(format!(
            "Re-run `sentrux check` and confirm the lead issue clears for {}.",
            describe_list(likely_fix_sites)
        ));
    } else if !inspection_context.is_empty() {
        steps.push(format!(
            "Re-run `sentrux check` and confirm the lead issue clears around {}.",
            describe_list(inspection_context)
        ));
    } else {
        steps.push(
            "Re-run `sentrux check` and confirm this issue no longer appears in the lead actions."
                .to_string(),
        );
    }

    if guidance_kind_profile(kind).verify_after_gate {
        steps.push(VERIFY_AFTER_GATE_MESSAGE.to_string());
    }

    steps
}

fn do_not_touch_yet_for_kind(kind: &str) -> Vec<String> {
    guidance_kind_profile(kind)
        .do_not_touch_yet
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>()
}

fn guidance_family(kind: &str) -> GuidanceFamily {
    guidance_kind_profile(kind).family
}

pub(crate) fn fix_hint_for_value(finding: &Value, kind: &str) -> Option<String> {
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

fn forbidden_raw_read_fix_hint(finding: &Value) -> Option<String> {
    let preferred_accessor = evidence_value_for_prefix(finding, PREFERRED_ACCESSOR_PREFIX);
    let canonical_owner = evidence_value_for_prefix(finding, CANONICAL_OWNER_PREFIX);
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

fn session_introduced_clone_fix_hint(finding: &Value) -> Option<String> {
    let introduced_duplicate = evidence_value_for_prefix(finding, INTRODUCED_DUPLICATE_PREFIX);
    let preferred_owner = evidence_value_for_prefix(finding, PREFERRED_OWNER_PREFIX);
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

fn clone_propagation_drift_fix_hint(finding: &Value) -> Option<String> {
    let changed_member = evidence_value_for_prefix(finding, CHANGED_CLONE_MEMBER_PREFIX);
    let unchanged_sibling = evidence_value_for_prefix(finding, UNCHANGED_CLONE_SIBLING_PREFIX);
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

fn clone_fix_hint_for_kind(finding: &Value, kind: &str) -> Option<String> {
    match kind {
        "session_introduced_clone" => session_introduced_clone_fix_hint(finding),
        "clone_propagation_drift" => clone_propagation_drift_fix_hint(finding),
        "touched_clone_family" => {
            if let Some(unchanged_sibling) =
                evidence_value_for_prefix(finding, UNCHANGED_CLONE_SIBLING_PREFIX)
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

fn file_scale_fix_hint_for_kind(finding: &Value) -> Option<String> {
    let split_axes = string_array_values(finding, "candidate_split_axes", 2);
    let related_surfaces = string_array_values(finding, "related_surfaces", 2)
        .into_iter()
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

fn evidence_value_for_prefix(finding: &Value, prefix: &str) -> Option<String> {
    finding
        .get("evidence")
        .and_then(Value::as_array)
        .and_then(|values| {
            values.iter().find_map(|value| {
                value
                    .as_str()
                    .and_then(|evidence| evidence.strip_prefix(prefix))
                    .map(str::to_string)
            })
        })
}

fn string_array_values(finding: &Value, key: &str, limit: usize) -> Vec<String> {
    finding
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .take(limit.max(1))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn likely_fix_sites_for_value(finding: &Value, kind: &str) -> Vec<String> {
    let explicit_sites = finding
        .get("likely_fix_sites")
        .and_then(Value::as_array)
        .map(|sites| {
            sites
                .iter()
                .filter_map(|site| {
                    site.get("site")
                        .and_then(Value::as_str)
                        .or_else(|| site.as_str())
                        .map(str::to_string)
                })
                .take(5)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !explicit_sites.is_empty() {
        return explicit_sites;
    }

    match kind {
        "large_file" | "dependency_sprawl" | "unstable_hotspot" | "hotspot" => {
            finding_files(finding)
                .into_iter()
                .take(1)
                .collect::<Vec<_>>()
        }
        "cycle_cluster" => cycle_cut_fix_sites(finding),
        "session_introduced_clone" => session_introduced_clone_fix_sites(finding),
        "clone_propagation_drift" => clone_propagation_drift_fix_sites(finding),
        "touched_clone_family" => touched_clone_family_fix_sites(finding),
        "authoritative_import_bypass" | "concept_boundary_pressure" => finding_files(finding)
            .into_iter()
            .take(1)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    }
}

fn inspection_context_for_finding(finding: &Value) -> Vec<String> {
    finding_files(finding)
        .into_iter()
        .take(3)
        .collect::<Vec<_>>()
}

fn obligation_likely_fix_sites(obligation: &Value) -> Vec<String> {
    obligation
        .get("missing_sites")
        .and_then(Value::as_array)
        .map(|sites| {
            sites
                .iter()
                .filter_map(|site| {
                    let path = site
                        .get("site")
                        .and_then(Value::as_str)
                        .or_else(|| site.get("path").and_then(Value::as_str))?;
                    let line_suffix = site
                        .get("line")
                        .and_then(Value::as_u64)
                        .map(|line| format!(":{line}"))
                        .unwrap_or_default();
                    Some(format!("{path}{line_suffix}"))
                })
                .take(5)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn inspection_context_for_obligation(obligation: &Value) -> Vec<String> {
    obligation_files(obligation)
        .into_iter()
        .take(3)
        .collect::<Vec<_>>()
}

fn describe_split_axes(split_axes: &[String]) -> String {
    describe_list(
        &split_axes
            .iter()
            .map(|split_axis| format!("the {split_axis}"))
            .collect::<Vec<_>>(),
    )
}

fn cycle_cut_fix_sites(finding: &Value) -> Vec<String> {
    let Some(best_cut) = evidence_value_for_prefix(finding, BEST_CUT_CANDIDATE_PREFIX) else {
        return Vec::new();
    };
    let Some((from, to)) = best_cut.split_once(" -> ") else {
        return Vec::new();
    };
    vec![sanitize_fix_site(from), sanitize_fix_site(to)]
}

fn session_introduced_clone_fix_sites(finding: &Value) -> Vec<String> {
    evidence_fix_sites(
        finding,
        &[INTRODUCED_DUPLICATE_PREFIX, PREFERRED_OWNER_PREFIX],
    )
}

fn clone_propagation_drift_fix_sites(finding: &Value) -> Vec<String> {
    evidence_fix_sites(
        finding,
        &[CHANGED_CLONE_MEMBER_PREFIX, UNCHANGED_CLONE_SIBLING_PREFIX],
    )
}

fn touched_clone_family_fix_sites(finding: &Value) -> Vec<String> {
    let fix_sites = clone_propagation_drift_fix_sites(finding);
    if !fix_sites.is_empty() {
        return fix_sites;
    }

    finding_files(finding)
        .into_iter()
        .take(2)
        .collect::<Vec<_>>()
}

fn sanitize_fix_site(site: &str) -> String {
    site.trim()
        .split_once(" (")
        .map(|(prefix, _)| prefix)
        .unwrap_or(site.trim())
        .to_string()
}

fn evidence_fix_sites(finding: &Value, prefixes: &[&str]) -> Vec<String> {
    prefixes
        .iter()
        .filter_map(|prefix| evidence_value_for_prefix(finding, prefix))
        .map(|site| sanitize_fix_site(&site))
        .collect::<Vec<_>>()
}

fn describe_list(values: &[String]) -> String {
    match values {
        [] => String::new(),
        [value] => value.clone(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let last = values.last().expect("non-empty list");
            let leading = &values[..values.len() - 1];
            format!("{}, and {}", leading.join(", "), last)
        }
    }
}

fn looks_like_test_surface(path: &str) -> bool {
    path.contains(".test.")
        || path.contains(".spec.")
        || path.contains(".architecture.test.")
        || path.contains("/__tests__/")
        || path.ends_with("_test.rs")
}

pub(crate) fn obligation_message(obligation: &Value, kind: &str) -> String {
    match obligation_family(kind) {
        ObligationFamily::Propagation => {
            let scope = obligation
                .get("concept_id")
                .or_else(|| obligation.get("concept"))
                .and_then(Value::as_str)
                .unwrap_or("changed contract");
            format!(
                "Propagation is incomplete for '{}': update the remaining sibling surfaces listed in the evidence.",
                scope
            )
        }
        ObligationFamily::Exhaustiveness => {
            let domain = obligation_domain_label(obligation);
            let missing_variants = obligation_missing_variants(obligation);
            let site_suffix = obligation_site_suffix(obligation);

            if !missing_variants.is_empty() {
                format!(
                    "Domain '{}' still needs explicit handling for variants [{}]{}.",
                    domain,
                    missing_variants.join(", "),
                    site_suffix
                )
            } else {
                format!(
                    "Domain '{}' still needs an explicit exhaustive branch{}.",
                    domain, site_suffix
                )
            }
        }
        ObligationFamily::Generic => obligation
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("Changed concept still has missing update sites")
            .to_string(),
    }
}

pub(crate) fn obligation_fix_hint(obligation: &Value, kind: &str) -> Option<String> {
    match obligation_family(kind) {
        ObligationFamily::Propagation => Some(INCOMPLETE_PROPAGATION_HINT.to_string()),
        ObligationFamily::Exhaustiveness => {
            let site_suffix = obligation_site_suffix(obligation);
            let missing_variants = obligation_missing_variants(obligation);
            if !missing_variants.is_empty() {
                return Some(format!(
                    "Handle the missing variants [{}] with an explicit exhaustive switch or mapping{site_suffix}, and keep the fallback/default path out of the production branch.",
                    missing_variants.join(", "),
                ));
            }

            Some(format!(
                "Add an explicit exhaustive switch or mapping{site_suffix}, and keep the fallback/default path out of the production branch."
            ))
        }
        ObligationFamily::Generic => Some(
            "Update the missing sites tied to the changed concept before continuing.".to_string(),
        ),
    }
}

fn obligation_domain_label(obligation: &Value) -> String {
    obligation
        .get("domain_symbol_name")
        .or_else(|| obligation.get("concept_id"))
        .or_else(|| obligation.get("concept"))
        .and_then(Value::as_str)
        .unwrap_or("closed domain")
        .to_string()
}

fn obligation_missing_variants(obligation: &Value) -> Vec<String> {
    obligation
        .get("missing_variants")
        .and_then(Value::as_array)
        .map(|variants| {
            variants
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn obligation_missing_site(obligation: &Value) -> Option<String> {
    let sites = obligation.get("missing_sites").and_then(Value::as_array)?;
    let site = sites.first()?;
    let path = site.get("path").and_then(Value::as_str)?;
    let line = site
        .get("line")
        .and_then(Value::as_u64)
        .map(|line| format!(":{line}"))
        .unwrap_or_default();

    Some(format!("{path}{line}"))
}

fn obligation_site_suffix(obligation: &Value) -> String {
    obligation_missing_site(obligation)
        .map(|site| format!(" at {site}"))
        .unwrap_or_default()
}

pub(crate) fn obligation_origin(obligation: &Value) -> IssueOrigin {
    if matches!(
        obligation.get("origin").and_then(Value::as_str),
        Some("zero_config")
    ) || obligation
        .get("concept_id")
        .or_else(|| obligation.get("concept"))
        .is_none()
    {
        IssueOrigin::ZeroConfig
    } else {
        IssueOrigin::Explicit
    }
}

pub(crate) fn obligation_confidence(obligation: &Value) -> IssueConfidence {
    match obligation_origin(obligation) {
        IssueOrigin::Explicit => IssueConfidence::High,
        IssueOrigin::ZeroConfig => IssueConfidence::Medium,
    }
}

pub(crate) fn obligation_trust_tier(obligation: &Value) -> &'static str {
    match obligation_origin(obligation) {
        IssueOrigin::Explicit => "trusted",
        IssueOrigin::ZeroConfig => "watchpoint",
    }
}

pub(crate) fn obligation_severity(obligation: &Value) -> FindingSeverity {
    if matches!(
        obligation_family(obligation_kind(obligation)),
        ObligationFamily::Exhaustiveness
    ) || obligation
        .get("missing_variants")
        .and_then(Value::as_array)
        .is_some_and(|variants| !variants.is_empty())
    {
        FindingSeverity::High
    } else if obligation_origin(obligation) == IssueOrigin::Explicit {
        FindingSeverity::High
    } else {
        FindingSeverity::Medium
    }
}

fn obligation_kind(obligation: &Value) -> &str {
    obligation.get("kind").and_then(Value::as_str).unwrap_or("")
}

pub(crate) fn obligation_score_0_10000(obligation: &Value) -> u32 {
    let severity_bonus = match obligation_severity(obligation) {
        FindingSeverity::High => 1800,
        FindingSeverity::Medium => 1000,
        FindingSeverity::Low => 200,
    };
    let origin_bonus = match obligation_origin(obligation) {
        IssueOrigin::Explicit => 1600,
        IssueOrigin::ZeroConfig => 600,
    };
    let site_bonus = obligation
        .get("missing_sites")
        .and_then(Value::as_array)
        .map(|sites| (sites.len().min(3) as u32) * 500)
        .unwrap_or_default();
    (6000 + severity_bonus + origin_bonus + site_bonus).min(10_000)
}

pub(crate) fn obligation_files(obligation: &Value) -> Vec<String> {
    let files = obligation
        .get("files")
        .and_then(Value::as_array)
        .map(|files| {
            files
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !files.is_empty() {
        return files;
    }

    obligation
        .get("missing_sites")
        .and_then(Value::as_array)
        .map(|sites| {
            sites
                .iter()
                .filter_map(|site| site.get("path").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn obligation_line(obligation: &Value) -> Option<u32> {
    obligation
        .get("missing_sites")
        .and_then(Value::as_array)
        .and_then(|sites| {
            sites
                .iter()
                .find_map(|site| site.get("line").and_then(Value::as_u64))
        })
        .map(|line| line as u32)
}

pub(crate) fn obligation_evidence(obligation: &Value) -> Vec<String> {
    obligation
        .get("missing_sites")
        .and_then(Value::as_array)
        .map(|sites| {
            sites
                .iter()
                .filter_map(|site| {
                    let path = site.get("path").and_then(Value::as_str)?;
                    let detail = site
                        .get("detail")
                        .and_then(Value::as_str)
                        .unwrap_or("missing site");
                    let line_suffix = site
                        .get("line")
                        .and_then(Value::as_u64)
                        .map(|line| format!(":{line}"))
                        .unwrap_or_default();
                    Some(format!("{path}{line_suffix} [{detail}]"))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn repair_packet_for_large_file_uses_shared_family_guidance() {
        let finding = json!({
            "files": ["src/app/mcp_server/handlers/agent_guidance.rs"],
            "candidate_split_axes": ["packet assembly"],
            "related_surfaces": ["src/app/mcp_server/handlers/agent_format.rs"],
            "kind": "large_file",
            "evidence": ["file scale signal"],
        });

        let packet = repair_packet_for_finding(&finding, "large_file");

        assert!(packet.risk_statement.contains("change friction"));
        assert!(packet
            .risk_statement
            .contains("future regressions harder to isolate"));
        assert_eq!(
            packet.smallest_safe_first_cut.as_deref(),
            Some("Split the file along the packet assembly and move the behavior that couples to src/app/mcp_server/handlers/agent_format.rs behind a smaller owner before adding more code here.")
        );
        assert_eq!(
            packet.do_not_touch_yet,
            vec!["Do not slice the file by line count alone; split along the cited boundary instead."]
        );
        assert_eq!(packet.verify_after.len(), 1);
        assert!(
            packet.verify_after[0].contains("agent_guidance.rs")
                || packet.verify_after[0].contains("src/app/mcp_server/handlers/agent_format.rs")
        );
    }

    #[test]
    fn repair_packet_for_clone_propagation_drift_uses_family_specific_hint() {
        let finding = json!({
            "files": ["src/app/mcp_server/handlers/agent_guidance.rs", "src/app/mcp_server/handlers/agent_format.rs"],
            "evidence": [
                "changed clone member: src/app/mcp_server/handlers/agent_guidance.rs",
                "unchanged clone sibling: src/app/mcp_server/handlers/agent_format.rs"
            ],
            "kind": "clone_propagation_drift"
        });

        let packet = repair_packet_for_finding(&finding, "clone_propagation_drift");

        assert_eq!(
            packet.smallest_safe_first_cut.as_deref(),
            Some("Sync src/app/mcp_server/handlers/agent_format.rs with the behavior change in src/app/mcp_server/handlers/agent_guidance.rs, or collapse both paths behind one shared owner.")
        );
        assert_eq!(
            packet.do_not_touch_yet,
            vec!["Do not create a third sibling path while fixing the duplicate behavior."]
        );
    }

    #[test]
    fn repair_packet_for_closed_domain_obligation_keeps_gate_verification() {
        let obligation = json!({
            "kind": "closed_domain_exhaustiveness",
            "concept_id": "signal_kind",
            "domain_symbol_name": "signal kind",
            "missing_variants": ["alpha", "beta"],
            "missing_sites": [
                {"path": "src/app/mcp_server/handlers/agent_guidance.rs", "line": 42, "detail": "missing exhaustive branch"}
            ]
        });

        let packet = repair_packet_for_obligation(&obligation, "closed_domain_exhaustiveness");

        assert_eq!(
            packet.risk_statement,
            "Finite-domain changes can silently miss one surface unless every required branch stays in sync."
        );
        assert!(packet
            .smallest_safe_first_cut
            .as_deref()
            .unwrap_or_default()
            .contains("Handle the missing variants [alpha, beta]"));
        assert!(packet
            .verify_after
            .iter()
            .any(|step| step.contains("Re-run `sentrux gate`")));
        assert_eq!(
            packet.do_not_touch_yet,
            vec![
                "Do not rely on a production fallback or default branch to hide missing variants."
            ]
        );
    }

    #[test]
    fn repair_packet_for_raw_read_keeps_accessor_based_hint() {
        let finding = json!({
            "files": ["src/app/mcp_server/handlers/agent_guidance.rs"],
            "evidence": [
                "preferred accessor: use_signal_summary",
                "canonical owner: SignalSummary"
            ],
            "kind": "forbidden_raw_read"
        });

        let packet = repair_packet_for_finding(&finding, "forbidden_raw_read");

        assert_eq!(
            packet.smallest_safe_first_cut.as_deref(),
            Some("Replace the raw read with use_signal_summary from SignalSummary instead of recreating the projection in the caller.")
        );
        assert!(packet
            .verify_after
            .iter()
            .any(|step| step.contains("agent_guidance.rs")));
    }
}
