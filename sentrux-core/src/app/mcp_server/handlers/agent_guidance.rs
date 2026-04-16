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

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RepairPacket {
    pub(crate) risk_statement: String,
    pub(crate) likely_fix_sites: Vec<String>,
    pub(crate) smallest_safe_first_cut: Option<String>,
    pub(crate) verify_after: Vec<String>,
    pub(crate) do_not_touch_yet: Vec<String>,
    pub(crate) completeness_0_10000: u32,
}

pub(crate) fn repair_packet_for_finding(finding: &Value, kind: &str) -> RepairPacket {
    let likely_fix_sites = likely_fix_sites_for_value(finding);
    let smallest_safe_first_cut = fix_hint_for_value(finding, kind);
    let risk_statement = finding_risk_statement(finding);
    let verify_after = verify_after_for_kind(kind, &likely_fix_sites);
    let do_not_touch_yet = do_not_touch_yet_for_kind(kind);
    let completeness_0_10000 = repair_packet_completeness_0_10000(
        !likely_fix_sites.is_empty(),
        smallest_safe_first_cut.is_some(),
        !verify_after.is_empty(),
        !do_not_touch_yet.is_empty(),
    );
    RepairPacket {
        risk_statement,
        likely_fix_sites,
        smallest_safe_first_cut,
        verify_after,
        do_not_touch_yet,
        completeness_0_10000,
    }
}

pub(crate) fn repair_packet_for_obligation(obligation: &Value, kind: &str) -> RepairPacket {
    let likely_fix_sites = obligation_likely_fix_sites(obligation);
    let smallest_safe_first_cut = obligation_fix_hint(obligation, kind);
    let risk_statement = obligation_risk_statement(obligation, kind);
    let verify_after = verify_after_for_kind(kind, &likely_fix_sites);
    let do_not_touch_yet = do_not_touch_yet_for_kind(kind);
    let completeness_0_10000 = repair_packet_completeness_0_10000(
        !likely_fix_sites.is_empty(),
        smallest_safe_first_cut.is_some(),
        !verify_after.is_empty(),
        !do_not_touch_yet.is_empty(),
    );
    RepairPacket {
        risk_statement,
        likely_fix_sites,
        smallest_safe_first_cut,
        verify_after,
        do_not_touch_yet,
        completeness_0_10000,
    }
}

fn repair_packet_completeness_0_10000(
    has_fix_sites: bool,
    has_first_cut: bool,
    has_verify_after: bool,
    has_do_not_touch_yet: bool,
) -> u32 {
    let mut score = 2000;
    if has_fix_sites {
        score += 2500;
    }
    if has_first_cut {
        score += 2500;
    }
    if has_verify_after {
        score += 2000;
    }
    if has_do_not_touch_yet {
        score += 1000;
    }
    score
}

fn finding_risk_statement(finding: &Value) -> String {
    build_finding_details(std::slice::from_ref(finding), 1)
        .into_iter()
        .next()
        .map(|detail| detail.impact)
        .unwrap_or_else(|| {
            "If ignored, this finding will keep adding change friction and make future regressions harder to isolate."
                .to_string()
        })
}

fn obligation_risk_statement(_obligation: &Value, kind: &str) -> String {
    if kind == "incomplete_propagation" {
        return "Related contract surfaces are no longer aligned, so runtime paths can diverge or partially break."
            .to_string();
    }
    if kind == "closed_domain_exhaustiveness" {
        return "Finite-domain changes can silently miss one surface unless every required branch stays in sync."
            .to_string();
    }
    "Changed concept follow-through is still incomplete, so the patch can look finished while one required surface still drifts."
        .to_string()
}

fn verify_after_for_kind(kind: &str, likely_fix_sites: &[String]) -> Vec<String> {
    let mut steps = Vec::new();
    if !likely_fix_sites.is_empty() {
        steps.push(format!(
            "Re-run `sentrux check` and confirm the lead issue clears for {}.",
            describe_list(likely_fix_sites)
        ));
    } else {
        steps.push(
            "Re-run `sentrux check` and confirm this issue no longer appears in the lead actions."
                .to_string(),
        );
    }

    if matches!(
        kind,
        "incomplete_propagation"
            | "closed_domain_exhaustiveness"
            | "state_model_missing_exhaustive_switch"
            | "state_model_missing_assert_never"
    ) {
        steps.push(
            "Re-run `sentrux gate` and confirm no missing follow-through or exhaustiveness blocker remains."
                .to_string(),
        );
    }

    steps
}

fn do_not_touch_yet_for_kind(kind: &str) -> Vec<String> {
    match kind {
        "large_file" => vec![
            "Do not slice the file by line count alone; split along the cited boundary instead."
                .to_string(),
        ],
        "dependency_sprawl" => vec![
            "Do not add another direct entry-surface import while fixing this; move behavior behind a narrower owner."
                .to_string(),
        ],
        "cycle_cluster" => vec![
            "Do not try to untangle the whole cluster at once; cut the highest-leverage seam first."
                .to_string(),
        ],
        "session_introduced_clone" | "clone_propagation_drift" | "touched_clone_family" => vec![
            "Do not create a third sibling path while fixing the duplicate behavior.".to_string(),
        ],
        "closed_domain_exhaustiveness" => vec![
            "Do not rely on a production fallback or default branch to hide missing variants."
                .to_string(),
        ],
        "incomplete_propagation" => vec![
            "Do not treat the source-side change as complete until every required sibling surface is updated."
                .to_string(),
        ],
        _ => Vec::new(),
    }
}

