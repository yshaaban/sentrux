use super::super::agent_format::{IssueConfidence, IssueOrigin};
use super::super::finding_files;
use crate::metrics::v2::FindingSeverity;
use serde_json::Value;

pub(crate) const PREFERRED_ACCESSOR_PREFIX: &str = "preferred accessor: ";
pub(crate) const CANONICAL_OWNER_PREFIX: &str = "canonical owner: ";
pub(crate) const INTRODUCED_DUPLICATE_PREFIX: &str = "introduced duplicate: ";
pub(crate) const PREFERRED_OWNER_PREFIX: &str = "preferred owner: ";
pub(crate) const CHANGED_CLONE_MEMBER_PREFIX: &str = "changed clone member: ";
pub(crate) const UNCHANGED_CLONE_SIBLING_PREFIX: &str = "unchanged clone sibling: ";
pub(crate) const BEST_CUT_CANDIDATE_PREFIX: &str = "best cut candidate: ";
pub(crate) const LARGE_FILE_DO_NOT_TOUCH: &[&str] =
    &["Do not slice the file by line count alone; split along the cited boundary instead."];
pub(crate) const DEPENDENCY_SPRAWL_DO_NOT_TOUCH: &[&str] = &[
    "Do not add another direct entry-surface import while fixing this; move behavior behind a narrower owner.",
];
pub(crate) const CYCLE_CLUSTER_DO_NOT_TOUCH: &[&str] =
    &["Do not try to untangle the whole cluster at once; cut the highest-leverage seam first."];
pub(crate) const CLONE_FAMILY_DO_NOT_TOUCH: &[&str] =
    &["Do not create a third sibling path while fixing the duplicate behavior."];
pub(crate) const CLOSED_DOMAIN_DO_NOT_TOUCH: &[&str] =
    &["Do not rely on a production fallback or default branch to hide missing variants."];
pub(crate) const INCOMPLETE_PROPAGATION_DO_NOT_TOUCH: &[&str] = &[
    "Do not treat the source-side change as complete until every required sibling surface is updated.",
];
pub(crate) const VERIFY_AFTER_GATE_MESSAGE: &str =
    "Re-run `sentrux gate` and confirm no missing follow-through or exhaustiveness blocker remains.";
pub(crate) const DEFAULT_FINDING_RISK_STATEMENT: &str =
    "If ignored, this finding will keep adding change friction and make future regressions harder to isolate.";
pub(crate) const DEFAULT_OBLIGATION_RISK_STATEMENT: &str =
    "Changed concept follow-through is still incomplete, so the patch can look finished while one required surface still drifts.";
pub(crate) const INCOMPLETE_PROPAGATION_RISK_STATEMENT: &str =
    "Related contract surfaces are no longer aligned, so runtime paths can diverge or partially break.";
pub(crate) const CLOSED_DOMAIN_EXHAUSTIVENESS_RISK_STATEMENT: &str =
    "Finite-domain changes can silently miss one surface unless every required branch stays in sync.";
pub(crate) const FORBIDDEN_RAW_READ_HINT: &str =
    "Route the read through the concept's canonical accessor instead of reading raw state.";
pub(crate) const FORBIDDEN_WRITER_HINT: &str =
    "Move the write behind an allowed writer or update the rule if the new writer is intentional.";
pub(crate) const MULTI_WRITER_HINT: &str =
    "Reduce the concept to one authoritative writer or document the additional writer explicitly.";
pub(crate) const CLOSED_DOMAIN_HINT: &str =
    "Handle the missing variants with an explicit exhaustive switch or mapping, and keep the fallback/default branch out of the production path.";
pub(crate) const STATE_MODEL_EXHAUSTIVE_HINT: &str =
    "Restore the exhaustive switch and assert-never guard for the state model.";
pub(crate) const LARGE_FILE_HINT: &str =
    "Split the file along the boundary suggested by the evidence and keep the public surface thin.";
pub(crate) const DEPENDENCY_SPRAWL_HINT: &str =
    "Extract a narrower facade or move behavior behind an existing module boundary.";
pub(crate) const AUTHORITATIVE_IMPORT_BYPASS_HINT: &str =
    "Route the caller back through the concept's canonical import surface instead of deep-importing the owner.";
pub(crate) const CONCEPT_BOUNDARY_PRESSURE_HINT: &str =
    "Move the shared concept access behind one owner before another sibling bypasses the same boundary.";
pub(crate) const UNSTABLE_HOTSPOT_HINT: &str =
    "Stabilize the hotspot before adding more change pressure.";
pub(crate) const HOTSPOT_HINT: &str =
    "Pull orchestration or side effects behind a narrower owner before adding more behavior here.";
pub(crate) const CYCLE_CLUSTER_HINT: &str =
    "Cut the highest-leverage cycle seam first and re-run check.";
pub(crate) const CLONE_FAMILY_HINT: &str =
    "Extract shared behavior or collapse the duplicated flow.";
pub(crate) const INCOMPLETE_PROPAGATION_HINT: &str =
    "Update the remaining sibling surfaces listed in the evidence before considering the change complete.";
pub(crate) const MISSING_TEST_COVERAGE_HINT: &str =
    "Add a sibling test covering the new production surface.";
pub(crate) const ZERO_CONFIG_BOUNDARY_VIOLATION_HINT: &str =
    "Replace the deep import with the module's public API.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuidanceFamily {
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
pub(crate) struct GuidanceKindProfile {
    pub(crate) family: GuidanceFamily,
    pub(crate) do_not_touch_yet: &'static [&'static str],
    pub(crate) verify_after_gate: bool,
}

pub(crate) fn guidance_kind_profile(kind: &str) -> GuidanceKindProfile {
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
pub(crate) enum ObligationFamily {
    Propagation,
    Exhaustiveness,
    Generic,
}

pub(crate) fn obligation_family(kind: &str) -> ObligationFamily {
    match kind {
        "incomplete_propagation" => ObligationFamily::Propagation,
        "closed_domain_exhaustiveness" => ObligationFamily::Exhaustiveness,
        _ => ObligationFamily::Generic,
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FindingGuidanceInput {
    pub(crate) files: Vec<String>,
    pub(crate) evidence: Vec<String>,
    pub(crate) likely_fix_sites: Vec<String>,
    pub(crate) inspection_context: Vec<String>,
    pub(crate) candidate_split_axes: Vec<String>,
    pub(crate) related_surfaces: Vec<String>,
}

impl FindingGuidanceInput {
    pub(crate) fn from_value(finding: &Value) -> Self {
        Self {
            files: finding_files(finding),
            evidence: string_array_field(finding, "evidence", usize::MAX),
            likely_fix_sites: likely_fix_site_field(finding),
            inspection_context: string_array_field(finding, "inspection_context", usize::MAX),
            candidate_split_axes: string_array_field(finding, "candidate_split_axes", usize::MAX),
            related_surfaces: string_array_field(finding, "related_surfaces", usize::MAX),
        }
    }

    pub(crate) fn evidence_value(&self, prefix: &str) -> Option<String> {
        self.evidence
            .iter()
            .find_map(|value| value.strip_prefix(prefix).map(str::to_string))
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ObligationSiteInput {
    pub(crate) path: String,
    pub(crate) line: Option<u32>,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ObligationGuidanceInput {
    pub(crate) concept_id: Option<String>,
    pub(crate) concept: Option<String>,
    pub(crate) domain_symbol_name: Option<String>,
    pub(crate) summary: Option<String>,
    pub(crate) files: Vec<String>,
    pub(crate) missing_sites: Vec<ObligationSiteInput>,
    pub(crate) missing_variants: Vec<String>,
    pub(crate) origin: Option<IssueOrigin>,
    pub(crate) confidence: Option<IssueConfidence>,
    pub(crate) trust_tier: Option<String>,
    pub(crate) severity: Option<FindingSeverity>,
    pub(crate) score_0_10000: Option<u32>,
}

impl ObligationGuidanceInput {
    pub(crate) fn from_value(obligation: &Value) -> Self {
        let missing_sites = obligation_missing_site_inputs(obligation);
        let mut files = string_array_field(obligation, "files", usize::MAX);
        if files.is_empty() {
            files = missing_sites
                .iter()
                .map(|site| site.path.clone())
                .collect::<Vec<_>>();
        }

        Self {
            concept_id: string_field(obligation, "concept_id"),
            concept: string_field(obligation, "concept"),
            domain_symbol_name: string_field(obligation, "domain_symbol_name"),
            summary: string_field(obligation, "summary"),
            files,
            missing_sites,
            missing_variants: string_array_field(obligation, "missing_variants", usize::MAX),
            origin: obligation
                .get("origin")
                .and_then(Value::as_str)
                .and_then(parse_issue_origin),
            confidence: obligation
                .get("confidence")
                .and_then(Value::as_str)
                .and_then(parse_issue_confidence),
            trust_tier: string_field(obligation, "trust_tier"),
            severity: obligation
                .get("severity")
                .and_then(Value::as_str)
                .and_then(parse_finding_severity),
            score_0_10000: obligation
                .get("score_0_10000")
                .and_then(Value::as_u64)
                .map(|value| value as u32),
        }
    }

    pub(crate) fn scope_label(&self) -> &str {
        self.concept_id
            .as_deref()
            .or(self.concept.as_deref())
            .unwrap_or("changed contract")
    }
}

pub(crate) fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

pub(crate) fn string_array_field(value: &Value, key: &str, limit: usize) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .take(limit)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn likely_fix_site_field(value: &Value) -> Vec<String> {
    value
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
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn obligation_missing_site_inputs(obligation: &Value) -> Vec<ObligationSiteInput> {
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
                    Some(ObligationSiteInput {
                        path: path.to_string(),
                        line: site
                            .get("line")
                            .and_then(Value::as_u64)
                            .map(|line| line as u32),
                        detail: site
                            .get("detail")
                            .and_then(Value::as_str)
                            .unwrap_or("missing site")
                            .to_string(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn parse_issue_origin(value: &str) -> Option<IssueOrigin> {
    match value {
        "explicit" => Some(IssueOrigin::Explicit),
        "zero_config" => Some(IssueOrigin::ZeroConfig),
        _ => None,
    }
}

pub(crate) fn parse_issue_confidence(value: &str) -> Option<IssueConfidence> {
    match value {
        "high" => Some(IssueConfidence::High),
        "medium" => Some(IssueConfidence::Medium),
        "experimental" => Some(IssueConfidence::Experimental),
        _ => None,
    }
}

pub(crate) fn parse_finding_severity(value: &str) -> Option<FindingSeverity> {
    match value {
        "high" => Some(FindingSeverity::High),
        "medium" => Some(FindingSeverity::Medium),
        "low" => Some(FindingSeverity::Low),
        _ => None,
    }
}

pub(crate) fn fallback_obligation_origin(obligation: &ObligationGuidanceInput) -> IssueOrigin {
    if obligation.concept_id.is_none() && obligation.concept.is_none() {
        IssueOrigin::ZeroConfig
    } else {
        IssueOrigin::Explicit
    }
}

pub(crate) fn describe_list(values: &[String]) -> String {
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

pub(crate) fn looks_like_test_surface(path: &str) -> bool {
    path.contains(".test.")
        || path.contains(".spec.")
        || path.contains(".architecture.test.")
        || path.contains("/__tests__/")
        || path.ends_with("_test.rs")
}