pub(crate) fn fix_hint_for_value(finding: &Value, kind: &str) -> Option<String> {
    if kind == "forbidden_raw_read" {
        return forbidden_raw_read_fix_hint(finding);
    }

    if kind == "session_introduced_clone" {
        return session_introduced_clone_fix_hint(finding);
    }

    if kind == "clone_propagation_drift" {
        return clone_propagation_drift_fix_hint(finding);
    }

    if kind == "touched_clone_family" {
        if let Some(unchanged_sibling) =
            evidence_value_for_prefix(finding, UNCHANGED_CLONE_SIBLING_PREFIX)
        {
            return Some(format!(
                "Inspect sibling clone {unchanged_sibling} before finishing the patch, or collapse the duplicate paths behind one shared helper."
            ));
        }
    }

    if kind == "large_file" {
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
    }

    fix_hint_for_kind(kind)
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

fn likely_fix_sites_for_value(finding: &Value) -> Vec<String> {
    let likely_fix_sites = finding
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
    if !likely_fix_sites.is_empty() {
        return likely_fix_sites;
    }

    finding_files(finding)
        .into_iter()
        .take(3)
        .collect::<Vec<_>>()
}

fn obligation_likely_fix_sites(obligation: &Value) -> Vec<String> {
    let likely_fix_sites = obligation
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
        .unwrap_or_default();
    if !likely_fix_sites.is_empty() {
        return likely_fix_sites;
    }

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
    if kind == "incomplete_propagation" {
        let scope = obligation
            .get("concept_id")
            .or_else(|| obligation.get("concept"))
            .and_then(Value::as_str)
            .unwrap_or("changed contract");
        return format!(
            "Propagation is incomplete for '{}': update the remaining sibling surfaces listed in the evidence.",
            scope
        );
    }

    if kind == "closed_domain_exhaustiveness" {
        let domain = obligation_domain_label(obligation);
        let missing_variants = obligation_missing_variants(obligation);
        let site_suffix = obligation_site_suffix(obligation);

        if !missing_variants.is_empty() {
            return format!(
                "Domain '{}' still needs explicit handling for variants [{}]{}.",
                domain,
                missing_variants.join(", "),
                site_suffix
            );
        }

        return format!(
            "Domain '{}' still needs an explicit exhaustive branch{}.",
            domain, site_suffix
        );
    }

    obligation
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("Changed concept still has missing update sites")
        .to_string()
}

pub(crate) fn obligation_fix_hint(obligation: &Value, kind: &str) -> Option<String> {
    let hint = match kind {
        "incomplete_propagation" => {
            "Update the remaining sibling surfaces listed in the evidence before considering the change complete."
        }
        "closed_domain_exhaustiveness" => {
            let site_suffix = obligation_site_suffix(obligation);
            let missing_variants = obligation_missing_variants(obligation);
            if !missing_variants.is_empty() {
                return Some(format!(
                    "Handle the missing variants [{}] with an explicit exhaustive switch or mapping{site_suffix}, and keep the fallback/default path out of the production branch.",
                    missing_variants.join(", "),
                ));
            }

            return Some(format!(
                "Add an explicit exhaustive switch or mapping{site_suffix}, and keep the fallback/default path out of the production branch."
            ));
        }
        _ => "Update the missing sites tied to the changed concept before continuing.",
    };
    Some(hint.to_string())
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
    ) || obligation.get("concept_id").is_none()
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
    if obligation
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "closed_domain_exhaustiveness")
        || obligation
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

fn fix_hint_for_kind(kind: &str) -> Option<String> {
    let hint = match kind {
        "forbidden_raw_read" => {
            "Route the read through the concept's canonical accessor instead of reading raw state."
        }
        "forbidden_writer" | "writer_outside_allowlist" => {
            "Move the write behind an allowed writer or update the rule if the new writer is intentional."
        }
        "multi_writer_concept" => {
            "Reduce the concept to one authoritative writer or document the additional writer explicitly."
        }
        "closed_domain_exhaustiveness" => {
            "Handle the missing variants with an explicit exhaustive switch or mapping, and keep the fallback/default branch out of the production path."
        }
        "state_model_missing_exhaustive_switch" | "state_model_missing_assert_never" => {
            "Restore the exhaustive switch and assert-never guard for the state model."
        }
        "large_file" => "Split the file along the boundary suggested by the evidence and keep the public surface thin.",
        "dependency_sprawl" => {
            "Extract a narrower facade or move behavior behind an existing module boundary."
        }
        "unstable_hotspot" => "Stabilize the hotspot before adding more change pressure.",
        "cycle_cluster" => "Cut the highest-leverage cycle seam first and re-run check.",
        "exact_clone_group" | "clone_group" | "clone_family" => {
            "Extract shared behavior or collapse the duplicated flow."
        }
        "session_introduced_clone" => {
            "Collapse the new duplicate now: extract the shared behavior or route both call sites through the same owner before they drift."
        }
        "clone_propagation_drift" => {
            "Sync the unchanged sibling clone with the changed path, or collapse both behind one shared owner before behavior drifts."
        }
        "touched_clone_family" => {
            "Inspect the sibling clone surfaces before finishing the patch, even if you keep the duplicate for now."
        }
        "incomplete_propagation" => {
            "Update the remaining sibling surfaces listed in the evidence before considering the change complete."
        }
        "missing_test_coverage" => "Add a sibling test covering the new production surface.",
        "zero_config_boundary_violation" => {
            "Replace the deep import with the module's public API."
        }
        _ => return None,
    };
    Some(hint.to_string())
}
